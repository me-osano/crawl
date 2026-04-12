use clap::Args;
use anyhow::Result;
use serde_json::json;
use crate::{client::CrawlClient, output};

#[derive(Args)]
pub struct MediaArgs {
    /// Show active player status
    #[arg(long)]
    pub status: bool,

    /// List all MPRIS players
    #[arg(long)]
    pub list: bool,

    /// Play / resume
    #[arg(long)]
    pub play: bool,

    /// Pause
    #[arg(long)]
    pub pause: bool,

    /// Toggle play/pause
    #[arg(long)]
    pub toggle: bool,

    /// Skip to next track
    #[arg(long)]
    pub next: bool,

    /// Go to previous track
    #[arg(long)]
    pub prev: bool,

    /// Set volume (0.0–1.0)
    #[arg(long, value_name = "FLOAT")]
    pub volume: Option<f64>,

    /// Target a specific player by name (default: most recent active)
    #[arg(long, value_name = "PLAYER")]
    pub player: Option<String>,
}

pub async fn run(client: CrawlClient, args: MediaArgs, json: bool) -> Result<()> {
    if args.play || args.toggle {
        client.post("/media/play", json!({ "player": args.player })).await?;
        output::print_ok("Playing");
    } else if args.pause {
        client.post("/media/pause", json!({ "player": args.player })).await?;
        output::print_ok("Paused");
    } else if args.next {
        client.post("/media/next", json!({ "player": args.player })).await?;
        output::print_ok("Next track");
    } else if args.prev {
        client.post("/media/prev", json!({ "player": args.player })).await?;
        output::print_ok("Previous track");
    } else if let Some(vol) = args.volume {
        client.post("/media/volume", json!({ "volume": vol, "player": args.player })).await?;
        output::print_ok(&format!("Volume set to {:.0}%", vol * 100.0));
    } else if args.list {
        let res = client.get("/media/players").await?;
        output::print_value(&res, json);
    } else {
        let res = client.get("/media/active").await?;
        if json {
            output::print_value(&res, true);
        } else {
            let player = res["player_name"].as_str().unwrap_or("none");
            let status = res["status"].as_str().unwrap_or("stopped");
            let title  = res["title"].as_str().unwrap_or("—");
            let artist = res["artist"].as_str().unwrap_or("—");
            let album  = res["album"].as_str().unwrap_or("—");
            println!("Media  [{player}  {status}]");
            output::print_table(&[
                ("title",  title.to_string()),
                ("artist", artist.to_string()),
                ("album",  album.to_string()),
            ]);
        }
    }
    Ok(())
}
