//! crawl-proc: Process listing and management via sysinfo.
//!
//! Exposes process enumeration, search, kill, and PID watching.
//! The domain task is mostly reactive; it parks until HTTP handlers call it.

use crawl_ipc::types::ProcessInfo;
use serde::{Deserialize, Serialize};
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, RefreshKind, Signal, System};
use thiserror::Error;
use tokio::sync::broadcast;
use tracing::info;
use crawl_ipc::events::CrawlEvent;

// ── Config ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Default sort field: cpu | mem | pid | name
    pub default_sort: String,
    /// Default number of top processes to return
    pub default_top: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self { default_sort: "cpu".into(), default_top: 30 }
    }
}

// ── Error ────────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum ProcError {
    #[error("process not found: PID {0}")]
    NotFound(u32),
    #[error("permission denied killing PID {0}")]
    PermissionDenied(u32),
    #[error("signal failed: {0}")]
    SignalFailed(String),
}

// ── Domain runner ─────────────────────────────────────────────────────────────

pub async fn run(_cfg: Config, _tx: broadcast::Sender<CrawlEvent>) -> anyhow::Result<()> {
    info!("crawl-proc starting");
    // Process domain is fully request-driven.
    // Future enhancement: watch a set of PIDs and emit ProcEvents when they exit.
    std::future::pending::<()>().await;
    Ok(())
}

// ── Public query API ──────────────────────────────────────────────────────────

pub fn list_processes(sort_by: &str, top: usize) -> Vec<ProcessInfo> {
    let mut sys = System::new_with_specifics(
        RefreshKind::new().with_processes(ProcessRefreshKind::everything()),
    );
    sys.refresh_processes(ProcessesToUpdate::All);

    let mut procs: Vec<ProcessInfo> = sys.processes().values().map(|p| ProcessInfo {
        pid:         p.pid().as_u32(),
        name:        p.name().to_string_lossy().to_string(),
        cpu_percent: p.cpu_usage(),
        mem_rss_kb:  p.memory() / 1024,
        status:      format!("{:?}", p.status()),
        user:        None, // TODO: resolve uid via users crate
        cmd:         p.cmd().iter().map(|s| s.to_string_lossy().to_string()).collect(),
    }).collect();

    match sort_by {
        "mem"  => procs.sort_by(|a, b| b.mem_rss_kb.cmp(&a.mem_rss_kb)),
        "pid"  => procs.sort_by_key(|p| p.pid),
        "name" => procs.sort_by(|a, b| a.name.cmp(&b.name)),
        _      => procs.sort_by(|a, b| b.cpu_percent.partial_cmp(&a.cpu_percent).unwrap_or(std::cmp::Ordering::Equal)),
    }

    procs.truncate(top);
    procs
}

pub fn find_processes(name: &str) -> Vec<ProcessInfo> {
    let all = list_processes("cpu", usize::MAX);
    all.into_iter()
        .filter(|p| p.name.to_lowercase().contains(&name.to_lowercase()))
        .collect()
}

pub fn kill_process(pid: u32, force: bool) -> Result<(), ProcError> {
    let mut sys = System::new_with_specifics(
        RefreshKind::new().with_processes(ProcessRefreshKind::everything()),
    );
    sys.refresh_processes(ProcessesToUpdate::All);

    let sysinfo_pid = sysinfo::Pid::from_u32(pid);
    let process = sys.process(sysinfo_pid).ok_or(ProcError::NotFound(pid))?;

    let signal = if force { Signal::Kill } else { Signal::Term };
    process.kill_with(signal).ok_or_else(|| ProcError::SignalFailed(format!("kill({pid}) failed")))?;
    Ok(())
}

/// Watch a PID and return when it exits. Polls every 500ms.
pub async fn watch_pid(pid: u32) -> Result<String, ProcError> {
    let mut sys = System::new_with_specifics(
        RefreshKind::new().with_processes(ProcessRefreshKind::everything()),
    );
    sys.refresh_processes(ProcessesToUpdate::All);
    let name = match sys.process(sysinfo::Pid::from_u32(pid)) {
        Some(proc_) => proc_.name().to_string_lossy().to_string(),
        None => {
            return Err(ProcError::NotFound(pid));
        }
    };

    if name.is_empty() {
        return Err(ProcError::NotFound(pid));
    }

    loop {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        sys.refresh_processes(ProcessesToUpdate::All);
        if sys.process(sysinfo::Pid::from_u32(pid)).is_none() {
            break;
        }
    }

    Ok(name)
}
