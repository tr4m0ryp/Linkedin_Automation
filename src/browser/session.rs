use crate::error::{LinkedInError, Result};
use super::types::BrowserConfig;
use super::network_monitor::NetworkMonitor;
use thirtyfour::prelude::*;
use std::time::Duration;
use tracing::{info, debug};

pub struct BrowserSession {
    driver: WebDriver,
    #[allow(dead_code)]
    config: BrowserConfig,
    network_monitor: Option<NetworkMonitor>,
}

impl BrowserSession {
    pub async fn new(config: BrowserConfig, enable_monitoring: bool) -> Result<Self> {
        info!("Initializing browser session");

        let mut caps = DesiredCapabilities::chrome();

        let mut chrome_args = vec![
            format!("--user-data-dir={}", config.session_dir),
            "--disable-blink-features=AutomationControlled".to_string(),
            "--disable-dev-shm-usage".to_string(),
            "--no-sandbox".to_string(),
            "--disable-web-security".to_string(),
            format!("--remote-debugging-port={}", config.debug_port),
        ];

        if config.headless {
            chrome_args.push("--headless=new".to_string());
        }

        caps.set_disable_dev_shm_usage()?;
        caps.add_arg("--disable-blink-features=AutomationControlled")?;
        caps.add_arg(&format!("--user-data-dir={}", config.session_dir))?;
        caps.add_arg("--no-sandbox")?;
        caps.add_arg(&format!("--remote-debugging-port={}", config.debug_port))?;

        if config.headless {
            caps.set_headless()?;
        }

        debug!("Connecting to WebDriver at: {}", config.webdriver_url);
        let driver = WebDriver::new(&config.webdriver_url, caps).await?;

        driver.set_page_load_timeout(Duration::from_secs(30)).await?;

        let network_monitor = if enable_monitoring {
            info!("Initializing network monitor on port {}", config.debug_port);
            Some(NetworkMonitor::new(config.debug_port).await?)
        } else {
            None
        };

        info!("Browser session initialized successfully");

        Ok(Self {
            driver,
            config,
            network_monitor,
        })
    }

    /// Access the underlying WebDriver instance.
    pub fn driver(&self) -> &WebDriver {
        &self.driver
    }

    /// Return the current page URL.
    pub async fn current_url(&self) -> Result<String> {
        let url = self.driver.current_url().await?;
        Ok(url.to_string())
    }

    pub async fn goto(&self, url: &str) -> Result<()> {
        info!("Navigating to: {}", url);
        self.driver.goto(url).await?;
        Ok(())
    }

    pub async fn wait_for_user(&self, message: &str) -> Result<()> {
        info!("{}", message);
        info!("Press Enter in the terminal when done...");

        let mut input = String::new();
        std::io::stdin().read_line(&mut input).map_err(|e| {
            LinkedInError::BrowserError(format!("Failed to read input: {}", e))
        })?;

        Ok(())
    }

    pub fn get_monitor(&self) -> Option<&NetworkMonitor> {
        self.network_monitor.as_ref()
    }

    pub fn get_monitor_mut(&mut self) -> Option<&mut NetworkMonitor> {
        self.network_monitor.as_mut()
    }

    pub async fn close(self) -> Result<()> {
        info!("Closing browser session");

        if let Some(monitor) = self.network_monitor {
            monitor.stop().await?;
        }

        self.driver.quit().await?;
        info!("Browser session closed");
        Ok(())
    }
}
