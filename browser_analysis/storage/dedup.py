"""Deduplication logic for logged requests."""
import hashlib
import json
from typing import Dict, Optional, Set


class RequestDeduplicator:
    """In-memory deduplication cache for network requests."""

    def __init__(self):
        """Initialize empty deduplication cache."""
        self.seen_hashes: Set[str] = set()

    def _compute_hash(self, request_data: Dict) -> str:
        """
        Compute hash of request for deduplication.

        Args:
            request_data: Request data dictionary

        Returns:
            SHA256 hash of normalized request data
        """
        # Create a deterministic string from request URL + method + post_data
        url = request_data.get('url', '')
        method = request_data.get('method', '')
        post_data = request_data.get('post_data', '')

        # Normalize the data
        normalized = f"{method}:{url}:{post_data}"

        # Compute hash
        return hashlib.sha256(normalized.encode('utf-8')).hexdigest()

    def is_duplicate(self, request_data: Dict) -> bool:
        """
        Check if request is a duplicate.

        Args:
            request_data: Request data dictionary

        Returns:
            True if request has been seen before, False otherwise
        """
        request_hash = self._compute_hash(request_data)
        return request_hash in self.seen_hashes

    def mark_as_seen(self, request_data: Dict) -> str:
        """
        Mark request as seen and return its hash.

        Args:
            request_data: Request data dictionary

        Returns:
            Hash of the request
        """
        request_hash = self._compute_hash(request_data)
        self.seen_hashes.add(request_hash)
        return request_hash

    def clear(self):
        """Clear deduplication cache."""
        self.seen_hashes.clear()

    def size(self) -> int:
        """Return number of unique requests seen."""
        return len(self.seen_hashes)
