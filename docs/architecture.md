# System Architecture Documentation

## 1. High-Level Architecture

MarketWatch is a personal stock market visualization and top-movers tracking terminal built in **Rust 2024**. It provides both an embedded **Web Terminal Dashboard** (localhost:3000) and a **Native GUI** (`egui`/`eframe`), along with scheduled systemd automated scanning for Indian markets (NSE/BSE).

```mermaid
flowchart TD
    subgraph UI ["User Interfaces"]
        Web["Web Dashboard<br/>(TradingView Lightweight Charts v4)"]
        GUI["Native Desktop GUI<br/>(egui / eframe)"]
        CLI["CLI Commands & Scanner<br/>(clap v4)"]
    end

    subgraph Backend ["MarketWatch Core Backend (Rust)"]
        Server["Axum Web Server (localhost:3000)"]
        CachedProv["CachedProvider<br/>(Smart Market-Aware Cache Engine)"]
        ScannerMod["Scanner Engine<br/>(09:30 & 15:30 IST Catch-up)"]
    end

    subgraph Storage ["Local Storage & Cache"]
        SQLite[("Embedded SQLite<br/>~/.config/marketwatch/marketwatch.db<br/>(candles + cache_sync_meta)")]
        ScanJSON["Scanner State JSON<br/>~/.config/marketwatch/scanner_state.json"]
        StocksTOML["Stock Universe TOML<br/>~/.config/marketwatch/stocks.toml"]
    end

    subgraph External ["External Data Provider"]
        YahooAPI["Yahoo Finance Chart API<br/>(query2.finance.yahoo.com)"]
    end

    Web <-->|REST API JSON| Server
    Server --> CachedProv
    GUI --> CachedProv
    CLI --> ScannerMod

    CachedProv -->|Cache Hit < 1ms| SQLite
    CachedProv -->|Cache Miss / TTL Expired| YahooAPI
    YahooAPI -->|New Candles| CachedProv
    CachedProv -->|Upsert & Update Meta| SQLite

    ScannerMod -->|Persist Scan Results| ScanJSON
    Server -->|Read/Write Universe| StocksTOML
```

---

## 2. Component Breakdown

### A. Web Terminal Dashboard (`src/web/`)
* **Single-Page Application**: Embedded directly into the Rust binary at compile-time using `include_str!("index.html")`. Zero external file dependencies to deploy or run.
* **Charting Library**: TradingView Lightweight Charts (v4.2.1) loaded from CDN.
* **Modern Dual-Sidebar / Rail Layout (v2)**:
  * **⚡ Slim Navigation Rail (56px)**: Quick toggle between Single Chart, Multi-Chart Grid, Watchlist Drawer, Intel Drawer, Theme switcher, and Command Palette (<kbd>Ctrl+K</kbd>).
  * **🎛️ Minimalist Top Bar**: Live symbol & percentage pill, segmented timeframe switchers (`5m` to `1w`), zoom dial, cache status badge, and drawer triggers.
  * **🧠 Collapsible Intel & System Drawer**:
    * **AI Insights**: Algorithmic pivot levels (Classic P, R1, R2, S1, S2) and live candlestick pattern recognition (Engulfing, Hammer, Doji).
    * **Fundamentals**: Ticker statistics, market exchange, loaded bar count, and data freshness.
    * **System & Logs**: 1-click copyable systemctl commands, live journalctl logs monitor, and SQLite cache stats.
  * **🔍 Dynamic Zoom Dial**: Adjustable candle spacing dial (1 to 10 pixels), persisted via `localStorage`.
  * **Broker-Style Crosshair & Hover Tooltip**: Real-time candle percentage change badge displayed directly at the cursor and in the header OHLC banner.
  * **Right-Side Candle Padding**: Last candle offset away from Y-axis for easy future-projection reading.
  * **Top 150 Watchlist & Screener**: Live search, category badges, top movers filtering (gainers/losers by 1%, 2%, 3%, 5% threshold).
  * **Multi-Chart Grid Mode**: View mini-charts for all market movers simultaneously.

### B. Storage & Caching Layer (`src/storage/`, `src/domain/calendar.rs`, & `src/provider/sync.rs`)
* **`MarketDb`**: Embedded SQLite wrapper managing connection pooling with WAL mode, foreign key cascade deletes, and transaction-safe upserts. Tracks `cache_sync_meta` and internal session gap detection.
* **`MarketCalendar`**: High-precision Indian stock market (NSE/BSE) trading schedule and holiday master. Manages IST conversions (`+05:30`), weekend detection, official holiday validation (2024–2027), session slot generators (e.g. 25 daily 15m slots), and boundary alignment flooring.
* **`CandleSyncService`**: Unified coordinator for both Web API and Native Desktop GUI callers:
  * **In-Flight Concurrency Coalescing**: Uses `tokio::sync::watch` channels to ensure multiple simultaneous requests for the exact same `(symbol, timeframe)` pair trigger exactly one upstream Yahoo Finance fetch; all other callers wait and read the committed result.
  * **Gap-Aware Incremental Sync**: Determines missing ranges, querying Yahoo with targeted `period1`/`period2` bounds clamped to provider lookback limits (e.g., 59 days for 5m/15m).
  * **Intra-Response Slot Flooring**: Floores floating seconds (e.g. `11:36:42` -> `11:30:00`) and resolves same-slot candle updates in-place without generating duplicate database records.
  * **Graceful Degradation**: Serves cached historical candles (`X-Cache: STALE-FALLBACK`) when upstream networks fail, while returning explicit errors if the cache is empty.
* **`CachedProvider`**: Native desktop UI adapter delegating directly to `CandleSyncService`.

### C. Market Data Provider (`src/provider/yahoo.rs`)
* Handles raw HTTP communication with Yahoo Finance chart APIs (`query2` / `query1`).
* Supports both standard range queries and incremental `period1`/`period2` window requests with lookback clamping.
* Translates raw JSON into application domain [`Candle`](file:///home/kiran/work/Rust/MarketWatch/src/domain/candle.rs) and [`StockMover`](file:///home/kiran/work/Rust/MarketWatch/src/domain/mover.rs) structs.

### D. Automated Scheduler & Service (`src/scanner/`)
* Runs automated universe scans at **09:30 AM** (morning session) and **03:30 PM** (market close) IST.
* Includes persistent **missed-scan catch-up**: If the laptop was turned off during the scheduled time, running or launching MarketWatch automatically detects the missed slot and performs the scan.
* Generates native desktop notifications via `notify-send`.

---

## 3. Web API Endpoints Reference

All endpoints return JSON responses.

| Method | Endpoint | Query Parameters | Description |
| :--- | :--- | :--- | :--- |
| `GET` | `/` | None | Serves embedded single-page dashboard HTML |
| `GET` | `/favicon.svg` | None | Serves application SVG favicon |
| `GET` | `/api/stocks` | None | Lists all stocks in the universe configuration |
| `POST` | `/api/stocks` | JSON body: `StockEntry` | Adds a new stock to the universe |
| `DELETE`| `/api/stocks/{symbol}` | Path: `symbol` | Removes a stock from the universe |
| `GET` | `/api/candles` | `symbol`, `timeframe`, `force` (optional) | Retrieves OHLCV candle bars (checks SQLite cache first) |
| `GET` | `/api/cache/meta` | `symbol`, `timeframe` | Returns cache synchronization metadata for the pair |
| `GET` | `/api/scan` | `threshold`, `force` | Runs or returns a live mover scan across all stocks |
| `GET` | `/api/scan/cached` | None | Retrieves the most recent scan result from disk cache |
| `GET` | `/api/scan/state` | None | Returns cron schedule and catch-up execution state |

---

## 4. File Structure

```
MarketWatch/
├── Cargo.toml                  # Dependencies (rusqlite, axum, tokio, egui, clap)
├── docs/                       # Project documentation
│   ├── architecture.md         # System architecture & component guide
│   └── database_schema.md      # SQLite schema, indices, and freshness rules
├── src/
│   ├── main.rs                 # CLI entrypoint and mode dispatcher
│   ├── app.rs                  # Application orchestration for CLI & GUI
│   ├── cli.rs                  # Command-line argument parsing
│   ├── config.rs               # Stock universe TOML loader/saver
│   ├── error.rs                # Typed MarketError definitions
│   ├── domain/                 # Core domain entities
│   │   ├── candle.rs           # Normalized Candle struct (OHLCV)
│   │   ├── mover.rs            # StockMover representation
│   │   └── timeframe.rs        # Timeframe enum (5m, 15m, 30m, 1h, 1d, 1w)
│   ├── provider/               # Data provider implementations
│   │   ├── mod.rs              # MarketDataProvider trait
│   │   ├── cached.rs           # CachedProvider SQLite wrapper
│   │   └── yahoo.rs            # Yahoo Finance HTTP provider
│   ├── scanner/                # Screener and cron automation
│   │   ├── mod.rs              # Multi-threaded batch scanner
│   │   ├── notifier.rs         # Desktop notification dispatcher
│   │   ├── service.rs          # Systemd unit generator and manager
│   │   └── state.rs            # ScanState persistence and catch-up logic
│   ├── storage/                # Database persistence
│   │   └── mod.rs              # MarketDb SQLite engine with WAL mode
│   ├── ui/                     # Native GUI
│   │   ├── chart.rs            # egui candlestick chart rendering
│   │   └── mod.rs
│   └── web/                    # Embedded Web Dashboard
│       ├── mod.rs              # Axum HTTP routes and handlers
│       ├── index.html          # Frontend HTML/CSS/JavaScript
│       └── favicon.svg         # SVG Favicon
```
