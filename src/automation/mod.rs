mod connection_sender;
mod csv_reader;
pub mod discovery;
pub mod humanizer;
mod runner;
mod types;

pub use csv_reader::CsvManager;
pub use humanizer::{
    ActivityWindow, BreakScheduler, DecoyBrowser, Humanizer, LogNormalDelay, SessionStats,
};
pub use runner::Runner;
pub use types::{ConnectionAttempt, ConnectionResult, CsvProfile, Degree};

use crate::error::LinkedInError;

/// Errors the orchestrator must react to (auth dropped, server-side back-off).
pub(crate) fn is_fatal(e: &LinkedInError) -> bool {
    matches!(
        e,
        LinkedInError::SessionExpired | LinkedInError::RateLimitExceeded { .. }
    )
}
