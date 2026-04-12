use clap::Args;
use anyhow::Result;
use crate::{client::CrawlClient, output};

#[derive(Args)]
pub struct PowerArgs {
    #[arg(long)] pub battery: bool,
    #[arg(long)] pub status: bool,
}

pub async fn run(client: CrawlClient, _args: PowerArgs, json: bool) -> Result<()> {
    let res = client.get("/power/battery").await?;
    if json {
        output::print_value(&res, true);
    } else {
        let pct   = res["percent"].as_f64().unwrap_or(0.0);
        let state = res["state"].as_str().unwrap_or("unknown");
        let ac    = res["on_ac"].as_bool().unwrap_or(false);
        let tte   = res["time_to_empty_secs"].as_i64()
            .map(|s| format!("{}h {:02}m", s / 3600, (s % 3600) / 60))
            .unwrap_or_else(|| "—".into());
        let ttf   = res["time_to_full_secs"].as_i64()
            .map(|s| format!("{}h {:02}m", s / 3600, (s % 3600) / 60))
            .unwrap_or_else(|| "—".into());
        println!("Power");
        output::print_table(&[
            ("battery",       format!("{pct:.0}%  ({state})")),
            ("AC connected",  ac.to_string()),
            ("time to empty", tte),
            ("time to full",  ttf),
        ]);
    }
    Ok(())
}
