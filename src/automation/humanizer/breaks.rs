//! D5.E: long-break scheduling between bursts of sends.
//!
//! After every `next_break_at_count` sends the scheduler returns a randomly
//! sized break duration; the runner sleeps for that duration. The threshold
//! is re-rolled after each break so the cadence stays unpredictable.

use crate::config::HumanizerConfig;
use rand::Rng;
use std::time::Duration;
use tracing::debug;

/// Schedule and emit mid-session break durations.
pub struct BreakScheduler {
    /// Sends-since-last-break threshold that, once reached, triggers a break.
    next_break_at_count: u32,
    /// Inclusive lower bound for the next threshold roll.
    min_sends: u32,
    /// Inclusive upper bound for the next threshold roll.
    max_sends: u32,
    /// Inclusive lower bound for the next break duration roll, in seconds.
    min_duration_secs: u64,
    /// Inclusive upper bound for the next break duration roll, in seconds.
    max_duration_secs: u64,
}

impl BreakScheduler {
    /// Build a scheduler from configuration. The first break threshold is
    /// rolled immediately so behavior is randomized from the very first send.
    pub fn from_config(cfg: &HumanizerConfig) -> Self {
        let min_sends = cfg.break_every_min_sends.max(1);
        let max_sends = cfg.break_every_max_sends.max(min_sends);
        let min_duration_secs = cfg.break_duration_min_secs;
        let max_duration_secs = cfg.break_duration_max_secs.max(min_duration_secs);

        let next_break_at_count = roll_threshold(min_sends, max_sends);
        Self {
            next_break_at_count,
            min_sends,
            max_sends,
            min_duration_secs,
            max_duration_secs,
        }
    }

    /// Inspect the running send count after a successful send. Returns
    /// `Some(duration)` when a break should be taken (and re-rolls the
    /// threshold internally), otherwise `None`.
    ///
    /// `sends_since_last_break` is the count maintained by `SessionStats`. It
    /// is the responsibility of the caller to reset that counter to zero
    /// after sleeping for the returned break.
    pub fn check_after_send(&mut self, sends_since_last_break: u32) -> Option<Duration> {
        if sends_since_last_break < self.next_break_at_count {
            return None;
        }
        let secs = roll_duration(self.min_duration_secs, self.max_duration_secs);
        let new_threshold = roll_threshold(self.min_sends, self.max_sends);
        debug!(
            previous_threshold = self.next_break_at_count,
            sends_since_last_break = sends_since_last_break,
            break_secs = secs,
            new_threshold = new_threshold,
            "BreakScheduler emitting break"
        );
        self.next_break_at_count = new_threshold;
        Some(Duration::from_secs(secs))
    }

    /// Currently scheduled threshold (sends-between-breaks). Useful for tests
    /// and observability.
    pub fn next_threshold(&self) -> u32 {
        self.next_break_at_count
    }
}

fn roll_threshold(min: u32, max: u32) -> u32 {
    let mut rng = rand::thread_rng();
    if min >= max {
        return min;
    }
    rng.gen_range(min..=max)
}

fn roll_duration(min_secs: u64, max_secs: u64) -> u64 {
    let mut rng = rand::thread_rng();
    if min_secs >= max_secs {
        return min_secs;
    }
    rng.gen_range(min_secs..=max_secs)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> HumanizerConfig {
        HumanizerConfig {
            break_every_min_sends: 3,
            break_every_max_sends: 7,
            break_duration_min_secs: 1200,
            break_duration_max_secs: 3600,
            ..HumanizerConfig::default()
        }
    }

    #[test]
    fn threshold_starts_within_configured_bounds() {
        for _ in 0..32 {
            let s = BreakScheduler::from_config(&cfg());
            let t = s.next_threshold();
            assert!((3..=7).contains(&t), "threshold {} out of [3,7]", t);
        }
    }

    #[test]
    fn check_after_send_returns_none_below_threshold() {
        let mut s = BreakScheduler::from_config(&cfg());
        let t = s.next_threshold();
        if t > 1 {
            assert!(s.check_after_send(t - 1).is_none());
        }
    }

    #[test]
    fn check_after_send_returns_break_when_threshold_reached() {
        let mut s = BreakScheduler::from_config(&cfg());
        let t = s.next_threshold();
        let dur = s.check_after_send(t).expect("break expected at threshold");
        assert!(dur >= Duration::from_secs(1200));
        assert!(dur <= Duration::from_secs(3600));
    }

    #[test]
    fn threshold_rerolls_after_break() {
        let mut s = BreakScheduler::from_config(&cfg());
        let first = s.next_threshold();
        let _ = s.check_after_send(first);
        let second = s.next_threshold();
        assert!((3..=7).contains(&second));
    }
}
