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

        // Schema setup
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
                PRIMARY KEY (symbol, timeframe, timestamp)
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
                PRIMARY KEY (symbol, timeframe)
            );",
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
            is_trading_day && current_minutes >= OPEN_MINUTES && current_minutes < CLOSE_MINUTES;

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
}
