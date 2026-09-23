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

/// Validate a ticker symbol before interpolating it into a request URL.
///
/// Only Yahoo ticker characters are allowed; anything else (`?`, `&`, `#`,
/// `/`, whitespace, …) would alter the request path/query.
fn validate_symbol(symbol: &str) -> Result<(), MarketError> {
    let ok = !symbol.is_empty()
        && symbol.len() <= 32
        && symbol
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '^' | '='));
    if ok {
        Ok(())
    } else {
        Err(MarketError::InvalidSymbol(symbol.to_string()))
    }
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
        validate_symbol(symbol)?;
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

            let yahoo_response: YahooChartResponse = match serde_json::from_str(&body) {
                Ok(r) => r,
                Err(e) => {
                    // A truncated/garbage 200 body from query2 should still
                    // fall through to query1 instead of aborting the loop.
                    last_error = Some(MarketError::Parse(format!("Yahoo JSON parse error: {e}")));
                    continue;
                }
            };

            return convert_to_mover(&yahoo_response, symbol, name);
        }

        Err(last_error.unwrap_or_else(|| MarketError::NoData {
            symbol: symbol.to_string(),
        }))
    }
    /// Fetch OHLCV candle data for a symbol and timeframe within an explicit timestamp range.
    ///
    /// Automatically clamps queries to Yahoo Finance's interval limits (e.g. 59 days for 5m/15m).
    pub async fn fetch_candles_range(
        &self,
        symbol: &str,
        timeframe: Timeframe,
        period1: i64,
        period2: i64,
    ) -> Result<Vec<Candle>, MarketError> {
        validate_symbol(symbol)?;
        if period2 < period1 {
            return Err(MarketError::InvalidInput(format!(
                "Invalid range for '{symbol}': period2 ({period2}) < period1 ({period1})"
            )));
        }
        let now_epoch = chrono::Utc::now().timestamp();
        let clamped_period1 = match timeframe {
            Timeframe::Min5 | Timeframe::Min15 => period1.max(now_epoch - 59 * 86400),
            Timeframe::Min30 | Timeframe::Hour1 => period1.max(now_epoch - 720 * 86400),
            Timeframe::Day1 | Timeframe::Week1 => period1,
        };
        if clamped_period1 != period1 {
            // Surface the truncation: callers (gap recovery) treat a short
            // response as a filled gap, so history older than the clamp would
            // otherwise be lost without any signal.
            eprintln!(
                "Warning: Yahoo only serves ~{} of history for {timeframe}; requested start {} truncated to {} for '{symbol}'",
                match timeframe {
                    Timeframe::Min5 | Timeframe::Min15 => "59 days",
                    _ => "720 days",
                },
                period1,
                clamped_period1,
            );
        }
        let clamped_period2 = period2.max(clamped_period1 + 60);

        let p1_str = clamped_period1.to_string();
        let p2_str = clamped_period2.to_string();
        let params = [
            ("interval", timeframe.yahoo_interval()),
            ("period1", p1_str.as_str()),
            ("period2", p2_str.as_str()),
        ];

        self.execute_chart_request(symbol, timeframe, &params).await
    }

    /// Internal HTTP execution loop with query2/query1 endpoint fallback.
    async fn execute_chart_request(
        &self,
        symbol: &str,
        timeframe: Timeframe,
        query_params: &[(&str, &str)],
    ) -> Result<Vec<Candle>, MarketError> {
        validate_symbol(symbol)?;
        let endpoints = [
            format!("https://query2.finance.yahoo.com/v8/finance/chart/{symbol}"),
            format!("https://query1.finance.yahoo.com/v8/finance/chart/{symbol}"),
        ];

        let mut last_error = None;

        for url in endpoints {
            let response = match self.client.get(&url).query(query_params).send().await {
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

            let yahoo_response: YahooChartResponse = match serde_json::from_str(&body) {
                Ok(r) => r,
                Err(e) => {
                    last_error = Some(MarketError::Parse(format!("Yahoo JSON parse error: {e}")));
                    continue;
                }
            };

            return convert_to_candles_aligned(&yahoo_response, symbol, Some(timeframe));
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
        let params = [
            ("interval", timeframe.yahoo_interval()),
            ("range", timeframe.yahoo_range()),
        ];
        self.execute_chart_request(symbol, timeframe, &params).await
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

/// Convert a Yahoo chart response into normalized candles (without alignment, for test backward compatibility).
#[cfg(test)]
fn convert_to_candles(
    response: &YahooChartResponse,
    symbol: &str,
) -> Result<Vec<Candle>, MarketError> {
    convert_to_candles_aligned(response, symbol, None)
}

/// Convert a Yahoo chart response into normalized candles, optionally aligning timestamps to timeframe boundaries.
fn convert_to_candles_aligned(
    response: &YahooChartResponse,
    symbol: &str,
    timeframe: Option<Timeframe>,
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

    // Null volume is common on forming/pre-market ticks: keep the price candle
    // with volume 0 instead of discarding it (which created artificial gaps).
    // Null OHLC still discards the entry.
    let iter = timestamps
        .iter()
        .zip(&quote.open)
        .zip(&quote.high)
        .zip(&quote.low)
        .zip(&quote.close)
        .zip(&quote.volume)
        .filter_map(|(((((ts, o), h), l), c), v)| {
            Some((*ts, (*o)?, (*h)?, (*l)?, (*c)?, v.unwrap_or(0)))
        });

    for (raw_ts, open, high, low, close, volume) in iter {
        if raw_ts <= 0 {
            continue;
        }
        let ts = if let Some(tf) = timeframe {
            crate::domain::calendar::MarketCalendar::align_timestamp(raw_ts, tf)
        } else {
            raw_ts
        };
        // Never let an out-of-range network timestamp become a 1970 epoch
        // candle and poison first_candle_ts / gap detection.
        let datetime = match DateTime::from_timestamp(ts, 0) {
            Some(d) => d,
            None => continue,
        };

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

    // Sort by timestamp and deduplicate multiple ticks that floored to the exact same boundary slot
    candles.sort_by_key(|c| c.timestamp);
    let mut deduped: Vec<Candle> = Vec::with_capacity(candles.len());
    for c in candles {
        if let Some(last) = deduped.last_mut().filter(|l| l.timestamp == c.timestamp) {
            last.high = last.high.max(c.high);
            last.low = last.low.min(c.low);
            last.close = c.close;
            last.volume = last.volume.saturating_add(c.volume);
            continue;
        }
        deduped.push(c);
    }

    Ok(deduped)
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
        });

    let meta_change_percent = meta.and_then(|m| m.regular_market_change_percent);

    // Keep price/prev_close/change_percent internally consistent:
    // - if prev_close is missing but Yahoo gave a percent, back it out;
    // - if both are missing, there is no usable quote (NoData) rather than a
    //   synthetic flat 0% mover that scanners silently ignore.
    let (prev_close, change_percent) = match (prev_close, meta_change_percent) {
        (Some(pc), _) if pc.abs() > 0.0001 => {
            let change = price - pc;
            let pct = meta_change_percent.unwrap_or((change / pc) * 100.0);
            (pc, pct)
        }
        (Some(pc), _) => (pc, meta_change_percent.unwrap_or(0.0)),
        (None, Some(pct)) => {
            let pc = if (100.0 + pct).abs() > 0.0001 {
                price / (1.0 + pct / 100.0)
            } else {
                price
            };
            (pc, pct)
        }
        (None, None) => {
            return Err(MarketError::NoData {
                symbol: symbol.to_string(),
            });
        }
    };

    let change = price - prev_close;

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
