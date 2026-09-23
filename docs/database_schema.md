# Database Schema Documentation

## Overview

MarketWatch uses an embedded **SQLite** database (`marketwatch.db`) for caching historical candlestick (OHLCV) market data and tracking cache synchronization metadata.

- **Storage Location**:
  - Primary: `~/.config/marketwatch/marketwatch.db`
  - Local Fallback: `./cache/marketwatch.db`
- **Engine**: SQLite 3 (compiled statically into binary via `rusqlite` bundled)
- **Concurrency Mode**: **WAL (Write-Ahead Logging)** enabled (`PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;`) for microsecond read latency (< 0.2ms) and non-blocking concurrent reads/writes.

---

## Tables

### 1. `tickers` (Primary Ticker Registry)

Stores canonical metadata for all tracked stocks in the universe. Acts as the parent table for time-series candles and sync metadata.

```sql
CREATE TABLE IF NOT EXISTS tickers (
    symbol      TEXT PRIMARY KEY NOT NULL,
    name        TEXT NOT NULL,
    exchange    TEXT NOT NULL DEFAULT 'NSE',
    is_active   INTEGER NOT NULL DEFAULT 1,
    created_at  INTEGER NOT NULL
);
```

#### Columns

| Column | Type | Nullable | Description |
| :--- | :--- | :--- | :--- |
| `symbol` | TEXT | NO | NSE stock ticker symbol (Primary Key, e.g. `'RELIANCE.NS'`, `'TCS.NS'`) |
| `name` | TEXT | NO | Full company name (e.g. `'Reliance Industries Ltd'`) |
| `exchange` | TEXT | NO | Stock exchange identifier (defaults to `'NSE'`) |
| `is_active` | INTEGER | NO | Flag indicating if ticker is actively tracked (`1` = active, `0` = inactive) |
| `created_at` | INTEGER | NO | Unix epoch timestamp of when ticker was registered |

---

### 2. `candles` (Time-Series Store)

Stores individual OHLCV candlestick bars across all tracked stocks and timeframes, linked via foreign key to `tickers`.

```sql
CREATE TABLE IF NOT EXISTS candles (
    symbol          TEXT    NOT NULL,
    timeframe       TEXT    NOT NULL,
    timestamp       INTEGER NOT NULL,
    open            REAL    NOT NULL,
    high            REAL    NOT NULL,
    low             REAL    NOT NULL,
    close           REAL    NOT NULL,
    volume          INTEGER NOT NULL,
    PRIMARY KEY (symbol, timeframe, timestamp),
    FOREIGN KEY (symbol) REFERENCES tickers(symbol) ON DELETE CASCADE
) WITHOUT ROWID;

CREATE INDEX IF NOT EXISTS idx_candles_lookup 
ON candles (symbol, timeframe, timestamp ASC);
```

#### Columns

| Column | Type | Nullable | Description |
| :--- | :--- | :--- | :--- |
| `symbol` | TEXT | NO | NSE stock symbol referencing `tickers(symbol)` (Foreign Key, `ON DELETE CASCADE`) |
| `timeframe` | TEXT | NO | Timeframe interval code (`'5m'`, `'15m'`, `'30m'`, `'1h'`, `'1d'`, `'1w'`) |
| `timestamp` | INTEGER | NO | Unix epoch timestamp in seconds (start of the candlestick period) |
| `open` | REAL | NO | Opening price during the period (₹) |
| `high` | REAL | NO | Highest price traded during the period (₹) |
| `low` | REAL | NO | Lowest price traded during the period (₹) |
| `close` | REAL | NO | Closing / last traded price during the period (₹) |
| `volume` | INTEGER | NO | Number of shares traded during the period |

#### Key Design Decisions

* **Foreign Key `(symbol) REFERENCES tickers(symbol) ON DELETE CASCADE`**: Ensures referential integrity; deleting a ticker automatically cascades and removes all associated candlestick history.
* **Composite Primary Key `(symbol, timeframe, timestamp)`**: Guarantees that duplicate bars for the same point in time cannot exist.
* **`WITHOUT ROWID`**: Optimizes B-tree storage by eliminating the hidden 64-bit rowid integer, saving disk space and speeding up range scans on composite primary keys.
* **`ON CONFLICT ... DO UPDATE` (Upsert)**: When new data arrives, an unclosed / forming candle updates its `high`, `low`, `close`, and `volume` in-place without duplicating rows.

---

### 3. `cache_sync_meta` (Sync Metadata & Freshness Tracker)

Stores synchronization state per `(symbol, timeframe)` pair, linked via foreign key to `tickers`. There is exactly one row per stock per timeframe (~900 small rows total for 150 stocks across 6 timeframes).

```sql
CREATE TABLE IF NOT EXISTS cache_sync_meta (
    symbol           TEXT    NOT NULL,
    timeframe        TEXT    NOT NULL,
    last_synced_at   INTEGER NOT NULL,
    first_candle_ts  INTEGER NOT NULL,
    last_candle_ts   INTEGER NOT NULL,
    candle_count     INTEGER NOT NULL,
    last_verified_at INTEGER DEFAULT 0,
    is_gap_detected  INTEGER DEFAULT 0,
    PRIMARY KEY (symbol, timeframe),
    FOREIGN KEY (symbol) REFERENCES tickers(symbol) ON DELETE CASCADE
);
```

#### Columns

| Column | Type | Nullable | Description |
| :--- | :--- | :--- | :--- |
| `symbol` | TEXT | NO | NSE stock symbol referencing `tickers(symbol)` (Foreign Key, `ON DELETE CASCADE`) |
| `timeframe` | TEXT | NO | Timeframe code (e.g. `'15m'`, `'1d'`) |
| `last_synced_at` | INTEGER | NO | Unix epoch timestamp of when Yahoo Finance API was last queried |
| `first_candle_ts` | INTEGER | NO | Timestamp of earliest stored candle for this symbol & timeframe |
| `last_candle_ts` | INTEGER | NO | Timestamp of latest stored candle for this symbol & timeframe |
| `candle_count` | INTEGER | NO | Total number of candles stored in the `candles` table |
| `last_verified_at`| INTEGER | YES | Unix epoch timestamp of when internal gaps were last verified |
| `is_gap_detected` | INTEGER | YES | Flag (`1` or `0`) indicating whether missing historical session slots exist |

---

### 4. `market_holidays` (Exchange Calendar & Holiday Overrides)

Stores official Indian stock exchange (NSE/BSE) trading holidays and operational overrides.

```sql
CREATE TABLE IF NOT EXISTS market_holidays (
    holiday_date TEXT PRIMARY KEY NOT NULL,
    description  TEXT NOT NULL,
    exchange     TEXT NOT NULL DEFAULT 'NSE'
);
```

#### Columns

| Column | Type | Nullable | Description |
| :--- | :--- | :--- | :--- |
| `holiday_date` | TEXT | NO | Date in `YYYY-MM-DD` format (Primary Key) |
| `description`  | TEXT | NO | Name of the holiday (e.g. `'Diwali-Laxmi Pujan'`, `'Independence Day'`) |
| `exchange`     | TEXT | NO | Exchange identifier (defaults to `'NSE'`) |

---

## Gap-Aware Incremental Cache Architecture

When candles are requested via the Web API or Native GUI, [`CandleSyncService`](file:///home/kiran/work/Rust/MarketWatch/src/provider/sync.rs) executes the gap-aware synchronization lifecycle:

```mermaid
flowchart TD
    Req["Request (symbol, timeframe, force)"] --> CheckInFlight{"In-Flight Request<br/>Already Running for Key?"}
    
    CheckInFlight -- "Yes (Concurrent)" --> Subscribe["Subscribe to watch channel<br/>(Wait for primary task)"]
    Subscribe --> ReadCoalesced["Read SQLite DB directly<br/>(Return X-Cache: COALESCED-HIT)"]

    CheckInFlight -- "No (Primary)" --> Register["Register in-flight sender"]
    Register --> CheckFresh{"Is Cache Fresh & Gap-Free?<br/>(!force && is_fresh && detect_gaps empty)"}
    
    CheckFresh -- "Yes" --> ReadHit["Read from SQLite DB<br/>(Return X-Cache: HIT)"]
    
    CheckFresh -- "No (Gap or Stale)" --> CheckMeta{"Stored Candles Exist?"}
    
    CheckMeta -- "Yes (Incremental)" --> RangeQuery["Fetch Missing Range Only<br/>period1 = max(last_candle_ts, 59-day clamp)<br/>period2 = now()"]
    CheckMeta -- "No (Initial/Force)" --> FullQuery["Fetch Full Timeframe Window"]
    
    RangeQuery --> ExecFetch["Execute Yahoo Request"]
    FullQuery --> ExecFetch
    
    ExecFetch -- "Success" --> AlignFloor["Align tick seconds to session slot boundaries<br/>Deduplicate same-slot updates"]
    AlignFloor --> Upsert["Upsert to SQLite<br/>INSERT ... ON CONFLICT DO UPDATE"]
    Upsert --> Broadcast["Notify coalesced subscribers & return MISS"]
    
    ExecFetch -- "Network Error" --> Fallback{"Stored DB Cache Exists?"}
    Fallback -- "Yes" --> StaleReturn["Return cached candles<br/>(Return X-Cache: STALE-FALLBACK)"]
    Fallback -- "No" --> ErrorReturn["Return 502 Bad Gateway / Network Error"]
```

### Network Fallback
If the network drops or Yahoo Finance rate-limits (HTTP 429), the provider catches the error and serves the existing SQLite cached candles rather than throwing an error to the user.

---

## Example Queries

### Read Candles (Ascending Time)
```sql
SELECT timestamp, open, high, low, close, volume 
FROM candles 
WHERE symbol = 'RELIANCE.NS' AND timeframe = '15m' 
ORDER BY timestamp ASC;
```

### Upsert Candle Bar
```sql
INSERT INTO candles (symbol, timeframe, timestamp, open, high, low, close, volume)
VALUES ('RELIANCE.NS', '15m', 1790057200, 1243.30, 1252.70, 1242.50, 1248.20, 654707)
ON CONFLICT(symbol, timeframe, timestamp) DO UPDATE SET
    open = excluded.open,
    high = excluded.high,
    low = excluded.low,
    close = excluded.close,
    volume = excluded.volume;
```

### Update Sync Metadata
```sql
INSERT INTO cache_sync_meta (symbol, timeframe, last_synced_at, first_candle_ts, last_candle_ts, candle_count)
VALUES ('RELIANCE.NS', '15m', 1790057205, 1789530300, 1790057202, 111)
ON CONFLICT(symbol, timeframe) DO UPDATE SET
    last_synced_at  = excluded.last_synced_at,
    first_candle_ts = excluded.first_candle_ts,
    last_candle_ts  = excluded.last_candle_ts,
    candle_count    = excluded.candle_count;
```
