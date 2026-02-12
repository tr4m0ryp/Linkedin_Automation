use clap::Parser;
use tracing::{info, error};

use linkedin_automation::automation::Runner;
use linkedin_automation::config::load_config;
use linkedin_automation::Result;

#[derive(Parser, Debug)]
#[command(
    name = "linkedin-automation",
    about = "CSV-driven LinkedIn connection automation"
)]
struct Args {
    /// Enable verbose (debug-level) logging.
    #[arg(short, long)]
    verbose: bool,

    /// Dry run -- iterate profiles but do not click Connect.
    #[arg(short, long)]
    dry_run: bool,

    /// Path to the profiles CSV file.
    #[arg(long, default_value = "linkedin_profiles.csv")]
    csv_path: Option<String>,

    /// Path to the .env configuration file.
    #[arg(short, long, default_value = ".env")]
    config: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let log_level = if args.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(format!("linkedin_automation={}", log_level))
        .with_target(false)
        .init();

    let mut config = load_config(&args.config)?;

    // CLI overrides
    if let Some(csv_path) = args.csv_path {
        config.automation.csv_path = csv_path;
    }

    info!("LinkedIn Connection Automation");
    info!("CSV: {}", config.automation.csv_path);
    info!(
        "Delay on success: {}-{} minutes",
        config.automation.min_delay_min,
        config.automation.max_delay_min,
    );

    if args.dry_run {
        info!("Mode: DRY RUN -- no connections will be sent");
    }

    let runner = Runner::new(config, args.dry_run);

    if let Err(e) = runner.run().await {
        error!("Automation failed: {}", e);
        return Err(e);
    }

    info!("Done.");
    Ok(())
}
