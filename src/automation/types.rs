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

/// Network-distance classification for a profile.
///
/// Persisted in the CSV as the `degree` column. The serialized form is `""`
/// for `Unknown`, `"2"` for `Second`, and `"3"` for `ThirdOrMore`. A
/// 1st-degree connection (already connected) is folded into `Second` for
/// purposes of "this is a candidate to send to" -- callers gate on
/// `connection_state` separately when they need to distinguish (D3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Degree {
    /// 2nd-degree connection (sendable candidate).
    Second,
    /// 3rd-degree or more distant. Skipped by the ranker (D3).
    ThirdOrMore,
    /// Not yet fetched from the LinkedIn API.
    #[default]
    Unknown,
}

impl Degree {
    /// Parse the serialized form persisted in the CSV.
    ///
    /// Recognizes `"2"` -> `Second`, `"3"` -> `ThirdOrMore`. Any other value
    /// (including the empty string and whitespace) maps to `Unknown` so legacy
    /// rows and future-extended values fail soft.
    pub fn from_csv_value(value: &str) -> Self {
        match value.trim() {
            "2" => Self::Second,
            "3" => Self::ThirdOrMore,
            _ => Self::Unknown,
        }
    }

    /// Map a raw `memberDistance` code (per `ProfileData::member_distance`)
    /// to the ranker-facing `Degree` enum.
    ///
    /// `Some(1)` (already 1st-degree) and `Some(2)` collapse to `Second`
    /// because both are sendable / connectable signals (D3). Larger codes
    /// (3rd-degree, OUT_OF_NETWORK reported as `Some(4)`, etc.) map to
    /// `ThirdOrMore`. `None` -- distance was not present or could not be
    /// parsed -- falls back to `Unknown` so the discovery pass treats the
    /// row as still needing a re-check.
    pub fn from_member_distance(d: Option<i32>) -> Self {
        match d {
            Some(1) | Some(2) => Self::Second,
            Some(_) => Self::ThirdOrMore,
            None => Self::Unknown,
        }
    }
}

impl std::fmt::Display for Degree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Second => write!(f, "2"),
            Self::ThirdOrMore => write!(f, "3"),
            Self::Unknown => write!(f, ""),
        }
    }
}

/// A single row from the CSV file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsvProfile {
    /// LinkedIn profile URL (e.g. `https://www.linkedin.com/in/<vanity>/`).
    pub linkedin_url: String,
    /// Whether a connection request has already been sent for this row.
    pub is_sent: bool,
    /// Cached network-distance classification (D4).
    #[serde(default)]
    pub degree: Degree,
    /// Timestamp of the last `degree` check, for staleness checks (D4).
    #[serde(default)]
    pub degree_checked_at: Option<DateTime<Utc>>,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn degree_from_csv_value_parses_known_values() {
        assert_eq!(Degree::from_csv_value("2"), Degree::Second);
        assert_eq!(Degree::from_csv_value("3"), Degree::ThirdOrMore);
        assert_eq!(Degree::from_csv_value(""), Degree::Unknown);
        assert_eq!(Degree::from_csv_value("  "), Degree::Unknown);
        assert_eq!(Degree::from_csv_value("foo"), Degree::Unknown);
    }

    #[test]
    fn degree_display_round_trips_through_csv_value() {
        for d in [Degree::Second, Degree::ThirdOrMore, Degree::Unknown] {
            let s = d.to_string();
            assert_eq!(Degree::from_csv_value(&s), d);
        }
    }

    #[test]
    fn degree_from_member_distance_maps_known_codes() {
        assert_eq!(Degree::from_member_distance(Some(1)), Degree::Second);
        assert_eq!(Degree::from_member_distance(Some(2)), Degree::Second);
        assert_eq!(Degree::from_member_distance(Some(3)), Degree::ThirdOrMore);
        assert_eq!(Degree::from_member_distance(Some(4)), Degree::ThirdOrMore);
        assert_eq!(Degree::from_member_distance(Some(99)), Degree::ThirdOrMore);
        assert_eq!(Degree::from_member_distance(None), Degree::Unknown);
    }
}
