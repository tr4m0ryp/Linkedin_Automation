//! Application configuration loaded from environment variables.

use crate::error::{LinkedInError, Result};
use crate::browser::BrowserConfig;

/// Top-level application configuration.
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub browser: BrowserConfig,
    pub automation: AutomationSettings,
}

/// Settings controlling the connection automation loop.
#[derive(Debug, Clone)]
pub struct AutomationSettings {
    pub csv_path: String,
    pub min_delay_min: u64,
    pub max_delay_min: u64,
}

impl Default for AutomationSettings {
    fn default() -> Self {
        Self {
            csv_path: "linkedin_profiles.csv".to_string(),
            min_delay_min: 10,
            max_delay_min: 15,
        }
    }
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

fn env_or_parse<T: std::str::FromStr>(key: &str, default: T) -> T {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// Load configuration from the `.env` file and environment variables.
pub fn load_config(env_path: &str) -> Result<AppConfig> {
    dotenv::from_filename(env_path).ok();

    let browser = BrowserConfig {
        browser_type: env_or("BROWSER_TYPE", "chromium"),
        headless: env_or_parse("HEADLESS", false),
        session_dir: env_or("SESSION_DIR", "sessions/linkedin_session"),
        webdriver_url: env_or("WEBDRIVER_URL", "http://localhost:4444"),
        debug_port: env_or_parse("DEBUG_PORT", 9222),
    };

    let automation = AutomationSettings {
        csv_path: env_or("CSV_PATH", "linkedin_profiles.csv"),
        min_delay_min: env_or_parse("MIN_DELAY_MIN", 10),
        max_delay_min: env_or_parse("MAX_DELAY_MIN", 15),
    };

    validate_config(&browser, &automation)?;

    Ok(AppConfig { browser, automation })
}

fn validate_config(
    _browser: &BrowserConfig,
    automation: &AutomationSettings,
) -> Result<()> {
    if automation.min_delay_min > automation.max_delay_min {
        return Err(LinkedInError::ConfigError(
            "MIN_DELAY_MIN must be <= MAX_DELAY_MIN".to_string(),
        ));
    }
    Ok(())
}
