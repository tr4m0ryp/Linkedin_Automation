//! Cookie persistence and session validation.
//!
//! Loads/saves a reqwest-compatible cookie jar from a JSON file and provides
//! helpers to extract the CSRF token from the JSESSIONID cookie.

use crate::error::{LinkedInError, Result};
use cookie_store::CookieStore;
use reqwest::Url;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Load persisted cookies from a JSON file into a shared cookie jar.
///
/// Returns an `Arc<reqwest_cookie_store::CookieStoreMutex>` suitable for
/// passing to `reqwest::ClientBuilder::cookie_provider`.
pub fn load_cookies(
    path: &str,
) -> Result<Arc<reqwest_cookie_store::CookieStoreMutex>> {
    let store = if Path::new(path).exists() {
        let file = fs::File::open(path).map_err(|e| {
            LinkedInError::StorageError(format!("Cannot open cookie file {}: {}", path, e))
        })?;
        let reader = std::io::BufReader::new(file);
        #[allow(deprecated)]
        CookieStore::load_json(reader)
            .map_err(|e| LinkedInError::StorageError(format!("Cookie parse error: {}", e)))?
    } else {
        debug!("No cookie file at {} -- starting with empty jar", path);
        CookieStore::default()
    };

    Ok(Arc::new(reqwest_cookie_store::CookieStoreMutex::new(store)))
}

/// Serialize the cookie jar to a JSON file.
pub fn save_cookies(
    jar: &reqwest_cookie_store::CookieStoreMutex,
    path: &str,
) -> Result<()> {
    if let Some(parent) = Path::new(path).parent() {
        fs::create_dir_all(parent).map_err(|e| {
            LinkedInError::StorageError(format!("Cannot create directory: {}", e))
        })?;
    }
    let file = fs::File::create(path).map_err(|e| {
        LinkedInError::StorageError(format!("Cannot create cookie file: {}", e))
    })?;
    let mut writer = std::io::BufWriter::new(file);
    let store = jar.lock().expect("cookie store lock poisoned");
    #[allow(deprecated)]
    store.save_json(&mut writer).map_err(|e| {
        LinkedInError::StorageError(format!("Cookie serialize error: {}", e))
    })?;
    debug!("Cookies saved to {}", path);
    Ok(())
}

/// Extract the CSRF token from the JSESSIONID cookie value.
///
/// LinkedIn sets JSESSIONID to something like `"ajax:123456789..."`. The CSRF
/// token is the part after `ajax:`, with surrounding quotes stripped.
pub fn extract_csrf_token(jar: &reqwest_cookie_store::CookieStoreMutex) -> Option<String> {
    let store = jar.lock().expect("cookie store lock poisoned");
    let linkedin_url = Url::parse("https://www.linkedin.com").ok()?;
    let cookies: Vec<_> = store.matches(&linkedin_url).into_iter().collect();
    debug!("Cookies matching linkedin.com: {}", cookies.len());
    for cookie in &cookies {
        debug!("  cookie: {}={}", cookie.name(), &cookie.value()[..cookie.value().len().min(20)]);
        if cookie.name() == "JSESSIONID" {
            let value = cookie.value().trim_matches('"');
            if let Some(token) = value.strip_prefix("ajax:") {
                debug!("Extracted CSRF token from JSESSIONID");
                return Some(token.to_string());
            }
            debug!("JSESSIONID found but no ajax: prefix, using raw value");
            return Some(value.to_string());
        }
    }
    debug!("JSESSIONID cookie not found");
    None
}

/// Quick check whether the current session cookies are still valid.
///
/// Sends a lightweight GET to the LinkedIn feed API and checks for a
/// successful response.
pub async fn validate_session(cookie_file: &str, user_agent: &str) -> Result<bool> {
    let jar = load_cookies(cookie_file)?;
    let csrf = match extract_csrf_token(&jar) {
        Some(t) => t,
        None => {
            warn!("No CSRF token found in cookies -- session invalid");
            return Ok(false);
        }
    };

    let client = reqwest::Client::builder()
        .cookie_provider(jar)
        .user_agent(user_agent)
        .build()
        .map_err(|e| LinkedInError::ApiError(format!("Client build failed: {}", e)))?;

    let resp = client
        .get("https://www.linkedin.com/voyager/api/me")
        .header("csrf-token", format!("ajax:{}", csrf))
        .header("x-restli-protocol-version", "2.0.0")
        .send()
        .await;

    match resp {
        Ok(r) => {
            let valid = r.status().is_success();
            if valid {
                info!("Session is valid");
            } else {
                warn!("Session check returned HTTP {}", r.status().as_u16());
            }
            Ok(valid)
        }
        Err(e) => {
            warn!("Session validation request failed: {}", e);
            Ok(false)
        }
    }
}
