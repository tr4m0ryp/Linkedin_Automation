//! Phased orchestrator that drives the full connection workflow per D4.
//!
//! Each iteration of the outer loop:
//!   1. Discovery pass -- label unsent profiles by `Degree`.
//!   2. Send pass targeting `Degree::Second`.
//!   3. If discovery yielded zero new 2nds AND the send pass sent zero,
//!      fall through to one final 3rd-degree send pass and break.
//!
//! Every read and write goes through the `Humanizer` so the workload stays
//! inside the D5 anti-detection envelope (window, decoy browsing, lognormal
//! delays, scheduled breaks, per-day cap, random skip).

use super::connection_sender;
use super::csv_reader::CsvManager;
use super::discovery;
use super::humanizer::{Humanizer, DEFAULT_STATE_PATH};
use super::types::{ConnectionResult, Degree};
use crate::config::AppConfig;
use crate::error::{LinkedInError, Result};
use crate::linkedin_api::{self, LinkedInClient, SessionConfig};
use tracing::{debug, error, info, warn};

/// Top-level orchestrator. Owns config + run flags but no per-run state --
/// the per-run `Humanizer`, CSV manager, and client are constructed inside
/// `run` so a failed run does not poison a future run.
pub struct Runner {
    config: AppConfig,
    dry_run: bool,
    force_login: bool,
}

impl Runner {
    /// Build a runner from validated config plus CLI flags. Side-effect free.
    pub fn new(config: AppConfig, dry_run: bool, force_login: bool) -> Self {
        Self {
            config,
            dry_run,
            force_login,
        }
    }

    /// Drive the phased discovery / send / re-rank loop to convergence.
    ///
    /// `Result::Err` only on fatal conditions (`SessionExpired`,
    /// `RateLimitExceeded`, irrecoverable I/O). Per-row failures are logged
    /// and skipped.
    pub async fn run(&self) -> Result<()> {
        let csv = CsvManager::new(&self.config.automation.csv_path);
        let (total, unsent) = csv.counts()?;
        info!(total, unsent, "CSV loaded");
        if unsent == 0 {
            info!("No unsent profiles remaining. Nothing to do.");
            return Ok(());
        }

        let client = self.build_client().await?;
        let mut humanizer = Humanizer::from_config(&self.config.humanizer, DEFAULT_STATE_PATH);
        humanizer.stats.reset_if_new_day();

        let recheck_days = self.config.humanizer.degree_recheck_days;
        let mut total_sent: u32 = 0;
        let mut pass_idx: u32 = 0;

        loop {
            pass_idx += 1;
            info!(pass = pass_idx, "PHASE: discovery");
            let new_seconds =
                discovery::run_discovery_pass(&csv, &client, &mut humanizer, recheck_days).await?;

            info!(pass = pass_idx, "PHASE: send (Degree::Second)");
            let sent_this_pass = self
                .run_send_pass(&csv, &client, &mut humanizer, Degree::Second)
                .await?;
            total_sent = total_sent.saturating_add(sent_this_pass);

            if new_seconds == 0 && sent_this_pass == 0 {
                info!("re-rank converged, processing 3rd-degree fall-through");
                let final_third = self
                    .run_send_pass(&csv, &client, &mut humanizer, Degree::ThirdOrMore)
                    .await?;
                total_sent = total_sent.saturating_add(final_third);
                break;
            }
        }

        info!(total_sent, passes = pass_idx, "automation complete");
        if self.dry_run {
            info!("Mode: DRY RUN (no connections were actually sent)");
        }
        Ok(())
    }

    /// Send connection requests to every unsent profile already labeled with
    /// `target_degree`. Returns the count of successful sends in this pass.
    async fn run_send_pass(
        &self,
        csv: &CsvManager,
        client: &LinkedInClient,
        humanizer: &mut Humanizer,
        target_degree: Degree,
    ) -> Result<u32> {
        let candidates = csv.read_unsent_with_degree(target_degree)?;
        info!(
            target = ?target_degree,
            count = candidates.len(),
            "send pass start"
        );

        let mut sent: u32 = 0;
        for profile in candidates {
            // D5.D: stay inside the activity window and respect the daily cap.
            humanizer.wait_for_window_open().await;
            if humanizer.cap_reached() {
                info!(
                    cap = self.config.humanizer.daily_send_cap,
                    "daily cap reached, stopping pass"
                );
                break;
            }

            // D5.B: decoy browse before each real action.
            if let Err(e) = humanizer.pre_action(client).await {
                if is_fatal(&e) {
                    return Err(e);
                }
                debug!(error = %e, "decoy browse failed before send, continuing");
            }

            // D5.C: random skip-the-send. We still browsed via pre_action so
            // the trace looks like a session of casual browsing without a
            // follow-up invite.
            if humanizer.should_skip_send() {
                debug!(url = %profile.linkedin_url, "humanizer: skip-the-send");
                continue;
            }

            let attempt =
                connection_sender::send_connection(client, &profile.linkedin_url, self.dry_run)
                    .await;

            match &attempt.result {
                ConnectionResult::Success => {
                    if !self.dry_run {
                        if let Err(e) = csv.mark_sent(&profile.linkedin_url) {
                            error!(url = %profile.linkedin_url, error = %e, "mark_sent failed");
                        }
                    }
                    sent = sent.saturating_add(1);
                    // Persist counters, optionally take a long break, then
                    // sleep the inter-send delay. Fatal storage errors
                    // propagate; in practice `after_send` only logs save
                    // failures so this almost always returns Ok.
                    humanizer.after_send().await?;
                },
                ConnectionResult::Pending | ConnectionResult::AlreadyConnected => {
                    debug!(
                        url = %profile.linkedin_url,
                        result = %attempt.result,
                        "marking sent, no delay"
                    );
                    if !self.dry_run {
                        if let Err(e) = csv.mark_sent(&profile.linkedin_url) {
                            error!(url = %profile.linkedin_url, error = %e, "mark_sent failed");
                        }
                    }
                },
                ConnectionResult::RateLimited => {
                    warn!(
                        url = %profile.linkedin_url,
                        "send pass: rate-limited by LinkedIn, aborting pass"
                    );
                    return Err(LinkedInError::RateLimitExceeded { retry_after: 60 });
                },
                ConnectionResult::ButtonNotFound
                | ConnectionResult::ModalError
                | ConnectionResult::Error(_) => {
                    warn!(
                        url = %profile.linkedin_url,
                        result = %attempt.result,
                        "send pass: skipping due to error"
                    );
                },
            }
        }

        info!(sent, target = ?target_degree, "send pass complete");
        Ok(sent)
    }

    /// Build the LinkedIn API client, running one-time login if needed.
    async fn build_client(&self) -> Result<LinkedInClient> {
        let cookie_file = &self.config.api.cookie_file;
        let user_agent = &self.config.api.user_agent;

        let needs_login = self.force_login
            || !std::path::Path::new(cookie_file).exists()
            || !linkedin_api::validate_session(cookie_file, user_agent).await?;

        let csrf_token = if needs_login {
            if !self.force_login && std::path::Path::new(cookie_file).exists() {
                info!("Session expired or invalid. Re-authenticating...");
            }
            linkedin_api::one_time_login(cookie_file, user_agent).await?
        } else {
            // Load CSRF from existing cookies.
            let jar = linkedin_api::load_cookies(cookie_file)?;
            crate::linkedin_api::session::extract_csrf_token(&jar).unwrap_or_default()
        };

        if csrf_token.is_empty() {
            warn!("CSRF token is empty -- API calls may fail");
        }

        let session_config = SessionConfig {
            cookie_file: cookie_file.clone(),
            user_agent: user_agent.clone(),
            csrf_token,
        };

        LinkedInClient::new(&session_config)
    }
}

/// Errors the orchestrator must react to (auth dropped, server-side back-off).
fn is_fatal(e: &LinkedInError) -> bool {
    matches!(
        e,
        LinkedInError::SessionExpired | LinkedInError::RateLimitExceeded { .. }
    )
}
