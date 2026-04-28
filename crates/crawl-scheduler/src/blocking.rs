//! Blocking (sync) scheduler variant for use outside async contexts

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::task::TaskState;

pub struct BlockingScheduler {
    tasks: Arc<Mutex<HashMap<&'static str, TaskState>>>,
}

impl BlockingScheduler {
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn register(&self, id: &'static str, interval: Duration, jitter_pct: f32) {
        let state = TaskState::new(id, interval, jitter_pct);
        let mut tasks = self.tasks.lock().unwrap();
        tasks.insert(id, state);
    }

    pub fn should_run(&self, id: &'static str) -> bool {
        let tasks = self.tasks.lock().unwrap();
        matches!(tasks.get(id).map(|t| t.should_run()), Some(true))
    }

    pub fn mark_ran(&self, id: &'static str) {
        let mut tasks = self.tasks.lock().unwrap();
        if let Some(state) = tasks.get_mut(id) {
            state.mark_ran();
        }
    }

    pub fn wait_until_ready(&self, id: &'static str) {
        loop {
            if self.should_run(id) {
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}

impl Default for BlockingScheduler {
    fn default() -> Self {
        Self::new()
    }
}
