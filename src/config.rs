//! Application configuration loaded from environment variables.

use crate::error::{LinkedInError, Result};

/// Top-level application configuration.
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub api: ApiConfig,
    pub automation: AutomationSettings,
}

/// Configuration for the LinkedIn API client.
#[derive(Debug, Clone)]
pub struct ApiConfig {
    /// Path to the persisted cookie JSON file.
    pub cookie_file: String,
    /// Browser user-agent string to mimic.
    pub user_agent: String,
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

const DEFAULT_USER_AGENT: &str =
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 \
     (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

/// Load configuration from the `.env` file and environment variables.
pub fn load_config(env_path: &str) -> Result<AppConfig> {
    dotenv::from_filename(env_path).ok();

    let api = ApiConfig {
        cookie_file: env_or("COOKIE_FILE", "sessions/linkedin_cookies.json"),
        user_agent: env_or("USER_AGENT", DEFAULT_USER_AGENT),
    };

    let automation = AutomationSettings {
        csv_path: env_or("CSV_PATH", "linkedin_profiles.csv"),
        min_delay_min: env_or_parse("MIN_DELAY_MIN", 10),
        max_delay_min: env_or_parse("MAX_DELAY_MIN", 15),
    };

    validate_config(&automation)?;

    Ok(AppConfig { api, automation })
}

fn validate_config(automation: &AutomationSettings) -> Result<()> {
    if automation.min_delay_min > automation.max_delay_min {
        return Err(LinkedInError::ConfigError(
            "MIN_DELAY_MIN must be <= MAX_DELAY_MIN".to_string(),
        ));
    }
    Ok(())
}
