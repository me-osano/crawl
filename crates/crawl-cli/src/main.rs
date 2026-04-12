mod client;
mod cmd;
mod output;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "crawl",
    version,
    about = "System services CLI — Bluetooth, network, audio, brightness and more",
    long_about = None,
)]
struct Cli {
    /// Override the daemon socket path
    #[arg(long, env = "CRAWL_SOCKET", global = true)]
    socket: Option<String>,

    /// Output raw JSON instead of formatted output
    #[arg(long, short = 'j', global = true)]
    json: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Bluetooth management
    Bluetooth(cmd::bluetooth::BtArgs),
    /// Network management
    Network(cmd::network::NetArgs),
    /// Notification control
    Notify(cmd::notify::NotifyArgs),
    /// Clipboard access
    Clip(cmd::clip::ClipArgs),
    /// System monitoring (CPU, memory, disk)
    Sysmon(cmd::sysmon::SysmonArgs),
    /// Display brightness control
    Brightness(cmd::brightness::BrightnessArgs),
    /// Process management
    Proc(cmd::proc_::ProcArgs),
    /// Media player control (MPRIS)
    Media(cmd::media::MediaArgs),
    /// Battery and power status
    Power(cmd::power::PowerArgs),
    /// Disk and removable media management
    Disk(cmd::disk::DiskArgs),
    /// Audio volume and devices
    Audio(cmd::audio::AudioArgs),
    /// Daemon control
    Daemon(cmd::daemon::DaemonArgs),
}

#[tokio::main]
async fn main() -> Result<()> {
    let _ = color_eyre::install();

    let cli = Cli::parse();

    let socket_path = cli.socket.unwrap_or_else(|| {
        let runtime_dir = std::env::var("XDG_RUNTIME_DIR")
            .unwrap_or_else(|_| format!("/run/user/{}", unsafe { libc::getuid() }));
        format!("{runtime_dir}/crawl.sock")
    });

    let client = client::CrawlClient::new(socket_path);
    let json_mode = cli.json;

    match cli.command {
        Commands::Bluetooth(args)         => cmd::bluetooth::run(client, args, json_mode).await?,
        Commands::Network(args)           => cmd::network::run(client, args, json_mode).await?,
        Commands::Notify(args)            => cmd::notify::run(client, args, json_mode).await?,
        Commands::Clip(args)             => cmd::clip::run(client, args, json_mode).await?,
        Commands::Sysmon(args)           => cmd::sysmon::run(client, args, json_mode).await?,
        Commands::Brightness(args)       => cmd::brightness::run(client, args, json_mode).await?,
        Commands::Proc(args)             => cmd::proc_::run(client, args, json_mode).await?,
        Commands::Media(args)            => cmd::media::run(client, args, json_mode).await?,
        Commands::Power(args)            => cmd::power::run(client, args, json_mode).await?,
        Commands::Disk(args)             => cmd::disk::run(client, args, json_mode).await?,
        Commands::Audio(args)            => cmd::audio::run(client, args, json_mode).await?,
        Commands::Daemon(args)           => cmd::daemon::run(client, args, json_mode).await?,
    }

    Ok(())
}
