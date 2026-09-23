//! Local SQLite persistence and cache layer for candlestick (OHLCV) market data.
//!
//! Provides fast in-process querying (< 0.2ms) and market-aware cache freshness checks
//! for Indian Stock Exchanges (NSE/BSE, UTC+5:30).

use chrono::{DateTime, Utc};
use rusqlite::{Connection, params};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::domain::{Candle, CreateNoteInput, StockNote, UpdateNoteInput};
use crate::error::MarketError;

/// A tracked stock ticker entry in the primary SQLite registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ticker {
    pub symbol: String,
    pub name: String,
    pub exchange: String,
    pub is_active: bool,
    pub created_at: i64,
}

/// Metadata tracking cache state and last API synchronization per symbol & timeframe.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheSyncMeta {
    pub symbol: String,
    pub timeframe: String,
    pub last_synced_at: i64,
    pub first_candle_ts: i64,
    pub last_candle_ts: i64,
    pub candle_count: usize,
    pub last_verified_at: i64,
    pub is_gap_detected: bool,
}

/// SQLite database handle for caching OHLCV candles.
#[derive(Clone)]
pub struct MarketDb {
    conn: Arc<Mutex<Connection>>,
}

impl MarketDb {
    /// Open or create the default SQLite database.
    ///
    /// Resolves to:
    /// 1. `~/.config/marketwatch/marketwatch.db`
    /// 2. `./cache/marketwatch.db` (fallback)
    pub fn open_default() -> Result<Self, MarketError> {
        let path = Self::default_path();
        Self::open(&path)
    }

    /// Open or create a database at a specific path.
    pub fn open(path: &Path) -> Result<Self, MarketError> {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let conn = Connection::open(path)?;
        Self::init_connection(conn)
    }

    /// Open an in-memory database (useful for testing).
    #[allow(dead_code)]
    pub fn open_in_memory() -> Result<Self, MarketError> {
        let conn = Connection::open_in_memory()?;
        Self::init_connection(conn)
    }

    fn init_connection(conn: Connection) -> Result<Self, MarketError> {
        // WAL mode for high concurrency and ultra-fast writes
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;",
        )?;

        // Schema setup: Primary tickers registry table
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS tickers (
                symbol      TEXT PRIMARY KEY NOT NULL,
                name        TEXT NOT NULL,
                exchange    TEXT NOT NULL DEFAULT 'NSE',
                is_active   INTEGER NOT NULL DEFAULT 1,
                created_at  INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS market_holidays (
                holiday_date        TEXT PRIMARY KEY NOT NULL,
                description         TEXT NOT NULL,
                is_trading_holiday  INTEGER NOT NULL DEFAULT 1
            );",
        )?;

        // Check if candles table already exists
        let candles_table_exists: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='candles'",
                [],
                |row| {
                    let count: i64 = row.get(0)?;
                    Ok(count > 0)
                },
            )
            .unwrap_or(false);

        if candles_table_exists {
            // Seed tickers table with any existing symbols in candles or cache_sync_meta
            let _ = conn.execute_batch(
                "INSERT OR IGNORE INTO tickers (symbol, name, exchange, is_active, created_at)
                 SELECT DISTINCT symbol, symbol, 'NSE', 1, CAST(strftime('%s', 'now') AS INTEGER) FROM candles;
                 INSERT OR IGNORE INTO tickers (symbol, name, exchange, is_active, created_at)
                 SELECT DISTINCT symbol, symbol, 'NSE', 1, CAST(strftime('%s', 'now') AS INTEGER) FROM cache_sync_meta;",
            );

            // Check if candles already has foreign key configured
            let candles_has_fk: bool = {
                let mut stmt = conn.prepare("PRAGMA foreign_key_list(candles)")?;
                let mut rows = stmt.query([])?;
                rows.next()?.is_some()
            };

            if !candles_has_fk {
                // Migrate candles and cache_sync_meta to include FOREIGN KEY constraints
                conn.execute_batch(
                    "PRAGMA foreign_keys = OFF;
                     BEGIN TRANSACTION;

                     CREATE TABLE candles_new (
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

                     INSERT INTO candles_new SELECT * FROM candles;
                     DROP TABLE candles;
                     ALTER TABLE candles_new RENAME TO candles;
                     CREATE INDEX IF NOT EXISTS idx_candles_lookup 
                     ON candles (symbol, timeframe, timestamp ASC);

                     CREATE TABLE cache_sync_meta_new (
                         symbol          TEXT    NOT NULL,
                         timeframe       TEXT    NOT NULL,
                         last_synced_at  INTEGER NOT NULL,
                         first_candle_ts INTEGER NOT NULL,
                         last_candle_ts  INTEGER NOT NULL,
                         candle_count    INTEGER NOT NULL,
                         last_verified_at INTEGER DEFAULT 0,
                         is_gap_detected INTEGER DEFAULT 0,
                         PRIMARY KEY (symbol, timeframe),
                         FOREIGN KEY (symbol) REFERENCES tickers(symbol) ON DELETE CASCADE
                     );

                     INSERT INTO cache_sync_meta_new SELECT symbol, timeframe, last_synced_at, first_candle_ts, last_candle_ts, candle_count, 0, 0 FROM cache_sync_meta;
                     DROP TABLE cache_sync_meta;
                     ALTER TABLE cache_sync_meta_new RENAME TO cache_sync_meta;

                     COMMIT;
                     PRAGMA foreign_keys = ON;",
                )?;
            } else {
                // Table already has foreign keys. Ensure columns last_verified_at and is_gap_detected exist
                let mut stmt = conn.prepare("PRAGMA table_info(cache_sync_meta)")?;
                let columns: std::collections::HashSet<String> = stmt
                    .query_map([], |row| row.get::<_, String>(1))?
                    .filter_map(|r| r.ok())
                    .collect();

                if !columns.contains("last_verified_at") {
                    let _ = conn.execute("ALTER TABLE cache_sync_meta ADD COLUMN last_verified_at INTEGER DEFAULT 0;", []);
                }
                if !columns.contains("is_gap_detected") {
                    let _ = conn.execute(
                        "ALTER TABLE cache_sync_meta ADD COLUMN is_gap_detected INTEGER DEFAULT 0;",
                        [],
                    );
                }
            }
        } else {
            // Fresh database setup with foreign keys defined upfront
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS candles (
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

                CREATE TABLE IF NOT EXISTS cache_sync_meta (
                    symbol          TEXT    NOT NULL,
                    timeframe       TEXT    NOT NULL,
                    last_synced_at  INTEGER NOT NULL,
                    first_candle_ts INTEGER NOT NULL,
                    last_candle_ts  INTEGER NOT NULL,
                    candle_count    INTEGER NOT NULL,
                    last_verified_at INTEGER DEFAULT 0,
                    is_gap_detected INTEGER DEFAULT 0,
                    PRIMARY KEY (symbol, timeframe),
                    FOREIGN KEY (symbol) REFERENCES tickers(symbol) ON DELETE CASCADE
                );",
            )?;
        }

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS stock_notes (
                id                  INTEGER PRIMARY KEY AUTOINCREMENT,
                symbol              TEXT    NOT NULL,
                timeframe           TEXT    NOT NULL DEFAULT '15m',
                candle_timestamp    INTEGER,
                price_at_note       REAL    NOT NULL,
                title               TEXT    NOT NULL,
                content             TEXT    NOT NULL,
                tags                TEXT    DEFAULT '',
                status              TEXT    NOT NULL DEFAULT 'open',
                target_price        REAL,
                stop_loss           REAL,
                outcome_note        TEXT,
                verified_at         INTEGER,
                reference_note_ids  TEXT    DEFAULT '[]',
                created_at          INTEGER NOT NULL,
                updated_at          INTEGER NOT NULL,
                FOREIGN KEY (symbol) REFERENCES tickers(symbol) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_stock_notes_symbol 
            ON stock_notes (symbol, created_at DESC);

            CREATE INDEX IF NOT EXISTS idx_stock_notes_status 
            ON stock_notes (status);",
        )?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Default path for the database file.
    pub fn default_path() -> PathBuf {
        if let Some(home) = std::env::var_os("HOME") {
            let path = PathBuf::from(home).join(".config/marketwatch/marketwatch.db");
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            return path;
        }

        PathBuf::from("cache/marketwatch.db")
    }

    /// Retrieve all cached candles for a given symbol and timeframe in ascending order.
    pub fn get_candles(&self, symbol: &str, timeframe: &str) -> Result<Vec<Candle>, MarketError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| MarketError::Database(e.to_string()))?;
        let mut stmt = conn.prepare_cached(
            "SELECT timestamp, open, high, low, close, volume 
             FROM candles 
             WHERE symbol = ?1 AND timeframe = ?2 
             ORDER BY timestamp ASC",
        )?;

        let candle_iter = stmt.query_map(params![symbol, timeframe], |row| {
            let ts: i64 = row.get(0)?;
            let open: f64 = row.get(1)?;
            let high: f64 = row.get(2)?;
            let low: f64 = row.get(3)?;
            let close: f64 = row.get(4)?;
            let volume: u64 = row.get(5)?;

            let datetime = DateTime::from_timestamp(ts, 0).unwrap_or_default();

            Ok(Candle {
                timestamp: ts,
                datetime,
                open,
                high,
                low,
                close,
                volume,
            })
        })?;

        let mut candles = Vec::new();
        for candle in candle_iter {
            candles.push(candle?);
        }

        Ok(candles)
    }

    /// Save candles into SQLite with upsert semantics and update metadata.
    pub fn save_candles(
        &self,
        symbol: &str,
        timeframe: &str,
        candles: &[Candle],
        synced_at: i64,
    ) -> Result<(), MarketError> {
        if candles.is_empty() {
            return Ok(());
        }

        let mut conn = self
            .conn
            .lock()
            .map_err(|e| MarketError::Database(e.to_string()))?;
        let tx = conn.transaction()?;

        // Ensure parent ticker exists in tickers table to satisfy foreign key constraint
        tx.execute(
            "INSERT OR IGNORE INTO tickers (symbol, name, exchange, is_active, created_at)
             VALUES (?1, ?1, 'NSE', 1, ?2)",
            params![symbol, synced_at],
        )?;

        {
            let mut stmt = tx.prepare_cached(
                "INSERT INTO candles (symbol, timeframe, timestamp, open, high, low, close, volume)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT(symbol, timeframe, timestamp) DO UPDATE SET
                    open = excluded.open,
                    high = excluded.high,
                    low = excluded.low,
                    close = excluded.close,
                    volume = excluded.volume",
            )?;

            for c in candles {
                stmt.execute(params![
                    symbol,
                    timeframe,
                    c.timestamp,
                    c.open,
                    c.high,
                    c.low,
                    c.close,
                    c.volume
                ])?;
            }
        }

        // Compute updated aggregate bounds
        let (first_ts, last_ts, count): (i64, i64, usize) = tx.query_row(
            "SELECT MIN(timestamp), MAX(timestamp), COUNT(*) 
             FROM candles 
             WHERE symbol = ?1 AND timeframe = ?2",
            params![symbol, timeframe],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;

        // Update metadata
        tx.execute(
            "INSERT INTO cache_sync_meta (symbol, timeframe, last_synced_at, first_candle_ts, last_candle_ts, candle_count, last_verified_at, is_gap_detected)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0)
             ON CONFLICT(symbol, timeframe) DO UPDATE SET
                last_synced_at = excluded.last_synced_at,
                first_candle_ts = excluded.first_candle_ts,
                last_candle_ts = excluded.last_candle_ts,
                candle_count = excluded.candle_count,
                last_verified_at = excluded.last_verified_at,
                is_gap_detected = 0",
            params![symbol, timeframe, synced_at, first_ts, last_ts, count, synced_at],
        )?;

        tx.commit()?;
        Ok(())
    }

    /// Retrieve sync metadata for a symbol and timeframe.
    pub fn get_sync_meta(
        &self,
        symbol: &str,
        timeframe: &str,
    ) -> Result<Option<CacheSyncMeta>, MarketError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| MarketError::Database(e.to_string()))?;
        let mut stmt = conn.prepare_cached(
            "SELECT last_synced_at, first_candle_ts, last_candle_ts, candle_count,
                    COALESCE(last_verified_at, 0), COALESCE(is_gap_detected, 0) 
             FROM cache_sync_meta 
             WHERE symbol = ?1 AND timeframe = ?2",
        )?;

        let mut rows = stmt.query(params![symbol, timeframe])?;
        if let Some(row) = rows.next()? {
            let is_gap_int: i64 = row.get(5)?;
            Ok(Some(CacheSyncMeta {
                symbol: symbol.to_string(),
                timeframe: timeframe.to_string(),
                last_synced_at: row.get(0)?,
                first_candle_ts: row.get(1)?,
                last_candle_ts: row.get(2)?,
                candle_count: row.get(3)?,
                last_verified_at: row.get(4)?,
                is_gap_detected: is_gap_int != 0,
            }))
        } else {
            Ok(None)
        }
    }

    /// Retrieve candles within a specific timestamp range in ascending order.
    #[allow(dead_code)]
    pub fn get_candles_range(
        &self,
        symbol: &str,
        timeframe: &str,
        start_ts: i64,
        end_ts: i64,
    ) -> Result<Vec<Candle>, MarketError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| MarketError::Database(e.to_string()))?;
        let mut stmt = conn.prepare_cached(
            "SELECT timestamp, open, high, low, close, volume 
             FROM candles 
             WHERE symbol = ?1 AND timeframe = ?2 AND timestamp >= ?3 AND timestamp <= ?4
             ORDER BY timestamp ASC",
        )?;

        let candle_iter = stmt.query_map(params![symbol, timeframe, start_ts, end_ts], |row| {
            let ts: i64 = row.get(0)?;
            let open: f64 = row.get(1)?;
            let high: f64 = row.get(2)?;
            let low: f64 = row.get(3)?;
            let close: f64 = row.get(4)?;
            let volume: u64 = row.get(5)?;

            let datetime = DateTime::from_timestamp(ts, 0).unwrap_or_default();

            Ok(Candle {
                timestamp: ts,
                datetime,
                open,
                high,
                low,
                close,
                volume,
            })
        })?;

        let mut candles = Vec::new();
        for candle in candle_iter {
            candles.push(candle?);
        }

        Ok(candles)
    }

    /// Retrieve the single most recent candle for a given symbol and timeframe.
    #[allow(dead_code)]
    pub fn get_latest_candle(
        &self,
        symbol: &str,
        timeframe: &str,
    ) -> Result<Option<Candle>, MarketError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| MarketError::Database(e.to_string()))?;
        let mut stmt = conn.prepare_cached(
            "SELECT timestamp, open, high, low, close, volume 
             FROM candles 
             WHERE symbol = ?1 AND timeframe = ?2 
             ORDER BY timestamp DESC LIMIT 1",
        )?;

        let mut rows = stmt.query(params![symbol, timeframe])?;
        if let Some(row) = rows.next()? {
            let ts: i64 = row.get(0)?;
            let open: f64 = row.get(1)?;
            let high: f64 = row.get(2)?;
            let low: f64 = row.get(3)?;
            let close: f64 = row.get(4)?;
            let volume: u64 = row.get(5)?;

            let datetime = DateTime::from_timestamp(ts, 0).unwrap_or_default();

            Ok(Some(Candle {
                timestamp: ts,
                datetime,
                open,
                high,
                low,
                close,
                volume,
            }))
        } else {
            Ok(None)
        }
    }

    /// Retrieve all stored candle timestamps in ascending order for gap checking.
    pub fn get_all_stored_timestamps(
        &self,
        symbol: &str,
        timeframe: &str,
    ) -> Result<Vec<i64>, MarketError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| MarketError::Database(e.to_string()))?;
        let mut stmt = conn.prepare_cached(
            "SELECT timestamp 
             FROM candles 
             WHERE symbol = ?1 AND timeframe = ?2 
             ORDER BY timestamp ASC",
        )?;

        let rows = stmt.query_map(params![symbol, timeframe], |row| row.get(0))?;
        let mut timestamps = Vec::new();
        for ts in rows {
            timestamps.push(ts?);
        }
        Ok(timestamps)
    }

    /// Detect missing internal candle ranges for a given symbol and timeframe.
    ///
    /// Compares stored timestamps in SQLite with the expected schedule from `MarketCalendar`.
    /// Returns contiguous missing timestamp ranges `(gap_start, gap_end)`.
    pub fn detect_gaps(
        &self,
        symbol: &str,
        timeframe: crate::domain::Timeframe,
    ) -> Result<Vec<(i64, i64)>, MarketError> {
        let tf_label = timeframe.label();
        let meta = match self.get_sync_meta(symbol, tf_label)? {
            Some(m) if m.candle_count >= 2 => m,
            _ => return Ok(Vec::new()),
        };

        let expected_slots = crate::domain::calendar::MarketCalendar::expected_slots_between(
            meta.first_candle_ts,
            meta.last_candle_ts,
            timeframe,
        );

        if expected_slots.is_empty() {
            return Ok(Vec::new());
        }

        let stored_ts = self.get_all_stored_timestamps(symbol, tf_label)?;
        let stored_set: std::collections::HashSet<i64> = stored_ts.into_iter().collect();

        let mut gaps = Vec::new();
        let mut current_gap: Option<(i64, i64)> = None;

        for slot in expected_slots {
            if !stored_set.contains(&slot) {
                match current_gap {
                    Some((start, _)) => current_gap = Some((start, slot)),
                    None => current_gap = Some((slot, slot)),
                }
            } else if let Some((start, end)) = current_gap.take() {
                gaps.push((start, end));
            }
        }

        if let Some((start, end)) = current_gap {
            gaps.push((start, end));
        }

        Ok(gaps)
    }

    /// Mark whether a gap was detected for a symbol and timeframe in sync metadata.
    #[allow(dead_code)]
    pub fn mark_gap_detected(
        &self,
        symbol: &str,
        timeframe: &str,
        is_gap: bool,
    ) -> Result<(), MarketError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| MarketError::Database(e.to_string()))?;
        conn.execute(
            "UPDATE cache_sync_meta SET is_gap_detected = ?1 WHERE symbol = ?2 AND timeframe = ?3",
            params![if is_gap { 1 } else { 0 }, symbol, timeframe],
        )?;
        Ok(())
    }

    /// Robust, gap-aware check if the cached data is fresh enough to skip external API calls.
    ///
    /// Verifies that:
    /// 1. Stored data reaches the latest expected market candle for the trading calendar.
    /// 2. If the current candle is actively forming, checks that its age is within `ttl_secs`.
    pub fn is_fresh(
        &self,
        symbol: &str,
        timeframe: &str,
        ttl_secs: i64,
    ) -> Result<bool, MarketError> {
        let meta = match self.get_sync_meta(symbol, timeframe)? {
            Some(m) if m.candle_count > 0 => m,
            _ => return Ok(false),
        };

        let tf: crate::domain::Timeframe = match timeframe.parse() {
            Ok(t) => t,
            Err(_) => return Ok(false),
        };

        if meta.is_gap_detected {
            return Ok(false);
        }

        let now_ist = crate::domain::calendar::now_ist();
        let expected = crate::domain::calendar::MarketCalendar::latest_expected_candle(now_ist, tf);

        // If the latest stored candle hasn't even reached the expected latest slot, it's stale!
        if meta.last_candle_ts < expected.timestamp {
            return Ok(false);
        }

        // If the latest expected candle is currently forming in live market:
        if expected.is_forming {
            let elapsed = chrono::Utc::now().timestamp() - meta.last_synced_at;
            return Ok(elapsed <= ttl_secs);
        }

        // Market closed and data reaches the final closed slot: 100% fresh!
        Ok(true)
    }

    /// Upsert a ticker into the primary tickers table.
    pub fn upsert_ticker(&self, ticker: &Ticker) -> Result<(), MarketError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| MarketError::Database(e.to_string()))?;
        conn.execute(
            "INSERT INTO tickers (symbol, name, exchange, is_active, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(symbol) DO UPDATE SET
                name = excluded.name,
                exchange = excluded.exchange,
                is_active = excluded.is_active",
            params![
                ticker.symbol,
                ticker.name,
                ticker.exchange,
                if ticker.is_active { 1 } else { 0 },
                ticker.created_at,
            ],
        )?;
        Ok(())
    }

    /// Synchronize stock universe entries from configuration into the `tickers` table.
    pub fn sync_tickers(&self, stocks: &[crate::config::StockEntry]) -> Result<(), MarketError> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| MarketError::Database(e.to_string()))?;
        let now = Utc::now().timestamp();
        let tx = conn.transaction()?;
        {
            let mut stmt = tx.prepare_cached(
                "INSERT INTO tickers (symbol, name, exchange, is_active, created_at)
                 VALUES (?1, ?2, 'NSE', 1, ?3)
                 ON CONFLICT(symbol) DO UPDATE SET
                    name = excluded.name,
                    is_active = 1",
            )?;
            for s in stocks {
                stmt.execute(params![s.symbol, s.name, now])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Retrieve a ticker by its symbol.
    #[allow(dead_code)]
    pub fn get_ticker(&self, symbol: &str) -> Result<Option<Ticker>, MarketError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| MarketError::Database(e.to_string()))?;
        let mut stmt = conn.prepare_cached(
            "SELECT symbol, name, exchange, is_active, created_at
             FROM tickers
             WHERE symbol = ?1",
        )?;
        let mut rows = stmt.query(params![symbol])?;
        if let Some(row) = rows.next()? {
            let is_active_int: i64 = row.get(3)?;
            Ok(Some(Ticker {
                symbol: row.get(0)?,
                name: row.get(1)?,
                exchange: row.get(2)?,
                is_active: is_active_int != 0,
                created_at: row.get(4)?,
            }))
        } else {
            Ok(None)
        }
    }

    /// Retrieve all tickers ordered by symbol.
    #[allow(dead_code)]
    pub fn get_all_tickers(&self) -> Result<Vec<Ticker>, MarketError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| MarketError::Database(e.to_string()))?;
        let mut stmt = conn.prepare_cached(
            "SELECT symbol, name, exchange, is_active, created_at
             FROM tickers
             ORDER BY symbol ASC",
        )?;
        let ticker_iter = stmt.query_map([], |row| {
            let is_active_int: i64 = row.get(3)?;
            Ok(Ticker {
                symbol: row.get(0)?,
                name: row.get(1)?,
                exchange: row.get(2)?,
                is_active: is_active_int != 0,
                created_at: row.get(4)?,
            })
        })?;
        let mut tickers = Vec::new();
        for t in ticker_iter {
            tickers.push(t?);
        }
        Ok(tickers)
    }

    /// Delete a ticker by symbol. Due to `ON DELETE CASCADE`, all associated
    /// candles and sync metadata are also deleted automatically.
    pub fn delete_ticker(&self, symbol: &str) -> Result<bool, MarketError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| MarketError::Database(e.to_string()))?;
        let affected = conn.execute("DELETE FROM tickers WHERE symbol = ?1", params![symbol])?;
        Ok(affected > 0)
    }

    /// Create a new stock analysis note.
    pub fn create_note(&self, input: &CreateNoteInput) -> Result<StockNote, MarketError> {
        let sym_upper = input.symbol.trim().to_uppercase();
        let now = Utc::now().timestamp();
        let ref_json =
            serde_json::to_string(&input.reference_note_ids).unwrap_or_else(|_| "[]".to_string());

        // Ensure symbol exists in tickers table to satisfy foreign key constraint
        let _ = self.upsert_ticker(&Ticker {
            symbol: sym_upper.clone(),
            name: sym_upper.clone(),
            exchange: "NSE".to_string(),
            is_active: true,
            created_at: now,
        });

        let conn = self
            .conn
            .lock()
            .map_err(|e| MarketError::Database(e.to_string()))?;

        conn.execute(
            "INSERT INTO stock_notes (
                symbol, timeframe, candle_timestamp, price_at_note, title, content,
                tags, status, target_price, stop_loss, outcome_note, verified_at,
                reference_note_ids, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'open', ?8, ?9, NULL, NULL, ?10, ?11, ?11)",
            params![
                sym_upper,
                input.timeframe,
                input.candle_timestamp,
                input.price_at_note,
                input.title.trim(),
                input.content.trim(),
                input.tags.trim(),
                input.target_price,
                input.stop_loss,
                ref_json,
                now,
            ],
        )?;

        let id = conn.last_insert_rowid();

        Ok(StockNote {
            id,
            symbol: sym_upper,
            timeframe: input.timeframe.clone(),
            candle_timestamp: input.candle_timestamp,
            price_at_note: input.price_at_note,
            title: input.title.trim().to_string(),
            content: input.content.trim().to_string(),
            tags: input.tags.trim().to_string(),
            status: "open".to_string(),
            target_price: input.target_price,
            stop_loss: input.stop_loss,
            outcome_note: None,
            verified_at: None,
            reference_note_ids: input.reference_note_ids.clone(),
            created_at: now,
            updated_at: now,
        })
    }

    /// Retrieve a single stock analysis note by ID.
    pub fn get_note(&self, id: i64) -> Result<Option<StockNote>, MarketError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| MarketError::Database(e.to_string()))?;

        let mut stmt = conn.prepare_cached(
            "SELECT id, symbol, timeframe, candle_timestamp, price_at_note, title, content,
                    tags, status, target_price, stop_loss, outcome_note, verified_at,
                    reference_note_ids, created_at, updated_at
             FROM stock_notes
             WHERE id = ?1",
        )?;

        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(map_note_row(row)?))
        } else {
            Ok(None)
        }
    }

    /// List stock analysis notes with optional symbol and status filters.
    pub fn list_notes(
        &self,
        symbol: Option<&str>,
        status: Option<&str>,
    ) -> Result<Vec<StockNote>, MarketError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| MarketError::Database(e.to_string()))?;

        let mut sql =
            "SELECT id, symbol, timeframe, candle_timestamp, price_at_note, title, content,
                              tags, status, target_price, stop_loss, outcome_note, verified_at,
                              reference_note_ids, created_at, updated_at
                       FROM stock_notes WHERE 1=1"
                .to_string();

        let sym_upper = symbol.map(|s| s.trim().to_uppercase());
        if sym_upper.is_some() {
            sql.push_str(" AND symbol = ?1");
        }
        if status.is_some() {
            if sym_upper.is_some() {
                sql.push_str(" AND status = ?2");
            } else {
                sql.push_str(" AND status = ?1");
            }
        }
        sql.push_str(" ORDER BY created_at DESC, id DESC");

        let mut stmt = conn.prepare(&sql)?;

        let note_iter = match (&sym_upper, &status) {
            (Some(sym), Some(st)) => stmt.query_map(params![sym, st], map_note_row)?,
            (Some(sym), None) => stmt.query_map(params![sym], map_note_row)?,
            (None, Some(st)) => stmt.query_map(params![st], map_note_row)?,
            (None, None) => stmt.query_map([], map_note_row)?,
        };

        let mut notes = Vec::new();
        for n in note_iter {
            notes.push(n?);
        }
        Ok(notes)
    }

    /// Update an existing stock analysis note.
    pub fn update_note(
        &self,
        id: i64,
        input: &UpdateNoteInput,
    ) -> Result<Option<StockNote>, MarketError> {
        let existing = match self.get_note(id)? {
            Some(n) => n,
            None => return Ok(None),
        };

        let updated_title = input.title.as_deref().unwrap_or(&existing.title).trim();
        let updated_content = input.content.as_deref().unwrap_or(&existing.content).trim();
        let updated_tags = input.tags.as_deref().unwrap_or(&existing.tags).trim();
        let updated_status = input.status.as_deref().unwrap_or(&existing.status).trim();
        let updated_target = input.target_price.or(existing.target_price);
        let updated_stop = input.stop_loss.or(existing.stop_loss);
        let updated_outcome = input.outcome_note.clone().or(existing.outcome_note);
        let updated_verified = input.verified_at.or(existing.verified_at);
        let updated_refs = input
            .reference_note_ids
            .as_ref()
            .unwrap_or(&existing.reference_note_ids);
        let ref_json = serde_json::to_string(updated_refs).unwrap_or_else(|_| "[]".to_string());
        let now = Utc::now().timestamp();

        let conn = self
            .conn
            .lock()
            .map_err(|e| MarketError::Database(e.to_string()))?;

        conn.execute(
            "UPDATE stock_notes SET
                title = ?1,
                content = ?2,
                tags = ?3,
                status = ?4,
                target_price = ?5,
                stop_loss = ?6,
                outcome_note = ?7,
                verified_at = ?8,
                reference_note_ids = ?9,
                updated_at = ?10
             WHERE id = ?11",
            params![
                updated_title,
                updated_content,
                updated_tags,
                updated_status,
                updated_target,
                updated_stop,
                updated_outcome,
                updated_verified,
                ref_json,
                now,
                id,
            ],
        )?;

        drop(conn);
        self.get_note(id)
    }

    /// Delete a stock analysis note by ID.
    pub fn delete_note(&self, id: i64) -> Result<bool, MarketError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| MarketError::Database(e.to_string()))?;

        let affected = conn.execute("DELETE FROM stock_notes WHERE id = ?1", params![id])?;
        Ok(affected > 0)
    }
}

fn map_note_row(row: &rusqlite::Row) -> rusqlite::Result<StockNote> {
    let id: i64 = row.get(0)?;
    let symbol: String = row.get(1)?;
    let timeframe: String = row.get(2)?;
    let candle_timestamp: Option<i64> = row.get(3)?;
    let price_at_note: f64 = row.get(4)?;
    let title: String = row.get(5)?;
    let content: String = row.get(6)?;
    let tags: String = row.get(7)?;
    let status: String = row.get(8)?;
    let target_price: Option<f64> = row.get(9)?;
    let stop_loss: Option<f64> = row.get(10)?;
    let outcome_note: Option<String> = row.get(11)?;
    let verified_at: Option<i64> = row.get(12)?;
    let ref_json: String = row.get(13)?;
    let created_at: i64 = row.get(14)?;
    let updated_at: i64 = row.get(15)?;

    let reference_note_ids: Vec<i64> = serde_json::from_str(&ref_json).unwrap_or_default();

    Ok(StockNote {
        id,
        symbol,
        timeframe,
        candle_timestamp,
        price_at_note,
        title,
        content,
        tags,
        status,
        target_price,
        stop_loss,
        outcome_note,
        verified_at,
        reference_note_ids,
        created_at,
        updated_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_candle(ts: i64, open: f64, close: f64) -> Candle {
        Candle {
            timestamp: ts,
            datetime: DateTime::from_timestamp(ts, 0).unwrap(),
            open,
            high: open.max(close) + 2.0,
            low: open.min(close) - 2.0,
            close,
            volume: 50_000,
        }
    }

    #[test]
    fn test_sqlite_in_memory_crud() {
        let db = MarketDb::open_in_memory().expect("in-memory db open");

        let candles = vec![
            make_test_candle(1000, 100.0, 105.0),
            make_test_candle(1900, 105.0, 102.0),
        ];

        db.save_candles("TCS.NS", "15m", &candles, 2000).unwrap();

        let loaded = db.get_candles("TCS.NS", "15m").unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].timestamp, 1000);
        assert_eq!(loaded[0].close, 105.0);
        assert_eq!(loaded[1].timestamp, 1900);
        assert_eq!(loaded[1].close, 102.0);

        let meta = db.get_sync_meta("TCS.NS", "15m").unwrap().unwrap();
        assert_eq!(meta.candle_count, 2);
        assert_eq!(meta.last_synced_at, 2000);
        assert_eq!(meta.first_candle_ts, 1000);
        assert_eq!(meta.last_candle_ts, 1900);
    }

    #[test]
    fn test_upsert_replaces_existing_candle() {
        let db = MarketDb::open_in_memory().expect("in-memory db open");

        let candle_v1 = vec![make_test_candle(1000, 100.0, 105.0)];
        db.save_candles("INFY.NS", "1d", &candle_v1, 1500).unwrap();

        // New tick updates close of the active candle
        let candle_v2 = vec![make_test_candle(1000, 100.0, 110.0)];
        db.save_candles("INFY.NS", "1d", &candle_v2, 1600).unwrap();

        let loaded = db.get_candles("INFY.NS", "1d").unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].close, 110.0); // updated in-place without duplicate
    }

    #[test]
    fn test_tickers_crud_and_cascade_delete() {
        let db = MarketDb::open_in_memory().expect("in-memory db open");

        // 1. Upsert a ticker
        let ticker = Ticker {
            symbol: "RELIANCE.NS".to_string(),
            name: "Reliance Industries Ltd".to_string(),
            exchange: "NSE".to_string(),
            is_active: true,
            created_at: 1000,
        };
        db.upsert_ticker(&ticker).unwrap();

        let fetched = db.get_ticker("RELIANCE.NS").unwrap().expect("ticker found");
        assert_eq!(fetched.name, "Reliance Industries Ltd");
        assert_eq!(fetched.exchange, "NSE");
        assert!(fetched.is_active);

        // 2. Add candles and sync meta for this ticker
        let candles = vec![
            make_test_candle(1000, 2500.0, 2520.0),
            make_test_candle(2000, 2520.0, 2550.0),
        ];
        db.save_candles("RELIANCE.NS", "15m", &candles, 2500)
            .unwrap();

        assert_eq!(db.get_candles("RELIANCE.NS", "15m").unwrap().len(), 2);
        assert!(db.get_sync_meta("RELIANCE.NS", "15m").unwrap().is_some());

        // 3. Delete ticker -> Foreign Key CASCADE should wipe associated candles and sync meta
        let deleted = db.delete_ticker("RELIANCE.NS").unwrap();
        assert!(deleted);

        assert!(db.get_ticker("RELIANCE.NS").unwrap().is_none());
        assert_eq!(db.get_candles("RELIANCE.NS", "15m").unwrap().len(), 0);
        assert!(db.get_sync_meta("RELIANCE.NS", "15m").unwrap().is_none());
    }

    #[test]
    fn test_sync_tickers_from_config() {
        let db = MarketDb::open_in_memory().expect("in-memory db open");

        let stocks = vec![
            crate::config::StockEntry {
                symbol: "HDFCBANK.NS".to_string(),
                name: "HDFC Bank Ltd".to_string(),
            },
            crate::config::StockEntry {
                symbol: "ICICIBANK.NS".to_string(),
                name: "ICICI Bank Ltd".to_string(),
            },
        ];

        db.sync_tickers(&stocks).unwrap();

        let all = db.get_all_tickers().unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].symbol, "HDFCBANK.NS");
        assert_eq!(all[0].name, "HDFC Bank Ltd");
        assert_eq!(all[1].symbol, "ICICIBANK.NS");
    }

    #[test]
    fn test_migration_from_unconstrained_schema() {
        // Create an unconstrained raw sqlite connection mimicking old schema
        let raw_conn = Connection::open_in_memory().unwrap();
        raw_conn
            .execute_batch(
                "CREATE TABLE candles (
                    symbol TEXT NOT NULL,
                    timeframe TEXT NOT NULL,
                    timestamp INTEGER NOT NULL,
                    open REAL NOT NULL,
                    high REAL NOT NULL,
                    low REAL NOT NULL,
                    close REAL NOT NULL,
                    volume INTEGER NOT NULL,
                    PRIMARY KEY (symbol, timeframe, timestamp)
                ) WITHOUT ROWID;

                CREATE TABLE cache_sync_meta (
                    symbol TEXT NOT NULL,
                    timeframe TEXT NOT NULL,
                    last_synced_at INTEGER NOT NULL,
                    first_candle_ts INTEGER NOT NULL,
                    last_candle_ts INTEGER NOT NULL,
                    candle_count INTEGER NOT NULL,
                    PRIMARY KEY (symbol, timeframe)
                );

                INSERT INTO candles VALUES ('WIPRO.NS', '1d', 1000, 400.0, 410.0, 395.0, 405.0, 10000);
                INSERT INTO cache_sync_meta VALUES ('WIPRO.NS', '1d', 1200, 1000, 1000, 1);",
            )
            .unwrap();

        // Run MarketDb::init_connection which performs automatic migration
        let db = MarketDb::init_connection(raw_conn).unwrap();

        // Check that WIPRO.NS was populated into tickers
        let ticker = db.get_ticker("WIPRO.NS").unwrap().expect("ticker migrated");
        assert_eq!(ticker.symbol, "WIPRO.NS");

        // Check that existing candles and meta were preserved
        let candles = db.get_candles("WIPRO.NS", "1d").unwrap();
        assert_eq!(candles.len(), 1);
        assert_eq!(candles[0].close, 405.0);

        let meta = db
            .get_sync_meta("WIPRO.NS", "1d")
            .unwrap()
            .expect("meta preserved");
        assert_eq!(meta.candle_count, 1);

        // Check that cascade delete now works on migrated table
        db.delete_ticker("WIPRO.NS").unwrap();
        assert_eq!(db.get_candles("WIPRO.NS", "1d").unwrap().len(), 0);
        assert!(db.get_sync_meta("WIPRO.NS", "1d").unwrap().is_none());
    }

    #[test]
    fn test_is_fresh_scenarios() {
        let db = MarketDb::open_in_memory().unwrap();
        let now_ist = crate::domain::calendar::now_ist();
        let expected = crate::domain::calendar::MarketCalendar::latest_expected_candle(
            now_ist,
            crate::domain::Timeframe::Min15,
        );
        let now_epoch = chrono::Utc::now().timestamp();

        // 1. Missing latest candle: stored timestamp is strictly older than expected
        let old_ts = expected.timestamp - 3600;
        let c_old = make_test_candle(old_ts, 100.0, 101.0);
        db.save_candles("TEST.NS", "15m", &[c_old], now_epoch)
            .unwrap();
        assert!(
            !db.is_fresh("TEST.NS", "15m", 300).unwrap(),
            "Older candle must not be fresh"
        );

        // 2. Complete coverage: stored candle reaches expected timestamp
        let c_curr = make_test_candle(expected.timestamp, 101.0, 102.0);
        db.save_candles("TEST.NS", "15m", std::slice::from_ref(&c_curr), now_epoch)
            .unwrap();

        if expected.is_forming {
            // Fresh when recently synced
            assert!(
                db.is_fresh("TEST.NS", "15m", 300).unwrap(),
                "Forming candle within TTL is fresh"
            );

            // Stale when synced long ago
            let old_sync_time = now_epoch - 600;
            db.save_candles(
                "TEST.NS",
                "15m",
                std::slice::from_ref(&c_curr),
                old_sync_time,
            )
            .unwrap();
            assert!(
                !db.is_fresh("TEST.NS", "15m", 300).unwrap(),
                "Forming candle beyond TTL is stale"
            );
        } else {
            // Market closed: 100% fresh regardless of last_synced_at
            let old_sync_time = now_epoch - 86400;
            db.save_candles(
                "TEST.NS",
                "15m",
                std::slice::from_ref(&c_curr),
                old_sync_time,
            )
            .unwrap();
            assert!(
                db.is_fresh("TEST.NS", "15m", 300).unwrap(),
                "Market closed with complete coverage is fresh"
            );
        }

        // 3. Gap detected: even if coverage seems complete, a detected gap makes it stale
        db.mark_gap_detected("TEST.NS", "15m", true).unwrap();
        assert!(
            !db.is_fresh("TEST.NS", "15m", 300).unwrap(),
            "Gap detected must make cache stale"
        );
    }

    #[test]
    fn test_stock_notes_crud_and_cascade_delete() {
        let db = MarketDb::open_in_memory().unwrap();

        // 1. Create Note 1
        let note1 = db
            .create_note(&CreateNoteInput {
                symbol: "HDFCBANK.NS".to_string(),
                timeframe: "15m".to_string(),
                candle_timestamp: Some(1790000000),
                price_at_note: 1650.50,
                title: "Momentum breakout setup".to_string(),
                content: "Breakout above 1640 with volume surge".to_string(),
                tags: "breakout, momentum".to_string(),
                target_price: Some(1720.0),
                stop_loss: Some(1615.0),
                reference_note_ids: vec![],
            })
            .unwrap();

        assert_eq!(note1.id, 1);
        assert_eq!(note1.symbol, "HDFCBANK.NS");
        assert_eq!(note1.price_at_note, 1650.50);
        assert_eq!(note1.status, "open");

        // 2. Create Note 2 referencing Note 1
        let note2 = db
            .create_note(&CreateNoteInput {
                symbol: "HDFCBANK.NS".to_string(),
                timeframe: "15m".to_string(),
                candle_timestamp: Some(1790003600),
                price_at_note: 1680.00,
                title: "Follow-up consolidation".to_string(),
                content: "Holding above breakout level".to_string(),
                tags: "consolidation".to_string(),
                target_price: Some(1720.0),
                stop_loss: Some(1645.0),
                reference_note_ids: vec![note1.id],
            })
            .unwrap();

        assert_eq!(note2.id, 2);
        assert_eq!(note2.reference_note_ids, vec![1]);

        // 3. List notes by symbol
        let hdfc_notes = db.list_notes(Some("HDFCBANK.NS"), None).unwrap();
        assert_eq!(hdfc_notes.len(), 2);
        assert_eq!(hdfc_notes[0].id, 2); // Ordered by created_at DESC

        // 4. Update note 1 (verification / audit)
        let updated = db
            .update_note(
                note1.id,
                &UpdateNoteInput {
                    status: Some("validated".to_string()),
                    outcome_note: Some("Hit target of 1720 on day 3".to_string()),
                    verified_at: Some(chrono::Utc::now().timestamp()),
                    ..Default::default()
                },
            )
            .unwrap()
            .expect("Note exists");

        assert_eq!(updated.status, "validated");
        assert_eq!(
            updated.outcome_note.as_deref(),
            Some("Hit target of 1720 on day 3")
        );

        // 5. Test filtering by status
        let open_notes = db.list_notes(None, Some("open")).unwrap();
        assert_eq!(open_notes.len(), 1);
        assert_eq!(open_notes[0].id, 2);

        let validated_notes = db.list_notes(None, Some("validated")).unwrap();
        assert_eq!(validated_notes.len(), 1);
        assert_eq!(validated_notes[0].id, 1);

        // 6. Test delete_note
        let deleted = db.delete_note(note2.id).unwrap();
        assert!(deleted);
        assert!(db.get_note(note2.id).unwrap().is_none());

        // 7. Test cascade delete: deleting ticker cascades and deletes its notes
        db.delete_ticker("HDFCBANK.NS").unwrap();
        let notes_after = db.list_notes(Some("HDFCBANK.NS"), None).unwrap();
        assert_eq!(notes_after.len(), 0);
    }
}
