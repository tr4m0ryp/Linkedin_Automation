//! Batch orchestrator that drives the full connection workflow.
//!
//! 1. Load CSV, filter unsent profiles
//! 2. Initialize LinkedInClient (load cookies or run one-time login)
//! 3. Loop: send_connection(), mark CSV, delay on success only
//! 4. Print summary

use crate::config::AppConfig;
use crate::error::Result;
use crate::linkedin_api::{self, LinkedInClient, SessionConfig};
use super::connection_sender;
use super::csv_reader::CsvManager;
use super::types::{ConnectionAttempt, ConnectionResult};
use rand::Rng;
use tokio::time::{sleep, Duration};
use tracing::{info, warn, error, debug};

pub struct Runner {
    config: AppConfig,
    dry_run: bool,
    force_login: bool,
}

impl Runner {
    pub fn new(config: AppConfig, dry_run: bool, force_login: bool) -> Self {
        Self { config, dry_run, force_login }
    }

    pub async fn run(&self) -> Result<()> {
        let csv = CsvManager::new(&self.config.automation.csv_path);

        let (total, unsent) = csv.counts()?;
        info!("CSV loaded: {} total profiles, {} unsent", total, unsent);

        if unsent == 0 {
            info!("No unsent profiles remaining. Nothing to do.");
            return Ok(());
        }

        let profiles = csv.read_unsent()?;
        info!("Will process all {} unsent profiles", profiles.len());

        let client = self.build_client().await?;

        let mut attempts: Vec<ConnectionAttempt> = Vec::new();
        let mut sent_count: u32 = 0;
        let profile_count = profiles.len();

        for profile in &profiles {
            let attempt = connection_sender::send_connection(
                &client,
                &profile.linkedin_url,
                self.dry_run,
            )
            .await;

            match &attempt.result {
                ConnectionResult::Success => {
                    if !self.dry_run {
                        if let Err(e) = csv.mark_sent(&profile.linkedin_url) {
                            error!("Failed to mark CSV row as sent: {}", e);
                        }
                    }
                    sent_count += 1;

                    if !self.dry_run {
                        let delay_min = rand::thread_rng().gen_range(
                            self.config.automation.min_delay_min
                                ..=self.config.automation.max_delay_min,
                        );
                        let delay_secs = delay_min * 60;
                        info!("Connection sent. Waiting {} minutes before next.", delay_min);
                        sleep(Duration::from_secs(delay_secs)).await;
                    }
                }
                ConnectionResult::Pending | ConnectionResult::AlreadyConnected => {
                    debug!(
                        "Profile {}: {} -- marking sent, no delay",
                        profile.linkedin_url, attempt.result
                    );
                    if !self.dry_run {
                        if let Err(e) = csv.mark_sent(&profile.linkedin_url) {
                            error!("Failed to mark CSV row as sent: {}", e);
                        }
                    }
                }
                ConnectionResult::ButtonNotFound
                | ConnectionResult::ModalError
                | ConnectionResult::RateLimited
                | ConnectionResult::Error(_) => {
                    warn!(
                        "Profile {}: {} -- skipping",
                        profile.linkedin_url, attempt.result
                    );
                }
            }

            attempts.push(attempt);

            info!(
                "Progress: {}/{} (sent so far: {})",
                attempts.len(),
                profile_count,
                sent_count,
            );
        }

        self.print_summary(&attempts, sent_count);
        Ok(())
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
            // Load CSRF from existing cookies
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

    fn print_summary(&self, attempts: &[ConnectionAttempt], sent_count: u32) {
        let success = attempts
            .iter()
            .filter(|a| a.result == ConnectionResult::Success)
            .count();
        let already = attempts
            .iter()
            .filter(|a| a.result == ConnectionResult::AlreadyConnected)
            .count();
        let pending = attempts
            .iter()
            .filter(|a| a.result == ConnectionResult::Pending)
            .count();
        let errors = attempts
            .iter()
            .filter(|a| matches!(a.result, ConnectionResult::Error(_)))
            .count();
        let not_found = attempts
            .iter()
            .filter(|a| a.result == ConnectionResult::ButtonNotFound)
            .count();

        info!("--- Session Summary ---");
        info!("Total attempted: {}", attempts.len());
        info!("Successful:      {}", success);
        info!("Already connected: {}", already);
        info!("Already pending: {}", pending);
        info!("Button not found: {}", not_found);
        info!("Errors:          {}", errors);
        info!("CSV rows updated: {}", sent_count);

        if self.dry_run {
            info!("Mode: DRY RUN (no connections were actually sent)");
        }
    }
}
