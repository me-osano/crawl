use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    pub enabled: bool,
}

impl Config {
    pub async fn run(&self, _tx: tokio::sync::broadcast::Sender<crawl_ipc::CrawlEvent>) -> anyhow::Result<()> {
        Ok(())
    }
}