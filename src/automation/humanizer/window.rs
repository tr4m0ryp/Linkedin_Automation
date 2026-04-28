//! D5.D: daily activity window plus per-day session counters.
//!
//! `ActivityWindow` parses two `"HH:MM"` strings into a local-time interval
//! and answers "is the runner allowed to act right now?". `SessionStats`
//! persists daily send counters across process restarts so a crash mid-day
//! does not reset the cap.

use crate::config::HumanizerConfig;
use crate::error::{LinkedInError, Result};
use chrono::{DateTime, Duration as ChronoDuration, Local, NaiveTime};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;
use tracing::{debug, warn};

/// Local-time send window parsed from `HumanizerConfig::daily_window_*`.
#[derive(Debug, Clone, Copy)]
pub struct ActivityWindow {
    /// Window open time, local zone.
    pub start: NaiveTime,
    /// Window close time, local zone.
    pub end: NaiveTime,
}

impl ActivityWindow {
    /// Parse the configured `"HH:MM"` strings. Falls back to a 09:00-19:00
    /// window if either string is malformed.
    pub fn from_config(cfg: &HumanizerConfig) -> Self {
        let start = parse_hhmm(&cfg.daily_window_start).unwrap_or_else(|| {
            warn!(
                value = cfg.daily_window_start.as_str(),
                "Unparsable DAILY_WINDOW_START -- defaulting to 09:00"
            );
            NaiveTime::from_hms_opt(9, 0, 0).unwrap_or_default()
        });
        let end = parse_hhmm(&cfg.daily_window_end).unwrap_or_else(|| {
            warn!(
                value = cfg.daily_window_end.as_str(),
                "Unparsable DAILY_WINDOW_END -- defaulting to 19:00"
            );
            NaiveTime::from_hms_opt(19, 0, 0).unwrap_or_default()
        });
        Self { start, end }
    }

    /// True if the local time of `now` is in `[start, end]`. When the window
    /// crosses midnight (`end < start`) the predicate is `>= start || <= end`.
    pub fn is_open_now(&self) -> bool {
        let now = Local::now().time();
        self.contains(now)
    }

    /// Pure helper for testing: does `t` fall inside the window?
    pub fn contains(&self, t: NaiveTime) -> bool {
        if self.start <= self.end {
            t >= self.start && t <= self.end
        } else {
            // Window crosses midnight (e.g. 22:00 - 06:00).
            t >= self.start || t <= self.end
        }
    }

    /// Duration from `now` until the next window open. Returns `Duration::ZERO`
    /// if the window is currently open.
    pub fn time_until_open(&self) -> Duration {
        if self.is_open_now() {
            return Duration::ZERO;
        }

        let now = Local::now();
        let today_start = match now
            .date_naive()
            .and_time(self.start)
            .and_local_timezone(Local)
        {
            chrono::LocalResult::Single(dt) => dt,
            _ => return Duration::from_secs(60),
        };

        let target = if today_start > now {
            today_start
        } else {
            today_start + ChronoDuration::days(1)
        };

        let delta = target.signed_duration_since(now);
        match delta.to_std() {
            Ok(d) => d,
            Err(_) => Duration::from_secs(60),
        }
    }
}

fn parse_hhmm(s: &str) -> Option<NaiveTime> {
    NaiveTime::parse_from_str(s.trim(), "%H:%M").ok()
}

/// Per-day counters persisted to disk so a restart same-day preserves caps.
///
/// Stored as compact JSON. Missing or unreadable file -> `Default::default()`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionStats {
    /// Local-zone date key, format `YYYY-MM-DD`. Used to detect day rollover.
    pub date: String,
    /// Connection requests successfully issued today.
    pub sends_today: u32,
    /// Sends since the last long break -- drives `BreakScheduler`.
    pub sends_since_last_break: u32,
    /// Local timestamp of the most recent send, if any.
    pub last_send_at: Option<DateTime<Local>>,
}

impl SessionStats {
    /// Load from disk. Returns `Default::default()` on any error.
    pub fn load(path: &str) -> Self {
        match std::fs::read_to_string(path) {
            Ok(s) => serde_json::from_str(&s).unwrap_or_else(|e| {
                warn!(path = path, error = %e, "Failed to parse session stats; resetting");
                Self::default()
            }),
            Err(e) => {
                debug!(path = path, error = %e, "No existing session stats; starting fresh");
                Self::default()
            },
        }
    }

    /// Persist to disk, creating parent directories as needed.
    pub fn save(&self, path: &str) -> Result<()> {
        if let Some(parent) = Path::new(path).parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    LinkedInError::StorageError(format!(
                        "Failed to create stats directory {:?}: {}",
                        parent, e
                    ))
                })?;
            }
        }
        let body = serde_json::to_string_pretty(self).map_err(|e| {
            LinkedInError::StorageError(format!("Failed to serialize session stats: {}", e))
        })?;
        std::fs::write(path, body).map_err(|e| {
            LinkedInError::StorageError(format!("Failed to write session stats {}: {}", path, e))
        })?;
        Ok(())
    }

    /// If today (local) differs from the stored `date`, zero the counters and
    /// stamp the new date.
    pub fn reset_if_new_day(&mut self) {
        let today = Local::now().format("%Y-%m-%d").to_string();
        if self.date != today {
            debug!(
                old = self.date.as_str(),
                new = today.as_str(),
                "Session stats day rollover"
            );
            self.date = today;
            self.sends_today = 0;
            self.sends_since_last_break = 0;
        }
    }

    /// Record one successful send; update timestamp and counters.
    pub fn record_send(&mut self) {
        self.reset_if_new_day();
        self.sends_today = self.sends_today.saturating_add(1);
        self.sends_since_last_break = self.sends_since_last_break.saturating_add(1);
        self.last_send_at = Some(Local::now());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_open_now_for_normal_window_around_noon() {
        let cfg = HumanizerConfig {
            daily_window_start: "00:00".to_string(),
            daily_window_end: "23:59".to_string(),
            ..HumanizerConfig::default()
        };
        let w = ActivityWindow::from_config(&cfg);
        // A 24-hour window must contain the current local time.
        assert!(w.is_open_now());
    }

    #[test]
    fn contains_handles_simple_window() {
        let w = ActivityWindow {
            start: NaiveTime::from_hms_opt(9, 0, 0).unwrap(),
            end: NaiveTime::from_hms_opt(17, 0, 0).unwrap(),
        };
        assert!(w.contains(NaiveTime::from_hms_opt(12, 0, 0).unwrap()));
        assert!(w.contains(NaiveTime::from_hms_opt(9, 0, 0).unwrap()));
        assert!(w.contains(NaiveTime::from_hms_opt(17, 0, 0).unwrap()));
        assert!(!w.contains(NaiveTime::from_hms_opt(8, 59, 0).unwrap()));
        assert!(!w.contains(NaiveTime::from_hms_opt(17, 1, 0).unwrap()));
    }

    #[test]
    fn contains_handles_overnight_window() {
        let w = ActivityWindow {
            start: NaiveTime::from_hms_opt(22, 0, 0).unwrap(),
            end: NaiveTime::from_hms_opt(6, 0, 0).unwrap(),
        };
        assert!(w.contains(NaiveTime::from_hms_opt(23, 0, 0).unwrap()));
        assert!(w.contains(NaiveTime::from_hms_opt(2, 0, 0).unwrap()));
        assert!(!w.contains(NaiveTime::from_hms_opt(12, 0, 0).unwrap()));
    }

    #[test]
    fn parse_hhmm_accepts_padded_and_trims() {
        assert!(parse_hhmm("09:00").is_some());
        assert!(parse_hhmm("  19:30 ").is_some());
        assert!(parse_hhmm("not a time").is_none());
    }

    #[test]
    fn record_send_increments_counters() {
        let mut stats = SessionStats::default();
        let before = stats.sends_today;
        stats.record_send();
        assert_eq!(stats.sends_today, before + 1);
        assert_eq!(stats.sends_since_last_break, 1);
        assert!(stats.last_send_at.is_some());
    }

    #[test]
    fn reset_if_new_day_zeros_counters_when_date_changes() {
        let mut stats = SessionStats {
            date: "1970-01-01".to_string(),
            sends_today: 5,
            sends_since_last_break: 3,
            last_send_at: None,
        };
        stats.reset_if_new_day();
        assert_eq!(stats.sends_today, 0);
        assert_eq!(stats.sends_since_last_break, 0);
        assert_ne!(stats.date, "1970-01-01");
    }
}
