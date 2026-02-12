"""Request filtering logic for LinkedIn API calls."""
from typing import Callable
from playwright.async_api import Request


class RequestFilter:
    """Filters network requests to capture only relevant LinkedIn API calls."""

    def __init__(self, endpoints: list[str], priority_keywords: list[str], api_only: bool = True):
        """
        Initialize request filter.

        Args:
            endpoints: List of endpoint patterns to match (e.g., ["/voyager/api/"])
            priority_keywords: Keywords that indicate high-priority requests
            api_only: If True, only log XHR/Fetch requests
        """
        self.endpoints = endpoints
        self.priority_keywords = priority_keywords
        self.api_only = api_only

    def should_log_request(self, request: Request) -> bool:
        """
        Determine if a request should be logged.

        Args:
            request: Playwright Request object

        Returns:
            True if request should be logged, False otherwise
        """
        url = request.url
        resource_type = request.resource_type

        # Filter by resource type (XHR/Fetch only if api_only is True)
        if self.api_only and resource_type not in ['xhr', 'fetch']:
            return False

        # Must match one of the configured endpoints
        if not any(endpoint in url for endpoint in self.endpoints):
            return False

        # Exclude static resources
        static_extensions = ['.jpg', '.jpeg', '.png', '.gif', '.css', '.js', '.woff', '.woff2', '.ttf']
        if any(url.lower().endswith(ext) for ext in static_extensions):
            return False

        return True

    def is_sdui_request(self, request: Request) -> bool:
        """
        Check if request targets the SDUI/RSC action framework.

        Args:
            request: Playwright Request object

        Returns:
            True if request is an SDUI server-request action
        """
        return '/flagship-web/rsc-action/' in request.url

    def is_connection_sdui(self, request: Request) -> bool:
        """
        Check if request is an SDUI-based connection request.

        Args:
            request: Playwright Request object

        Returns:
            True if request is an addConnection SDUI action
        """
        url = request.url.lower()
        return 'addaaddconnection' in url or 'addconnection' in url

    def is_priority_request(self, request: Request) -> bool:
        """
        Check if request is high-priority (e.g., connection-related).

        Args:
            request: Playwright Request object

        Returns:
            True if request matches priority keywords
        """
        url = request.url.lower()
        if any(keyword.lower() in url for keyword in self.priority_keywords):
            return True
        # SDUI connection actions are always high priority
        return self.is_connection_sdui(request)

    def create_filter_function(self) -> Callable[[Request], bool]:
        """
        Create a filter function for use with network logger.

        Returns:
            Filter function that takes a Request and returns bool
        """
        return self.should_log_request
