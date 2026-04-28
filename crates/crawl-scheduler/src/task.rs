//! Task state management for the scheduler

use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShouldRun {
    Yes,
    No,
}

#[derive(Clone)]
pub struct TaskState {
    pub id: &'static str,
    pub interval: Duration,
    pub jitter: Duration,
    pub next_run: Instant,
    pub last_run: Instant,
}

impl TaskState {
    pub fn new(id: &'static str, interval: Duration, jitter_pct: f32) -> Self {
        let jitter = Duration::from_secs_f64(interval.as_secs_f64() * jitter_pct as f64);
        let now = Instant::now();

        let initial_delay = if jitter.is_zero() {
            Duration::ZERO
        } else {
            Duration::from_secs_f64(fastrand::u64(0..=jitter.as_millis() as u64) as f64 / 1000.0)
        };

        Self {
            id,
            interval,
            jitter,
            next_run: now + initial_delay,
            last_run: now - interval,
        }
    }

    pub fn should_run(&self) -> ShouldRun {
        if Instant::now() >= self.next_run {
            ShouldRun::Yes
        } else {
            ShouldRun::No
        }
    }

    pub fn mark_ran(&mut self) {
        let now = Instant::now();
        let jitter_value = if self.jitter.is_zero() {
            Duration::ZERO
        } else {
            Duration::from_secs_f64(
                fastrand::u64(0..=self.jitter.as_millis() as u64) as f64 / 1000.0,
            )
        };
        self.last_run = now;
        self.next_run = now + self.interval + jitter_value;
    }

    pub fn reset(&mut self) {
        let now = Instant::now();
        self.next_run = now;
        self.last_run = now - self.interval;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_state_initial() {
        let task = TaskState::new("test", Duration::from_secs(1), 0.1);
        assert!(matches!(task.should_run(), ShouldRun::Yes));
    }

    #[test]
    fn test_task_state_after_mark_ran() {
        let mut task = TaskState::new("test", Duration::from_secs(1), 0.0);
        task.mark_ran();
        assert!(matches!(task.should_run(), ShouldRun::No));
    }
}
