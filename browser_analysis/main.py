"""Main entry point for LinkedIn network logger."""
import asyncio
import signal
from datetime import datetime
from pathlib import Path
import click
from rich.console import Console
from rich.panel import Panel
from rich.live import Live
from rich.table import Table

from config import load_config
from browser import BrowserLauncher, SessionManager
from interceptor import NetworkLogger, RequestFilter
from storage import JSONWriter


class LinkedInLogger:
    """Main application coordinator."""

    def __init__(self, config, restore_session: bool = False):
        """
        Initialize LinkedIn logger.

        Args:
            config: Application configuration
            restore_session: Whether to restore saved session
        """
        self.config = config
        self.restore_session = restore_session
        self.console = Console()

        # Generate session ID
        self.session_id = f"session_{datetime.utcnow().strftime('%Y-%m-%d_%H-%M-%S')}"

        # Initialize components
        self.json_writer = JSONWriter(config.logging.log_dir)
        self.session_manager = SessionManager(config.session.session_dir)

        # Browser components (initialized in run())
        self.browser = None
        self.context = None
        self.page = None
        self.network_logger = None

        # Session management
        self.running = True
        self.start_time = None

    def _show_welcome(self):
        """Display welcome message."""
        welcome_text = f"""[bold]LinkedIn Network Logger[/bold]
Session ID: [cyan]{self.session_id}[/cyan]
Browser: [green]{self.config.browser.browser_type}[/green]
Filters: LinkedIn API only (/voyager/*)

Logs will be saved to:
  • Raw: {self.config.logging.log_dir}/raw/{self.session_id}/
  • Summary: {self.config.logging.log_dir}/aggregated/{self.session_id}.json
"""
        self.console.print(Panel(welcome_text, title="Configuration", border_style="blue"))

    async def _setup_browser(self):
        """Launch and configure browser."""
        self.console.print("\n[yellow]Launching browser...[/yellow]")

        # Determine session state path
        session_state_path = None
        if self.restore_session and self.session_manager.has_saved_session():
            session_state_path = self.session_manager.state_file
            self.console.print("[green]✓[/green] Restoring saved session")

        # Launch browser
        launcher = BrowserLauncher(
            browser_type=self.config.browser.browser_type,
            headless=self.config.browser.headless,
            session_state_path=session_state_path
        )
        self.browser, self.context, self.page = await launcher.launch()

        # Setup network logger
        request_filter = RequestFilter(
            endpoints=self.config.filtering.endpoints,
            priority_keywords=self.config.filtering.priority_keywords,
            api_only=self.config.filtering.api_only
        )

        self.network_logger = NetworkLogger(
            session_id=self.session_id,
            json_writer=self.json_writer,
            filter_func=request_filter.should_log_request,
            console=self.console,
            verbose=self.config.output.verbose
        )

        await self.network_logger.attach_to_page(self.page)

        self.console.print("[green]✓[/green] Browser launched and network logging attached\n")

    async def _navigate_to_linkedin(self):
        """Navigate to LinkedIn."""
        self.console.print("[yellow]Navigating to LinkedIn...[/yellow]")
        await self.page.goto("https://www.linkedin.com")
        self.console.print("[green]✓[/green] Loaded https://www.linkedin.com\n")

    async def _wait_for_login(self):
        """Wait for user to log in."""
        if self.restore_session and self.session_manager.has_saved_session():
            self.console.print("[cyan]Session restored. Checking login status...[/cyan]\n")
        else:
            self.console.print(
                "[yellow]Please log in to LinkedIn manually in the browser window.[/yellow]\n"
            )
            self.console.print("Waiting for login...\n")

        # Wait for feed or profile to indicate successful login
        try:
            await self.page.wait_for_url("**/feed/**", timeout=300000)  # 5 min timeout
            self.console.print("[green]✓ Logged in detected[/green]\n")
            return True
        except Exception:
            # Check if already on feed
            if "/feed/" in self.page.url:
                self.console.print("[green]✓ Already logged in[/green]\n")
                return True
            return False

    async def _auto_save_session(self):
        """Periodically save session."""
        while self.running:
            await asyncio.sleep(self.config.session.save_interval_sec)
            if self.running and self.config.session.auto_save:
                await self.session_manager.save_session(self.context)

    async def _show_live_stats(self):
        """Display live statistics."""
        with Live(console=self.console, refresh_per_second=1) as live:
            while self.running:
                stats = self.network_logger.get_statistics()
                duration = datetime.utcnow() - self.start_time
                hours, remainder = divmod(int(duration.total_seconds()), 3600)
                minutes, seconds = divmod(remainder, 60)

                table = Table(show_header=False, box=None)
                table.add_row(
                    f"[cyan]API Requests:[/cyan] {stats['logged_requests']}",
                    f"[cyan]Connection Requests:[/cyan] {stats['connection_requests']}",
                    f"[cyan]Duration:[/cyan] {hours}h {minutes}m {seconds}s"
                )

                live.update(Panel(table, title="Session Statistics", border_style="green"))
                await asyncio.sleep(1)

    async def _shutdown(self):
        """Graceful shutdown."""
        self.console.print("\n\n[yellow]Shutting down...[/yellow]")
        self.running = False

        # Save session
        if self.config.session.auto_save and self.context:
            await self.session_manager.save_session(self.context)
            self.console.print("[green]✓[/green] Session saved")

        # Save aggregated summary
        stats = self.network_logger.get_statistics()
        duration = datetime.utcnow() - self.start_time

        summary = {
            'session_id': self.session_id,
            'start_time': self.start_time.isoformat(),
            'end_time': datetime.utcnow().isoformat(),
            'duration_seconds': int(duration.total_seconds()),
            **stats
        }

        summary_path = await self.json_writer.write_aggregated_summary(
            self.session_id,
            summary
        )
        self.console.print(f"[green]✓[/green] Summary saved: {summary_path}")

        # Close browser
        if self.browser:
            await self.browser.close()

        # Show final stats
        self.console.print("\n" + "="*60)
        self.console.print(f"[bold]Session Complete: {self.session_id}[/bold]")
        self.console.print(f"Total API Requests: {stats['logged_requests']}")
        self.console.print(f"Connection Requests: {stats['connection_requests']}")
        self.console.print(f"Duration: {duration}")
        self.console.print("="*60 + "\n")

    async def run(self):
        """Run the main application."""
        self.start_time = datetime.utcnow()

        try:
            # Show welcome
            self._show_welcome()

            # Setup browser
            await self._setup_browser()

            # Navigate to LinkedIn
            await self._navigate_to_linkedin()

            # Wait for login
            logged_in = await self._wait_for_login()
            if not logged_in:
                self.console.print("[red]Login timeout. Exiting.[/red]")
                return

            # Start auto-save task
            if self.config.session.auto_save:
                asyncio.create_task(self._auto_save_session())

            # Show instructions
            self.console.print(
                "\n[bold green]Network logging active![/bold green]\n"
                "Make connection requests manually in the browser.\n"
                "Press Ctrl+C to stop logging and save results.\n"
            )

            # Keep running until interrupted
            while self.running:
                await asyncio.sleep(1)

        except KeyboardInterrupt:
            pass
        finally:
            await self._shutdown()


@click.group()
def cli():
    """LinkedIn Network Logger - Capture LinkedIn API calls."""
    pass


@cli.command()
@click.option('--restore', is_flag=True, help='Restore previous session (skip login)')
@click.option('--env-file', type=click.Path(exists=True), help='Path to .env file')
def capture(restore: bool, env_file: str):
    """Start network traffic capture session."""
    # Load configuration
    config = load_config(env_file)

    # Run application
    app = LinkedInLogger(config, restore_session=restore)

    # Setup signal handlers
    loop = asyncio.new_event_loop()
    asyncio.set_event_loop(loop)

    for sig in (signal.SIGINT, signal.SIGTERM):
        loop.add_signal_handler(sig, lambda: asyncio.create_task(app._shutdown()))

    try:
        loop.run_until_complete(app.run())
    finally:
        loop.close()


if __name__ == '__main__':
    cli()
