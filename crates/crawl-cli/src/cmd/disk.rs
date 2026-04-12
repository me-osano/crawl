use clap::Args;
use anyhow::Result;
use serde_json::json;
use crate::{client::CrawlClient, output};

#[derive(Args)]
pub struct DiskArgs {
    #[arg(long)] pub list: bool,
    #[arg(long, value_name = "DEVICE")] pub mount: Option<String>,
    #[arg(long, value_name = "DEVICE")] pub unmount: Option<String>,
    #[arg(long, value_name = "DEVICE")] pub eject: Option<String>,
}

pub async fn run(client: CrawlClient, args: DiskArgs, json: bool) -> Result<()> {
    if let Some(dev) = args.mount {
        client.post("/disk/mount", json!({ "device": dev })).await?;
        output::print_ok(&format!("Mounted {dev}"));
    } else if let Some(dev) = args.unmount {
        client.post("/disk/unmount", json!({ "device": dev })).await?;
        output::print_ok(&format!("Unmounted {dev}"));
    } else if let Some(dev) = args.eject {
        client.post("/disk/eject", json!({ "device": dev })).await?;
        output::print_ok(&format!("Ejected {dev}"));
    } else {
        let res = client.get("/disk/list").await?;
        if json {
            output::print_value(&res, true);
        } else if let Some(devices) = res.as_array() {
            println!("  {:<12}  {:<10}  {:<20}  {:<10}  mount", "device", "size", "label", "fs");
            for d in devices {
                let dev    = d["device"].as_str().unwrap_or("?");
                let size   = d["size_bytes"].as_u64().unwrap_or(0) / 1_073_741_824;
                let label  = d["label"].as_str().unwrap_or("—");
                let fs     = d["filesystem"].as_str().unwrap_or("—");
                let mount  = d["mount_point"].as_str().unwrap_or("—");
                let rem    = if d["removable"].as_bool().unwrap_or(false) { " [removable]" } else { "" };
                println!("  {dev:<12}  {size:>6} GiB  {label:<20}  {fs:<10}  {mount}{rem}");
            }
        }
    }
    Ok(())
}
