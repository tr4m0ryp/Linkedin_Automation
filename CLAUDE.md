# Coding Rules

## CRITICAL: NO EMOJIS - EVER
**This is the most important rule and must be followed without exception.**

- ABSOLUTELY NO emojis in any file.
- This includes:
  - Source code (.rs files)
  - Documentation (.md files)
  - Comments and docstrings
  - Test files
  - Configuration files
  - Log messages
  - Error messages
  - Conversational responses
  - Commit messages
  - ANY text output whatsoever
- Emojis are unprofessional and violate project standards
- Use descriptive plain text instead
- Before outputting ANY text, verify it contains NO emojis

ANother rule:
Avoid the creation md files. This will burn a lot of tokens, and increase our spendings.
Only use if it is necessary.

## Primary Language
- **Rust** is the primary programming language for this project.
- Use shell scripts only for auxiliary tooling (build helpers, deployment scripts).
- Target Rust edition: **2021** or later.

## File Conventions - CRITICAL
- **NEVER create `.md` files** - They waste tokens and clutter the codebase
- **This `CLAUDE.md` and ` is the ONLY exception** - It contains project rules and AI instructions
- **NEVER create:**
  - README files (keep existing one minimal)
  - SETUP guides or summaries
  - MIGRATION guides
  - LANGUAGE_ANALYSIS documents
  - CONTRIBUTING guides
  - CHANGELOG files
  - Any other documentation files
- **All documentation belongs in CLAUDE.md** if absolutely necessary
- **Code should be self-documenting** via clear naming and doc comments
- **Token efficiency is critical** - Every unnecessary file wastes context window
- Use `rustfmt` for consistent code formatting (enforced via CI).
- Use `clippy` for linting (enforced via CI).

## File Size Limit (STRICT)
- **Maximum 300 lines per source file.** No exceptions.
- When a module approaches ~200 lines, proactively split it into submodules.
- Group related functions into logically named submodules.
- Use `mod.rs` to re-export public items, keeping external callers unchanged.
- Rust's module system naturally supports clean separation.

## Code Quality Standards
- **No emojis anywhere** — no emojis in code, comments, documentation, commit messages, or any output.
- **No `unwrap()` or `expect()` in production code** — use proper error handling with `Result<T, E>`.
- **No `panic!()` unless truly unrecoverable** — prefer returning errors.
- **All public APIs must have documentation comments** (`///`).
- **All errors must be properly typed** — use `thiserror` for error definitions.
- **Async code must be cancellation-safe** — avoid holding locks across await points.
- **Use tracing, not println!** — structured logging via `tracing` crate.

## Module Splitting Pattern

When a Rust source file exceeds ~200 lines:

1. Create a subdirectory with the module name.
2. Move code into `subdirectory/mod.rs` or split into multiple files.
3. Use `pub use` in `mod.rs` to re-export public items.
4. Keep the parent module's public API unchanged.

Example:
```rust
// Before: browser.rs (250 lines)
pub struct Browser { ... }
impl Browser { ... }
pub fn helper() { ... }

// After: browser/mod.rs
mod types;
mod session;
mod helpers;

pub use types::Browser;
pub use session::BrowserSession;
pub use helpers::*;
```

## Dependencies (Cargo.toml)

### Core Dependencies
```toml
[dependencies]
# Async runtime
tokio = { version = "1.41", features = ["full"] }
tokio-util = "0.7"

# Browser automation (choose one primary)
thirtyfour = "0.33"              # WebDriver protocol (recommended)
# chromiumoxide = "0.7"          # Chrome DevTools Protocol (alternative)
# fantoccini = "0.21"            # Another WebDriver option

# HTTP client
reqwest = { version = "0.12", features = ["json", "cookies", "rustls-tls"] }

# JSON serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Error handling
thiserror = "2.0"
anyhow = "1.0"

# Configuration
dotenv = "0.15"
envy = "0.4"                     # Deserialize env vars into structs

# Database (choose based on needs)
sqlx = { version = "0.8", features = ["runtime-tokio-rustls", "postgres", "chrono", "uuid"] }
# rusqlite = "0.32"              # SQLite alternative

# Logging and tracing
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }

# Date/time
chrono = { version = "0.4", features = ["serde"] }

# Rate limiting
governor = "0.7"                 # Token bucket rate limiter

# Cryptography (for session tokens, hashing)
sha2 = "0.10"
hex = "0.4"

# CLI
clap = { version = "4.5", features = ["derive", "env"] }

# Utilities
uuid = { version = "1.11", features = ["v4", "serde"] }
url = "2.5"
regex = "1.11"
rand = "0.8"

# Caching
moka = { version = "0.12", features = ["future"] }

# HTML parsing
scraper = "0.21"                 # CSS selector-based HTML parsing
select = "0.6"                   # Alternative parser

# Telegram bot (optional)
teloxide = { version = "0.14", features = ["macros", "rustls"] }

[dev-dependencies]
mockito = "1.6"                  # HTTP mocking
wiremock = "0.6"                 # HTTP mocking alternative
tokio-test = "0.4"
```

## Anti-Detection Strategy

LinkedIn employs sophisticated bot detection. Requirements:

### 1. Browser Fingerprinting Evasion
- Use real Chrome/Firefox via WebDriver (not headless when possible).
- Randomize user-agent, viewport size, canvas fingerprint.
- Disable automation markers (`navigator.webdriver`, etc.).

### 2. Human-Like Behavior
- Random delays between actions (500ms - 3s).
- Mouse movement simulation.
- Scroll behavior (gradual, not instant).
- Typing delays (50ms - 150ms per character).

### 3. Session Persistence
- Save and reuse cookies across runs.
- Persist local storage and session storage.
- Maintain consistent browser profiles.

### 4. Rate Limiting
- Max 50 profile views per hour.
- Max 20 connection requests per day.
- Max 10 messages per hour.
- Exponential backoff on errors.

### 5. Proxy Rotation (Optional)
- Rotate residential proxies for different sessions.
- Validate proxy health before use.
- Avoid datacenter IPs (easily flagged).

## Error Handling

All errors must be properly typed using `thiserror`:

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum LinkedInError {
    #[error("Authentication failed: {0}")]
    AuthFailed(String),

    #[error("Element not found: {selector}")]
    ElementNotFound { selector: String },

    #[error("Rate limit exceeded: retry after {retry_after}s")]
    RateLimitExceeded { retry_after: u64 },

    #[error("Session expired")]
    SessionExpired,

    #[error("Browser error: {0}")]
    BrowserError(#[from] thirtyfour::error::WebDriverError),

    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),

    #[error("Parse error: {0}")]
    ParseError(String),
}

pub type Result<T> = std::result::Result<T, LinkedInError>;
```

## Logging

Use structured logging with `tracing`:

```rust
use tracing::{info, warn, error, debug, instrument};

#[instrument(skip(browser), fields(profile_id = %profile_id))]
async fn scrape_profile(browser: &Browser, profile_id: &str) -> Result<Profile> {
    info!("Starting profile scrape");
    debug!("Navigating to profile page");

    // ... implementation ...

    info!(connections = profile.connections.len(), "Profile scraped successfully");
    Ok(profile)
}
```

## Testing Strategy

### Unit Tests
- Test pure functions in isolation.
- Mock external dependencies (HTTP, database).
- Use `tokio-test` for async tests.

### Integration Tests
- Test full workflows with real browser (use Selenium standalone).
- Use test LinkedIn accounts (avoid production).
- Mock LinkedIn responses when possible.

### Example Test Structure
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tokio;

    #[tokio::test]
    async fn test_login_success() {
        // Setup
        let config = test_config();
        let browser = BrowserSession::new(&config).await.unwrap();

        // Execute
        let result = login(&browser, "test@example.com", "password").await;

        // Assert
        assert!(result.is_ok());

        // Cleanup
        browser.close().await.unwrap();
    }
}
```

## Performance Considerations

- **Async all I/O** — never block the executor.
- **Connection pooling** — reuse HTTP connections.
- **Caching** — cache profile data for 24 hours.
- **Parallel scraping** — use `tokio::spawn` for concurrent profile scrapes (respect rate limits).
- **Lazy loading** — don't fetch data until needed.

## Security

- **Never commit `.env`** — use `.env.example` for templates.
- **Encrypt sensitive data** — use `aes-gcm` for stored credentials.
- **Validate all inputs** — prevent injection attacks.
- **Secure dependencies** — run `cargo audit` regularly.
- **Least privilege** — run with minimal required permissions.

## Git Workflow

- **Branch naming**: `feature/description`, `fix/description`
- **Commit messages**: Conventional Commits format
- **PR requirements**: Pass CI (clippy, rustfmt, tests)
- **Code review**: Required for all changes

## CI/CD Pipeline

GitHub Actions workflow:
1. `cargo fmt --check` — enforce formatting
2. `cargo clippy -- -D warnings` — enforce linting
3. `cargo test` — run tests
4. `cargo audit` — security audit
5. `cargo build --release` — verify production build

## License Compliance

- **Respect LinkedIn's Terms of Service** — this tool is for authorized use only.
- **No mass data collection** — comply with GDPR/CCPA.
- **Rate limit strictly** — avoid overloading LinkedIn's servers.
- **User consent** — only automate with explicit permission.

## Additional Notes

- Prefer `async/await` over blocking operations.
- Use `Arc<T>` and `Mutex<T>` for shared state across tasks.
- Avoid `Rc<T>` and `RefCell<T>` (not thread-safe).
- Use `tokio::time::sleep` instead of `std::thread::sleep`.
- All public functions should return `Result<T>` for proper error propagation.
