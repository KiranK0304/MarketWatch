//! Local SQLite persistence and cache layer for candlestick (OHLCV) market data.
//!
//! Provides fast in-process querying (< 0.2ms) and market-aware cache freshness checks
//! for Indian Stock Exchanges (NSE/BSE, UTC+5:30).

use chrono::{DateTime, Datelike, FixedOffset, Timelike, Utc};
use rusqlite::{Connection, params};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::domain::Candle;
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
                         PRIMARY KEY (symbol, timeframe),
                         FOREIGN KEY (symbol) REFERENCES tickers(symbol) ON DELETE CASCADE
                     );

                     INSERT INTO cache_sync_meta_new SELECT * FROM cache_sync_meta;
                     DROP TABLE cache_sync_meta;
                     ALTER TABLE cache_sync_meta_new RENAME TO cache_sync_meta;

                     COMMIT;
                     PRAGMA foreign_keys = ON;",
                )?;
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
                    PRIMARY KEY (symbol, timeframe),
                    FOREIGN KEY (symbol) REFERENCES tickers(symbol) ON DELETE CASCADE
                );",
            )?;
        }

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
            "INSERT INTO cache_sync_meta (symbol, timeframe, last_synced_at, first_candle_ts, last_candle_ts, candle_count)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(symbol, timeframe) DO UPDATE SET
                last_synced_at = excluded.last_synced_at,
                first_candle_ts = excluded.first_candle_ts,
                last_candle_ts = excluded.last_candle_ts,
                candle_count = excluded.candle_count",
            params![symbol, timeframe, synced_at, first_ts, last_ts, count],
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
            "SELECT last_synced_at, first_candle_ts, last_candle_ts, candle_count 
             FROM cache_sync_meta 
             WHERE symbol = ?1 AND timeframe = ?2",
        )?;

        let mut rows = stmt.query(params![symbol, timeframe])?;
        if let Some(row) = rows.next()? {
            Ok(Some(CacheSyncMeta {
                symbol: symbol.to_string(),
                timeframe: timeframe.to_string(),
                last_synced_at: row.get(0)?,
                first_candle_ts: row.get(1)?,
                last_candle_ts: row.get(2)?,
                candle_count: row.get(3)?,
            }))
        } else {
            Ok(None)
        }
    }

    /// Smart check if the cached data is fresh enough to skip external API calls.
    ///
    /// Rules:
    /// 1. If no cache or 0 candles exist -> Stale (false).
    /// 2. If market is currently closed (Weekend, before 09:15 IST, or after 15:30 IST):
    ///    - If `last_synced_at` >= the most recent market close timestamp -> 100% Fresh (true).
    /// 3. If market is currently open (09:15 to 15:30 IST on a weekday):
    ///    - If `now - last_synced_at` <= `ttl_secs` -> Fresh (true).
    ///    - Otherwise -> Stale (false).
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

        let ist_offset = FixedOffset::east_opt(5 * 3600 + 30 * 60)
            .unwrap_or_else(|| FixedOffset::east_opt(0).unwrap());
        let now_ist = Utc::now().with_timezone(&ist_offset);
        let now_epoch = Utc::now().timestamp();

        let is_trading_day = matches!(
            now_ist.weekday(),
            chrono::Weekday::Mon
                | chrono::Weekday::Tue
                | chrono::Weekday::Wed
                | chrono::Weekday::Thu
                | chrono::Weekday::Fri
        );

        let current_minutes = now_ist.hour() * 60 + now_ist.minute();
        const OPEN_MINUTES: u32 = 9 * 60 + 15; // 09:15 AM IST
        const CLOSE_MINUTES: u32 = 15 * 60 + 30; // 03:30 PM IST

        let is_market_open =
            is_trading_day && (OPEN_MINUTES..CLOSE_MINUTES).contains(&current_minutes);

        if is_market_open {
            // During market hours, check TTL window
            let elapsed = now_epoch - meta.last_synced_at;
            Ok(elapsed <= ttl_secs)
        } else {
            // Market is closed. Find the epoch timestamp of the most recent market close (15:30 IST)
            let last_close_epoch = get_last_market_close_epoch(now_ist);
            Ok(meta.last_synced_at >= last_close_epoch)
        }
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
}

/// Calculate the Unix epoch timestamp for the most recent market close (15:30 IST).
fn get_last_market_close_epoch(now_ist: DateTime<FixedOffset>) -> i64 {
    let weekday = now_ist.weekday();
    let current_minutes = now_ist.hour() * 60 + now_ist.minute();
    const CLOSE_MINUTES: u32 = 15 * 60 + 30;

    // How many days back was the last trading day's close?
    let days_back = match weekday {
        chrono::Weekday::Mon => {
            if current_minutes >= CLOSE_MINUTES {
                0
            } else {
                3 // Previous Friday
            }
        }
        chrono::Weekday::Sat => 1, // Friday
        chrono::Weekday::Sun => 2, // Friday
        _ => {
            // Tue, Wed, Thu, Fri
            if current_minutes >= CLOSE_MINUTES {
                0 // Today at 15:30
            } else {
                1 // Yesterday at 15:30
            }
        }
    };

    let target_date = now_ist.date_naive() - chrono::Duration::days(days_back);
    let target_close = target_date
        .and_hms_opt(15, 30, 0)
        .expect("Valid 15:30 time");

    let ist_offset = *now_ist.offset();
    let close_datetime =
        DateTime::<FixedOffset>::from_naive_utc_and_offset(target_close - ist_offset, ist_offset);

    close_datetime.timestamp()
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
        db.save_candles("RELIANCE.NS", "15m", &candles, 2500).unwrap();

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

        let meta = db.get_sync_meta("WIPRO.NS", "1d").unwrap().expect("meta preserved");
        assert_eq!(meta.candle_count, 1);

        // Check that cascade delete now works on migrated table
        db.delete_ticker("WIPRO.NS").unwrap();
        assert_eq!(db.get_candles("WIPRO.NS", "1d").unwrap().len(), 0);
        assert!(db.get_sync_meta("WIPRO.NS", "1d").unwrap().is_none());
    }
}
