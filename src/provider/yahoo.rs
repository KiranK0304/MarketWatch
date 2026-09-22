//! Yahoo Finance market data provider.
//!
//! Fetches OHLCV data from Yahoo's v8 chart API and converts it into
//! application-level `Candle` structs. All Yahoo-specific types are private
//! to this module.

use chrono::DateTime;
use reqwest::Client;
use serde::Deserialize;

use crate::domain::{Candle, StockMover, Timeframe};
use crate::error::MarketError;
use crate::provider::MarketDataProvider;

/// Yahoo Finance data provider.
pub struct YahooProvider {
    client: Client,
}

impl YahooProvider {
    /// Create a new Yahoo provider with sensible defaults.
    pub fn new() -> Result<Self, MarketError> {
        let client = Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36")
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .map_err(MarketError::Network)?;

        Ok(Self { client })
    }

    /// Fetch latest market quote and price change for a stock mover.
    pub async fn fetch_mover(&self, symbol: &str, name: &str) -> Result<StockMover, MarketError> {
        let endpoints = [
            format!(
                "https://query2.finance.yahoo.com/v8/finance/chart/{symbol}?interval=1d&range=5d"
            ),
            format!(
                "https://query1.finance.yahoo.com/v8/finance/chart/{symbol}?interval=1d&range=5d"
            ),
        ];

        let mut last_error = None;

        for url in endpoints {
            let response = match self.client.get(&url).send().await {
                Ok(resp) => resp,
                Err(e) => {
                    last_error = Some(MarketError::Network(e));
                    continue;
                }
            };

            let status = response.status();
            if !status.is_success() {
                let body = response.text().await.unwrap_or_default();
                last_error = Some(MarketError::Provider {
                    status: status.as_u16(),
                    symbol: symbol.to_string(),
                    body,
                });
                continue;
            }

            let body = match response.text().await {
                Ok(b) => b,
                Err(e) => {
                    last_error = Some(MarketError::Network(e));
                    continue;
                }
            };

            let yahoo_response: YahooChartResponse = serde_json::from_str(&body)
                .map_err(|e| MarketError::Parse(format!("Yahoo JSON parse error: {e}")))?;

            return convert_to_mover(&yahoo_response, symbol, name);
        }

        Err(last_error.unwrap_or_else(|| MarketError::NoData {
            symbol: symbol.to_string(),
        }))
    }
}

impl MarketDataProvider for YahooProvider {
    async fn fetch_candles(
        &self,
        symbol: &str,
        timeframe: Timeframe,
    ) -> Result<Vec<Candle>, MarketError> {
        // query2 is significantly more reliable and less prone to aggressive 429 rate limiting
        let endpoints = [
            format!(
                "https://query2.finance.yahoo.com/v8/finance/chart/{}",
                symbol
            ),
            format!(
                "https://query1.finance.yahoo.com/v8/finance/chart/{}",
                symbol
            ),
        ];

        let mut last_error = None;

        for url in endpoints {
            let response = match self
                .client
                .get(&url)
                .query(&[
                    ("interval", timeframe.yahoo_interval()),
                    ("range", timeframe.yahoo_range()),
                ])
                .send()
                .await
            {
                Ok(resp) => resp,
                Err(e) => {
                    last_error = Some(MarketError::Network(e));
                    continue;
                }
            };

            let status = response.status();
            if !status.is_success() {
                let body = response.text().await.unwrap_or_default();
                last_error = Some(MarketError::Provider {
                    status: status.as_u16(),
                    symbol: symbol.to_string(),
                    body,
                });
                continue;
            }

            let body = match response.text().await {
                Ok(b) => b,
                Err(e) => {
                    last_error = Some(MarketError::Network(e));
                    continue;
                }
            };

            let yahoo_response: YahooChartResponse = serde_json::from_str(&body)
                .map_err(|e| MarketError::Parse(format!("Yahoo JSON parse error: {e}")))?;

            return convert_to_candles(&yahoo_response, symbol);
        }

        Err(last_error.unwrap_or_else(|| MarketError::NoData {
            symbol: symbol.to_string(),
        }))
    }
}

// ---------------------------------------------------------------------------
// Yahoo-specific response types (private to this module)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct YahooChartResponse {
    chart: YahooChart,
}

#[derive(Debug, Deserialize)]
struct YahooChart {
    result: Option<Vec<YahooChartResult>>,
    error: Option<YahooError>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct YahooError {
    code: Option<String>,
    description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct YahooChartResult {
    meta: Option<YahooMeta>,
    timestamp: Option<Vec<i64>>,
    indicators: YahooIndicators,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct YahooMeta {
    regular_market_price: Option<f64>,
    chart_previous_close: Option<f64>,
    previous_close: Option<f64>,
    regular_market_change_percent: Option<f64>,
    regular_market_day_high: Option<f64>,
    regular_market_day_low: Option<f64>,
    regular_market_volume: Option<u64>,
    regular_market_time: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct YahooIndicators {
    quote: Vec<YahooQuote>,
}

#[derive(Debug, Deserialize)]
struct YahooQuote {
    open: Vec<Option<f64>>,
    high: Vec<Option<f64>>,
    low: Vec<Option<f64>>,
    close: Vec<Option<f64>>,
    volume: Vec<Option<u64>>,
}

// ---------------------------------------------------------------------------
// Conversion: Yahoo response → Vec<Candle>
// ---------------------------------------------------------------------------

/// Convert a Yahoo chart response into normalized candles.
/// Skips entries where any OHLCV field is null.
fn convert_to_candles(
    response: &YahooChartResponse,
    symbol: &str,
) -> Result<Vec<Candle>, MarketError> {
    // Check for API-level errors
    if let Some(ref err) = response.chart.error {
        let desc = err
            .description
            .as_deref()
            .unwrap_or("Unknown Yahoo API error");
        return Err(MarketError::Parse(format!(
            "Yahoo API error for '{}': {}",
            symbol, desc
        )));
    }

    let results = response
        .chart
        .result
        .as_ref()
        .ok_or_else(|| MarketError::NoData {
            symbol: symbol.to_string(),
        })?;

    let result = results.first().ok_or_else(|| MarketError::NoData {
        symbol: symbol.to_string(),
    })?;

    let timestamps = result
        .timestamp
        .as_ref()
        .ok_or_else(|| MarketError::NoData {
            symbol: symbol.to_string(),
        })?;

    let quote = result
        .indicators
        .quote
        .first()
        .ok_or_else(|| MarketError::Parse("Missing quote data in response".to_string()))?;

    // Validate array lengths match
    let len = timestamps.len();
    if quote.open.len() != len
        || quote.high.len() != len
        || quote.low.len() != len
        || quote.close.len() != len
        || quote.volume.len() != len
    {
        return Err(MarketError::Parse(
            "Mismatched array lengths in Yahoo response".to_string(),
        ));
    }

    let mut candles = Vec::with_capacity(len);

    let iter = timestamps
        .iter()
        .zip(&quote.open)
        .zip(&quote.high)
        .zip(&quote.low)
        .zip(&quote.close)
        .zip(&quote.volume)
        .filter_map(|(((((ts, o), h), l), c), v)| Some((*ts, (*o)?, (*h)?, (*l)?, (*c)?, (*v)?)));

    for (ts, open, high, low, close, volume) in iter {
        let datetime = DateTime::from_timestamp(ts, 0).unwrap_or_default();

        candles.push(Candle {
            timestamp: ts,
            datetime,
            open,
            high,
            low,
            close,
            volume,
        });
    }

    if candles.is_empty() {
        return Err(MarketError::NoData {
            symbol: symbol.to_string(),
        });
    }

    Ok(candles)
}

/// Convert a Yahoo chart response into a normalized `StockMover`.
fn convert_to_mover(
    response: &YahooChartResponse,
    symbol: &str,
    name: &str,
) -> Result<StockMover, MarketError> {
    if let Some(ref err) = response.chart.error {
        let desc = err
            .description
            .as_deref()
            .unwrap_or("Unknown Yahoo API error");
        return Err(MarketError::Parse(format!(
            "Yahoo API error for '{symbol}': {desc}"
        )));
    }

    let results = response
        .chart
        .result
        .as_ref()
        .ok_or_else(|| MarketError::NoData {
            symbol: symbol.to_string(),
        })?;

    let result = results.first().ok_or_else(|| MarketError::NoData {
        symbol: symbol.to_string(),
    })?;

    let meta = result.meta.as_ref();
    let quote = result.indicators.quote.first();

    let close_prices: Vec<f64> = quote
        .map(|q| q.close.iter().filter_map(|&c| c).collect())
        .unwrap_or_default();

    let price = meta
        .and_then(|m| m.regular_market_price)
        .or_else(|| close_prices.last().copied())
        .ok_or_else(|| MarketError::NoData {
            symbol: symbol.to_string(),
        })?;

    let prev_close = meta
        .and_then(|m| m.chart_previous_close.or(m.previous_close))
        .or_else(|| {
            if close_prices.len() >= 2 {
                Some(close_prices[close_prices.len() - 2])
            } else {
                None
            }
        })
        .unwrap_or(price);

    let change = price - prev_close;
    let change_percent = meta
        .and_then(|m| m.regular_market_change_percent)
        .unwrap_or_else(|| {
            if prev_close.abs() > 0.0001 {
                (change / prev_close) * 100.0
            } else {
                0.0
            }
        });

    let volume = meta
        .and_then(|m| m.regular_market_volume)
        .or_else(|| quote.and_then(|q| q.volume.iter().filter_map(|&v| v).next_back()))
        .unwrap_or(0);

    let day_high = meta
        .and_then(|m| m.regular_market_day_high)
        .or_else(|| quote.and_then(|q| q.high.iter().filter_map(|&h| h).next_back()))
        .unwrap_or(price);

    let day_low = meta
        .and_then(|m| m.regular_market_day_low)
        .or_else(|| quote.and_then(|q| q.low.iter().filter_map(|&l| l).next_back()))
        .unwrap_or(price);

    let timestamp = meta
        .and_then(|m| m.regular_market_time)
        .or_else(|| result.timestamp.as_ref().and_then(|ts| ts.last().copied()))
        .unwrap_or_else(|| chrono::Utc::now().timestamp());

    Ok(StockMover {
        symbol: symbol.to_string(),
        name: name.to_string(),
        price,
        prev_close,
        change,
        change_percent,
        volume,
        day_high,
        day_low,
        timestamp,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sample Yahoo Finance JSON response for testing.
    const SAMPLE_RESPONSE: &str = r#"
{
    "chart": {
        "result": [{
            "timestamp": [1700000000, 1700000900, 1700001800],
            "indicators": {
                "quote": [{
                    "open": [100.0, 101.0, 102.0],
                    "high": [101.5, 102.5, 103.0],
                    "low": [99.5, 100.5, 101.0],
                    "close": [101.0, 102.0, 102.5],
                    "volume": [1000000, 1500000, 1200000]
                }]
            }
        }],
        "error": null
    }
}
"#;

    /// Response with some null values (common in Yahoo data).
    const RESPONSE_WITH_NULLS: &str = r#"
{
    "chart": {
        "result": [{
            "timestamp": [1700000000, 1700000900, 1700001800],
            "indicators": {
                "quote": [{
                    "open": [100.0, null, 102.0],
                    "high": [101.5, null, 103.0],
                    "low": [99.5, null, 101.0],
                    "close": [101.0, null, 102.5],
                    "volume": [1000000, null, 1200000]
                }]
            }
        }],
        "error": null
    }
}
"#;

    /// Error response from Yahoo.
    const ERROR_RESPONSE: &str = r#"
{
    "chart": {
        "result": null,
        "error": {
            "code": "Not Found",
            "description": "No data found, symbol may be delisted"
        }
    }
}
"#;

    /// Empty result (no timestamps).
    const EMPTY_RESPONSE: &str = r#"
{
    "chart": {
        "result": [{
            "timestamp": null,
            "indicators": {
                "quote": [{
                    "open": [],
                    "high": [],
                    "low": [],
                    "close": [],
                    "volume": []
                }]
            }
        }],
        "error": null
    }
}
"#;

    #[test]
    fn parse_valid_response() {
        let resp: YahooChartResponse = serde_json::from_str(SAMPLE_RESPONSE).unwrap();
        let candles = convert_to_candles(&resp, "TEST.NS").unwrap();

        assert_eq!(candles.len(), 3);
        assert_eq!(candles[0].open, 100.0);
        assert_eq!(candles[0].close, 101.0);
        assert_eq!(candles[0].volume, 1_000_000);
        assert_eq!(candles[2].high, 103.0);
    }

    #[test]
    fn skip_null_values() {
        let resp: YahooChartResponse = serde_json::from_str(RESPONSE_WITH_NULLS).unwrap();
        let candles = convert_to_candles(&resp, "TEST.NS").unwrap();

        // Middle candle (all nulls) should be skipped
        assert_eq!(candles.len(), 2);
        assert_eq!(candles[0].timestamp, 1_700_000_000);
        assert_eq!(candles[1].timestamp, 1_700_001_800);
    }

    #[test]
    fn handle_error_response() {
        let resp: YahooChartResponse = serde_json::from_str(ERROR_RESPONSE).unwrap();
        let result = convert_to_candles(&resp, "INVALID.NS");

        assert!(result.is_err());
        let err = result.unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("No data found"));
    }

    #[test]
    fn handle_empty_timestamps() {
        let resp: YahooChartResponse = serde_json::from_str(EMPTY_RESPONSE).unwrap();
        let result = convert_to_candles(&resp, "EMPTY.NS");

        assert!(matches!(result, Err(MarketError::NoData { .. })));
    }

    #[test]
    fn candles_are_ordered() {
        let resp: YahooChartResponse = serde_json::from_str(SAMPLE_RESPONSE).unwrap();
        let candles = convert_to_candles(&resp, "TEST.NS").unwrap();

        for window in candles.windows(2) {
            assert!(window[0].timestamp <= window[1].timestamp);
        }
    }

    #[test]
    fn convert_to_mover_with_meta() {
        const META_RESPONSE: &str = r#"
        {
            "chart": {
                "result": [{
                    "meta": {
                        "regularMarketPrice": 1250.0,
                        "chartPreviousClose": 1200.0,
                        "regularMarketChangePercent": 4.166,
                        "regularMarketVolume": 5000000,
                        "regularMarketDayHigh": 1260.0,
                        "regularMarketDayLow": 1195.0,
                        "regularMarketTime": 1700000000
                    },
                    "timestamp": [1700000000],
                    "indicators": {
                        "quote": [{
                            "open": [1210.0],
                            "high": [1260.0],
                            "low": [1195.0],
                            "close": [1250.0],
                            "volume": [5000000]
                        }]
                    }
                }],
                "error": null
            }
        }
        "#;
        let resp: YahooChartResponse = serde_json::from_str(META_RESPONSE).unwrap();
        let mover = convert_to_mover(&resp, "RELIANCE.NS", "Reliance Industries").unwrap();
        assert_eq!(mover.symbol, "RELIANCE.NS");
        assert_eq!(mover.price, 1250.0);
        assert_eq!(mover.prev_close, 1200.0);
        assert_eq!(mover.change, 50.0);
        assert!((mover.change_percent - 4.166).abs() < 0.001);
        assert!(mover.matches_threshold(3.0));
        assert!(mover.is_gainer());
    }
}
