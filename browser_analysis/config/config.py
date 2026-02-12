"""Configuration loader and validator using pydantic."""
import os
from pathlib import Path
from typing import Optional
from pydantic import BaseModel, Field, validator
from dotenv import load_dotenv


class BrowserConfig(BaseModel):
    """Browser-related configuration."""
    browser_type: str = Field(default="chromium")
    headless: bool = Field(default=False)

    @validator('browser_type')
    def validate_browser_type(cls, v):
        valid_types = ['chromium', 'firefox', 'webkit']
        if v not in valid_types:
            raise ValueError(f"browser_type must be one of {valid_types}")
        return v


class SessionConfig(BaseModel):
    """Session management configuration."""
    session_dir: Path = Field(default=Path("sessions/linkedin_session"))
    auto_save: bool = Field(default=True)
    save_interval_sec: int = Field(default=300)

    @validator('session_dir')
    def validate_session_dir(cls, v):
        return Path(v)


class LoggingConfig(BaseModel):
    """Logging configuration."""
    log_dir: Path = Field(default=Path("logs"))
    log_raw_requests: bool = Field(default=True)
    log_aggregated_summary: bool = Field(default=True)

    @validator('log_dir')
    def validate_log_dir(cls, v):
        return Path(v)


class FilterConfig(BaseModel):
    """Request filtering configuration."""
    api_only: bool = Field(default=True)
    endpoints: list[str] = Field(default=[
        "/voyager/api/",
        "/voyager/",
        "/flagship-web/rsc-action/",
    ])
    priority_keywords: list[str] = Field(default=[
        "growth",
        "invitation",
        "normInvitations",
        "connection",
        "addaAddConnection",
        "addConnection",
        "server-request",
    ])


class OutputConfig(BaseModel):
    """Terminal output configuration."""
    real_time_terminal: bool = Field(default=True)
    verbose: bool = Field(default=True)


class AppConfig(BaseModel):
    """Main application configuration."""
    browser: BrowserConfig = Field(default_factory=BrowserConfig)
    session: SessionConfig = Field(default_factory=SessionConfig)
    logging: LoggingConfig = Field(default_factory=LoggingConfig)
    filtering: FilterConfig = Field(default_factory=FilterConfig)
    output: OutputConfig = Field(default_factory=OutputConfig)

    # Base directory for resolving relative paths
    base_dir: Path = Field(default=Path.cwd())


def load_config(env_file: Optional[str] = None) -> AppConfig:
    """Load configuration from .env file and environment variables."""
    if env_file:
        load_dotenv(env_file)
    else:
        load_dotenv()

    base_dir = Path.cwd()

    # Browser config
    browser = BrowserConfig(
        browser_type=os.getenv("BROWSER_TYPE", "chromium"),
        headless=os.getenv("HEADLESS", "false").lower() == "true"
    )

    # Session config
    session = SessionConfig(
        session_dir=base_dir / os.getenv("SESSION_DIR", "sessions/linkedin_session"),
        auto_save=os.getenv("SESSION_AUTO_SAVE", "true").lower() == "true",
        save_interval_sec=int(os.getenv("SESSION_SAVE_INTERVAL_SEC", "300"))
    )

    # Logging config
    logging = LoggingConfig(
        log_dir=base_dir / os.getenv("LOG_DIR", "logs"),
        log_raw_requests=os.getenv("LOG_RAW_REQUESTS", "true").lower() == "true",
        log_aggregated_summary=os.getenv("LOG_AGGREGATED_SUMMARY", "true").lower() == "true"
    )

    # Filtering config
    endpoints = os.getenv("FILTER_ENDPOINTS", "/voyager/api/,/voyager/").split(",")
    keywords = os.getenv("PRIORITY_KEYWORDS", "growth,invitation,normInvitations,connection").split(",")
    filtering = FilterConfig(
        api_only=os.getenv("FILTER_API_ONLY", "true").lower() == "true",
        endpoints=[e.strip() for e in endpoints],
        priority_keywords=[k.strip() for k in keywords]
    )

    # Output config
    output = OutputConfig(
        real_time_terminal=os.getenv("REAL_TIME_TERMINAL", "true").lower() == "true",
        verbose=os.getenv("TERMINAL_VERBOSE", "true").lower() == "true"
    )

    return AppConfig(
        browser=browser,
        session=session,
        logging=logging,
        filtering=filtering,
        output=output,
        base_dir=base_dir
    )
