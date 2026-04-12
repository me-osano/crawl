use clap::Args;
use anyhow::Result;
use serde_json::json;
use crate::{client::CrawlClient, output};

#[derive(Args)]
pub struct ProcArgs {
    #[arg(long)] pub list: bool,
    #[arg(long, value_name = "NAME")] pub find: Option<String>,
    #[arg(long, value_name = "PID")] pub kill: Option<u32>,
    #[arg(long)] pub force: bool,
    #[arg(long, value_name = "cpu|mem|pid|name", default_value = "cpu")] pub sort: String,
    #[arg(long, value_name = "N", default_value = "20")] pub top: usize,
}

pub async fn run(client: CrawlClient, args: ProcArgs, json: bool) -> Result<()> {
    if let Some(pid) = args.kill {
        client.post(&format!("/proc/{pid}/kill"), json!({ "force": args.force })).await?;
        output::print_ok(&format!("Sent signal to PID {pid}"));
    } else if let Some(name) = args.find {
        let res = client.get(&format!("/proc/find?name={name}")).await?;
        output::print_value(&res, json);
    } else {
        let res = client.get(&format!("/proc/list?sort={}&top={}", args.sort, args.top)).await?;
        if json {
            output::print_value(&res, true);
        } else if let Some(procs) = res.as_array() {
            println!("  {:<6}  {:<5}  {:<6}  name", "PID", "CPU%", "MEM");
            for p in procs {
                let pid  = p["pid"].as_u64().unwrap_or(0);
                let cpu  = p["cpu_percent"].as_f64().unwrap_or(0.0);
                let mem  = p["mem_rss_kb"].as_u64().unwrap_or(0) / 1024;
                let name = p["name"].as_str().unwrap_or("?");
                println!("  {pid:<6}  {cpu:>5.1}  {mem:>4}M  {name}");
            }
        }
    }
    Ok(())
}
