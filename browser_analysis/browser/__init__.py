"""Browser module for Playwright integration."""
from .launcher import BrowserLauncher
from .session_manager import SessionManager

__all__ = ['BrowserLauncher', 'SessionManager']
