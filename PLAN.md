# MarketWatch — Implementation Plan

## 1. Project Architecture

The application follows a layered architecture with clear module boundaries:

```
CLI Layer          → Parses arguments, interactive prompts
Application Layer  → Orchestrates the workflow: load config → fetch → render
Domain Layer       → Core types: Candle, Timeframe, StockEntry
Provider Layer     → Market data abstraction + Yahoo implementation
UI Layer           → egui/eframe window with candlestick chart
Config Layer       → TOML-based stock universe and app settings
```

Data flows in one direction:

```
CLI args
  → App loads config (stock universe)
  → App calls MarketDataProvider::fetch_candles()
  → Provider returns Vec<Candle>  (normalized)
  → App passes data to UI
  → UI renders candlestick chart in an egui window
```

There is no database. No persistent market data storage. Data lives in memory
from fetch to render.

---

## 2. Directory / File Structure

```
MarketWatch/
├── Cargo.toml
├── PLAN.md
├── config/
│   └── stocks.toml            # Stock universe (~300 entries)
├── src/
│   ├── main.rs                # Entry point: parse CLI, run app
│   ├── app.rs                 # Application orchestration
│   ├── cli.rs                 # CLI argument parsing (clap)
│   ├── config.rs              # Config loading (stocks.toml)
│   ├── domain/
│   │   ├── mod.rs
│   │   ├── candle.rs          # Candle struct
│   │   └── timeframe.rs       # Timeframe enum + parsing
│   ├── provider/
│   │   ├── mod.rs             # MarketDataProvider trait
│   │   └── yahoo.rs           # Yahoo Finance implementation
│   ├── ui/
│   │   ├── mod.rs
│   │   └── chart.rs           # Candlestick chart rendering
│   └── error.rs               # Application error types
├── output/                    # Future: exported charts
└── cache/                     # Future: data cache
```

**Rationale:** Each concern gets its own module. `domain/` holds types that are
provider-agnostic and UI-agnostic. `provider/` holds the trait and its
implementations behind it. `ui/` owns all rendering. `app.rs` ties them together.

---

## 3. Dependencies

| Crate | Purpose | Why selected | Alternatives considered |
|---|---|---|---|
| `clap` (derive) | CLI argument parsing | Most mature Rust CLI framework. Derive macros reduce boilerplate. | `argh` — lighter but less ecosystem support |
| `dialoguer` | Interactive terminal prompts (timeframe selection) | Simple API for select/input prompts | `inquire` — similar, slightly less mature |
| `tokio` | Async runtime | Industry standard for async Rust. Required by `reqwest`. | `async-std` — less ecosystem adoption |
| `reqwest` | HTTP client | Mature, ergonomic async HTTP with TLS. | `ureq` — sync-only, simpler but limiting |
| `serde` + `serde_json` | JSON deserialization | De facto standard for Rust serialization | None — no real alternative |
| `toml` | TOML config parsing | Needed for `stocks.toml`. Works with serde. | `config` crate — heavier than needed |
| `eframe` | GUI framework (wraps egui) | Provides window management + egui integration | `iced` — more complex for this use case |
| `egui_plot` | Plotting within egui | Official egui plotting. Supports BoxPlot which can render candlesticks. | `egui-charts` — full-featured but heavy dependency for MVP |
| `chrono` | Date/time handling | Mature, well-understood datetime library | `time` — viable but chrono has broader ecosystem use |
| `thiserror` | Error type derivation | Clean, ergonomic custom error types | Manual `impl` — more boilerplate |
| `anyhow` | Error propagation in main/app | Convenient `?` chaining with context | Just `Box<dyn Error>` — less ergonomic |

**Total: 11 crates** (plus their transitive dependencies). Each one earns its place.

### Crates explicitly NOT added:
- **SQLite/diesel/sqlx** — No database requirement
- **egui-charts** — Powerful but adds significant dependency weight. `egui_plot` + `BoxElem` is sufficient for MVP candlesticks
- **yahoo_finance_api** — We'll implement the Yahoo v8 chart API call directly with `reqwest`. The endpoint is simple (one HTTP GET + JSON parse), and owning the code gives us control over error handling, retries, and .NS symbol support without depending on a third-party crate's maintenance cycle

---

## 4. CLI Design

Using `clap` derive:

```rust
#[derive(Parser)]
#[command(name = "market", about = "Personal stock market terminal")]
struct Cli {
    /// Timeframe: 5m, 15m, 30m, 1h, 1d, 1w
    #[arg(short = 't', long = "timeframe")]
    timeframe: Option<String>,

    /// Stock symbol(s), comma-separated (future)
    #[arg(short = 's', long = "symbol")]
    symbol: Option<String>,
}
```

If `timeframe` is `None`, present interactive selection via `dialoguer::Select`.

The binary name `market` is set via `[[bin]]` in `Cargo.toml`:
```toml
[[bin]]
name = "market"
path = "src/main.rs"
```

---

## 5. Timeframe Representation

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Timeframe {
    Min5,
    Min15,
    Min30,
    Hour1,
    Day1,
    Week1,
}
```

Each variant knows:
- Its display label (`"5m"`, `"15m"`, etc.)
- Its Yahoo API interval string (`"5m"`, `"15m"`, `"30m"`, `"60m"`, `"1d"`, `"1wk"`)
- Its Yahoo API range string (`"5d"`, `"5d"`, `"1mo"`, `"6mo"`, `"2y"`, `"5y"`)
- Implemented via methods on the enum, not via string scattering

Parsing from CLI string:
```rust
impl FromStr for Timeframe { ... }
```

Adding a new timeframe = add a variant + update the match arms. No string
constants scattered elsewhere.

### Yahoo range mapping (based on API limits):

| Timeframe | Yahoo `interval` | Yahoo `range` | Approx candles |
|-----------|-----------------|---------------|----------------|
| 5m        | `5m`            | `5d`          | ~390           |
| 15m       | `15m`           | `5d`          | ~130           |
| 30m       | `30m`           | `1mo`         | ~260           |
| 1h        | `60m`           | `6mo`         | ~780           |
| 1d        | `1d`            | `2y`          | ~500           |
| 1w        | `1wk`           | `5y`          | ~260           |

These ranges are the maximum useful data Yahoo provides for each interval.
Intraday data (≤60m) is limited to recent days/weeks; daily/weekly goes back years.

---

## 6. Stock / Watchlist Representation

`config/stocks.toml`:
```toml
[[stocks]]
symbol = "RELIANCE.NS"
name = "Reliance Industries"

[[stocks]]
symbol = "TCS.NS"
name = "Tata Consultancy Services"
```

Rust types:
```rust
#[derive(Debug, Deserialize)]
pub struct StockConfig {
    pub stocks: Vec<StockEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StockEntry {
    pub symbol: String,
    pub name: String,
}
```

For the first milestone, the app loads the full universe and fetches data for
the **first stock** (or a hardcoded default). Multi-stock selection and `-s` flag
come in a later phase.

---

## 7. Market Data Provider Architecture

```rust
#[async_trait]
pub trait MarketDataProvider: Send + Sync {
    async fn fetch_candles(
        &self,
        symbol: &str,
        timeframe: Timeframe,
    ) -> Result<Vec<Candle>, MarketError>;
}
```

The trait lives in `src/provider/mod.rs`. The Yahoo implementation lives in
`src/provider/yahoo.rs`.

The application layer only sees `&dyn MarketDataProvider`. It never imports
Yahoo-specific types.

Adding a new provider = implement the trait in a new file. No changes to
`app.rs`, `ui/`, or `domain/`.

**Why `async_trait`?** Rust's native async trait support is now stable. We'll
use native async traits (no `async-trait` crate needed) since we're targeting
recent Rust editions.

---

## 8. Yahoo Finance Integration

### Endpoint
```
GET https://query1.finance.yahoo.com/v8/finance/chart/{symbol}
    ?interval={interval}
    &range={range}
```

### Request details
- Set `User-Agent` header to a standard browser string
- Use `reqwest::Client` with a 15-second timeout
- Symbol is URL-encoded (handles `.NS` suffix for NSE stocks)

### Response parsing
Yahoo returns JSON with this relevant structure:
```json
{
  "chart": {
    "result": [{
      "timestamp": [1234567890, ...],
      "indicators": {
        "quote": [{
          "open": [100.0, ...],
          "high": [101.0, ...],
          "low": [99.0, ...],
          "close": [100.5, ...],
          "volume": [1000000, ...]
        }]
      }
    }]
  }
}
```

We define Yahoo-specific serde structs inside `provider/yahoo.rs` (private).
They are immediately converted to `Vec<Candle>` before leaving the module.

### Error handling
- HTTP non-200 → `MarketError::ProviderError` with status code
- Empty `result` array → `MarketError::NoData`
- Missing/null OHLC values → skip that candle (Yahoo sometimes has null entries in arrays)
- Network timeout → `MarketError::NetworkError`
- JSON parse failure → `MarketError::ParseError`

### Rate limiting
- For MVP (single stock fetch), rate limiting is not a concern
- Architecture allows adding a rate limiter (e.g., `governor` crate) later
- 200ms delay between requests when fetching multiple stocks (future)

---

## 9. Data Normalization

```rust
#[derive(Debug, Clone)]
pub struct Candle {
    pub timestamp: i64,        // Unix timestamp (seconds)
    pub datetime: DateTime<Utc>, // Parsed datetime for display
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: u64,
}
```

Normalization happens inside the Yahoo provider:
1. Zip the `timestamp`, `open`, `high`, `low`, `close`, `volume` arrays
2. Skip entries where any OHLCV value is `null`
3. Convert timestamps to `DateTime<Utc>`
4. Return `Vec<Candle>` sorted by timestamp ascending

The `Candle` struct is provider-agnostic. Any future provider produces the same type.

---

## 10. Charting / Visualization Approach

### Library: `eframe` + `egui_plot`

Using `egui_plot::BoxPlot` with `BoxElem` to render candlesticks:

- Each `Candle` becomes a `BoxElem`:
  - x = candle index (sequential integer, with timestamp labels on axis)
  - `BoxSpread::new(low, min(open,close), min(open,close), max(open,close), high)`
  - `whisker_width(0.0)` — makes it look like a candlestick wick, not box-plot whiskers
  - Green fill if `close >= open`, red if `close < open`

### Chart features for MVP:
- ✅ Candlestick rendering (via BoxElem)
- ✅ Price axis (Y)
- ✅ Time axis (X) with date labels
- ✅ Zoom (built into egui_plot)
- ✅ Pan (built into egui_plot)
- ✅ Window title showing symbol + timeframe

### Deferred to later phases:
- ❌ Crosshair / cursor price inspection
- ❌ Stock switching within the UI
- ❌ Timeframe switching within the UI
- ❌ Technical indicators
- ❌ Drawing tools

### Window setup
```rust
eframe::run_native(
    "MarketWatch — {symbol} ({timeframe})",
    options,
    Box::new(|_cc| Ok(Box::new(ChartApp::new(candles, symbol, timeframe)))),
)
```

The `ChartApp` struct implements `eframe::App`. It holds the candle data and
renders it each frame.

---

## 11. Error Handling

Custom error enum using `thiserror`:

```rust
#[derive(Debug, thiserror::Error)]
pub enum MarketError {
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("Failed to parse response: {0}")]
    Parse(String),

    #[error("No data returned for {symbol}")]
    NoData { symbol: String },

    #[error("Provider error: HTTP {status} for {symbol}")]
    Provider { status: u16, symbol: String },

    #[error("Invalid symbol: {0}")]
    InvalidSymbol(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Invalid timeframe: {0}")]
    InvalidTimeframe(String),
}
```

In `main.rs`, use `anyhow::Result` for the top-level orchestration so we get
nice error context. The provider uses `MarketError` for typed errors.

User-facing errors print to stderr with context:
```
Error: No data returned for INVALID.NS
```

Not:
```
thread 'main' panicked at 'called `Result::unwrap()` on an `Err` value: ...'
```

---

## 12. Network Failures and Retries

For the MVP, a single attempt with a clear error message is sufficient.

The `reqwest::Client` is configured with:
- 15-second connect + response timeout
- Standard `User-Agent` header

If the request fails, the error propagates to `main()` and prints a useful message.

**Not implemented for MVP (but architecturally possible):**
- Exponential backoff retries
- Circuit breaker pattern
- Request queuing

---

## 13. Rate Limiting Considerations

Yahoo's undocumented API starts throttling around 2000-2500 requests/hour.

For the MVP:
- We fetch exactly **one stock per run**
- Rate limiting is a non-issue

For the future multi-stock phase:
- Add a 200-500ms delay between requests
- Use `tokio::time::sleep` between fetches
- Consider `governor` or `leaky-bucket` crate if more sophisticated control is needed
- Batch sequential fetches rather than parallel-blasting 300 requests

The `MarketDataProvider` trait does not encode rate limiting. Rate limiting
belongs in the provider implementation or an orchestration layer above it.

---

## 14. Configuration Management

### Config file location
`config/stocks.toml` relative to the binary's working directory (project root).

### Loading
```rust
pub fn load_stock_config(path: &Path) -> Result<StockConfig, MarketError> {
    let content = fs::read_to_string(path)
        .map_err(|e| MarketError::Config(format!("Cannot read {}: {}", path.display(), e)))?;
    let config: StockConfig = toml::from_str(&content)
        .map_err(|e| MarketError::Config(format!("Invalid TOML in {}: {}", path.display(), e)))?;
    Ok(config)
}
```

### Future extensibility
A `config/settings.toml` can be added later for:
- Default timeframe
- Default provider
- UI preferences
- Cache settings

Not needed for MVP.

---

## 15. Disk / File Handling

```
config/          → Stock universe (checked into repo, user-editable)
output/          → Future: chart exports (PNG, etc.)
cache/           → Future: cached market data
```

The `output/` and `cache/` directories are `.gitignore`d and created on demand,
not at startup.

MVP only reads from `config/stocks.toml`. No other disk I/O.

---

## 16. Testing Strategy

### Unit tests (in each module):

| Module | What to test |
|--------|-------------|
| `domain/timeframe.rs` | Parse `"5m"` → `Timeframe::Min5`, reject `"3m"`, round-trip display |
| `config.rs` | Parse valid TOML, reject malformed TOML, handle empty stock list |
| `provider/yahoo.rs` | Parse a sample Yahoo JSON response into `Vec<Candle>`, handle null values, handle error responses |
| `cli.rs` | Clap derives correctly, timeframe flag works |
| `error.rs` | Error display messages are human-readable |

### Integration tests:
- Not for MVP. Real Yahoo calls are flaky in CI.
- A future integration test could use `wiremock` or recorded responses.

### What we do NOT test:
- egui rendering (no good way to unit test GPU rendering)
- Actual HTTP calls to Yahoo (unreliable, unofficial API)

### Test data:
Embed sample Yahoo JSON responses as string constants in test modules. This
avoids test files on disk and makes tests self-contained.

---

## 17. Extensibility

The architecture supports these future additions without restructuring:

| Future feature | Where it plugs in |
|---|---|
| New provider (Polygon, Alpha Vantage) | New file in `provider/`, implement `MarketDataProvider` |
| Technical indicators (SMA, RSI) | New module `indicators/`, operates on `Vec<Candle>` |
| Multiple stock rendering | `app.rs` fetches multiple, `ui/` renders tabs or grid |
| Stock search / filter | `config.rs` gets a search method |
| Caching layer | Wraps `MarketDataProvider` (decorator pattern) |
| Export chart to PNG | `ui/` module adds export functionality |
| Alerts / screeners | New module, uses provider to check conditions |
| Portfolio tracking | New `portfolio/` module with its own config |

---

## 18. Implementation Phases

### Phase 1: Skeleton (this milestone)
1. `cargo init` with project structure
2. `Cargo.toml` with all dependencies
3. `domain/` — `Timeframe` and `Candle` types with tests
4. `config.rs` — TOML loading with tests
5. `cli.rs` — clap parsing + dialoguer fallback
6. `provider/` — `MarketDataProvider` trait + Yahoo implementation with tests
7. `ui/` — egui candlestick chart
8. `app.rs` — orchestration (wire everything together)
9. `main.rs` — entry point
10. `config/stocks.toml` — initial stock list (10-20 NSE stocks for testing)
11. Verify: `cargo check`, `cargo test`, `cargo clippy`, `cargo fmt --check`
12. Manual test: `cargo run -- -t 15m`

### Phase 2: Polish (after milestone 1)
- `-s SYMBOL` flag for specific stock selection
- Better chart axis labels and formatting
- Crosshair / hover tooltip
- Error recovery suggestions in terminal output

### Phase 3: Multi-stock (future)
- Fetch multiple stocks
- Rate-limited sequential/concurrent fetching
- Stock selector in the UI
- Tabbed or list view

### Phase 4: Advanced (future)
- Technical indicators overlay
- Caching layer
- Additional providers
- Chart export

---

## 19. Technical Risks and Limitations

| Risk | Severity | Mitigation |
|---|---|---|
| Yahoo API is unofficial and may break | **High** | Provider abstraction allows swapping. We own the HTTP code so we can adapt quickly. |
| Yahoo blocks requests (rate limiting) | Medium | Single-stock MVP is safe. Future: add delays, User-Agent rotation. |
| Intraday data limited to ~7-60 days | Medium | Document in UI. Use maximum useful range per timeframe. |
| `egui_plot` BoxElem candlestick rendering quality | Medium | Proven approach (community-documented). Can migrate to `egui-charts` later if needed. |
| Indian market (.NS) symbols may have quirks | Low | Test with actual NSE symbols. Yahoo supports `.NS` suffix. |
| egui/eframe platform compatibility | Low | eframe supports Linux, macOS, Windows. Well-tested. |

---

## 20. First Milestone — Definition of Done

The first milestone is complete when:

```bash
cargo run -- -t 15m
```

1. ✅ Parses `15m` into `Timeframe::Min15`
2. ✅ Loads `config/stocks.toml`
3. ✅ Fetches OHLC data from Yahoo Finance for the first stock
4. ✅ Converts Yahoo JSON into `Vec<Candle>`
5. ✅ Opens an egui window with a candlestick chart
6. ✅ Handles network/parse/data errors with useful messages
7. ✅ Clean module structure (no monolithic main.rs)
8. ✅ Tests for timeframe parsing, config loading, Yahoo JSON parsing
9. ✅ `cargo check` passes
10. ✅ `cargo test` passes
11. ✅ `cargo clippy` passes
12. ✅ `cargo fmt --check` passes

And when no timeframe is given:
```bash
cargo run
```
13. ✅ Interactive timeframe selection appears in terminal
