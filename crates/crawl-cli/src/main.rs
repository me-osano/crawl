mod cmd;
mod logo;
mod output;

pub use crawl_ipc::client::CrawlClient;
use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

fn default_socket_path() -> PathBuf {
    std::env::var("CRAWL_SOCKET")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let uid = std::env::var("UID")
                .ok()
                .and_then(|u| u.parse().ok())
                .unwrap_or(1000);
            PathBuf::from(format!("/run/user/{}/crawl.sock", uid))
        })
}

#[derive(Parser)]
#[command(
    name = "crawl",
    version,
    about = "System services CLI — display, brightness, wallpaper, audio",
    long_about = None,
)]
struct Cli {
    #[arg(long, short = 'j', global = true)]
    json: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Sysinfo(cmd::sysinfo::SysinfoArgs),
    Sysmon(cmd::sysmon::SysmonArgs),
    Proc(cmd::proc::ProcArgs),
    Audio(cmd::audio::AudioArgs),
    Display(cmd::display::DisplayArgs),
    Network(cmd::network::NetArgs),
    Bluetooth(cmd::bluetooth::BtArgs),
    Daemon(cmd::daemon::DaemonArgs),
}

#[tokio::main]
async fn main() -> Result<()> {
    let _ = color_eyre::install();

    let cli = Cli::parse();

    let socket_path = default_socket_path();
    let client = CrawlClient::new(socket_path);
    let json_mode = cli.json;

    match cli.command {
        Commands::Sysinfo(args) => cmd::sysinfo::run(client, args, json_mode).await?,
        Commands::Sysmon(args) => cmd::sysmon::run(client, args, json_mode).await?,
        Commands::Proc(args) => cmd::proc::run(client, args, json_mode).await?,
        Commands::Audio(args) => cmd::audio::run(client, args, json_mode).await?,
        Commands::Display(args) => cmd::display::run(client, args, json_mode).await?,
        Commands::Network(args) => cmd::network::run(client, args, json_mode).await?,
        Commands::Bluetooth(args) => cmd::bluetooth::run(client, args, json_mode).await?,
        Commands::Daemon(args) => cmd::daemon::run(client, args, json_mode).await?,
    }

    Ok(())
}