# LinkedIn Automation

High-performance LinkedIn automation tool built in Rust.

## Setup

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install WebDriver (Chrome)
brew install chromedriver  # macOS
# or download from https://chromedriver.chromium.org/

# Configure
cp .env.example .env
# Edit .env with your credentials

# Build and run
cargo build --release
cargo run -- --verbose
```

## Development

```bash
cargo test                    # Run tests
cargo fmt                     # Format code
cargo clippy -- -D warnings   # Lint
cargo audit                   # Security audit
```

See [CLAUDE.md](./CLAUDE.md) for coding rules and project structure.

## License

MIT License - For authorized use only. Respect LinkedIn's Terms of Service.
