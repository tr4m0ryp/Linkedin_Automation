"""Session management for persistent LinkedIn login."""
import json
from pathlib import Path
from typing import Optional
from playwright.async_api import BrowserContext
import aiofiles


class SessionManager:
    """Manages browser session persistence (cookies, localStorage)."""

    def __init__(self, session_dir: Path):
        """
        Initialize session manager.

        Args:
            session_dir: Directory to store session data
        """
        self.session_dir = Path(session_dir)
        self.session_dir.mkdir(parents=True, exist_ok=True)
        self.state_file = self.session_dir / "state.json"

    async def save_session(self, context: BrowserContext):
        """
        Save browser context state (cookies, localStorage).

        Args:
            context: Playwright BrowserContext to save
        """
        storage_state = await context.storage_state()

        async with aiofiles.open(self.state_file, 'w') as f:
            await f.write(json.dumps(storage_state, indent=2))

    async def restore_session(self, context: BrowserContext) -> bool:
        """
        Restore browser context from saved state.

        Args:
            context: Playwright BrowserContext to restore into

        Returns:
            True if session was restored, False if no saved session exists
        """
        if not self.state_file.exists():
            return False

        async with aiofiles.open(self.state_file, 'r') as f:
            content = await f.read()
            storage_state = json.loads(content)

        # Add cookies
        if 'cookies' in storage_state:
            await context.add_cookies(storage_state['cookies'])

        # Restore localStorage is handled via storage_state parameter in new_context
        return True

    def has_saved_session(self) -> bool:
        """
        Check if a saved session exists.

        Returns:
            True if session file exists, False otherwise
        """
        return self.state_file.exists()

    async def clear_session(self):
        """Clear saved session data."""
        if self.state_file.exists():
            self.state_file.unlink()
