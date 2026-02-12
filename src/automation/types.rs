//! Data types for the connection automation workflow.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Outcome of a single connection attempt.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ConnectionResult {
    /// Connect button clicked and confirmed as Pending.
    Success,
    /// Profile already shows a "Message" button (already connected).
    AlreadyConnected,
    /// Profile already shows "Pending" (request already sent).
    Pending,
    /// Could not find the Connect button on the page.
    ButtonNotFound,
    /// Rate limit indicator detected on the page.
    RateLimited,
    /// The "Add a note" modal did not behave as expected.
    ModalError,
    /// An unexpected error occurred.
    Error(String),
}

impl std::fmt::Display for ConnectionResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Success => write!(f, "Success"),
            Self::AlreadyConnected => write!(f, "Already connected"),
            Self::Pending => write!(f, "Pending"),
            Self::ButtonNotFound => write!(f, "Button not found"),
            Self::RateLimited => write!(f, "Rate limited"),
            Self::ModalError => write!(f, "Modal error"),
            Self::Error(msg) => write!(f, "Error: {}", msg),
        }
    }
}

/// A single row from the CSV file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsvProfile {
    pub linkedin_url: String,
    pub is_sent: bool,
}

/// Record of one connection attempt for logging/reporting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionAttempt {
    pub profile_url: String,
    pub result: ConnectionResult,
    pub timestamp: DateTime<Utc>,
    pub error_message: Option<String>,
}

impl ConnectionAttempt {
    pub fn new(profile_url: String, result: ConnectionResult) -> Self {
        let error_message = match &result {
            ConnectionResult::Error(msg) => Some(msg.clone()),
            _ => None,
        };
        Self {
            profile_url,
            result,
            timestamp: Utc::now(),
            error_message,
        }
    }
}
