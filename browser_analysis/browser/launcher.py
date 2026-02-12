"""Browser launcher with anti-detection measures."""
from pathlib import Path
from typing import Tuple, Optional
from playwright.async_api import async_playwright, Browser, BrowserContext, Page, Playwright
try:
    from playwright_stealth import stealth_async
    STEALTH_AVAILABLE = True
except ImportError:
    STEALTH_AVAILABLE = False


class BrowserLauncher:
    """Launches and configures Playwright browser with anti-detection."""

    def __init__(
        self,
        browser_type: str = "chromium",
        headless: bool = False,
        session_state_path: Optional[Path] = None
    ):
        """
        Initialize browser launcher.

        Args:
            browser_type: Browser to launch (chromium, firefox, webkit)
            headless: Whether to run in headless mode
            session_state_path: Path to saved session state for restoration
        """
        self.browser_type = browser_type
        self.headless = headless
        self.session_state_path = session_state_path
        self.playwright: Optional[Playwright] = None

    async def launch(self) -> Tuple[Browser, BrowserContext, Page]:
        """
        Launch browser with anti-detection measures.

        Returns:
            Tuple of (Browser, BrowserContext, Page)
        """
        # Start Playwright
        self.playwright = await async_playwright().start()

        # Get browser type
        if self.browser_type == "chromium":
            browser_launcher = self.playwright.chromium
        elif self.browser_type == "firefox":
            browser_launcher = self.playwright.firefox
        elif self.browser_type == "webkit":
            browser_launcher = self.playwright.webkit
        else:
            raise ValueError(f"Invalid browser type: {self.browser_type}")

        # Launch browser with anti-detection flags
        browser = await browser_launcher.launch(
            headless=self.headless,
            args=[
                '--disable-blink-features=AutomationControlled',
                '--disable-dev-shm-usage',
                '--no-sandbox'
            ]
        )

        # Create context with realistic settings
        context_options = {
            'viewport': {'width': 1920, 'height': 1080},
            'user_agent': (
                'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 '
                '(KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36'
            ),
            'locale': 'en-US',
            'timezone_id': 'America/New_York'
        }

        # Load saved session state if available
        if self.session_state_path and self.session_state_path.exists():
            context_options['storage_state'] = str(self.session_state_path)

        context = await browser.new_context(**context_options)

        # Create page
        page = await context.new_page()

        # Apply stealth if available
        if STEALTH_AVAILABLE:
            await stealth_async(page)

        return browser, context, page

    async def close(self):
        """Close Playwright instance."""
        if self.playwright:
            await self.playwright.stop()
