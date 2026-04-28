//! Anti-detection humanizer module (D5 tactics A-E).
//!
//! Composed of leaf submodules:
//! - `delay`   -- D5.A lognormal inter-send delay
//! - `window`  -- D5.D activity window plus persisted daily counters
//! - `breaks`  -- D5.E break scheduling
//! - `decoy`   -- D5.B decoy browsing orchestration
//!
//! Plus a top-level `Humanizer` facade that bundles the four sub-types and
//! the small bit of glue logic (D5.C skip-the-send probability, daily cap
//! check, post-send sleep wiring).

mod breaks;
mod decoy;
mod delay;
mod window;

pub use breaks::BreakScheduler;
pub use decoy::DecoyBrowser;
pub use delay::LogNormalDelay;
pub use window::{ActivityWindow, SessionStats};

use crate::config::HumanizerConfig;
use crate::error::Result;
use crate::linkedin_api::LinkedInClient;
use rand::Rng;
use tokio::time::sleep;
use tracing::{info, warn};

/// Default location for the persisted session-stats JSON file.
pub const DEFAULT_STATE_PATH: &str = "sessions/humanizer_state.json";

/// One-stop bundle of every humanizer sub-component.
///
/// The runner only constructs a `Humanizer` and invokes its high-level
/// methods (`pre_action`, `should_skip_send`, `cap_reached`, `after_send`,
/// `wait_for_window_open`); the inner sub-types stay accessible through the
/// public fields for tests or specialized callers that need finer control.
pub struct Humanizer {
    /// Lognormal delay sampler (D5.A).
    pub delay: LogNormalDelay,
    /// Local-time activity window (D5.D).
    pub window: ActivityWindow,
    /// Break scheduler (D5.E).
    pub breaks: BreakScheduler,
    /// Decoy browsing orchestrator (D5.B).
    pub decoy: DecoyBrowser,
    /// Persisted per-day counters (D5.D).
    pub stats: SessionStats,
    /// Probability in [0.0, 1.0] of skipping an otherwise-eligible send (D5.C).
    pub skip_send_probability: f64,
    /// Hard daily cap on connection requests.
    pub daily_send_cap: u32,
    /// Disk path used for the persisted `SessionStats`.
    pub state_path: String,
}

impl Humanizer {
    /// Build a `Humanizer` from configuration plus the JSON state path.
    ///
    /// Loads any pre-existing `SessionStats` from `state_path`; missing or
    /// unreadable files reset to defaults. The day rollover check fires
    /// immediately so a stale stats file from yesterday does not poison
    /// today's counters.
    pub fn from_config(cfg: &HumanizerConfig, state_path: &str) -> Self {
        let mut stats = SessionStats::load(state_path);
        stats.reset_if_new_day();
        Self {
            delay: LogNormalDelay::from_config(cfg),
            window: ActivityWindow::from_config(cfg),
            breaks: BreakScheduler::from_config(cfg),
            decoy: DecoyBrowser::from_config(cfg.me_ping_every_n_sends),
            stats,
            skip_send_probability: cfg.skip_send_probability,
            daily_send_cap: cfg.daily_send_cap,
            state_path: state_path.to_string(),
        }
    }

    /// D5.C: roll a uniform `[0,1)` and report whether to skip this send.
    pub fn should_skip_send(&self) -> bool {
        if self.skip_send_probability <= 0.0 {
            return false;
        }
        let mut rng = rand::thread_rng();
        rng.gen_bool(self.skip_send_probability.clamp(0.0, 1.0))
    }

    /// True when the daily send cap has been reached.
    pub fn cap_reached(&self) -> bool {
        self.stats.sends_today >= self.daily_send_cap
    }

    /// Pre-action humanization: warm the connection with decoy GETs and
    /// random reading pauses. Bubbles `SessionExpired` / `RateLimitExceeded`.
    pub async fn pre_action(&mut self, client: &LinkedInClient) -> Result<()> {
        self.decoy.browse_before_action(client).await
    }

    /// Post-send wiring: persist counters, optionally sleep for a long
    /// break, then sleep for the next inter-send delay. Cancellation-safe:
    /// no locks are held across the awaits.
    pub async fn after_send(&mut self) -> Result<()> {
        self.stats.record_send();
        if let Err(e) = self.stats.save(&self.state_path) {
            warn!(error = %e, "Failed to persist humanizer session stats");
        }

        let break_dur = self
            .breaks
            .check_after_send(self.stats.sends_since_last_break);
        if let Some(d) = break_dur {
            info!(
                seconds = d.as_secs(),
                "Humanizer taking long break before next send"
            );
            sleep(d).await;
            self.stats.sends_since_last_break = 0;
            if let Err(e) = self.stats.save(&self.state_path) {
                warn!(error = %e, "Failed to persist humanizer state after break");
            }
        }

        let delay_dur = self.delay.sample();
        info!(
            seconds = delay_dur.as_secs(),
            "Humanizer inter-send delay before next action"
        );
        sleep(delay_dur).await;
        Ok(())
    }

    /// Sleep until the configured activity window opens. Returns immediately
    /// when the window is already open.
    pub async fn wait_for_window_open(&self) {
        let wait = self.window.time_until_open();
        if wait.is_zero() {
            return;
        }
        info!(
            seconds = wait.as_secs(),
            "Outside activity window -- sleeping until next open"
        );
        sleep(wait).await;
    }
}
