//! Cached market data provider wrapping SQLite storage.
//!
//! Checks market-aware cache freshness before querying external APIs.
//! Falls back gracefully to cached data if external network requests fail.

use crate::domain::{Candle, Timeframe};
use crate::error::MarketError;
use crate::provider::MarketDataProvider;
use crate::storage::MarketDb;

/// A provider wrapper that transparently caches candles in a local SQLite database.
pub struct CachedProvider<P> {
    inner: P,
    db: MarketDb,
    ttl_secs: i64,
}

impl<P: MarketDataProvider> CachedProvider<P> {
    /// Create a new cached provider with default 60-second market-hours TTL.
    pub fn new(inner: P, db: MarketDb) -> Self {
        Self {
            inner,
            db,
            ttl_secs: 60,
        }
    }

    /// Access the underlying SQLite database handle.
    #[allow(dead_code)]
    pub fn db(&self) -> &MarketDb {
        &self.db
    }

    /// Fetch candles respecting cache freshness or forcing a remote sync.
    ///
    /// Returns `(candles, was_cached)`.
    pub async fn fetch_candles_with_policy(
        &self,
        symbol: &str,
        timeframe: Timeframe,
        force: bool,
    ) -> Result<(Vec<Candle>, bool), MarketError> {
        let tf_label = timeframe.label();

        // If not forcing refresh, check if DB cache is already fresh
        if !force {
            if self.db.is_fresh(symbol, tf_label, self.ttl_secs)? {
                let cached = self.db.get_candles(symbol, tf_label)?;
                if !cached.is_empty() {
                    return Ok((cached, true));
                }
            }
        }

        // Otherwise fetch from remote provider
        match self.inner.fetch_candles(symbol, timeframe).await {
            Ok(candles) => {
                let now = chrono::Utc::now().timestamp();
                self.db.save_candles(symbol, tf_label, &candles, now)?;

                // Return full accumulated candles from SQLite
                let all_candles = self.db.get_candles(symbol, tf_label)?;
                if !all_candles.is_empty() {
                    return Ok((all_candles, false));
                }
                Ok((candles, false))
            }
            Err(e) => {
                // Graceful fallback: If network request failed or was rate-limited,
                // check if we have any historical cached data in SQLite to display
                let cached = self.db.get_candles(symbol, tf_label)?;
                if !cached.is_empty() {
                    return Ok((cached, true));
                }
                Err(e)
            }
        }
    }
}

impl<P: MarketDataProvider> MarketDataProvider for CachedProvider<P> {
    async fn fetch_candles(
        &self,
        symbol: &str,
        timeframe: Timeframe,
    ) -> Result<Vec<Candle>, MarketError> {
        self.fetch_candles_with_policy(symbol, timeframe, false)
            .await
            .map(|(candles, _)| candles)
    }
}
