//! Normalized candlestick (OHLCV) data structure.
//!
//! This is the application's canonical representation of a single price bar.
//! All market data providers convert their responses into this type.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A single OHLCV candlestick bar.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct Candle {
    /// Unix timestamp in seconds.
    pub timestamp: i64,
    /// Parsed UTC datetime for display purposes.
    pub datetime: DateTime<Utc>,
    /// Opening price.
    pub open: f64,
    /// Highest price in the period.
    pub high: f64,
    /// Lowest price in the period.
    pub low: f64,
    /// Closing price.
    pub close: f64,
    /// Trading volume.
    pub volume: u64,
}

/// Standard floor pivot levels (Pivot, R1, S1, R2, S2).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FloorPivots {
    pub pivot: f64,
    pub r1: f64,
    pub s1: f64,
    pub r2: f64,
    pub s2: f64,
}

impl Candle {
    /// Returns true if the candle closed higher than or equal to its open (bullish).
    pub fn is_bullish(&self) -> bool {
        self.close >= self.open
    }

    /// Calculate standard classical floor pivot points from this bar.
    #[allow(dead_code)]
    pub fn floor_pivots(&self) -> FloorPivots {
        let p = (self.high + self.low + self.close) / 3.0;
        FloorPivots {
            pivot: p,
            r1: 2.0 * p - self.low,
            s1: 2.0 * p - self.high,
            r2: p + (self.high - self.low),
            s2: p - (self.high - self.low),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_candle(open: f64, close: f64) -> Candle {
        Candle {
            timestamp: 1_000_000,
            datetime: DateTime::from_timestamp(1_000_000, 0).unwrap(),
            open,
            high: open.max(close) + 1.0,
            low: open.min(close) - 1.0,
            close,
            volume: 100_000,
        }
    }

    #[test]
    fn bullish_candle() {
        assert!(make_candle(100.0, 105.0).is_bullish());
    }

    #[test]
    fn bearish_candle() {
        assert!(!make_candle(105.0, 100.0).is_bullish());
    }

    #[test]
    fn doji_candle_is_bullish() {
        // Equal open/close is considered bullish (neutral)
        assert!(make_candle(100.0, 100.0).is_bullish());
    }

    #[test]
    fn test_floor_pivots_calculation() {
        let candle = Candle {
            timestamp: 1_000_000,
            datetime: DateTime::from_timestamp(1_000_000, 0).unwrap(),
            open: 100.0,
            high: 110.0,
            low: 90.0,
            close: 100.0,
            volume: 50_000,
        };

        let pivots = candle.floor_pivots();
        assert!((pivots.pivot - 100.0).abs() < 1e-6);
        assert!((pivots.r1 - 110.0).abs() < 1e-6);
        assert!((pivots.s1 - 90.0).abs() < 1e-6);
        assert!((pivots.r2 - 120.0).abs() < 1e-6);
        assert!((pivots.s2 - 80.0).abs() < 1e-6);
    }
}
