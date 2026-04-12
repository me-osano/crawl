use clap::Args;
use anyhow::Result;
use serde_json::json;
use crate::{client::CrawlClient, output};

#[derive(Args)]
pub struct ClipArgs {
    #[arg(long)] pub get: bool,
    #[arg(long, value_name = "TEXT")] pub set: Option<String>,
    #[arg(long)] pub history: bool,
}

pub async fn run(client: CrawlClient, args: ClipArgs, json: bool) -> Result<()> {
    if let Some(text) = args.set {
        client.post("/clipboard", json!({ "content": text })).await?;
        output::print_ok("Clipboard updated");
    } else if args.history {
        let res = client.get("/clipboard/history").await?;
        output::print_value(&res, json);
    } else {
        let res = client.get("/clipboard").await?;
        if json { output::print_value(&res, true); }
        else { println!("{}", res["content"].as_str().unwrap_or("")); }
    }
    Ok(())
}
