use figment::{
    providers::{Env, Format, Serialized, Toml},
    Figment,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(skip)]
    pub config_path: PathBuf,
    pub daemon: DaemonConfig,
    pub sysinfo: crawl_sysinfo::config::Config,
    pub audio: crawl_audio::Config,
    pub display: crawl_display::DisplayConfig,
    pub bluetooth: crawl_bluetooth::Config,
    pub network: crawl_network::Config,
    pub sysmon: crawl_sysmon::Config,
    pub processes: crawl_proc::Config,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonConfig {
    pub log_level: String,
    pub socket_path: String,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        let runtime_dir = std::env::var("XDG_RUNTIME_DIR")
            .unwrap_or_else(|_| format!("/run/user/{}", unsafe { libc::getuid() }));
        Self {
            log_level: "info".into(),
            socket_path: format!("{runtime_dir}/crawl.sock"),
        }
    }
}

pub fn load() -> anyhow::Result<Config> {
    let config_home = std::env::var("XDG_CONFIG_HOME")
        .unwrap_or_else(|_| format!("{}/.config", std::env::var("HOME").unwrap_or_default()));
    let config_path = PathBuf::from(&config_home).join("crawl").join("config.toml");

    let config: Config = Figment::from(Serialized::defaults(Config::default()))
        .merge(Toml::file(&config_path))
        .merge(Env::prefixed("CRAWL_").split("__"))
        .extract()?;

    // Validate config
    config.validate()?;

    Ok(Config {
        config_path,
        ..config
    })
}

impl Config {
    /// Validate the configuration.
    fn validate(&self) -> anyhow::Result<()> {
        if self.daemon.socket_path.is_empty() {
            anyhow::bail!("daemon.socket_path cannot be empty");
        }

        if !(0.0..=100.0).contains(&self.sysmon.cpu_spike_threshold) {
            anyhow::bail!("sysmon.cpu_spike_threshold must be 0.0-100.0");
        }
        if !(0.0..=100.0).contains(&self.sysmon.mem_pressure_threshold) {
            anyhow::bail!("sysmon.mem_pressure_threshold must be 0.0-100.0");
        }
        if self.sysmon.poll_interval_ms == 0 {
            anyhow::bail!("sysmon.poll_interval_ms must be > 0");
        }

        if self.processes.top == 0 {
            anyhow::bail!("processes.top must be > 0");
        }
        if self.processes.top_interval_ms == 0 {
            anyhow::bail!("processes.top_interval_ms must be > 0");
        }

        Ok(())
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            config_path: PathBuf::new(),
            daemon: DaemonConfig::default(),
            sysinfo: crawl_sysinfo::config::Config::default(),
            audio: crawl_audio::Config::default(),
            display: crawl_display::DisplayConfig::default(),
            bluetooth: crawl_bluetooth::Config::default(),
            network: crawl_network::Config::default(),
            sysmon: crawl_sysmon::Config::default(),
            processes: crawl_proc::Config::default(),
        }
    }
}