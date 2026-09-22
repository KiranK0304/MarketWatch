//! Market data provider abstraction.
//!
//! Defines the `MarketDataProvider` trait that all data sources implement.
//! The application layer only depends on this trait, never on provider-specific types.

pub mod cached;
pub mod yahoo;

pub use cached::CachedProvider;

use crate::domain::{Candle, Timeframe};
use crate::error::MarketError;

/// Trait for fetching market data from any provider.
///
/// Implementors must convert their native response format into `Vec<Candle>`.
pub trait MarketDataProvider: Send + Sync {
    /// Fetch OHLCV candle data for a symbol and timeframe.
    ///
    /// Returns candles sorted by timestamp ascending.
    /// The provider determines the appropriate historical range.
    fn fetch_candles(
        &self,
        symbol: &str,
        timeframe: Timeframe,
    ) -> impl std::future::Future<Output = Result<Vec<Candle>, MarketError>> + Send;
}
