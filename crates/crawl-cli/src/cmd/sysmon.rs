use clap::Args;
use anyhow::Result;
use crate::{client::CrawlClient, output};
use crawl_ipc::{CrawlEvent, events::SysmonEvent};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

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

    /// Stream live updates (press Ctrl-C to stop)
    #[arg(long)]
    pub watch: bool,
}

pub async fn run(client: CrawlClient, args: SysmonArgs, json: bool) -> Result<()> {
    if args.watch {
        return watch_events(&client, json).await;
    }

    if args.cpu || (!args.mem && !args.disk) {
        let res = client.get("/sysmon/cpu").await?;
        if json {
            output::print_value(&res, true);
        } else {
            let agg = res["aggregate"].as_f64().unwrap_or(0.0);
            let load1 = res["load_avg"]["one"].as_f64().unwrap_or(0.0);
            let load5 = res["load_avg"]["five"].as_f64().unwrap_or(0.0);
            let load15 = res["load_avg"]["fifteen"].as_f64().unwrap_or(0.0);
            println!("CPU");
            output::print_table(&[
                ("usage",    format!("{agg:.1}%")),
                ("load avg", format!("{load1:.2}  {load5:.2}  {load15:.2}")),
            ]);
            if let Some(cores) = res["cores"].as_array() {
                let bar: Vec<String> = cores.iter()
                    .map(|c| format!("{:>5.1}%", c.as_f64().unwrap_or(0.0)))
                    .collect();
                println!("  cores    {}", bar.join("  "));
            }
        }
    }

    if args.mem {
        let res = client.get("/sysmon/mem").await?;
        if json {
            output::print_value(&res, true);
        } else {
            let total = res["total_kb"].as_u64().unwrap_or(0);
            let used  = res["used_kb"].as_u64().unwrap_or(0);
            let avail = res["available_kb"].as_u64().unwrap_or(0);
            let pct   = if total > 0 { used as f64 / total as f64 * 100.0 } else { 0.0 };
            println!("Memory");
            output::print_table(&[
                ("used",      format!("{} / {} MiB ({pct:.1}%)", used / 1024, total / 1024)),
                ("available", format!("{} MiB", avail / 1024)),
            ]);
        }
    }

    if args.disk {
        let res = client.get("/sysmon/disk").await?;
        if json {
            output::print_value(&res, true);
        } else if let Some(disks) = res.as_array() {
            println!("Disks");
            for d in disks {
                let mount = d["mount"].as_str().unwrap_or("?");
                let used  = d["used_bytes"].as_u64().unwrap_or(0) / 1_073_741_824;
                let total = d["total_bytes"].as_u64().unwrap_or(0) / 1_073_741_824;
                let pct   = if total > 0 { used as f64 / total as f64 * 100.0 } else { 0.0 };
                println!("  {mount:<20}  {used} / {total} GiB  ({pct:.1}%)");
            }
        }
    }

    Ok(())
}

async fn watch_events(client: &CrawlClient, json: bool) -> Result<()> {
    use anyhow::Context;
    use tokio::net::UnixStream;

    let stream = UnixStream::connect(client.socket_path())
        .await
        .with_context(|| format!(
            "failed to connect to crawl daemon at {:?}\n\
             Is crawl-daemon running? Try: systemctl --user start crawl",
            client.socket_path()
        ))?;

    let request = "GET /events HTTP/1.1\r\nHost: localhost\r\nAccept: text/event-stream\r\n\r\n";
    let (read, mut write) = stream.into_split();
    write.write_all(request.as_bytes()).await?;

    let mut reader = BufReader::new(read).lines();
    while let Some(line) = reader.next_line().await? {
        if let Some(data) = line.strip_prefix("data: ")
            && let Ok(CrawlEvent::Sysmon(sysmon)) = serde_json::from_str::<CrawlEvent>(data)
        {
            match sysmon {
                SysmonEvent::CpuUpdate { cpu } => {
                    if json {
                        output::print_value(&serde_json::to_value(cpu)?, true);
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
                SysmonEvent::MemUpdate { mem } => {
                    if json {
                        output::print_value(&serde_json::to_value(mem)?, true);
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
        }
    }

    Ok(())
}
