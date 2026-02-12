"""Network request/response logger using Playwright."""
import json
import uuid
from datetime import datetime
from typing import Callable, Dict, Any, Optional
from playwright.async_api import Page, Request, Response
from rich.console import Console

from storage import JSONWriter, RequestDeduplicator


class NetworkLogger:
    """Captures and logs network traffic from Playwright page."""

    def __init__(
        self,
        session_id: str,
        json_writer: JSONWriter,
        filter_func: Callable[[Request], bool],
        console: Optional[Console] = None,
        verbose: bool = True
    ):
        """
        Initialize network logger.

        Args:
            session_id: Session identifier for this logging session
            json_writer: JSONWriter instance for file operations
            filter_func: Function to filter which requests to log
            console: Rich Console for terminal output
            verbose: Whether to show detailed terminal output
        """
        self.session_id = session_id
        self.json_writer = json_writer
        self.filter_func = filter_func
        self.console = console or Console()
        self.verbose = verbose
        self.deduplicator = RequestDeduplicator()

        # Statistics
        self.stats = {
            'total_requests': 0,
            'logged_requests': 0,
            'connection_requests': 0,
            'unique_endpoints': set(),
            'endpoint_counts': {},
            'request_files': []
        }

        # Store pending requests (waiting for response)
        self.pending_requests: Dict[str, Dict[str, Any]] = {}

    def _generate_request_id(self) -> str:
        """Generate unique request ID."""
        return f"req_{uuid.uuid4().hex[:12]}"

    async def _on_request(self, request: Request):
        """Handle request event."""
        self.stats['total_requests'] += 1

        # Check if request should be logged
        if not self.filter_func(request):
            return

        # Create request data
        request_data = {
            'url': request.url,
            'method': request.method,
            'headers': dict(request.headers),
            'post_data': request.post_data
        }

        # Check for duplicates
        if self.deduplicator.is_duplicate(request_data):
            return

        # Mark as seen and generate ID
        request_hash = self.deduplicator.mark_as_seen(request_data)
        request_id = self._generate_request_id()

        # Store as pending (waiting for response)
        self.pending_requests[request.url] = {
            'request_id': request_id,
            'timestamp': datetime.utcnow().isoformat(),
            'request': request_data
        }

        if self.verbose:
            self.console.print(
                f"[dim]{datetime.now().strftime('%H:%M:%S')}[/dim] "
                f"[cyan]{request.method}[/cyan] {request.url}"
            )

    async def _on_response(self, response: Response):
        """Handle response event."""
        request = response.request

        # Check if we have pending request data
        if request.url not in self.pending_requests:
            return

        # Get pending request data
        pending_data = self.pending_requests.pop(request.url)

        # Parse response body
        response_body = None
        try:
            content_type = response.headers.get('content-type', '')
            if 'application/json' in content_type:
                response_body = await response.json()
            else:
                text = await response.text()
                response_body = text[:1000]  # Truncate long text responses
        except Exception as e:
            response_body = f"<Error parsing response: {str(e)}>"

        # Complete request/response data
        complete_data = {
            'session_id': self.session_id,
            'request_id': pending_data['request_id'],
            'timestamp': pending_data['timestamp'],
            'request': pending_data['request'],
            'response': {
                'status': response.status,
                'headers': dict(response.headers),
                'body': response_body
            }
        }

        # Write to file
        file_path = await self.json_writer.write_request_log(
            self.session_id,
            pending_data['request_id'],
            complete_data
        )

        # Update statistics
        self.stats['logged_requests'] += 1
        self.stats['request_files'].append(str(file_path))

        # Extract endpoint path for statistics
        endpoint = self._extract_endpoint(request.url)
        self.stats['unique_endpoints'].add(endpoint)
        self.stats['endpoint_counts'][endpoint] = \
            self.stats['endpoint_counts'].get(endpoint, 0) + 1

        # Check if connection request (Voyager or SDUI)
        url_lower = request.url.lower()
        is_connection = (
            'invitation' in url_lower
            or 'connection' in url_lower
            or 'addaaddconnection' in url_lower
        )
        if is_connection:
            self.stats['connection_requests'] += 1

        # Terminal output
        if self.verbose:
            status_color = "green" if response.status < 400 else "red"
            self.console.print(
                f"[dim]{datetime.now().strftime('%H:%M:%S')}[/dim] "
                f"[cyan]{request.method}[/cyan] {endpoint} → "
                f"[{status_color}]{response.status}[/{status_color}]"
            )

            # Show connection request details
            if is_connection:
                invitation_id = None
                if isinstance(response_body, dict) and 'value' in response_body:
                    invitation_id = response_body.get('value', {}).get('invitationId')
                if invitation_id:
                    self.console.print(
                        f"           -- Connection request sent (invitation_id: {invitation_id})"
                    )
                if 'addaaddconnection' in url_lower:
                    self.console.print(
                        f"           -- SDUI connection action detected"
                    )
                self.console.print(f"           -- Saved: {file_path.name}")

    def _extract_endpoint(self, url: str) -> str:
        """Extract endpoint path from full URL."""
        try:
            if '/voyager/' in url:
                endpoint = '/voyager/' + url.split('/voyager/')[1].split('?')[0]
            elif '/flagship-web/' in url:
                endpoint = '/flagship-web/' + url.split('/flagship-web/')[1].split('?')[0]
            else:
                endpoint = url.split('?')[0]
            return endpoint
        except Exception:
            return url

    async def attach_to_page(self, page: Page):
        """
        Attach logger to Playwright page.

        Args:
            page: Playwright Page to monitor
        """
        page.on('request', self._on_request)
        page.on('response', self._on_response)

    def get_statistics(self) -> Dict[str, Any]:
        """
        Get current session statistics.

        Returns:
            Dictionary of statistics
        """
        return {
            'total_requests': self.stats['total_requests'],
            'logged_requests': self.stats['logged_requests'],
            'connection_requests': self.stats['connection_requests'],
            'unique_endpoints': sorted(list(self.stats['unique_endpoints'])),
            'endpoint_counts': self.stats['endpoint_counts'],
            'request_files': self.stats['request_files']
        }
