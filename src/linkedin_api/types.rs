//! Data structures for the LinkedIn Voyager API client.

use serde::{Deserialize, Serialize};

/// Configuration needed to build an authenticated LinkedIn HTTP client.
#[derive(Debug, Clone)]
pub struct SessionConfig {
    /// Path to the persisted cookie JSON file.
    pub cookie_file: String,
    /// Browser user-agent string to mimic.
    pub user_agent: String,
    /// CSRF token extracted from JSESSIONID cookie.
    pub csrf_token: String,
}

/// Resolved profile information from a LinkedIn profile URL.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileData {
    /// The public profile identifier (e.g. "john-doe-12345").
    pub public_id: String,
    /// Non-iterable profile ID used in invitation payloads.
    pub member_id: String,
    /// Full profile URN (e.g. "urn:li:fsd_profile:ACoAAB...").
    pub profile_urn: String,
    /// First name.
    pub first_name: String,
    /// Last name.
    pub last_name: String,
    /// Current connection state with this profile.
    pub connection_state: ConnectionState,
}

/// Connection state between the authenticated user and a target profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ConnectionState {
    Connected,
    Pending,
    NotConnected,
    Unknown,
}

impl std::fmt::Display for ConnectionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Connected => write!(f, "Connected"),
            Self::Pending => write!(f, "Pending"),
            Self::NotConnected => write!(f, "Not connected"),
            Self::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Response from sending a connection invitation via the Voyager API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvitationResponse {
    /// Whether the invitation was accepted by the API.
    pub success: bool,
    /// HTTP status code from the API.
    pub status_code: u16,
    /// Raw response body (for debugging).
    pub body: String,
}
