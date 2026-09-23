//! Timeframe representation for market data intervals.
//!
//! Each variant maps to a Yahoo Finance API interval and range,
//! and can be parsed from CLI strings like "5m", "15m", "1d".

use std::fmt;
use std::str::FromStr;

/// Supported market data timeframes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Timeframe {
    Min5,
    Min15,
    Min30,
    Hour1,
    Day1,
    Week1,
}

impl Timeframe {
    /// All available timeframes, in order.
    pub const ALL: &[Timeframe] = &[
        Timeframe::Min5,
        Timeframe::Min15,
        Timeframe::Min30,
        Timeframe::Hour1,
        Timeframe::Day1,
        Timeframe::Week1,
    ];

    /// Human-readable label for display and CLI.
    pub fn label(self) -> &'static str {
        match self {
            Timeframe::Min5 => "5m",
            Timeframe::Min15 => "15m",
            Timeframe::Min30 => "30m",
            Timeframe::Hour1 => "1h",
            Timeframe::Day1 => "1d",
            Timeframe::Week1 => "1w",
        }
    }

    /// Yahoo Finance API interval parameter.
    pub fn yahoo_interval(self) -> &'static str {
        match self {
            Timeframe::Min5 => "5m",
            Timeframe::Min15 => "15m",
            Timeframe::Min30 => "30m",
            Timeframe::Hour1 => "60m",
            Timeframe::Day1 => "1d",
            Timeframe::Week1 => "1wk",
        }
    }

    /// Number of seconds represented by one candle interval.
    pub fn seconds(self) -> i64 {
        match self {
            Timeframe::Min5 => 5 * 60,
            Timeframe::Min15 => 15 * 60,
            Timeframe::Min30 => 30 * 60,
            Timeframe::Hour1 => 60 * 60,
            Timeframe::Day1 => 24 * 60 * 60,
            Timeframe::Week1 => 7 * 24 * 60 * 60,
        }
    }

    /// Yahoo Finance API range parameter.
    /// Returns the maximum useful historical range for each interval.
    pub fn yahoo_range(self) -> &'static str {
        match self {
            // Intraday: limited history
            Timeframe::Min5 => "5d",
            Timeframe::Min15 => "5d",
            Timeframe::Min30 => "1mo",
            Timeframe::Hour1 => "6mo",
            // Daily/weekly: years of history
            Timeframe::Day1 => "2y",
            Timeframe::Week1 => "5y",
        }
    }
}

impl fmt::Display for Timeframe {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.label())
    }
}

impl FromStr for Timeframe {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "5m" => Ok(Timeframe::Min5),
            "15m" => Ok(Timeframe::Min15),
            "30m" => Ok(Timeframe::Min30),
            "1h" | "60m" => Ok(Timeframe::Hour1),
            "1d" => Ok(Timeframe::Day1),
            "1w" | "1wk" => Ok(Timeframe::Week1),
            _ => {
                let valid: Vec<&str> = Timeframe::ALL.iter().map(|t| t.label()).collect();
                Err(format!(
                    "Invalid timeframe '{}'. Valid options: {}",
                    s,
                    valid.join(", ")
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_timeframes() {
        assert_eq!("5m".parse::<Timeframe>().unwrap(), Timeframe::Min5);
        assert_eq!("15m".parse::<Timeframe>().unwrap(), Timeframe::Min15);
        assert_eq!("30m".parse::<Timeframe>().unwrap(), Timeframe::Min30);
        assert_eq!("1h".parse::<Timeframe>().unwrap(), Timeframe::Hour1);
        assert_eq!("60m".parse::<Timeframe>().unwrap(), Timeframe::Hour1);
        assert_eq!("1d".parse::<Timeframe>().unwrap(), Timeframe::Day1);
        assert_eq!("1w".parse::<Timeframe>().unwrap(), Timeframe::Week1);
        assert_eq!("1wk".parse::<Timeframe>().unwrap(), Timeframe::Week1);
    }

    #[test]
    fn parse_case_insensitive() {
        assert_eq!("5M".parse::<Timeframe>().unwrap(), Timeframe::Min5);
        assert_eq!("1H".parse::<Timeframe>().unwrap(), Timeframe::Hour1);
        assert_eq!("1D".parse::<Timeframe>().unwrap(), Timeframe::Day1);
        assert_eq!("1W".parse::<Timeframe>().unwrap(), Timeframe::Week1);
    }

    #[test]
    fn parse_invalid_timeframe() {
        let err = "3m".parse::<Timeframe>().unwrap_err();
        assert!(err.contains("Invalid timeframe '3m'"));
        assert!(err.contains("5m"));

        assert!("".parse::<Timeframe>().is_err());
        assert!("abc".parse::<Timeframe>().is_err());
        assert!("2h".parse::<Timeframe>().is_err());
    }

    #[test]
    fn display_roundtrip() {
        for tf in Timeframe::ALL {
            let label = tf.to_string();
            let parsed: Timeframe = label.parse().unwrap();
            assert_eq!(*tf, parsed);
        }
    }

    #[test]
    fn yahoo_mappings() {
        assert_eq!(Timeframe::Min5.yahoo_interval(), "5m");
        assert_eq!(Timeframe::Hour1.yahoo_interval(), "60m");
        assert_eq!(Timeframe::Week1.yahoo_interval(), "1wk");

        assert_eq!(Timeframe::Min5.yahoo_range(), "5d");
        assert_eq!(Timeframe::Day1.yahoo_range(), "2y");
        assert_eq!(Timeframe::Week1.yahoo_range(), "5y");
    }

    #[test]
    fn interval_seconds() {
        assert_eq!(Timeframe::Min5.seconds(), 300);
        assert_eq!(Timeframe::Min15.seconds(), 900);
        assert_eq!(Timeframe::Hour1.seconds(), 3600);
        assert_eq!(Timeframe::Week1.seconds(), 604_800);
    }
}
