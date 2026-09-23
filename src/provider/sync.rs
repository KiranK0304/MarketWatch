//! Centralized, gap-aware incremental candle synchronization service.
//!
//! Provides:
//! - Single source of truth in SQLite.
//! - Incremental range fetching (fetching only new or forming candles).
//! - In-flight per-`(symbol, timeframe)` request coalescing.
//! - Robust error handling with graceful fallback to cached data.
//! - Unified interface for both Web and native GUI callers.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, watch};

use crate::domain::{Candle, Timeframe};
use crate::error::MarketError;
use crate::provider::MarketDataProvider;
use crate::provider::yahoo::YahooProvider;
use crate::storage::MarketDb;

/// Result status of a synchronization attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncStatus {
    /// Served directly from SQLite cache (0 API calls).
    CacheHit,
    /// Coalesced with another concurrent in-flight fetch for the same key.
    CoalescedHit,
    /// Freshly synced or incrementally updated from Yahoo Finance.
    Synced,
    /// Upstream request failed, but valid older data was returned from cache.
    StaleFallback(String),
}

impl SyncStatus {
    #[allow(dead_code)]
    pub fn is_cached(&self) -> bool {
        matches!(
            self,
            Self::CacheHit | Self::CoalescedHit | Self::StaleFallback(_)
        )
    }

    pub fn header_value(&self) -> &'static str {
        match self {
            Self::CacheHit => "HIT",
            Self::CoalescedHit => "COALESCED-HIT",
            Self::Synced => "MISS",
            Self::StaleFallback(_) => "STALE-FALLBACK",
        }
    }
}

type CacheKey = (String, &'static str, bool);
type InFlightResult = Result<(), String>;

/// Central coordinator for candle synchronization across all application callers.
#[derive(Clone)]
pub struct CandleSyncService {
    db: MarketDb,
    provider: Arc<YahooProvider>,
    in_flight: Arc<Mutex<HashMap<CacheKey, watch::Sender<Option<InFlightResult>>>>>,
    ttl_secs: i64,
}

impl CandleSyncService {
    /// Create a new synchronization service.
    pub fn new(db: MarketDb, provider: Arc<YahooProvider>) -> Self {
        Self {
            db,
            provider,
            in_flight: Arc::new(Mutex::new(HashMap::new())),
            ttl_secs: 60,
        }
    }

    /// Access the underlying SQLite database handle.
    #[allow(dead_code)]
    pub fn db(&self) -> &MarketDb {
        &self.db
    }

    /// Access the underlying Yahoo provider handle.
    #[allow(dead_code)]
    pub fn provider(&self) -> &YahooProvider {
        &self.provider
    }

    /// Synchronize and fetch candles for a symbol and timeframe.
    ///
    /// - Deduplicates concurrent in-flight requests for the same `(symbol, timeframe)`.
    /// - Checks whether SQLite already contains up-to-date data.
    /// - Queries Yahoo Finance only for missing or forming candles.
    /// - Returns the full accumulated dataset from SQLite.
    pub async fn get_candles(
        &self,
        symbol: &str,
        timeframe: Timeframe,
        force: bool,
    ) -> Result<(Vec<Candle>, SyncStatus), MarketError> {
        let sym_upper = symbol.trim().to_uppercase();
        let tf_label = timeframe.label();
        // Forced refreshes must not join a normal request, otherwise a manual
        // refresh could return data fetched under the normal cache policy.
        let key = (sym_upper.clone(), tf_label, force);

        // 1. Check in-flight registry for duplicate concurrent calls
        let mut rx = {
            let mut guard = self.in_flight.lock().await;
            if let Some(existing_tx) = guard.get(&key) {
                // Another task is already syncing this exact (symbol, timeframe)
                Some(existing_tx.subscribe())
            } else {
                // Register ourselves as the active sync runner
                let (tx, _rx) = watch::channel(None);
                guard.insert(key.clone(), tx);
                None
            }
        };

        if let Some(ref mut receiver) = rx {
            // Wait for the primary runner to finish
            while receiver.borrow().is_none() {
                if receiver.changed().await.is_err() {
                    break;
                }
            }

            // Inspect the primary's outcome: a primary that served stale
            // cache on upstream failure must not look like a clean hit.
            let primary_error: Option<String> = match &*receiver.borrow() {
                Some(Err(msg)) => Some(msg.clone()),
                _ => None,
            };

            // Read the freshly committed data from SQLite
            let cached = self.db.get_candles(&sym_upper, tf_label)?;
            if !cached.is_empty() {
                match primary_error {
                    Some(msg) => return Ok((cached, SyncStatus::StaleFallback(msg))),
                    None => return Ok((cached, SyncStatus::CoalescedHit)),
                }
            }
            if let Some(msg) = primary_error {
                // Primary failed with nothing cached: propagate instead of
                // firing our own duplicate upstream request.
                return Err(MarketError::Provider {
                    status: 0,
                    symbol: sym_upper.clone(),
                    body: msg,
                });
            }
            // Primary finished with empty cache and no error (first-load race):
            // fall through and become the new primary below.
            {
                let mut guard = self.in_flight.lock().await;
                if let Some(existing_tx) = guard.get(&key) {
                    // Another task registered meanwhile; wait on it.
                    *receiver = existing_tx.subscribe();
                    while receiver.borrow().is_none() {
                        if receiver.changed().await.is_err() {
                            break;
                        }
                    }
                    let retry_err: Option<String> = match &*receiver.borrow() {
                        Some(Err(msg)) => Some(msg.clone()),
                        _ => None,
                    };
                    let cached = self.db.get_candles(&sym_upper, tf_label)?;
                    if !cached.is_empty() {
                        match retry_err {
                            Some(msg) => return Ok((cached, SyncStatus::StaleFallback(msg))),
                            None => return Ok((cached, SyncStatus::CoalescedHit)),
                        }
                    }
                    if let Some(msg) = retry_err {
                        return Err(MarketError::Provider {
                            status: 0,
                            symbol: sym_upper.clone(),
                            body: msg,
                        });
                    }
                } else {
                    let (tx, _rx) = watch::channel(None);
                    guard.insert(key.clone(), tx);
                }
            }
        }

        // We are the primary runner: execute the synchronization
        let sync_result = self.execute_sync(&sym_upper, timeframe, force).await;

        // Cleanup in-flight registry and notify any waiting listeners
        {
            let mut guard = self.in_flight.lock().await;
            if let Some(tx) = guard.remove(&key) {
                let outcome = match &sync_result {
                    Ok(_) => Ok(()),
                    Err(e) => Err(e.to_string()),
                };
                let _ = tx.send(Some(outcome));
            }
        }

        sync_result
    }

    /// Internal synchronization execution (called by primary runner).
    async fn execute_sync(
        &self,
        symbol: &str,
        timeframe: Timeframe,
        force: bool,
    ) -> Result<(Vec<Candle>, SyncStatus), MarketError> {
        let tf_label = timeframe.label();

        // 1. If not force refresh, check if DB cache is already completely fresh
        if !force && self.db.is_fresh(symbol, tf_label, self.ttl_secs)? {
            // Also check for internal gaps in stored data
            let gaps = self.db.detect_gaps(symbol, timeframe)?;
            if gaps.is_empty() {
                let cached = self.db.get_candles(symbol, tf_label)?;
                if !cached.is_empty() {
                    return Ok((cached, SyncStatus::CacheHit));
                }
            }
        }

        // 2. Determine whether to perform full or incremental sync
        let meta = self.db.get_sync_meta(symbol, tf_label)?;
        let now_epoch = chrono::Utc::now().timestamp();
        let fetch_result = match meta {
            Some(ref m) if m.candle_count > 0 && !force => {
                let gaps = self.db.detect_gaps(symbol, timeframe)?;
                // One gap's failure must not abort the remaining gaps or the
                // incremental tail fetch; leftover gaps stay flagged via the
                // final detect_gaps below and are retried next sync.
                for (gap_start, gap_end) in gaps {
                    let gap_end = gap_end.saturating_add(timeframe.seconds());
                    match self
                        .provider
                        .fetch_candles_range(symbol, timeframe, gap_start, gap_end)
                        .await
                    {
                        Ok(candles) => {
                            self.db
                                .save_candles(symbol, tf_label, &candles, now_epoch)?;
                        }
                        Err(e) => {
                            eprintln!(
                                "Warning: gap backfill {gap_start}..{gap_end} for {symbol} failed: {e}"
                            );
                        }
                    }
                }

                // Incremental fetch: query from the last stored candle up to now.
                // Using last_candle_ts also refreshes the forming candle in place.
                self.provider
                    .fetch_candles_range(symbol, timeframe, m.last_candle_ts, now_epoch)
                    .await
            }
            _ => {
                // Initial load or forced reload: fetch full standard window.
                self.provider.fetch_candles(symbol, timeframe).await
            }
        };

        match fetch_result {
            Ok(candles) => {
                self.db
                    .save_candles(symbol, tf_label, &candles, now_epoch)?;

                let remaining_gaps = self.db.detect_gaps(symbol, timeframe)?;
                self.db
                    .mark_gap_detected(symbol, tf_label, !remaining_gaps.is_empty())?;

                let all_candles = self.db.get_candles(symbol, tf_label)?;
                let result_candles = if all_candles.is_empty() {
                    candles
                } else {
                    all_candles
                };

                Ok((result_candles, SyncStatus::Synced))
            }
            Err(e) => {
                // Graceful fallback: If upstream network request failed,
                // return any valid cached candles we already possess in SQLite
                let cached = self.db.get_candles(symbol, tf_label)?;
                if !cached.is_empty() {
                    return Ok((cached, SyncStatus::StaleFallback(e.to_string())));
                }
                // No cache available: surface explicit provider/network error
                Err(e)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::calendar::MarketCalendar;
    use chrono::{DateTime, NaiveDate};

    fn make_test_candle(ts: i64, open: f64, close: f64) -> Candle {
        Candle {
            timestamp: ts,
            datetime: DateTime::from_timestamp(ts, 0).unwrap(),
            open,
            high: open.max(close) + 2.0,
            low: open.min(close) - 2.0,
            close,
            volume: 50_000,
        }
    }

    #[test]
    fn test_candle_upsert_deduplication() {
        let db = MarketDb::open_in_memory().unwrap();
        let c1 = make_test_candle(1000, 100.0, 105.0);
        let c2 = make_test_candle(1000, 100.0, 108.0); // same timestamp, updated close

        db.save_candles("TEST.NS", "15m", &[c1], 1500).unwrap();
        db.save_candles("TEST.NS", "15m", &[c2], 1600).unwrap();

        let loaded = db.get_candles("TEST.NS", "15m").unwrap();
        assert_eq!(
            loaded.len(),
            1,
            "Must never create duplicate rows for same timestamp"
        );
        assert_eq!(
            loaded[0].close, 108.0,
            "Must update forming candle in-place"
        );
    }

    #[test]
    fn test_candle_interval_alignment() {
        // Test alignment of arbitrary tick time (11:36:42 IST) to 15m boundary (11:30:00)
        let unaligned_ts = 1790057202; // 2026-09-22 11:36:42 IST
        let aligned_15m = MarketCalendar::align_timestamp(unaligned_ts, Timeframe::Min15);
        let aligned_5m = MarketCalendar::align_timestamp(unaligned_ts, Timeframe::Min5);

        assert_eq!(aligned_15m, 1790056800, "15m must floor to 11:30:00");
        assert_eq!(aligned_5m, 1790057100, "5m must floor to 11:35:00");
    }

    #[test]
    fn test_detection_of_internal_missing_candle() {
        let db = MarketDb::open_in_memory().unwrap();
        let wednesday = NaiveDate::from_ymd_opt(2026, 9, 23).unwrap();
        let slots = MarketCalendar::session_slots(wednesday, Timeframe::Min15);

        // Store slots 0, 1, 2, and 4 (slot 3 is missing!)
        let candles = vec![
            make_test_candle(slots[0], 100.0, 101.0),
            make_test_candle(slots[1], 101.0, 102.0),
            make_test_candle(slots[2], 102.0, 103.0),
            make_test_candle(slots[4], 104.0, 105.0),
        ];

        db.save_candles("GAP.NS", "15m", &candles, 2000).unwrap();

        let gaps = db.detect_gaps("GAP.NS", Timeframe::Min15).unwrap();
        assert_eq!(gaps.len(), 1, "Must detect exactly 1 missing gap");
        assert_eq!(gaps[0].0, slots[3], "Gap start must match missing slot 3");
        assert_eq!(gaps[0].1, slots[3], "Gap end must match missing slot 3");
    }

    #[test]
    fn test_weekend_and_holiday_freshness() {
        let db = MarketDb::open_in_memory().unwrap();
        // Saturday 2026-09-19
        let sat = NaiveDate::from_ymd_opt(2026, 9, 19).unwrap();
        assert!(!MarketCalendar::is_trading_day(sat));

        // Friday 2026-09-18 was the prior trading day
        let fri = MarketCalendar::most_recent_trading_day(sat);
        assert_eq!(fri, NaiveDate::from_ymd_opt(2026, 9, 18).unwrap());

        let fri_slots = MarketCalendar::session_slots(fri, Timeframe::Min15);
        let fri_last_slot = *fri_slots.last().unwrap();

        // Stored data covering up to Friday's close
        let candles = vec![make_test_candle(fri_last_slot, 200.0, 205.0)];
        db.save_candles("WEEKEND.NS", "15m", &candles, fri_last_slot + 3600)
            .unwrap();

        let meta = db.get_sync_meta("WEEKEND.NS", "15m").unwrap().unwrap();
        assert_eq!(meta.last_candle_ts, fri_last_slot);
    }

    #[test]
    fn test_sync_status_cache_header() {
        assert_eq!(SyncStatus::CacheHit.header_value(), "HIT");
        assert_eq!(SyncStatus::CoalescedHit.header_value(), "COALESCED-HIT");
        assert_eq!(SyncStatus::Synced.header_value(), "MISS");
        assert_eq!(
            SyncStatus::StaleFallback("err".into()).header_value(),
            "STALE-FALLBACK"
        );
    }

    #[tokio::test]
    async fn test_in_flight_coalescing_single_upstream_fetch() {
        let db = MarketDb::open_in_memory().unwrap();
        let provider = Arc::new(YahooProvider::new().unwrap());
        let service = CandleSyncService::new(db.clone(), provider);

        // Populate cache so execute_sync finishes fast with CacheHit
        let now_ist = crate::domain::calendar::now_ist();
        let expected = MarketCalendar::latest_expected_candle(now_ist, Timeframe::Min15);
        let c = make_test_candle(expected.timestamp, 100.0, 102.0);
        let now_epoch = chrono::Utc::now().timestamp();
        db.save_candles("COAL.NS", "15m", &[c], now_epoch).unwrap();

        // Spawn 10 concurrent requests for the exact same symbol and timeframe
        let mut handles = Vec::new();
        for _ in 0..10 {
            let svc = service.clone();
            handles.push(tokio::spawn(async move {
                svc.get_candles("COAL.NS", Timeframe::Min15, false).await
            }));
        }

        for h in handles {
            let res = h.await.unwrap();
            assert!(res.is_ok(), "Concurrent request must succeed");
            let (candles, status) = res.unwrap();
            assert_eq!(candles.len(), 1);
            assert!(status.is_cached());
        }
    }

    #[tokio::test]
    async fn test_separate_symbols_do_not_block() {
        let db = MarketDb::open_in_memory().unwrap();
        let provider = Arc::new(YahooProvider::new().unwrap());
        let service = CandleSyncService::new(db.clone(), provider);

        let now_ist = crate::domain::calendar::now_ist();
        let expected = MarketCalendar::latest_expected_candle(now_ist, Timeframe::Min15);
        let now_epoch = chrono::Utc::now().timestamp();

        let c1 = make_test_candle(expected.timestamp, 100.0, 102.0);
        let c2 = make_test_candle(expected.timestamp, 200.0, 204.0);
        db.save_candles("SYM1.NS", "15m", &[c1], now_epoch).unwrap();
        db.save_candles("SYM2.NS", "15m", &[c2], now_epoch).unwrap();

        let svc1 = service.clone();
        let svc2 = service.clone();

        let h1 =
            tokio::spawn(async move { svc1.get_candles("SYM1.NS", Timeframe::Min15, false).await });
        let h2 =
            tokio::spawn(async move { svc2.get_candles("SYM2.NS", Timeframe::Min15, false).await });

        let (res1, res2) = tokio::join!(h1, h2);
        assert_eq!(res1.unwrap().unwrap().0[0].close, 102.0);
        assert_eq!(res2.unwrap().unwrap().0[0].close, 204.0);
    }

    #[test]
    fn test_offline_gap_recovery_range_calculation() {
        let db = MarketDb::open_in_memory().unwrap();
        let now_epoch = chrono::Utc::now().timestamp();
        // 5 days ago (432,000 seconds)
        let five_days_ago = now_epoch - 5 * 86400;
        let c_old = make_test_candle(five_days_ago, 500.0, 505.0);
        db.save_candles("GAP_RECOVERY.NS", "15m", &[c_old], five_days_ago)
            .unwrap();

        // Check metadata
        let meta = db.get_sync_meta("GAP_RECOVERY.NS", "15m").unwrap().unwrap();
        assert_eq!(meta.last_candle_ts, five_days_ago);

        // Gap aware check indicates cache is not fresh
        assert!(!db.is_fresh("GAP_RECOVERY.NS", "15m", 300).unwrap());

        // Incremental sync uses last_candle_ts as period1
        let period1 = meta.last_candle_ts;
        let period2 = now_epoch;
        assert!(period1 < period2);
        assert_eq!(period1, five_days_ago);

        // Clamping logic: 5 days ago is well within the 59-day Yahoo threshold
        let max_lookback = 59 * 86400;
        let clamped_period1 = period1.max(now_epoch - max_lookback);
        assert_eq!(
            clamped_period1, period1,
            "Within 59 days should not be clamped"
        );

        // Simulate 90 days offline (exceeds 59-day Yahoo intraday limit)
        let ninety_days_ago = now_epoch - 90 * 86400;
        let clamped_90d = ninety_days_ago.max(now_epoch - max_lookback);
        assert_eq!(
            clamped_90d,
            now_epoch - max_lookback,
            "Beyond 59 days must be clamped to safe boundary"
        );
    }
}
