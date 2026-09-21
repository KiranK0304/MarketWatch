//! Configuration loading for the stock universe.
//!
//! Reads `config/stocks.toml` and deserializes it into typed Rust structs.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::MarketError;

/// Top-level configuration containing the stock universe.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockConfig {
    pub stocks: Vec<StockEntry>,
}

/// A single stock in the universe.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockEntry {
    /// Ticker symbol (e.g. "RELIANCE.NS").
    pub symbol: String,
    /// Human-readable company name.
    pub name: String,
}

/// Load the stock configuration from a TOML file.
///
/// # Errors
/// Returns `MarketError::Config` if the file cannot be read or parsed.
pub fn load_stock_config(path: &Path) -> Result<StockConfig, MarketError> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| MarketError::Config(format!("Cannot read '{}': {}", path.display(), e)))?;

    let config: StockConfig = toml::from_str(&content)
        .map_err(|e| MarketError::Config(format!("Invalid TOML in '{}': {}", path.display(), e)))?;

    if config.stocks.is_empty() {
        return Err(MarketError::Config(
            "Stock list is empty. Add at least one [[stocks]] entry.".to_string(),
        ));
    }

    Ok(config)
}

/// Save the stock configuration to a TOML file.
///
/// # Errors
/// Returns `MarketError::Config` if serialization or writing fails.
pub fn save_stock_config(path: &Path, config: &StockConfig) -> Result<(), MarketError> {
    let toml_str = toml::to_string_pretty(config)
        .map_err(|e| MarketError::Config(format!("Failed to serialize stock config: {e}")))?;

    std::fs::write(path, toml_str).map_err(|e| {
        MarketError::Config(format!("Failed to write to '{}': {}", path.display(), e))
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn parse_valid_config() {
        let toml = r#"
[[stocks]]
symbol = "RELIANCE.NS"
name = "Reliance Industries"

[[stocks]]
symbol = "TCS.NS"
name = "Tata Consultancy Services"
"#;
        let config: StockConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.stocks.len(), 2);
        assert_eq!(config.stocks[0].symbol, "RELIANCE.NS");
        assert_eq!(config.stocks[1].name, "Tata Consultancy Services");
    }

    #[test]
    fn parse_empty_stock_list() {
        let toml = "stocks = []\n";
        let config: StockConfig = toml::from_str(toml).unwrap();
        assert!(config.stocks.is_empty());
    }

    #[test]
    fn parse_invalid_toml() {
        let result = toml::from_str::<StockConfig>("this is not valid toml {{{}}}");
        assert!(result.is_err());
    }

    #[test]
    fn load_from_file() {
        let dir = std::env::temp_dir().join("market_test_config");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test_stocks.toml");

        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(
            f,
            r#"
[[stocks]]
symbol = "INFY.NS"
name = "Infosys"
"#
        )
        .unwrap();

        let config = load_stock_config(&path).unwrap();
        assert_eq!(config.stocks.len(), 1);
        assert_eq!(config.stocks[0].symbol, "INFY.NS");

        // Cleanup
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn load_missing_file() {
        let result = load_stock_config(Path::new("/tmp/nonexistent_market_config.toml"));
        assert!(matches!(result, Err(MarketError::Config(_))));
    }

    #[test]
    fn reject_empty_stock_list_on_load() {
        let dir = std::env::temp_dir().join("market_test_empty");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("empty_stocks.toml");

        std::fs::write(&path, "stocks = []\n").unwrap();

        let result = load_stock_config(&path);
        assert!(matches!(result, Err(MarketError::Config(_))));

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn save_and_reload_config() {
        let dir = std::env::temp_dir().join("market_test_save");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("save_stocks.toml");

        let original = StockConfig {
            stocks: vec![
                StockEntry {
                    symbol: "SBIN.NS".to_string(),
                    name: "State Bank of India".to_string(),
                },
                StockEntry {
                    symbol: "ITC.NS".to_string(),
                    name: "ITC Limited".to_string(),
                },
            ],
        };

        save_stock_config(&path, &original).unwrap();
        let loaded = load_stock_config(&path).unwrap();

        assert_eq!(loaded.stocks.len(), 2);
        assert_eq!(loaded.stocks[0].symbol, "SBIN.NS");
        assert_eq!(loaded.stocks[1].name, "ITC Limited");

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }
}
