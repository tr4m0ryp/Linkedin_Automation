//! D5.B: orchestrate decoy browsing before real actions.
//!
//! Calls the lightweight GET helpers on `LinkedInClient` (`get_feed_updates`,
//! `get_notifications`, `ping_me`) in a randomized order with random
//! "reading" pauses between them. Total elapsed time per call is roughly
//! 30 seconds to 3 minutes. Auth and rate-limit errors propagate; everything
//! else is logged and swallowed since decoy traffic is non-critical.

use crate::error::{LinkedInError, Result};
use crate::linkedin_api::LinkedInClient;
use rand::seq::SliceRandom;
use rand::Rng;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, warn};

/// Internal label for which decoy endpoint to hit.
#[derive(Debug, Clone, Copy)]
enum Decoy {
    Feed,
    Notifications,
    Me,
}

/// Orchestrates a humanized sequence of decoy GETs.
pub struct DecoyBrowser {
    /// Issue a `/me` ping every N sends. The default mirrors `HumanizerConfig`.
    me_ping_every_n: u32,
    /// Counter incremented on every `browse_before_action` invocation.
    sends_since_me_ping: u32,
}

impl DecoyBrowser {
    /// Build a browser from a per-N-sends ping cadence (typically
    /// `HumanizerConfig::me_ping_every_n_sends`).
    pub fn from_config(me_ping_every_n: u32) -> Self {
        Self {
            me_ping_every_n: me_ping_every_n.max(1),
            sends_since_me_ping: 0,
        }
    }

    /// Run a humanized decoy sequence. Always hits feed and notifications;
    /// occasionally also pings `/me` (every `me_ping_every_n` invocations).
    /// Reading pauses are randomized in the [2s, 30s] range per stop.
    ///
    /// Errors from `LinkedInError::SessionExpired` or
    /// `LinkedInError::RateLimitExceeded` propagate so the caller can react.
    /// Any other error is logged at warn and swallowed.
    pub async fn browse_before_action(&mut self, client: &LinkedInClient) -> Result<()> {
        let mut plan = self.build_plan();

        let mut rng = rand::thread_rng();
        plan.shuffle(&mut rng);
        drop(rng);

        for (decoy, read_secs) in plan {
            self.fire_one(client, decoy).await?;
            debug!(read_secs = read_secs, decoy = ?decoy, "Decoy read pause");
            sleep(Duration::from_secs(read_secs)).await;
        }
        Ok(())
    }

    /// Build the per-action decoy plan. Public for testing only.
    fn build_plan(&mut self) -> Vec<(Decoy, u64)> {
        let mut rng = rand::thread_rng();
        let mut plan: Vec<(Decoy, u64)> = vec![
            (Decoy::Feed, rng.gen_range(8..=29)),
            (Decoy::Notifications, rng.gen_range(5..=19)),
        ];
        self.sends_since_me_ping = self.sends_since_me_ping.saturating_add(1);
        if self.sends_since_me_ping >= self.me_ping_every_n {
            plan.push((Decoy::Me, rng.gen_range(2..=7)));
            self.sends_since_me_ping = 0;
        }
        plan
    }

    /// Fire one decoy GET, mapping fatal errors to propagation and
    /// swallowing the rest with a warn log.
    async fn fire_one(&self, client: &LinkedInClient, decoy: Decoy) -> Result<()> {
        let result = match decoy {
            Decoy::Feed => client.get_feed_updates().await,
            Decoy::Notifications => client.get_notifications().await,
            Decoy::Me => client.ping_me().await,
        };
        match result {
            Ok(()) => Ok(()),
            Err(e) => match e {
                LinkedInError::SessionExpired | LinkedInError::RateLimitExceeded { .. } => Err(e),
                other => {
                    warn!(decoy = ?decoy, error = %other, "Decoy GET failed -- continuing");
                    Ok(())
                },
            },
        }
    }
}
