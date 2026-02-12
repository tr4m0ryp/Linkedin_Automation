//! One-time browser login for cookie extraction.
//!
//! Launches Chrome with a remote debugging port, waits for the user to
//! complete login manually, then extracts cookies via the Chrome DevTools
//! Protocol and persists them to disk.

use crate::error::{LinkedInError, Result};
use super::{cdp, session};
use std::process::{Child, Command};
use tracing::{info, debug};

/// Run the one-time login flow.
///
/// Launches Chrome, waits for the user to log in, extracts cookies via CDP,
/// saves them to `cookie_file`, and returns the CSRF token.
pub async fn one_time_login(cookie_file: &str, user_agent: &str) -> Result<String> {
    let debug_port: u16 = 9222;

    info!("Launching Chrome for one-time login...");
    let mut chrome = launch_chrome(debug_port, user_agent)?;

    info!("A Chrome window should have opened.");
    info!("Please log in to LinkedIn manually.");
    info!("Waiting for login (timeout: 5 minutes)...");

    let timeout = tokio::time::Duration::from_secs(300);
    let poll_interval = tokio::time::Duration::from_secs(3);
    let start = tokio::time::Instant::now();

    loop {
        if start.elapsed() >= timeout {
            let _ = chrome.kill();
            return Err(LinkedInError::Timeout { seconds: 300 });
        }

        if let Ok(true) = check_logged_in(debug_port).await {
            info!("Login detected.");
            break;
        }

        tokio::time::sleep(poll_interval).await;
    }

    let cookies_json = cdp::fetch_cdp_cookies(debug_port).await?;
    cdp::save_cdp_cookies_to_file(&cookies_json, cookie_file)?;

    let jar = session::load_cookies(cookie_file)?;
    let csrf = session::extract_csrf_token(&jar).unwrap_or_default();

    info!("Cookies saved to {}. Closing Chrome.", cookie_file);
    let _ = chrome.kill();

    Ok(csrf)
}

fn launch_chrome(debug_port: u16, user_agent: &str) -> Result<Child> {
    let chrome_bin = find_chrome_binary();
    debug!("Using Chrome binary: {}", chrome_bin);

    // CDP requires a non-default data directory. Use a dedicated profile
    // under the project's sessions/ dir so cookies persist across logins.
    let data_dir = std::path::Path::new("sessions").join("chrome_profile");
    std::fs::create_dir_all(&data_dir).ok();

    Command::new(&chrome_bin)
        .arg(format!("--remote-debugging-port={}", debug_port))
        .arg(format!("--user-data-dir={}", data_dir.display()))
        .arg("--no-first-run")
        .arg("--disable-blink-features=AutomationControlled")
        .arg(format!("--user-agent={}", user_agent))
        .arg("https://www.linkedin.com/login")
        .spawn()
        .map_err(|e| {
            LinkedInError::ApiError(format!(
                "Failed to launch Chrome ({}): {}. \
                 Make sure Chrome/Chromium is installed.",
                chrome_bin, e
            ))
        })
}

fn find_chrome_binary() -> String {
    let candidates = [
        "google-chrome",
        "google-chrome-stable",
        "chromium",
        "chromium-browser",
        "/usr/bin/google-chrome",
        "/usr/bin/chromium",
        "/usr/bin/chromium-browser",
    ];
    for bin in &candidates {
        if Command::new("which")
            .arg(bin)
            .output()
            .is_ok_and(|o| o.status.success())
        {
            return bin.to_string();
        }
    }
    "google-chrome".to_string()
}

/// Check via CDP whether the active page has navigated past the login page.
///
/// Matches `/feed`, `/mynetwork`, `/messaging`, `/notifications`, or any
/// authenticated LinkedIn page (i.e. not `/login`, `/checkpoint`, or `/uas`).
async fn check_logged_in(debug_port: u16) -> Result<bool> {
    let url = format!("http://127.0.0.1:{}/json", debug_port);
    let resp = reqwest::get(&url).await;
    match resp {
        Ok(r) => {
            let body: serde_json::Value = r.json().await.unwrap_or_default();
            if let Some(arr) = body.as_array() {
                for target in arr {
                    if let Some(page_url) = target.get("url").and_then(|v| v.as_str()) {
                        if is_logged_in_url(page_url) {
                            return Ok(true);
                        }
                    }
                }
            }
            Ok(false)
        }
        Err(_) => Ok(false),
    }
}

fn is_logged_in_url(url: &str) -> bool {
    let authenticated_paths = ["/feed", "/mynetwork", "/messaging", "/notifications", "/jobs", "/in/"];
    let login_paths = ["/login", "/checkpoint", "/uas/", "/authwall"];

    // Must be on linkedin.com
    if !url.contains("linkedin.com") {
        return false;
    }
    // Reject known login/challenge pages
    for path in &login_paths {
        if url.contains(path) {
            return false;
        }
    }
    // Accept known authenticated pages
    for path in &authenticated_paths {
        if url.contains(path) {
            return true;
        }
    }
    false
}
