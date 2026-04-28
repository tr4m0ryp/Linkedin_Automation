# LinkedIn Automation: Mutual-Connection-Ranked Outreach with Humanized Pacing

A Rust CLI that sends LinkedIn connection invitations to a CSV-defined target list, ranking each target by shared 1st-degree connections (the "X mutual connections" social-proof signal) and pacing the outreach against LinkedIn's anti-abuse heuristics through a five-tactic humanization layer.

The problem this solves: bulk-blasting connection invitations through LinkedIn's web UI gets accounts soft-restricted within days. The throttle isn't just request volume -- LinkedIn fingerprints request *patterns*: timing distribution, navigation graph, action determinism. Conversely, requests sent to targets with shared mutuals accept at rates several times higher than cold targets, and acceptance rate is itself one of the strongest anti-spam signals LinkedIn weights.

This tool addresses both axes. It hits LinkedIn's internal Voyager API directly (no Selenium, no headless Chrome at run time) using cookies extracted once from a manually authenticated browser session, ranks targets by mutual-connection bucket using the `memberDistance` field that profile resolution already returns, and routes every read through humanized decoy traffic so request patterns resemble organic browsing rather than scripted automation.

## How It Works

The runner alternates between two phases until the candidate pool stabilizes, then falls through to lower-priority targets:

```
+------------------------------+
|  Phase 1: Discovery          |  Resolve each unsent profile, label it
|    decoy browse              |  by memberDistance: "2" (mutuals) or
|    profile resolve           |  "3" (none). Persist degree and
|    write degree to CSV       |  timestamp back to the CSV.
+--------------+---------------+
               |
               v
+------------------------------+
|  Phase 2: Send 2nd-degrees   |  Pull all rows where degree="2" and
|    decoy browse              |  Is_Sent=0. Send invitations with
|    maybe skip (D5.C)         |  humanized pacing. Mark Is_Sent on
|    send invitation           |  success.
|    lognormal delay           |
+--------------+---------------+
               |
       no new 2nds AND
       no sends this pass?
               |
        no     |     yes
       +-------+-------+
       |               |
   loop back           v
                +------------------------------+
                |  Phase 3: 3rd-degree         |  Send remaining 3+
                |  fall-through                |  rows with the same
                +------------------------------+  pacing. Then stop.
```

After each send pass, accepted invitations expand the user's network. Some profiles previously labeled `3+` are now `2nd-degree` because they're connected to someone who just accepted. The next discovery pass picks them up. The loop converges naturally once no more 2nd-degrees can be discovered.

### The Humanization Layer

LinkedIn's anti-abuse system tracks request *patterns*, not just volume. Five tactics target distinct fingerprinting axes:

| Tactic | What it does | Why it matters |
|--------|-------------|----------------|
| **A. Lognormal delays** | Send-to-send gaps drawn from `LogNormal(mu=ln(720), sigma=0.6)`. Median ~12 min, with rare 30-60 min and 2 hr+ tails. | Uniform-distributed delays are themselves a tell. Real human gaps are skewed. |
| **B. Decoy browsing** | Before each real action, fire 1-3 of: feed updates, notifications, `/me` ping, with random "reading time" pauses. Plus a periodic `/me` every ~5 sends. | Breaks the deterministic "fetch -> invite" pattern. Refreshes rotating cookies (`__cf_bm`, `lidc`) as a side effect. |
| **C. Random skip-the-send** | ~7% of the time, browse a profile and don't send. Configurable. | Real users browse without always connecting. |
| **D. Daily window + cap** | Active only 09:00-19:00 system-local; max 18 sends/day (configurable). Counters persist across restarts. | LinkedIn's known soft cap is ~100/week; 18/day x 5 weekdays = 90/week. No 4 a.m. sends. |
| **E. Mid-session breaks** | After every 3-7 sends (random), inject a 20-60 min "lunch break" sleep. | Real sessions aren't continuous. |

### Why memberDistance, Not Exact Mutual Count

The ideal ranking signal is the exact count of shared connections. But LinkedIn migrated this data to a graphql endpoint (`voyagerSearchDashClusters`) whose `queryId` hash rotates with every web deploy (~weekly). Hardcoding a hash means the tool breaks every few weeks; auto-discovering the hash from JS bundles works but adds a fragile scrape step.

Instead, this tool uses `memberDistance`, which is **already returned for free** in the existing profile-resolve call (`identity/dash/profiles?decorationId=WebTopCardCore-16`):

- `DISTANCE_1` / `DISTANCE_2` -> 2nd-degree -> shares >=1 mutual -> **high priority**
- `DISTANCE_3` / `OUT_OF_NETWORK` -> 3rd-degree+ -> no mutuals -> **low priority**

A binary bucket loses fine ranking *within* a bucket but captures the dominant social-proof signal at zero API cost and survives every LinkedIn deploy.

## Platform Support

| Platform | Chrome (login only) | Rust | Status |
|----------|---------------------|------|--------|
| macOS    | required for one-time login | 1.78+ | Tested |
| Linux    | required for one-time login | 1.78+ | Should work; not actively tested |
| Windows  | required for one-time login | 1.78+ | Untested |

Once cookies are saved, neither Chrome nor a webdriver runs at automation time -- the tool makes plain HTTPS calls to `linkedin.com/voyager/api/...`.

## Quick Start

<details>
<summary><b>macOS</b></summary>

```bash
# 1. Rust toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 2. Chrome (skip if already installed)
brew install --cask google-chrome

# 3. Project
git clone https://github.com/tr4m0ryp/Linkedin_Automation.git
cd Linkedin_Automation
cp .env.example .env
# Defaults are sensible; edit only if you want to deviate.

# 4. Build
cargo build --release
```

</details>

<details>
<summary><b>Linux (Debian/Ubuntu)</b></summary>

```bash
# 1. Rust toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 2. Chrome
wget -q -O - https://dl.google.com/linux/linux_signing_key.pub | sudo apt-key add -
sudo sh -c 'echo "deb [arch=amd64] http://dl.google.com/linux/chrome/deb/ stable main" \
  >> /etc/apt/sources.list.d/google-chrome.list'
sudo apt update && sudo apt install -y google-chrome-stable

# 3. Project
git clone https://github.com/tr4m0ryp/Linkedin_Automation.git
cd Linkedin_Automation
cp .env.example .env
cargo build --release
```

</details>

## Usage

### 1. Prepare a target CSV

Two columns required, two more added on first run:

```csv
linkedin_url,Is_Sent
https://www.linkedin.com/in/example-1/,0
https://www.linkedin.com/in/example-2/,0
```

After the first discovery pass:

```csv
linkedin_url,Is_Sent,degree,degree_checked_at
https://www.linkedin.com/in/example-1/,1,2,2026-04-28T18:23:11+00:00
https://www.linkedin.com/in/example-2/,0,3,2026-04-28T18:24:42+00:00
```

The reader is backwards-compatible -- legacy two-column files are accepted; missing columns are treated as unfetched.

### 2. One-time browser login

```bash
cargo run --release -- --login
```

A Chrome window opens at `linkedin.com/login`. Log in by hand. The tool detects login completion, extracts cookies via the Chrome DevTools Protocol, and writes them to `sessions/linkedin_cookies.json`. Subsequent runs reuse those cookies until they expire.

### 3. Dry run -- validate without sending

```bash
cargo run --release -- --dry-run --csv-path linkedin_profiles.csv
```

Walks the discovery pass and reports what *would* be sent without firing any invitations. Useful for verifying the CSV parses, the session is valid, and the rate-limit budget is comfortable.

### 4. Real run

```bash
cargo run --release
```

The runner enters its phased loop. Expect:
- **Discovery** on a few hundred unsent rows: ~30-60 minutes with humanized pacing.
- **Send phase**: 18 sends/day means a 500-row CSV takes ~28 working days at full pace. This is intentional; the cap is what keeps the account healthy.
- All actions logged via `tracing`. Set `RUST_LOG=linkedin_automation=debug` for fine-grained tracing.

## Technical Details

### Module Layout

```
src/
  main.rs                      CLI entry, clap parsing, runner kickoff
  lib.rs                       Crate root, module wiring
  config.rs                    AppConfig + HumanizerConfig (envy parse)
  error.rs                     LinkedInError (thiserror) + Result alias
  automation/
    runner.rs                  Phased orchestrator (D4)
    discovery.rs               Single discovery pass
    connection_sender.rs       Per-profile send logic
    csv_reader/                CSV read/write with degree columns
    humanizer/
      mod.rs                   Humanizer facade
      delay.rs                 LogNormalDelay (D5.A)
      window.rs                ActivityWindow + SessionStats (D5.D)
      breaks.rs                BreakScheduler (D5.E)
      decoy.rs                 DecoyBrowser (D5.B)
    types.rs                   Degree enum, ProfileRow
  linkedin_api/
    client/
      mod.rs                   LinkedInClient (Voyager HTTP)
      decoys.rs                Decoy GET helpers (D5.B)
    cdp.rs                     Chrome DevTools Protocol client
    login.rs                   One-time browser login flow
    session.rs                 Cookie jar load/save, CSRF extract
    types.rs                   ProfileData, ConnectionState, etc.
```

Module split policy: 300-line cap per file, enforced. When a file approaches 200 lines it is proactively split into a directory module with `mod.rs` re-exporting the public API.

### Configuration Keys

All knobs live in `.env`. Defaults are conservative.

| Key | Default | What it controls |
|-----|---------|------------------|
| `CSV_PATH` | `linkedin_profiles.csv` | Target list path |
| `DAILY_WINDOW_START` / `_END` | `09:00` / `19:00` | Activity window in system-local time |
| `DAILY_SEND_CAP` | `18` | Max real invitations per day |
| `DEGREE_RECHECK_DAYS` | `30` | Refresh degree label if older than this |
| `SKIP_SEND_PROBABILITY` | `0.07` | Random skip-the-send rate (D5.C) |
| `BREAK_EVERY_MIN_SENDS` / `_MAX_` | `3` / `7` | Trigger lunch break after this many sends |
| `BREAK_DURATION_MIN_SECS` / `_MAX_` | `1200` / `3600` | Break length (20-60 min) |
| `DELAY_LOGNORMAL_MEDIAN_SECS` | `720` | Median send-to-send gap (12 min) |
| `DELAY_LOGNORMAL_SIGMA` | `0.6` | Distribution spread |
| `ME_PING_EVERY_N_SENDS` | `5` | `/voyager/api/me` decoy frequency |
| `COOKIE_FILE` | `sessions/linkedin_cookies.json` | Persisted cookie path |
| `USER_AGENT` | Chrome 120 desktop | UA pinned per session |
| `RUST_LOG` | `info` | Standard `tracing` level filter |

### Voyager Endpoints Used

| Endpoint | Purpose |
|----------|---------|
| `GET /voyager/api/me` | Auth ping; periodic decoy `/me` |
| `GET /voyager/api/identity/dash/profiles?decorationId=WebTopCardCore-16` | Resolve profile, get `memberDistance`, `entityUrn`, connection state |
| `POST /voyager/api/voyagerRelationshipsDashMemberRelationships?action=verifyQuotaAndCreateV2` | Send invitation |
| `GET /voyager/api/feed/updatesV2` | Decoy browse |
| `GET /voyager/api/me/notifications` (with fallback) | Decoy browse |

### Anti-Detection: What's Fingerprintable

The tool optimizes against patterns LinkedIn's anti-abuse pipeline is documented (in patent filings and post-mortems on third-party sites) to track:

- **Request rate distribution** -- uniform timing is a giveaway. The lognormal distribution mimics organic gap distributions.
- **Action graph** -- `fetch profile -> click connect` 1000 times in a row is unique to bots. Decoy browsing breaks this.
- **Time-of-day** -- a session at 4 a.m. local time is suspicious. The activity window prevents this.
- **Continuous activity** -- humans take breaks. The break scheduler enforces them.
- **Acceptance ratio** -- declining/ignored invitations are a strong signal. Mutual-ranked targets accept at higher rates, indirectly improving this ratio.

### Development

```bash
cargo test                    # 19 unit tests across humanizer + csv + types
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo audit                   # security advisories
```

See `CLAUDE.md` for project coding rules (file size limits, error-handling conventions, no-emojis policy).

## Roadmap

- **Exact mutual-connection counts** via graphql `voyagerSearchDashClusters` with auto-discovered `queryId`. Currently deferred -- the queryId hash rotates per LinkedIn deploy and would require an ongoing maintenance burden. The binary `memberDistance` bucket is good enough in practice.
- **Account-warmth-aware daily caps** -- younger LinkedIn accounts have lower thresholds. Currently a static 18/day for all accounts.
- **Per-day pacing variation** -- mimic weekday/weekend asymmetry. Currently the same window applies every day.
- **Configurable timezone** -- currently uses system-local; should be overridable.
- **Optional message customization** -- currently sends a blank `customMessage` field; future versions may template per-target messages from CSV columns.

## Disclaimer

This tool is intended for **authorized use on accounts you own**, in compliance with:

- LinkedIn's User Agreement and Professional Community Policies
- GDPR / CCPA where the targets reside
- Any platform-specific or jurisdictional consent requirements

Automating contact with people who have not consented to be contacted may violate the laws of your jurisdiction and is not a use case the maintainers endorse. Users are solely responsible for the lawfulness and ethics of their target lists.

LinkedIn may flag, restrict, or terminate accounts that use third-party automation regardless of how carefully traffic is humanized. Use at your own risk.

## License

MIT. See `LICENSE` (add one if missing -- without it, the code is legally unusable by anyone but the author).
