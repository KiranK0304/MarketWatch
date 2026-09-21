//! Application error types.
//!
//! Typed errors for market data operations. Uses `thiserror` for
//! ergonomic error derivation. `anyhow` is used at the application
//! boundary (main.rs) for convenient error propagation.

use std::fmt;

/// Errors that can occur during market data operations.
#[derive(Debug)]
#[allow(dead_code)]
pub enum MarketError {
    /// Network/HTTP request failure.
    Network(reqwest::Error),
    /// Failed to parse the provider's response.
    Parse(String),
    /// No data returned for the requested symbol.
    NoData { symbol: String },
    /// Provider returned an HTTP error status.
    Provider {
        status: u16,
        symbol: String,
        body: String,
    },
    /// Invalid stock symbol.
    InvalidSymbol(String),
    /// Configuration file error.
    Config(String),
    /// Invalid timeframe string.
    InvalidTimeframe(String),
}

impl fmt::Display for MarketError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MarketError::Network(e) => write!(f, "Network error: {e}"),
            MarketError::Parse(msg) => write!(f, "Failed to parse response: {msg}"),
            MarketError::NoData { symbol } => {
                write!(
                    f,
                    "No data returned for '{symbol}'. Check that the symbol is valid and the market is open."
                )
            }
            MarketError::Provider {
                status,
                symbol,
                body,
            } => {
                write!(f, "Provider error: HTTP {status} for '{symbol}': {body}")
            }
            MarketError::InvalidSymbol(s) => write!(f, "Invalid symbol: '{s}'"),
            MarketError::Config(msg) => write!(f, "Configuration error: {msg}"),
            MarketError::InvalidTimeframe(s) => write!(f, "Invalid timeframe: '{s}'"),
        }
    }
}

impl std::error::Error for MarketError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            MarketError::Network(e) => Some(e),
            _ => None,
        }
    }
}

impl From<reqwest::Error> for MarketError {
    fn from(e: reqwest::Error) -> Self {
        MarketError::Network(e)
    }
}
