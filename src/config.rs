//! Application configuration loaded from environment variables.

use crate::error::{LinkedInError, Result};

/// Top-level application configuration.
#[derive(Debug, Clone)]
pub struct AppConfig {
    /// LinkedIn API client settings.
    pub api: ApiConfig,
    /// Connection-loop runtime settings.
    pub automation: AutomationSettings,
    /// Anti-detection / humanizer knobs (per D5).
    pub humanizer: HumanizerConfig,
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

/// Anti-detection knobs that shape per-day, per-session pacing.
///
/// Every field has a sane default (see `Default` impl) so missing env keys do
/// not fail config loading. Used by the humanizer (Task 003) and the runner
/// (Task 004).
#[derive(Debug, Clone)]
pub struct HumanizerConfig {
    /// Local-time start of the daily sending window, "HH:MM" 24h.
    pub daily_window_start: String,
    /// Local-time end of the daily sending window, "HH:MM" 24h.
    pub daily_window_end: String,
    /// Hard cap on connection requests per local day.
    pub daily_send_cap: u32,
    /// Re-check 3rd-degree distance after this many days.
    pub degree_recheck_days: i64,
    /// Probability in [0.0, 1.0] of skipping an otherwise-eligible send.
    pub skip_send_probability: f64,
    /// Minimum number of sends between long breaks.
    pub break_every_min_sends: u32,
    /// Maximum number of sends between long breaks.
    pub break_every_max_sends: u32,
    /// Minimum break duration in seconds.
    pub break_duration_min_secs: u64,
    /// Maximum break duration in seconds.
    pub break_duration_max_secs: u64,
    /// Median (50th percentile) of the lognormal inter-send delay, in seconds.
    pub delay_lognormal_median_secs: f64,
    /// Sigma (shape) of the lognormal inter-send delay distribution.
    pub delay_lognormal_sigma: f64,
    /// Issue a "me" ping every N sends to mimic a logged-in user browsing.
    pub me_ping_every_n_sends: u32,
}

impl Default for HumanizerConfig {
    fn default() -> Self {
        Self {
            daily_window_start: "09:00".to_string(),
            daily_window_end: "19:00".to_string(),
            daily_send_cap: 18,
            degree_recheck_days: 30,
            skip_send_probability: 0.07,
            break_every_min_sends: 3,
            break_every_max_sends: 7,
            break_duration_min_secs: 1200,
            break_duration_max_secs: 3600,
            delay_lognormal_median_secs: 720.0,
            delay_lognormal_sigma: 0.6,
            me_ping_every_n_sends: 5,
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

const DEFAULT_USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 \
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

    let humanizer = load_humanizer_config();

    validate_config(&automation)?;
    validate_humanizer(&humanizer)?;

    Ok(AppConfig {
        api,
        automation,
        humanizer,
    })
}

fn load_humanizer_config() -> HumanizerConfig {
    let defaults = HumanizerConfig::default();
    HumanizerConfig {
        daily_window_start: env_or("DAILY_WINDOW_START", &defaults.daily_window_start),
        daily_window_end: env_or("DAILY_WINDOW_END", &defaults.daily_window_end),
        daily_send_cap: env_or_parse("DAILY_SEND_CAP", defaults.daily_send_cap),
        degree_recheck_days: env_or_parse("DEGREE_RECHECK_DAYS", defaults.degree_recheck_days),
        skip_send_probability: env_or_parse(
            "SKIP_SEND_PROBABILITY",
            defaults.skip_send_probability,
        ),
        break_every_min_sends: env_or_parse(
            "BREAK_EVERY_MIN_SENDS",
            defaults.break_every_min_sends,
        ),
        break_every_max_sends: env_or_parse(
            "BREAK_EVERY_MAX_SENDS",
            defaults.break_every_max_sends,
        ),
        break_duration_min_secs: env_or_parse(
            "BREAK_DURATION_MIN_SECS",
            defaults.break_duration_min_secs,
        ),
        break_duration_max_secs: env_or_parse(
            "BREAK_DURATION_MAX_SECS",
            defaults.break_duration_max_secs,
        ),
        delay_lognormal_median_secs: env_or_parse(
            "DELAY_LOGNORMAL_MEDIAN_SECS",
            defaults.delay_lognormal_median_secs,
        ),
        delay_lognormal_sigma: env_or_parse(
            "DELAY_LOGNORMAL_SIGMA",
            defaults.delay_lognormal_sigma,
        ),
        me_ping_every_n_sends: env_or_parse(
            "ME_PING_EVERY_N_SENDS",
            defaults.me_ping_every_n_sends,
        ),
    }
}

fn validate_config(automation: &AutomationSettings) -> Result<()> {
    if automation.min_delay_min > automation.max_delay_min {
        return Err(LinkedInError::ConfigError(
            "MIN_DELAY_MIN must be <= MAX_DELAY_MIN".to_string(),
        ));
    }
    Ok(())
}

fn validate_humanizer(h: &HumanizerConfig) -> Result<()> {
    if h.break_every_min_sends > h.break_every_max_sends {
        return Err(LinkedInError::ConfigError(
            "BREAK_EVERY_MIN_SENDS must be <= BREAK_EVERY_MAX_SENDS".to_string(),
        ));
    }
    if h.break_duration_min_secs > h.break_duration_max_secs {
        return Err(LinkedInError::ConfigError(
            "BREAK_DURATION_MIN_SECS must be <= BREAK_DURATION_MAX_SECS".to_string(),
        ));
    }
    if !(0.0..=1.0).contains(&h.skip_send_probability) {
        return Err(LinkedInError::ConfigError(
            "SKIP_SEND_PROBABILITY must be in [0.0, 1.0]".to_string(),
        ));
    }
    if h.delay_lognormal_median_secs <= 0.0 {
        return Err(LinkedInError::ConfigError(
            "DELAY_LOGNORMAL_MEDIAN_SECS must be > 0".to_string(),
        ));
    }
    if h.delay_lognormal_sigma <= 0.0 {
        return Err(LinkedInError::ConfigError(
            "DELAY_LOGNORMAL_SIGMA must be > 0".to_string(),
        ));
    }
    Ok(())
}
