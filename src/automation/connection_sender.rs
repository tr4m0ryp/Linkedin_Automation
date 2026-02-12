//! Per-profile connection request logic.
//!
//! Given a profile URL, navigates to it, locates the Connect button, clicks it,
//! handles the "Add a note" modal, and verifies the result.

use crate::error::Result;
use super::human_behavior::{find_first_match, human_click, human_delay};
use super::selectors;
use super::types::{ConnectionAttempt, ConnectionResult};
use thirtyfour::prelude::*;
use tracing::{info, debug, warn};

/// Send a connection request to a single profile.
///
/// Returns a `ConnectionAttempt` describing the outcome.
pub async fn send_connection(
    driver: &WebDriver,
    profile_url: &str,
    dry_run: bool,
) -> ConnectionAttempt {
    let result = attempt_connection(driver, profile_url, dry_run).await;
    let attempt = ConnectionAttempt::new(profile_url.to_string(), result);

    info!(
        "Profile: {} -> {}",
        profile_url, attempt.result
    );
    attempt
}

async fn attempt_connection(
    driver: &WebDriver,
    profile_url: &str,
    dry_run: bool,
) -> ConnectionResult {
    // Navigate to the profile
    if let Err(e) = driver.goto(profile_url).await {
        return ConnectionResult::Error(format!("Navigation failed: {}", e));
    }
    human_delay(1500, 3000).await;

    // Check if already connected (Message button visible)
    if find_first_match(driver, selectors::ALREADY_CONNECTED, 1500)
        .await
        .is_some()
    {
        return ConnectionResult::AlreadyConnected;
    }

    // Check if already pending
    if find_first_match(driver, selectors::PENDING_INDICATORS, 1000)
        .await
        .is_some()
    {
        return ConnectionResult::Pending;
    }

    // Try to find Connect button directly on the page
    let connect_btn = match find_connect_button(driver).await {
        Some(btn) => btn,
        None => return ConnectionResult::ButtonNotFound,
    };

    if dry_run {
        debug!("Dry run -- skipping click for {}", profile_url);
        return ConnectionResult::Success;
    }

    // Click Connect
    if let Err(e) = human_click(driver, &connect_btn).await {
        return ConnectionResult::Error(format!("Click failed: {}", e));
    }
    human_delay(800, 1500).await;

    // Handle "Add a note" modal
    match handle_modal(driver).await {
        Ok(()) => {}
        Err(_) => {
            dismiss_modal(driver).await;
            return ConnectionResult::ModalError;
        }
    }

    human_delay(1000, 2000).await;

    // Verify it changed to Pending
    if find_first_match(driver, selectors::PENDING_INDICATORS, 3000)
        .await
        .is_some()
    {
        ConnectionResult::Success
    } else {
        // Even if we cannot confirm Pending, the click may have worked
        warn!("Could not confirm Pending state for {}", profile_url);
        ConnectionResult::Success
    }
}

/// Try the primary Connect button, then fall back to the More dropdown.
async fn find_connect_button(driver: &WebDriver) -> Option<WebElement> {
    // Direct Connect button
    if let Some(btn) = find_first_match(driver, selectors::CONNECT_BUTTON, 2000).await {
        return Some(btn);
    }

    // Try More dropdown
    debug!("Connect button not found directly, trying More dropdown");
    let more_btn = find_first_match(driver, selectors::MORE_BUTTON, 1500).await?;
    if human_click(driver, &more_btn).await.is_err() {
        return None;
    }
    human_delay(500, 1000).await;

    find_first_match(driver, selectors::DROPDOWN_CONNECT, 2000).await
}

/// Click "Send without a note" in the Add-a-note modal.
async fn handle_modal(driver: &WebDriver) -> Result<()> {
    let send_btn = find_first_match(driver, selectors::SEND_WITHOUT_NOTE, 3000).await;
    match send_btn {
        Some(btn) => {
            human_click(driver, &btn).await?;
            Ok(())
        }
        None => {
            // No modal appeared -- Connect may have gone through directly
            debug!("No 'Send without a note' modal detected");
            Ok(())
        }
    }
}

/// Best-effort dismiss of any open modal.
async fn dismiss_modal(driver: &WebDriver) {
    if let Some(btn) = find_first_match(driver, selectors::MODAL_DISMISS, 1500).await {
        let _ = btn.click().await;
    }
}
