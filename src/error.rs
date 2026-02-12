//! Error types for LinkedIn automation

use thiserror::Error;

/// Main error type for LinkedIn automation operations
#[derive(Error, Debug)]
pub enum LinkedInError {
    /// Authentication failed
    #[error("Authentication failed: {0}")]
    AuthFailed(String),

    /// Element not found in DOM
    #[error("Element not found: {selector}")]
    ElementNotFound { selector: String },

    /// Rate limit exceeded
    #[error("Rate limit exceeded: retry after {retry_after}s")]
    RateLimitExceeded { retry_after: u64 },

    /// Session expired
    #[error("Session expired")]
    SessionExpired,

    /// Browser error
    #[error("Browser error: {0}")]
    BrowserError(String),

    /// HTTP error
    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),

    /// Parse error
    #[error("Parse error: {0}")]
    ParseError(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// Storage error
    #[error("Storage error: {0}")]
    StorageError(String),

    /// Network error
    #[error("Network error: {0}")]
    NetworkError(String),

    /// Timeout error
    #[error("Operation timed out after {seconds}s")]
    Timeout { seconds: u64 },

    /// CAPTCHA detected
    #[error("CAPTCHA detected - manual intervention required")]
    CaptchaDetected,

    /// Account locked or restricted
    #[error("Account appears to be locked or restricted")]
    AccountRestricted,

    /// Invalid credentials
    #[error("Invalid credentials provided")]
    InvalidCredentials,

    /// Generic I/O error
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    /// WebDriver error
    #[error("WebDriver error: {0}")]
    WebDriverError(Box<thirtyfour::error::WebDriverError>),

    /// CSV parsing or writing error
    #[error("CSV error: {0}")]
    CsvError(String),

    /// Generic error
    #[error("{0}")]
    Other(String),
}

impl From<thirtyfour::error::WebDriverError> for LinkedInError {
    fn from(err: thirtyfour::error::WebDriverError) -> Self {
        Self::WebDriverError(Box::new(err))
    }
}

/// Result type alias using LinkedInError
pub type Result<T> = std::result::Result<T, LinkedInError>;
