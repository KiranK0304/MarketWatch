//! Parallel stock scanner engine.
//!
//! Scans the stock universe concurrently using an async Semaphore to respect rate limits,
//! computes price movements, and filters movers exceeding the threshold.

use std::sync::Arc;
use tokio::sync::Semaphore;

use crate::config::StockEntry;
use crate::domain::{ScanResult, StockMover};
use crate::error::MarketError;
use crate::provider::yahoo::YahooProvider;

/// Concurrency limit for parallel Yahoo Finance requests.
const MAX_CONCURRENT_REQUESTS: usize = 15;

/// Scanner engine for detecting market movers across the stock universe.
pub struct Scanner {
    provider: Arc<YahooProvider>,
}

impl Scanner {
    /// Create a new scanner instance with the shared provider.
    pub fn new(provider: Arc<YahooProvider>) -> Self {
        Self { provider }
    }

    /// Scan the stock universe and filter movers meeting or exceeding `threshold_percent`.
    pub async fn scan(&self, stocks: &[StockEntry], threshold_percent: f64) -> Result<ScanResult, MarketError> {
        let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_REQUESTS));
        let mut handles = Vec::with_capacity(stocks.len());

        for stock in stocks {
            let provider = Arc::clone(&self.provider);
            let sem = Arc::clone(&semaphore);
            let sym = stock.symbol.clone();
            let name = stock.name.clone();

            let handle = tokio::spawn(async move {
                let _permit = sem.acquire().await.ok()?;
                match provider.fetch_mover(&sym, &name).await {
                    Ok(mover) => Some(mover),
                    Err(e) => {
                        eprintln!("Warning: Failed to fetch quote for {sym}: {e}");
                        None
                    }
                }
            });

            handles.push(handle);
        }

        let mut all_quotes = Vec::with_capacity(stocks.len());
        for handle in handles {
            if let Ok(Some(mover)) = handle.await {
                all_quotes.push(mover);
            }
        }

        let now = chrono::Utc::now();
        // Indian Standard Time is UTC + 5h 30m (19800 seconds)
        let ist_offset = chrono::FixedOffset::east_opt(5 * 3600 + 30 * 60)
            .unwrap_or_else(|| chrono::FixedOffset::east_opt(0).unwrap());
        let ist_now = now.with_timezone(&ist_offset);
        let scan_time = ist_now.format("%Y-%m-%d %H:%M:%S IST").to_string();

        // Filter movers by threshold
        let mut movers: Vec<StockMover> = all_quotes
            .iter()
            .filter(|m| m.matches_threshold(threshold_percent))
            .cloned()
            .collect();

        // Sort movers by absolute percentage move descending (largest moves first)
        movers.sort_by(|a, b| {
            b.change_percent
                .abs()
                .partial_cmp(&a.change_percent.abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Sort all quotes by absolute change percent as well
        all_quotes.sort_by(|a, b| {
            b.change_percent
                .abs()
                .partial_cmp(&a.change_percent.abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let gainers_count = movers.iter().filter(|m| m.is_gainer()).count();
        let losers_count = movers.iter().filter(|m| !m.is_gainer()).count();
        let movers_count = movers.len();
        let total_scanned = all_quotes.len();

        Ok(ScanResult {
            timestamp: now.timestamp(),
            scan_time,
            threshold_percent,
            total_scanned,
            movers_count,
            gainers_count,
            losers_count,
            movers,
            all_quotes,
        })
    }
}
