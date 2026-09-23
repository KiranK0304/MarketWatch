//! Cached market data provider wrapping SQLite storage and CandleSyncService.
//!
//! Provides transparent gap-aware caching, coalescing, and incremental sync.

use std::sync::Arc;

use crate::domain::{Candle, Timeframe};
use crate::error::MarketError;
use crate::provider::MarketDataProvider;
use crate::provider::sync::{CandleSyncService, SyncStatus};
use crate::provider::yahoo::YahooProvider;
use crate::storage::MarketDb;

/// A provider wrapper that transparently caches candles in a local SQLite database
/// using `CandleSyncService`.
pub struct CachedProvider {
    sync_service: CandleSyncService,
}

impl CachedProvider {
    /// Create a new cached provider backed by SQLite and Yahoo Finance.
    pub fn new(provider: YahooProvider, db: MarketDb) -> Self {
        Self {
            sync_service: CandleSyncService::new(db, Arc::new(provider)),
        }
    }

    /// Access the underlying `CandleSyncService`.
    #[allow(dead_code)]
    pub fn sync_service(&self) -> &CandleSyncService {
        &self.sync_service
    }

    /// Access the underlying SQLite database handle.
    #[allow(dead_code)]
    pub fn db(&self) -> &MarketDb {
        self.sync_service.db()
    }

    /// Fetch candles respecting cache freshness or forcing a remote sync.
    #[allow(dead_code)]
    pub async fn fetch_candles_with_policy(
        &self,
        symbol: &str,
        timeframe: Timeframe,
        force: bool,
    ) -> Result<(Vec<Candle>, SyncStatus), MarketError> {
        self.sync_service
            .get_candles(symbol, timeframe, force)
            .await
    }
}

impl MarketDataProvider for CachedProvider {
    async fn fetch_candles(
        &self,
        symbol: &str,
        timeframe: Timeframe,
    ) -> Result<Vec<Candle>, MarketError> {
        self.sync_service
            .get_candles(symbol, timeframe, false)
            .await
            .map(|(candles, _)| candles)
    }
}
