use crate::CrawlClient;
use crate::output::{self, CliRenderable};
use crawl_ipc::EventSubscription;
use anyhow::Result;
use clap::Args;
use crawl_ipc::events::{CrawlEvent, SysmonEvent};

#[derive(Args)]
pub struct SysmonArgs {
    /// Show CPU usage
    #[arg(long)]
    pub cpu: bool,

    /// Show memory usage
    #[arg(long)]
    pub mem: bool,

    /// Show disk usage
    #[arg(long)]
    pub disk: bool,

    /// Show network throughput
    #[arg(long)]
    pub net: bool,

    /// Show GPU info
    #[arg(long)]
    pub gpu: bool,

    /// Stream live updates (press Ctrl-C to stop)
    #[arg(long)]
    pub watch: bool,
}

pub async fn run(client: CrawlClient, args: SysmonArgs, json: bool) -> Result<()> {
    if args.watch {
        let json_flag = json;
        let sub = EventSubscription::new(client.socket_path().clone());
        sub.subscribe(move |event: CrawlEvent| {
            match event {
                CrawlEvent::Sysmon(SysmonEvent::CpuUpdate { cpu }) => {
                    if json_flag {
                        let _ = output::print_value(&serde_json::to_value(&cpu).unwrap(), true);
                    } else {
                        let load1 = cpu.load_avg.one;
                        let load5 = cpu.load_avg.five;
                        let load15 = cpu.load_avg.fifteen;
                        println!(
                            "CPU  {:>5.1}%   load {:.2} {:.2} {:.2}",
                            cpu.aggregate, load1, load5, load15
                        );
                    }
                }
                CrawlEvent::Sysmon(SysmonEvent::MemUpdate { mem }) => {
                    if json_flag {
                        let _ = output::print_value(&serde_json::to_value(&mem).unwrap(), true);
                    } else {
                        let total = mem.total_kb / 1024;
                        let used = mem.used_kb / 1024;
                        let pct = if mem.total_kb > 0 {
                            mem.used_kb as f64 / mem.total_kb as f64 * 100.0
                        } else {
                            0.0
                        };
                        println!("MEM  {used} / {total} MiB  ({pct:.1}%)");
                    }
                }
                _ => {}
            }
        }).await?;
        return Ok(());
    }

    let res = if args.cpu || (!args.mem && !args.disk && !args.net && !args.gpu) {
        client.cmd("SysmonCpu", serde_json::json!({})).await?
    } else if args.mem {
        client.cmd("SysmonMem", serde_json::json!({})).await?
    } else if args.disk {
        client.cmd("SysmonDisk", serde_json::json!({})).await?
    } else if args.net {
        client.cmd("SysmonNet", serde_json::json!({})).await?
    } else if args.gpu {
        client.cmd("SysmonGpu", serde_json::json!({})).await?
    } else {
        client.cmd("SysmonCpu", serde_json::json!({})).await?
    };

    if json {
        output::print_value(&res, true);
        return Ok(());
    }

    if res.is_array() {
        // Disk output
        for d in res.as_array().unwrap() {
            let mount = d["mount"].as_str().unwrap_or("?");
            let used  = d["used_bytes"].as_u64().unwrap_or(0) / 1_073_741_824;
            let total = d["total_bytes"].as_u64().unwrap_or(0) / 1_073_741_824;
            let pct   = if total > 0 { used as f64 / total as f64 * 100.0 } else { 0.0 };
            println!("  {mount:<20}  {used} / {total} GiB  ({pct:.1}%)");
        }
    } else {
        // CPU output
        output::print_header("CPU");
        let agg = res["aggregate"].as_f64().unwrap_or(0.0);
        let load1 = res["load_avg"]["one"].as_f64().unwrap_or(0.0);
        let load5 = res["load_avg"]["five"].as_f64().unwrap_or(0.0);
        let load15 = res["load_avg"]["fifteen"].as_f64().unwrap_or(0.0);

        let rows = vec![
            vec!["usage".to_string(), format!("{agg:.1}%")],
            vec!["load avg".to_string(), format!("{load1:.2}  {load5:.2}  {load15:.2}")],
        ];
        let renderable = CliRenderable::new(vec!["metric".to_string(), "value".to_string()], rows);
        println!("{}", renderable.render());

        if let Some(cores) = res["cores"].as_array() {
            let bar: Vec<String> = cores.iter()
                .map(|c| format!("{:>5.1}%", c.as_f64().unwrap_or(0.0)))
                .collect();
            println!("  cores    {}", bar.join("  "));
        }

        if args.mem {
            output::print_header("Memory");
            let total = res["total_kb"].as_u64().unwrap_or(0);
            let used  = res["used_kb"].as_u64().unwrap_or(0);
            let avail = res["available_kb"].as_u64().unwrap_or(0);
            let pct   = if total > 0 { used as f64 / total as f64 * 100.0 } else { 0.0 };
            let rows = vec![
                vec!["used".to_string(), format!("{} / {} MiB ({pct:.1}%)", used / 1024, total / 1024)],
                vec!["available".to_string(), format!("{} MiB", avail / 1024)],
            ];
            let renderable = CliRenderable::new(vec!["metric".to_string(), "value".to_string()], rows);
            println!("{}", renderable.render());
        }

        if args.net {
            output::print_header("Network");
            let rx = res["rx_bps"].as_u64().unwrap_or(0);
            let tx = res["tx_bps"].as_u64().unwrap_or(0);
            let rows = vec![
                vec!["rx".to_string(), format!("{} B/s", rx)],
                vec!["tx".to_string(), format!("{} B/s", tx)],
            ];
            let renderable = CliRenderable::new(vec!["metric".to_string(), "value".to_string()], rows);
            println!("{}", renderable.render());
        }

        if args.gpu {
            output::print_header("GPU");
            if res.is_null() {
                println!("status: unavailable");
            } else {
                let name = res["name"].as_str().unwrap_or("unknown");
                let temp = res["temperature_c"].as_f64().unwrap_or(0.0);
                let rows = vec![
                    vec!["name".to_string(), name.to_string()],
                    vec!["temp".to_string(), format!("{temp:.1} C")],
                ];
                let renderable = CliRenderable::new(vec!["metric".to_string(), "value".to_string()], rows);
                println!("{}", renderable.render());
            }
        }
    }

    Ok(())
}
