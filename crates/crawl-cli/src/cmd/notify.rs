use clap::Args;
use anyhow::Result;
use serde_json::json;
use crate::{client::CrawlClient, output};

#[derive(Args)]
pub struct NotifyArgs {
    #[arg(long)] pub list: bool,
    #[arg(long, value_name = "ID")] pub dismiss: Option<u32>,
    #[arg(long, value_name = "TEXT")] pub title: Option<String>,
    #[arg(long, value_name = "TEXT")] pub body: Option<String>,
    #[arg(long, value_name = "low|normal|critical")] pub urgency: Option<String>,
}

pub async fn run(client: CrawlClient, args: NotifyArgs, json: bool) -> Result<()> {
    if let Some(id) = args.dismiss {
        client.delete(&format!("/notify/{id}")).await?;
        output::print_ok(&format!("Dismissed notification {id}"));
    } else if args.title.is_some() || args.body.is_some() {
        let res = client.post("/notify/send", json!({
            "title": args.title.unwrap_or_default(),
            "body":  args.body.unwrap_or_default(),
            "urgency": args.urgency.unwrap_or_else(|| "normal".into()),
        })).await?;
        if json { output::print_value(&res, true); } else { output::print_ok("Notification sent"); }
    } else {
        let res = client.get("/notify/list").await?;
        output::print_value(&res, json);
    }
    Ok(())
}
