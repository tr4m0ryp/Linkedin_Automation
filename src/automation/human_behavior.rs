//! Human-like interaction helpers to reduce detection risk.
//!
//! All delays use `tokio::time::sleep` so they are async-friendly and
//! cancellation-safe.

use crate::error::Result;
use rand::Rng;
use thirtyfour::prelude::*;
use tokio::time::{sleep, Duration};
use tracing::debug;

/// Sleep for a random duration between `min_ms` and `max_ms`.
pub async fn human_delay(min_ms: u64, max_ms: u64) {
    let ms = rand::thread_rng().gen_range(min_ms..=max_ms);
    debug!("Human delay: {}ms", ms);
    sleep(Duration::from_millis(ms)).await;
}

/// Scroll an element into view using JavaScript `scrollIntoView`.
pub async fn scroll_to_element(driver: &WebDriver, element: &WebElement) -> Result<()> {
    driver
        .execute(
            "arguments[0].scrollIntoView({behavior: 'smooth', block: 'center'});",
            vec![element.to_json()?],
        )
        .await?;
    human_delay(300, 800).await;
    Ok(())
}

/// Scroll into view, pause, then click -- mimicking a real user.
pub async fn human_click(driver: &WebDriver, element: &WebElement) -> Result<()> {
    scroll_to_element(driver, element).await?;
    human_delay(200, 600).await;
    element.click().await?;
    Ok(())
}

/// Poll for an element matching `css_selector`, timing out after `timeout_ms`.
///
/// Returns `Ok(element)` if found, or `Err(Timeout)` if the deadline passes.
#[allow(dead_code)]
pub async fn wait_for_element(
    driver: &WebDriver,
    css_selector: &str,
    timeout_ms: u64,
) -> Result<WebElement> {
    let deadline = tokio::time::Instant::now() + Duration::from_millis(timeout_ms);
    loop {
        if let Ok(el) = driver.find(By::Css(css_selector)).await {
            return Ok(el);
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(crate::error::LinkedInError::Timeout {
                seconds: timeout_ms / 1000,
            });
        }
        sleep(Duration::from_millis(250)).await;
    }
}

/// Try each selector in order and return the first element found.
pub async fn find_first_match(
    driver: &WebDriver,
    selectors: &[&str],
    timeout_ms: u64,
) -> Option<WebElement> {
    let deadline = tokio::time::Instant::now() + Duration::from_millis(timeout_ms);
    loop {
        for selector in selectors {
            if let Ok(el) = driver.find(By::Css(*selector)).await {
                return Some(el);
            }
        }
        if tokio::time::Instant::now() >= deadline {
            return None;
        }
        sleep(Duration::from_millis(300)).await;
    }
}
