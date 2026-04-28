//! D5.B decoy GET helpers for the LinkedIn Voyager API.
//!
//! These methods fire lightweight reads (feed, notifications, me) between
//! real actions to make the traffic shape look like organic browsing.
//! Auth (401/403) and rate-limit (429) responses are surfaced as errors so
//! the humanizer can react; other non-2xx statuses are logged and swallowed
//! since decoy traffic is non-critical.

use super::LinkedInClient;
use crate::error::{LinkedInError, Result};
use tracing::{debug, warn};

impl LinkedInClient {
    /// D5.B decoy: fire a feed-load to look like organic browsing.
    ///
    /// Returns `Ok(())` on 2xx. Surfaces `SessionExpired` on 401/403 and
    /// `RateLimitExceeded` on 429 so the humanizer can react. Other non-2xx
    /// statuses are logged at warn and swallowed -- decoy traffic is
    /// non-critical.
    pub async fn get_feed_updates(&self) -> Result<()> {
        self.fire_decoy_get(
            "https://www.linkedin.com/voyager/api/feed/updatesV2?count=10&q=feed",
            "feed",
        )
        .await
    }

    /// D5.B decoy: fire a notifications check.
    ///
    /// Same status semantics as `get_feed_updates`. If the primary endpoint
    /// 404s, falls back to the voyagerIdentityDashNotificationCards endpoint.
    pub async fn get_notifications(&self) -> Result<()> {
        let primary = "https://www.linkedin.com/voyager/api/me/notifications";
        let fallback = "https://www.linkedin.com/voyager/api/\
            voyagerIdentityDashNotificationCards\
            ?q=filterVanityName&filterVanityName=ALL";

        let resp = self
            .client
            .get(primary)
            .headers(self.default_headers())
            .header("accept", "application/vnd.linkedin.normalized+json+2.1")
            .send()
            .await
            .map_err(|e| {
                LinkedInError::ApiError(format!("Decoy notifications request failed: {}", e))
            })?;

        let status = resp.status().as_u16();
        if status == 404 {
            debug!("Notifications primary 404 -- falling back");
            let _ = resp.bytes().await;
            return self
                .fire_decoy_get(fallback, "notifications-fallback")
                .await;
        }
        handle_decoy_status(resp, "notifications").await
    }

    /// D5.B decoy: lightweight session ping that refreshes session metadata.
    pub async fn ping_me(&self) -> Result<()> {
        self.fire_decoy_get("https://www.linkedin.com/voyager/api/me", "me")
            .await
    }

    /// Shared decoy GET implementation used by all three decoy helpers.
    async fn fire_decoy_get(&self, url: &str, label: &str) -> Result<()> {
        let resp = self
            .client
            .get(url)
            .headers(self.default_headers())
            .header("accept", "application/vnd.linkedin.normalized+json+2.1")
            .send()
            .await
            .map_err(|e| {
                LinkedInError::ApiError(format!("Decoy {} request failed: {}", label, e))
            })?;
        handle_decoy_status(resp, label).await
    }
}

/// Inspect a decoy response, propagate auth/rate-limit errors, and otherwise
/// drain the body to keep the connection alive.
async fn handle_decoy_status(resp: reqwest::Response, label: &str) -> Result<()> {
    let status = resp.status().as_u16();
    if status == 401 || status == 403 {
        return Err(LinkedInError::SessionExpired);
    }
    if status == 429 {
        return Err(LinkedInError::RateLimitExceeded { retry_after: 60 });
    }
    if !(200..300).contains(&status) {
        warn!(label = label, status = status, "Decoy GET returned non-2xx");
    } else {
        debug!(label = label, status = status, "Decoy GET ok");
    }
    let _ = resp.bytes().await;
    Ok(())
}
