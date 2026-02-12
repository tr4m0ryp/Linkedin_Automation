//! Per-profile connection request logic using the LinkedIn Voyager API.
//!
//! Resolves a profile URL to member data, checks connection state, and sends
//! an invitation via the API.

use crate::linkedin_api::{LinkedInClient, ConnectionState};
use super::types::{ConnectionAttempt, ConnectionResult};
use tracing::{info, debug, warn};

/// Send a connection request to a single profile via the API.
///
/// Returns a `ConnectionAttempt` describing the outcome.
pub async fn send_connection(
    client: &LinkedInClient,
    profile_url: &str,
    dry_run: bool,
) -> ConnectionAttempt {
    let result = attempt_connection(client, profile_url, dry_run).await;
    let attempt = ConnectionAttempt::new(profile_url.to_string(), result);

    info!("Profile: {} -> {}", profile_url, attempt.result);
    attempt
}

async fn attempt_connection(
    client: &LinkedInClient,
    profile_url: &str,
    dry_run: bool,
) -> ConnectionResult {
    // Resolve profile URL to member data
    let profile = match client.resolve_profile(profile_url).await {
        Ok(p) => p,
        Err(e) => {
            warn!("Failed to resolve profile {}: {}", profile_url, e);
            return ConnectionResult::Error(format!("Profile resolution failed: {}", e));
        }
    };

    debug!(
        "Resolved: {} {} (id={}, state={})",
        profile.first_name, profile.last_name, profile.member_id, profile.connection_state
    );

    // Check connection state -- return early if already connected or pending
    match profile.connection_state {
        ConnectionState::Connected => return ConnectionResult::AlreadyConnected,
        ConnectionState::Pending => return ConnectionResult::Pending,
        ConnectionState::NotConnected | ConnectionState::Unknown => {}
    }

    if dry_run {
        debug!("Dry run -- skipping invitation for {}", profile_url);
        return ConnectionResult::Success;
    }

    // Send the invitation
    match client.send_invitation(&profile).await {
        Ok(resp) if resp.success => ConnectionResult::Success,
        Ok(resp) if resp.body == "ALREADY_PENDING" => ConnectionResult::Pending,
        Ok(resp) => {
            warn!(
                "Invitation API returned {} for {}: {}",
                resp.status_code, profile_url, resp.body
            );
            ConnectionResult::Error(format!("API returned {}", resp.status_code))
        }
        Err(crate::error::LinkedInError::RateLimitExceeded { .. }) => {
            ConnectionResult::RateLimited
        }
        Err(e) => ConnectionResult::Error(format!("{}", e)),
    }
}
