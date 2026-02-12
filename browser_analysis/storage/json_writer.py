"""JSON file writer with atomic operations."""
import json
import tempfile
from pathlib import Path
from typing import Dict, Any
from datetime import datetime
import aiofiles
import aiofiles.os


class JSONWriter:
    """Handles atomic JSON file writes for request logs."""

    def __init__(self, base_dir: Path):
        """
        Initialize JSON writer.

        Args:
            base_dir: Base directory for all log files
        """
        self.base_dir = Path(base_dir)
        self.raw_dir = self.base_dir / "raw"
        self.aggregated_dir = self.base_dir / "aggregated"

        # Ensure directories exist
        self.raw_dir.mkdir(parents=True, exist_ok=True)
        self.aggregated_dir.mkdir(parents=True, exist_ok=True)

    async def write_request_log(
        self,
        session_id: str,
        request_id: str,
        data: Dict[str, Any]
    ) -> Path:
        """
        Write individual request log atomically.

        Args:
            session_id: Session identifier
            request_id: Unique request identifier
            data: Request/response data to write

        Returns:
            Path to written file
        """
        # Create session directory
        session_dir = self.raw_dir / session_id
        session_dir.mkdir(parents=True, exist_ok=True)

        # Generate filename with timestamp
        timestamp = datetime.utcnow().strftime("%Y-%m-%dT%H-%M-%S")
        filename = f"{timestamp}_{request_id}.json"
        target_path = session_dir / filename

        # Write atomically (temp file + rename)
        async with aiofiles.tempfile.NamedTemporaryFile(
            mode='w',
            dir=session_dir,
            delete=False,
            suffix='.tmp'
        ) as tmp_file:
            await tmp_file.write(json.dumps(data, indent=2))
            tmp_path = Path(tmp_file.name)

        # Atomic rename
        await aiofiles.os.rename(tmp_path, target_path)
        return target_path

    async def write_aggregated_summary(
        self,
        session_id: str,
        summary_data: Dict[str, Any]
    ) -> Path:
        """
        Write aggregated session summary.

        Args:
            session_id: Session identifier
            summary_data: Summary data to write

        Returns:
            Path to written file
        """
        filename = f"{session_id}.json"
        target_path = self.aggregated_dir / filename

        # Write atomically
        async with aiofiles.tempfile.NamedTemporaryFile(
            mode='w',
            dir=self.aggregated_dir,
            delete=False,
            suffix='.tmp'
        ) as tmp_file:
            await tmp_file.write(json.dumps(summary_data, indent=2))
            tmp_path = Path(tmp_file.name)

        # Atomic rename
        await aiofiles.os.rename(tmp_path, target_path)
        return target_path

    async def append_to_aggregated(
        self,
        session_id: str,
        key: str,
        value: Any
    ):
        """
        Append data to aggregated summary file.

        Args:
            session_id: Session identifier
            key: Key to update in summary
            value: Value to set
        """
        summary_path = self.aggregated_dir / f"{session_id}.json"

        # Read existing summary
        if summary_path.exists():
            async with aiofiles.open(summary_path, 'r') as f:
                content = await f.read()
                summary = json.loads(content)
        else:
            summary = {}

        # Update summary
        summary[key] = value

        # Write back
        await self.write_aggregated_summary(session_id, summary)
