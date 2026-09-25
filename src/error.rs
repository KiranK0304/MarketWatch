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
    /// Database/storage error.
    Database(String),
    /// Invalid timeframe string.
    InvalidTimeframe(String),
    /// Invalid input or validation failure.
    InvalidInput(String),
    /// Resource not found.
    NotFound(String),
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
                // Truncate potentially huge HTML bodies so CLI output stays readable.
                const MAX_BODY: usize = 300;
                let snippet = if body.len() > MAX_BODY {
                    let boundary = body.floor_char_boundary(MAX_BODY);
                    format!("{}… ({} bytes total)", &body[..boundary], body.len())
                } else {
                    body.clone()
                };
                write!(f, "Provider error: HTTP {status} for '{symbol}': {snippet}")
            }
            MarketError::InvalidSymbol(s) => write!(f, "Invalid symbol: '{s}'"),
            MarketError::Config(msg) => write!(f, "Configuration error: {msg}"),
            MarketError::Database(msg) => write!(f, "Database error: {msg}"),
            MarketError::InvalidTimeframe(s) => write!(f, "Invalid timeframe: '{s}'"),
            MarketError::InvalidInput(msg) => write!(f, "Invalid input: {msg}"),
            MarketError::NotFound(msg) => write!(f, "Not found: {msg}"),
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

impl From<rusqlite::Error> for MarketError {
    fn from(e: rusqlite::Error) -> Self {
        MarketError::Database(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_error_truncation_utf8_boundary() {
        // Construct a string with 299 'a' characters followed by a 4-byte emoji (🚀 = 4 bytes: 0xF0 0x9F 0x99 0x80)
        // Total bytes = 303. Index 300 falls inside the 4-byte emoji codepoint.
        let mut body = "a".repeat(299);
        body.push('🚀');

        let err = MarketError::Provider {
            status: 500,
            symbol: "TEST".to_string(),
            body,
        };

        let formatted = format!("{err}");
        assert!(formatted.contains("Provider error: HTTP 500 for 'TEST':"));
        assert!(formatted.contains("… (303 bytes total)"));
        // Boundary should floor before the emoji at byte 299
        assert!(formatted.contains(&"a".repeat(299)));
        assert!(!formatted.contains('🚀'));
    }

    #[test]
    fn test_provider_error_short_body() {
        let err = MarketError::Provider {
            status: 404,
            symbol: "TEST".to_string(),
            body: "Not found".to_string(),
        };
        let formatted = format!("{err}");
        assert_eq!(formatted, "Provider error: HTTP 404 for 'TEST': Not found");
    }
}

