use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct BrowserConfig {
    pub browser_type: String,
    pub headless: bool,
    pub session_dir: String,
    pub webdriver_url: String,
    pub debug_port: u16,
}

impl Default for BrowserConfig {
    fn default() -> Self {
        Self {
            browser_type: "chromium".to_string(),
            headless: false,
            session_dir: "sessions/linkedin_session".to_string(),
            webdriver_url: "http://localhost:4444".to_string(),
            debug_port: 9222,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkRequest {
    pub request_id: String,
    pub url: String,
    pub method: String,
    pub headers: HashMap<String, String>,
    pub post_data: Option<String>,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkResponse {
    pub request_id: String,
    pub url: String,
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
    pub timestamp: i64,
}
