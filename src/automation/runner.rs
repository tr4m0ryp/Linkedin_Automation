//! Batch orchestrator that drives the full connection workflow.
//!
//! 1. Load CSV, filter unsent profiles
//! 2. Initialize BrowserSession
//! 3. Navigate to LinkedIn, wait for manual login
//! 4. Loop: send_connection(), mark CSV, delay on success only
//! 5. Print summary

use crate::browser::BrowserSession;
use crate::config::AppConfig;
use crate::error::Result;
use super::connection_sender;
use super::csv_reader::CsvManager;
use super::types::{ConnectionAttempt, ConnectionResult};
use rand::Rng;
use tokio::time::{sleep, Duration};
use tracing::{info, warn, error, debug};

pub struct Runner {
    config: AppConfig,
    dry_run: bool,
}

impl Runner {
    pub fn new(config: AppConfig, dry_run: bool) -> Self {
        Self { config, dry_run }
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

        // Launch browser
        let session = BrowserSession::new(
            self.config.browser.clone(),
            false,
        )
        .await?;

        // Navigate to LinkedIn and wait for login
        session.goto("https://www.linkedin.com").await?;
        info!("Please log in to LinkedIn in the browser window.");
        self.wait_for_login(&session).await?;
        info!("Login detected. Starting connection loop.");

        let mut attempts: Vec<ConnectionAttempt> = Vec::new();
        let mut sent_count: u32 = 0;
        let profile_count = profiles.len();

        for profile in &profiles {
            let attempt = connection_sender::send_connection(
                session.driver(),
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
        session.close().await?;
        Ok(())
    }

    /// Poll the current URL until it contains `/feed/`, indicating successful login.
    async fn wait_for_login(&self, session: &BrowserSession) -> Result<()> {
        let timeout = Duration::from_secs(300);
        let start = tokio::time::Instant::now();

        loop {
            if let Ok(url) = session.current_url().await {
                if url.contains("/feed/") || url.contains("/feed") {
                    return Ok(());
                }
            }
            if start.elapsed() >= timeout {
                return Err(crate::error::LinkedInError::Timeout { seconds: 300 });
            }
            sleep(Duration::from_secs(2)).await;
        }
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
