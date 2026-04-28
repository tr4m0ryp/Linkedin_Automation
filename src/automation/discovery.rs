//! Discovery pass: walk all unsent profiles needing a degree label, resolve
//! each via the LinkedIn API, and persist the freshly observed
//! `Degree`/`degree_checked_at` back to the CSV.
//!
//! Per D4 the orchestrator runs this pass before each send pass so newly
//! promoted 2nd-degrees surface as the user's existing connections grow.
//! Every read goes through the humanizer's `pre_action` (D5.B decoy browse)
//! and `wait_for_window_open` (D5.D activity window) to stay within the
//! anti-detection envelope. Fatal errors -- `SessionExpired` and
//! `RateLimitExceeded` -- propagate so the orchestrator can stop or back
//! off; transient errors are logged and skipped.

use crate::automation::csv_reader::CsvManager;
use crate::automation::humanizer::Humanizer;
use crate::automation::types::Degree;
use crate::error::{LinkedInError, Result};
use crate::linkedin_api::LinkedInClient;
use chrono::Utc;
use tracing::{debug, error, info, warn};

/// Run a single discovery pass over the supplied CSV.
///
/// Returns the number of rows newly labeled `Degree::Second` so the
/// orchestrator can decide whether to keep iterating or fall through to
/// the 3rd-degree send pass (D4 termination).
pub async fn run_discovery_pass(
    csv: &CsvManager,
    client: &LinkedInClient,
    humanizer: &mut Humanizer,
    recheck_days: i64,
) -> Result<usize> {
    let rows = csv.read_unsent_needing_recheck(recheck_days)?;
    info!(count = rows.len(), "discovery pass: rows to fetch");

    let mut new_seconds: usize = 0;
    for row in rows {
        // D5.D: stay inside the daily activity window.
        humanizer.wait_for_window_open().await;

        // D5.B: decoy browse before each fetch. Fatal errors propagate;
        // transient decoy failures log + continue (decoy traffic is
        // non-critical context, not a blocker for the real fetch).
        if let Err(e) = humanizer.pre_action(client).await {
            if is_fatal(&e) {
                return Err(e);
            }
            debug!(error = %e, "decoy browse failed before discovery fetch, continuing");
        }

        // Resolve the profile and derive its degree from member_distance.
        let degree = match client.resolve_profile(&row.linkedin_url).await {
            Ok(p) => Degree::from_member_distance(p.member_distance),
            Err(e) if is_fatal(&e) => return Err(e),
            Err(e) => {
                warn!(url = %row.linkedin_url, error = %e, "profile resolve failed, skipping row");
                continue;
            },
        };

        // Persist the freshly observed degree + timestamp.
        if let Err(e) = csv.write_degree(&row.linkedin_url, degree, Utc::now()) {
            error!(url = %row.linkedin_url, error = %e, "csv write_degree failed");
            continue;
        }

        if degree == Degree::Second {
            new_seconds += 1;
        }
        debug!(url = %row.linkedin_url, ?degree, "discovery: labeled");
    }

    info!(new_seconds, "discovery pass complete");
    Ok(new_seconds)
}

/// Errors the orchestrator must react to (auth dropped, server-side back-off).
fn is_fatal(e: &LinkedInError) -> bool {
    matches!(
        e,
        LinkedInError::SessionExpired | LinkedInError::RateLimitExceeded { .. }
    )
}
