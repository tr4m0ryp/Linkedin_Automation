"""Storage module for JSON logging."""
from .json_writer import JSONWriter
from .dedup import RequestDeduplicator

__all__ = ['JSONWriter', 'RequestDeduplicator']
