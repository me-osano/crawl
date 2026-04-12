use figment::{
    providers::{Env, Format, Serialized, Toml},
    Figment,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Top-level daemon config — mirrors crawl.toml structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(skip)]
    pub config_path: PathBuf,
    pub daemon: DaemonConfig,
    pub bluetooth: crawl_bluetooth::Config,
    pub network: crawl_network::Config,
    pub notifications: crawl_notify::Config,
    pub clipboard: crawl_clipboard::Config,
    pub sysmon: crawl_sysmon::Config,
    pub brightness: crawl_brightness::Config,
    pub processes: crawl_proc::Config,
    pub media: crawl_media::Config,
    pub power: crawl_power::Config,
    pub disk: crawl_disk::Config,
    pub audio: crawl_audio::Config,
    pub theme: crawl_theme::Config,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonConfig {
    /// Path to the Unix socket. Defaults to $XDG_RUNTIME_DIR/crawl.sock
    pub socket_path: String,
    pub log_level: String,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        let runtime_dir = std::env::var("XDG_RUNTIME_DIR")
            .unwrap_or_else(|_| format!("/run/user/{}", unsafe { libc::getuid() }));
        Self {
            socket_path: format!("{runtime_dir}/crawl.sock"),
            log_level: "info".into(),
        }
    }
}

pub fn load() -> anyhow::Result<Config> {
    let config_home = std::env::var("XDG_CONFIG_HOME")
        .unwrap_or_else(|_| format!("{}/.config", std::env::var("HOME").unwrap_or_default()));
    let config_path = PathBuf::from(&config_home).join("crawl").join("crawl.toml");

    let mut config: Config = Figment::from(Serialized::defaults(Config::default()))
        .merge(Toml::file(&config_path))
        .merge(Env::prefixed("CRAWL_").split("__"))
        .extract()?;

    config.config_path = config_path;
    Ok(config)
}

impl Default for Config {
    fn default() -> Self {
        Self {
            config_path: PathBuf::new(),
            daemon: DaemonConfig::default(),
            bluetooth: crawl_bluetooth::Config::default(),
            network: crawl_network::Config::default(),
            notifications: crawl_notify::Config::default(),
            clipboard: crawl_clipboard::Config::default(),
            sysmon: crawl_sysmon::Config::default(),
            brightness: crawl_brightness::Config::default(),
            processes: crawl_proc::Config::default(),
            media: crawl_media::Config::default(),
            power: crawl_power::Config::default(),
            disk: crawl_disk::Config::default(),
            audio: crawl_audio::Config::default(),
            theme: crawl_theme::Config::default(),
        }
    }
}
