//! # Crawl Scheduler
//!
//! Central scheduler with jitter for timing domain tasks.
//!
//! ## Architecture
//!
//! ```text
//! Scheduler ──► TaskRegistry ──► TaskState (per task)
//!                            ├── interval
//!                            ├── jitter
//!                            └── next_run
//! ```
//!
//! ## Usage
//!
//! Domains can optionally use the scheduler for timing:
//!
//! ```rust
//! use crawl_scheduler::{Scheduler, ShouldRun};
//! use std::time::Duration;
//!
//! pub async fn run(cfg: Config, tx: Sender) -> Result<()> {
//!     run_with_scheduler(cfg, tx, None).await
//! }
//!
//! pub async fn run_with_scheduler(
//!     cfg: Config,
//!     tx: Sender,
//!     scheduler: Option<Scheduler>,
//! ) -> Result<()> {
//!     let task_id = "mytask";
//!     if let Some(ref sched) = scheduler {
//!         sched.register(task_id, Duration::from_secs(1), 0.1).await;
//!     }
//!
//!     loop {
//!         if let Some(ref sched) = scheduler {
//!             sched.wait_interval(task_id, 500).await;
//!             if sched.should_run(task_id).await == ShouldRun::Yes {
//!                 // do work
//!                 sched.mark_ran(task_id).await;
//!             } else {
//!                 continue;
//!             }
//!         } else {
//!             tokio::time::sleep(Duration::from_secs(1)).await;
//!         }
//!     }
//! }
//! ```
//!
//! ## Task Registration
//!
//! | Task | Interval | Jitter |
//! |------|----------|--------|
//! | sysmon | 1s | 5% |
//! | network_fast | 5s | 10% |
//! | network_slow | 30s | 15% |
//! | bluetooth | 5s | 10% |
//! | clipboard | 500ms | 10% |

pub mod scheduler;
pub mod task;

#[cfg(feature = "sync")]
pub mod blocking;

pub use scheduler::Scheduler;
pub use task::{ShouldRun, TaskState};

#[cfg(feature = "sync")]
pub use blocking::BlockingScheduler;