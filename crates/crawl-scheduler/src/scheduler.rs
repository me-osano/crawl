//! Central scheduler for timing domain tasks

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

use crate::task::{ShouldRun, TaskState};

#[derive(Clone)]
pub struct Scheduler {
    tasks: Arc<Mutex<HashMap<&'static str, TaskState>>>,
}

impl Scheduler {
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn register(&self, id: &'static str, interval: Duration, jitter_pct: f32) {
        let state = TaskState::new(id, interval, jitter_pct);
        let mut tasks = self.tasks.lock().await;
        tasks.insert(id, state);
    }

    pub async fn unregister(&self, id: &'static str) {
        let mut tasks = self.tasks.lock().await;
        tasks.remove(id);
    }

    pub async fn should_run(&self, id: &'static str) -> ShouldRun {
        let tasks = self.tasks.lock().await;
        match tasks.get(id) {
            Some(state) => state.should_run(),
            None => ShouldRun::Yes,
        }
    }

    pub async fn mark_ran(&self, id: &'static str) {
        let mut tasks = self.tasks.lock().await;
        if let Some(state) = tasks.get_mut(id) {
            state.mark_ran();
        }
    }

    pub async fn reset(&self, id: &'static str) {
        let mut tasks = self.tasks.lock().await;
        if let Some(state) = tasks.get_mut(id) {
            state.reset();
        }
    }

    pub async fn wait_interval(&self, id: &'static str, poll_interval_ms: u64) {
        loop {
            match self.should_run(id).await {
                ShouldRun::Yes => return,
                ShouldRun::No => {
                    tokio::time::sleep(Duration::from_millis(poll_interval_ms)).await;
                }
            }
        }
    }

    pub async fn tasks(&self) -> Vec<&'static str> {
        let tasks = self.tasks.lock().await;
        tasks.keys().copied().collect()
    }

    pub async fn task_count(&self) -> usize {
        let tasks = self.tasks.lock().await;
        tasks.len()
    }
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_register() {
        let scheduler = Scheduler::new();
        scheduler.register("test", Duration::from_secs(1), 0.1).await;
        assert_eq!(scheduler.task_count().await, 1);
    }

    #[tokio::test]
    async fn test_should_run() {
        let scheduler = Scheduler::new();
        scheduler.register("test", Duration::from_secs(1), 0.1).await;
        assert!(matches!(scheduler.should_run("test").await, ShouldRun::Yes));
    }

    #[tokio::test]
    async fn test_mark_ran() {
        let scheduler = Scheduler::new();
        scheduler.register("test", Duration::from_secs(1), 0.0).await;
        scheduler.mark_ran("test").await;
        assert!(matches!(scheduler.should_run("test").await, ShouldRun::No));
    }

    #[tokio::test]
    async fn test_unregister() {
        let scheduler = Scheduler::new();
        scheduler.register("test", Duration::from_secs(1), 0.1).await;
        scheduler.unregister("test").await;
        assert_eq!(scheduler.task_count().await, 0);
    }
}