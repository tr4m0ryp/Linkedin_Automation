//! Minimal Chrome DevTools Protocol client for cookie extraction.
//!
//! Implements a raw WebSocket connection (no external WS crate) to send a
//! single CDP command (`Network.getAllCookies`) and parse the response.

use crate::error::{LinkedInError, Result};
use base64::Engine;
use cookie_store::CookieStore;
use tracing::{info, debug};

/// Fetch all CDP targets, find the page target, and retrieve all cookies
/// via `Network.getAllCookies`.
pub async fn fetch_cdp_cookies(debug_port: u16) -> Result<serde_json::Value> {
    let targets_url = format!("http://127.0.0.1:{}/json", debug_port);
    let targets: serde_json::Value = reqwest::get(&targets_url)
        .await
        .map_err(|e| LinkedInError::ApiError(format!("CDP targets fetch failed: {}", e)))?
        .json()
        .await
        .map_err(|e| LinkedInError::ApiError(format!("CDP targets parse failed: {}", e)))?;

    let ws_url = targets
        .as_array()
        .and_then(|arr| {
            arr.iter()
                .find(|t| t.get("type").and_then(|v| v.as_str()) == Some("page"))
        })
        .and_then(|t| t.get("webSocketDebuggerUrl").and_then(|v| v.as_str()))
        .ok_or_else(|| LinkedInError::ApiError("No page target found in CDP".to_string()))?;

    debug!("CDP WebSocket URL: {}", ws_url);
    fetch_cookies_via_raw_ws(ws_url)
}

/// Save CDP cookies to a JSON file and also build a cookie_store file.
///
/// Writes the raw CDP cookie array for later re-import, and builds a
/// cookie_store-compatible file that reqwest can load.
pub fn save_cdp_cookies_to_file(
    cdp_response: &serde_json::Value,
    cookie_file: &str,
) -> Result<()> {
    let cookies = cdp_response
        .pointer("/result/cookies")
        .and_then(|v| v.as_array())
        .ok_or_else(|| {
            LinkedInError::ApiError(format!(
                "No cookies in CDP response: {}",
                cdp_response
            ))
        })?;

    // Build a cookie_store with Max-Age so session cookies get persisted.
    let mut store = CookieStore::default();
    let mut inserted = 0u32;
    // 30 days in seconds
    let max_age = 60 * 60 * 24 * 30;

    for c in cookies {
        let name = c.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let value = c.get("value").and_then(|v| v.as_str()).unwrap_or("");
        let domain = c.get("domain").and_then(|v| v.as_str()).unwrap_or("");
        let path = c.get("path").and_then(|v| v.as_str()).unwrap_or("/");
        let secure = c.get("secure").and_then(|v| v.as_bool()).unwrap_or(false);
        let http_only = c.get("httpOnly").and_then(|v| v.as_bool()).unwrap_or(false);

        if name.is_empty() || domain.is_empty() {
            continue;
        }

        let clean_domain = domain.trim_start_matches('.');
        // Always set Max-Age so cookie_store treats them as persistent
        let mut set_cookie = format!(
            "{}={}; Domain={}; Path={}; Max-Age={}",
            name, value, clean_domain, path, max_age
        );
        if secure {
            set_cookie.push_str("; Secure");
        }
        if http_only {
            set_cookie.push_str("; HttpOnly");
        }

        let url_str = format!("https://{}{}", clean_domain, path);
        if let Ok(url) = reqwest::Url::parse(&url_str) {
            if store.parse(&set_cookie, &url).is_ok() {
                inserted += 1;
            }
        }
    }

    debug!("Inserted {}/{} cookies into store", inserted, cookies.len());

    if let Some(parent) = std::path::Path::new(cookie_file).parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            LinkedInError::StorageError(format!("Cannot create directory: {}", e))
        })?;
    }
    let file = std::fs::File::create(cookie_file).map_err(|e| {
        LinkedInError::StorageError(format!("Cannot create cookie file: {}", e))
    })?;
    let mut writer = std::io::BufWriter::new(file);
    #[allow(deprecated)]
    store.save_json(&mut writer).map_err(|e| {
        LinkedInError::StorageError(format!("Cookie save error: {}", e))
    })?;

    info!("Saved {} cookies to {}", inserted, cookie_file);
    Ok(())
}

/// Minimal raw WebSocket CDP client.
fn fetch_cookies_via_raw_ws(ws_url: &str) -> Result<serde_json::Value> {
    use std::io::{Read, Write};
    use std::net::TcpStream;

    let url = ws_url
        .strip_prefix("ws://")
        .ok_or_else(|| LinkedInError::ApiError("Invalid CDP WebSocket URL".to_string()))?;

    let (host_port, path) = url.split_once('/').unwrap_or((url, ""));
    let path = format!("/{}", path);

    debug!("Connecting to CDP at {}", host_port);
    let mut stream = TcpStream::connect(host_port).map_err(|e| {
        LinkedInError::ApiError(format!("CDP TCP connection failed: {}", e))
    })?;
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(15)))
        .ok();

    let key = base64::engine::general_purpose::STANDARD.encode(b"cdp-cookie-fetch");
    let handshake = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: {}\r\nSec-WebSocket-Version: 13\r\n\r\n",
        path, host_port, key
    );
    stream.write_all(handshake.as_bytes()).map_err(|e| {
        LinkedInError::ApiError(format!("WS handshake write failed: {}", e))
    })?;

    let mut resp_buf = [0u8; 4096];
    let n = stream.read(&mut resp_buf).map_err(|e| {
        LinkedInError::ApiError(format!("WS handshake read failed: {}", e))
    })?;
    let resp = String::from_utf8_lossy(&resp_buf[..n]);
    if !resp.contains("101") {
        return Err(LinkedInError::ApiError(format!(
            "WebSocket upgrade failed: {}",
            resp.lines().next().unwrap_or("unknown")
        )));
    }
    debug!("WebSocket handshake successful");

    let cmd = serde_json::json!({ "id": 1, "method": "Network.getAllCookies" });
    let cmd_bytes = serde_json::to_vec(&cmd).unwrap_or_default();
    send_ws_frame(&mut stream, &cmd_bytes)?;
    debug!("Sent Network.getAllCookies command");

    // Read frames until we get our response (id == 1).
    let max_attempts = 20;
    for attempt in 0..max_attempts {
        let data = match read_ws_frame(&mut stream) {
            Ok(d) => d,
            Err(e) => {
                debug!("Frame read error on attempt {}: {}", attempt, e);
                break;
            }
        };

        let parsed: serde_json::Value = match serde_json::from_slice(&data) {
            Ok(v) => v,
            Err(_) => continue,
        };

        if parsed.get("id").and_then(|v| v.as_u64()) == Some(1) {
            debug!("Got CDP response on attempt {}", attempt);
            let _ = stream.shutdown(std::net::Shutdown::Both);
            return Ok(parsed);
        }

        let method = parsed.get("method").and_then(|v| v.as_str()).unwrap_or("?");
        debug!("Skipping CDP event: {}", method);
    }

    let _ = stream.shutdown(std::net::Shutdown::Both);
    Err(LinkedInError::ApiError(
        "Did not receive CDP response after reading multiple frames".to_string(),
    ))
}

fn send_ws_frame(stream: &mut std::net::TcpStream, payload: &[u8]) -> Result<()> {
    use std::io::Write;
    let len = payload.len();
    let mask_key: [u8; 4] = rand::random();
    let mut frame = vec![0x81u8];
    if len < 126 {
        frame.push(0x80 | len as u8);
    } else if len < 65536 {
        frame.push(0x80 | 126);
        frame.extend_from_slice(&(len as u16).to_be_bytes());
    } else {
        frame.push(0x80 | 127);
        frame.extend_from_slice(&(len as u64).to_be_bytes());
    }
    frame.extend_from_slice(&mask_key);
    let masked: Vec<u8> = payload
        .iter()
        .enumerate()
        .map(|(i, b)| b ^ mask_key[i % 4])
        .collect();
    frame.extend_from_slice(&masked);
    stream.write_all(&frame).map_err(|e| {
        LinkedInError::ApiError(format!("WS frame write failed: {}", e))
    })
}

fn read_ws_frame(stream: &mut std::net::TcpStream) -> Result<Vec<u8>> {
    use std::io::Read;
    let mut header = [0u8; 2];
    stream.read_exact(&mut header).map_err(|e| {
        LinkedInError::ApiError(format!("WS frame header read failed: {}", e))
    })?;
    let masked = (header[1] & 0x80) != 0;
    let mut len = (header[1] & 0x7F) as u64;
    if len == 126 {
        let mut buf = [0u8; 2];
        stream.read_exact(&mut buf).map_err(|e| {
            LinkedInError::ApiError(format!("WS length read failed: {}", e))
        })?;
        len = u16::from_be_bytes(buf) as u64;
    } else if len == 127 {
        let mut buf = [0u8; 8];
        stream.read_exact(&mut buf).map_err(|e| {
            LinkedInError::ApiError(format!("WS length read failed: {}", e))
        })?;
        len = u64::from_be_bytes(buf);
    }
    let mask_key = if masked {
        let mut buf = [0u8; 4];
        stream.read_exact(&mut buf).map_err(|e| {
            LinkedInError::ApiError(format!("WS mask read failed: {}", e))
        })?;
        Some(buf)
    } else {
        None
    };
    let mut payload = vec![0u8; len as usize];
    stream.read_exact(&mut payload).map_err(|e| {
        LinkedInError::ApiError(format!("WS payload read failed: {}", e))
    })?;
    if let Some(key) = mask_key {
        for (i, b) in payload.iter_mut().enumerate() {
            *b ^= key[i % 4];
        }
    }
    Ok(payload)
}
