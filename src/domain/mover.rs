//! Domain model for stock movers and scanner results.

use serde::{Deserialize, Serialize};

/// Detailed price change and quote info for a single stock.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StockMover {
    /// Stock symbol (e.g. "RELIANCE.NS")
    pub symbol: String,
    /// Company display name
    pub name: String,
    /// Latest / regular market price (INR)
    pub price: f64,
    /// Previous trading session closing price
    pub prev_close: f64,
    /// Absolute change (price - prev_close)
    pub change: f64,
    /// Percentage change ((price - prev_close) / prev_close * 100)
    pub change_percent: f64,
    /// Regular trading volume
    pub volume: u64,
    /// Day high price
    pub day_high: f64,
    /// Day low price
    pub day_low: f64,
    /// Unix timestamp of last quote
    pub timestamp: i64,
}

impl StockMover {
    /// Returns true if stock is green (up or neutral)
    pub fn is_gainer(&self) -> bool {
        self.change_percent >= 0.0
    }

    /// Returns true if price movement meets or exceeds the given threshold percentage
    pub fn matches_threshold(&self, threshold_percent: f64) -> bool {
        self.change_percent.abs() >= threshold_percent.abs()
    }
}

/// Aggregated result of scanning the stock universe.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    /// Unix timestamp when scan finished
    pub timestamp: i64,
    /// Human-readable scan time (IST)
    pub scan_time: String,
    /// Threshold percent used for filtering (e.g. 3.0 for ±3%)
    pub threshold_percent: f64,
    /// Total number of stocks scanned in universe
    pub total_scanned: usize,
    /// Number of stocks matching the threshold
    pub movers_count: usize,
    /// Number of gainers matching threshold
    pub gainers_count: usize,
    /// Number of losers matching threshold
    pub losers_count: usize,
    /// Filtered list of movers matching the threshold, sorted by largest absolute move first
    pub movers: Vec<StockMover>,
    /// All scanned stocks with quotes for instant local filtering
    pub all_quotes: Vec<StockMover>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_mover(symbol: &str, change_percent: f64) -> StockMover {
        StockMover {
            symbol: symbol.to_string(),
            name: symbol.to_string(),
            price: 100.0 * (1.0 + change_percent / 100.0),
            prev_close: 100.0,
            change: change_percent,
            change_percent,
            volume: 1_000_000,
            day_high: 105.0,
            day_low: 95.0,
            timestamp: 1700000000,
        }
    }

    #[test]
    fn test_mover_threshold_matching() {
        let gainer = make_mover("TATAMOTORS.NS", 3.5);
        let loser = make_mover("INFY.NS", -3.2);
        let mild = make_mover("TCS.NS", 1.2);

        assert!(gainer.matches_threshold(3.0));
        assert!(loser.matches_threshold(3.0));
        assert!(!mild.matches_threshold(3.0));

        assert!(gainer.matches_threshold(2.0));
        assert!(loser.matches_threshold(2.0));
        assert!(!mild.matches_threshold(2.0));

        assert!(gainer.is_gainer());
        assert!(!loser.is_gainer());
    }
}
