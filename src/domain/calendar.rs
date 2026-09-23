//! Indian stock exchange (NSE/BSE) trading calendar and session scheduling.
//!
//! Handles:
//! - Indian Standard Time (IST, UTC+05:30) conversion.
//! - Market hours (09:15 AM to 03:30 PM IST).
//! - Official NSE trading holidays (2024–2027) with database override support.
//! - Timeframe slot boundary generation and timestamp alignment.
//! - Expected latest candle calculation and internal gap detection.

use chrono::{DateTime, Datelike, Duration, FixedOffset, NaiveDate, Timelike, Utc, Weekday};
use std::collections::HashSet;

use crate::domain::Timeframe;

/// Standard market open time: 09:15 AM IST
pub const MARKET_OPEN_HOUR: u32 = 9;
pub const MARKET_OPEN_MINUTE: u32 = 15;
pub const MARKET_OPEN_MINUTES_TOTAL: u32 = MARKET_OPEN_HOUR * 60 + MARKET_OPEN_MINUTE; // 555 min

/// Standard market close time: 03:30 PM IST
pub const MARKET_CLOSE_HOUR: u32 = 15;
pub const MARKET_CLOSE_MINUTE: u32 = 30;
pub const MARKET_CLOSE_MINUTES_TOTAL: u32 = MARKET_CLOSE_HOUR * 60 + MARKET_CLOSE_MINUTE; // 930 min

/// Return the fixed timezone offset for Indian Standard Time (UTC+05:30).
pub fn ist_offset() -> FixedOffset {
    FixedOffset::east_opt(5 * 3600 + 30 * 60).expect("Valid +05:30 offset")
}

/// Get the current date and time in Indian Standard Time (IST).
pub fn now_ist() -> DateTime<FixedOffset> {
    Utc::now().with_timezone(&ist_offset())
}

/// Expected latest candle state at a given point in time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExpectedCandle {
    /// Unix timestamp in seconds (aligned to timeframe slot boundary).
    pub timestamp: i64,
    /// Whether this candle is currently forming (in-progress trade session).
    pub is_forming: bool,
}

/// Indian Market Trading Calendar.
pub struct MarketCalendar;

impl MarketCalendar {
    /// Return the static set of known official NSE trading holidays (YYYY-MM-DD).
    pub fn standard_holidays() -> HashSet<&'static str> {
        let mut h = HashSet::new();
        // 2024
        h.insert("2024-01-22"); // Special Ram Mandir Pran Pratishtha
        h.insert("2024-01-26"); // Republic Day
        h.insert("2024-03-08"); // Mahashivratri
        h.insert("2024-03-25"); // Holi
        h.insert("2024-03-29"); // Good Friday
        h.insert("2024-04-11"); // Id-Ul-Fitr
        h.insert("2024-04-17"); // Ram Navami
        h.insert("2024-05-01"); // Maharashtra Day
        h.insert("2024-05-20"); // General Election
        h.insert("2024-06-17"); // Bakri Id
        h.insert("2024-07-17"); // Muharram
        h.insert("2024-08-15"); // Independence Day
        h.insert("2024-10-02"); // Mahatma Gandhi Jayanti
        h.insert("2024-11-01"); // Diwali Laxmi Pujan
        h.insert("2024-11-15"); // Gurunanak Jayanti
        h.insert("2024-11-20"); // Assembly Election
        h.insert("2024-12-25"); // Christmas

        // 2025
        h.insert("2025-02-26"); // Mahashivratri
        h.insert("2025-03-14"); // Holi
        h.insert("2025-03-31"); // Id-Ul-Fitr
        h.insert("2025-04-10"); // Mahavir Jayanti
        h.insert("2025-04-14"); // Dr. Ambedkar Jayanti
        h.insert("2025-04-18"); // Good Friday
        h.insert("2025-05-01"); // Maharashtra Day
        h.insert("2025-08-15"); // Independence Day
        h.insert("2025-08-27"); // Ganesh Chaturthi
        h.insert("2025-10-02"); // Mahatma Gandhi Jayanti / Dussehra
        h.insert("2025-10-21"); // Diwali Laxmi Pujan
        h.insert("2025-10-22"); // Diwali Balipratipada
        h.insert("2025-11-05"); // Gurunanak Jayanti
        h.insert("2025-12-25"); // Christmas

        // 2026
        h.insert("2026-01-26"); // Republic Day (Mon)
        h.insert("2026-03-03"); // Holi (Tue)
        h.insert("2026-03-20"); // Id-Ul-Fitr (Fri)
        h.insert("2026-03-31"); // Mahavir Jayanti (Tue)
        h.insert("2026-04-03"); // Good Friday (Fri)
        h.insert("2026-04-14"); // Dr. Ambedkar Jayanti (Tue)
        h.insert("2026-05-01"); // Maharashtra Day (Fri)
        h.insert("2026-05-27"); // Bakri Id (Wed)
        h.insert("2026-06-25"); // Muharram (Thu)
        h.insert("2026-09-14"); // Ganesh Chaturthi (Mon)
        h.insert("2026-10-02"); // Mahatma Gandhi Jayanti (Fri)
        h.insert("2026-10-20"); // Dussehra (Tue)
        h.insert("2026-11-24"); // Gurunanak Jayanti (Tue)
        h.insert("2026-12-25"); // Christmas (Fri)

        // 2027
        h.insert("2027-01-26"); // Republic Day (Tue)
        h.insert("2027-03-22"); // Holi (Mon)
        h.insert("2027-03-26"); // Good Friday (Fri)
        h.insert("2027-04-14"); // Dr. Ambedkar Jayanti (Wed)
        h.insert("2027-10-29"); // Diwali (Fri)

        h
    }

    /// Check if a date is a recognized trading day (Monday–Friday and not an official holiday).
    pub fn is_trading_day(date: NaiveDate) -> bool {
        let weekday = date.weekday();
        if matches!(weekday, Weekday::Sat | Weekday::Sun) {
            return false;
        }

        let date_str = date.format("%Y-%m-%d").to_string();
        !Self::standard_holidays().contains(date_str.as_str())
    }

    /// Check if the market is open at the given timestamp in IST.
    #[allow(dead_code)]
    pub fn is_market_open(now_ist: DateTime<FixedOffset>) -> bool {
        let date = now_ist.date_naive();
        if !Self::is_trading_day(date) {
            return false;
        }

        let current_minutes = now_ist.hour() * 60 + now_ist.minute();
        (MARKET_OPEN_MINUTES_TOTAL..MARKET_CLOSE_MINUTES_TOTAL).contains(&current_minutes)
    }

    /// Find the most recent trading day on or before `date`.
    pub fn most_recent_trading_day(mut date: NaiveDate) -> NaiveDate {
        while !Self::is_trading_day(date) {
            date -= Duration::days(1);
        }
        date
    }

    /// Find the prior trading day strictly before `date`.
    pub fn previous_trading_day(mut date: NaiveDate) -> NaiveDate {
        date -= Duration::days(1);
        while !Self::is_trading_day(date) {
            date -= Duration::days(1);
        }
        date
    }

    /// Find the next trading day strictly after `date`.
    #[allow(dead_code)]
    pub fn next_trading_day(mut date: NaiveDate) -> NaiveDate {
        date += Duration::days(1);
        while !Self::is_trading_day(date) {
            date += Duration::days(1);
        }
        date
    }

    /// Generate all expected candle slot start timestamps (in ascending order)
    /// for a single full trading session on `date`.
    pub fn session_slots(date: NaiveDate, tf: Timeframe) -> Vec<i64> {
        if !Self::is_trading_day(date) {
            return Vec::new();
        }

        let offset = ist_offset();
        let open_dt = date
            .and_hms_opt(MARKET_OPEN_HOUR, MARKET_OPEN_MINUTE, 0)
            .expect("Valid 09:15");
        let open_dt_ist =
            DateTime::<FixedOffset>::from_naive_utc_and_offset(open_dt - offset, offset);
        let open_ts = open_dt_ist.timestamp();

        match tf {
            Timeframe::Min5 => {
                // 75 slots: 09:15, 09:20, ..., 15:25
                (0..75).map(|k| open_ts + k * 300).collect()
            }
            Timeframe::Min15 => {
                // 25 slots: 09:15, 09:30, ..., 15:15
                (0..25).map(|k| open_ts + k * 900).collect()
            }
            Timeframe::Min30 => {
                // 13 slots: 09:15, 09:45, ..., 14:45 (12 slots) + 15:15 (last 15m slot)
                let mut slots: Vec<i64> = (0..12).map(|k| open_ts + k * 1800).collect();
                // 15:15 slot = 09:15 + 6 hours = 09:15 + 21600
                slots.push(open_ts + 6 * 3600);
                slots
            }
            Timeframe::Hour1 => {
                // 7 slots: 09:15, 10:15, 11:15, 12:15, 13:15, 14:15, 15:15
                (0..7).map(|k| open_ts + k * 3600).collect()
            }
            Timeframe::Day1 => {
                // 1 slot per trading day: Yahoo uses 03:45 UTC = 09:15 IST
                vec![open_ts]
            }
            Timeframe::Week1 => {
                // Weekly candle timestamp aligns to Monday 00:00:00 IST of that week
                let days_from_mon = date.weekday().num_days_from_monday();
                let mon_date = date - Duration::days(days_from_mon as i64);
                let mon_dt = mon_date.and_hms_opt(0, 0, 0).expect("Valid 00:00");
                let mon_ist =
                    DateTime::<FixedOffset>::from_naive_utc_and_offset(mon_dt - offset, offset);
                vec![mon_ist.timestamp()]
            }
        }
    }

    /// Align an arbitrary incoming candle timestamp to its proper timeframe slot boundary.
    ///
    /// Fixes Yahoo live tick drift where forming candles have timestamps like `11:36:42`
    /// instead of `11:30:00`.
    pub fn align_timestamp(ts: i64, tf: Timeframe) -> i64 {
        let offset = ist_offset();
        let dt_ist = DateTime::from_timestamp(ts, 0)
            .unwrap_or_default()
            .with_timezone(&offset);

        let date = dt_ist.date_naive();
        let open_dt = date
            .and_hms_opt(MARKET_OPEN_HOUR, MARKET_OPEN_MINUTE, 0)
            .expect("Valid 09:15");
        let open_ist = DateTime::<FixedOffset>::from_naive_utc_and_offset(open_dt - offset, offset);
        let open_ts = open_ist.timestamp();

        match tf {
            Timeframe::Min5 => {
                if ts <= open_ts {
                    return open_ts;
                }
                let delta = ts - open_ts;
                let slot = (delta / 300).min(74);
                open_ts + slot * 300
            }
            Timeframe::Min15 => {
                if ts <= open_ts {
                    return open_ts;
                }
                let delta = ts - open_ts;
                let slot = (delta / 900).min(24);
                open_ts + slot * 900
            }
            Timeframe::Min30 => {
                if ts <= open_ts {
                    return open_ts;
                }
                let delta = ts - open_ts;
                // If past 15:15 (6 hours = 21600 seconds)
                if delta >= 6 * 3600 {
                    return open_ts + 6 * 3600;
                }
                let slot = (delta / 1800).min(11);
                open_ts + slot * 1800
            }
            Timeframe::Hour1 => {
                if ts <= open_ts {
                    return open_ts;
                }
                let delta = ts - open_ts;
                let slot = (delta / 3600).min(6);
                open_ts + slot * 3600
            }
            Timeframe::Day1 => {
                // Yahoo NSE daily candles are always 03:45 UTC / 09:15 IST
                open_ts
            }
            Timeframe::Week1 => {
                let days_from_mon = date.weekday().num_days_from_monday();
                let mon_date = date - Duration::days(days_from_mon as i64);
                let mon_dt = mon_date.and_hms_opt(0, 0, 0).expect("Valid 00:00");
                let mon_ist =
                    DateTime::<FixedOffset>::from_naive_utc_and_offset(mon_dt - offset, offset);
                mon_ist.timestamp()
            }
        }
    }

    /// Calculate the latest expected candle slot and whether it is currently forming
    /// for a given point in time in IST.
    pub fn latest_expected_candle(now: DateTime<FixedOffset>, tf: Timeframe) -> ExpectedCandle {
        let date = now.date_naive();
        let current_minutes = now.hour() * 60 + now.minute();
        let is_trading = Self::is_trading_day(date);

        if tf == Timeframe::Week1 {
            let days_from_mon = date.weekday().num_days_from_monday();
            let mon_date = date - Duration::days(days_from_mon as i64);
            let offset = ist_offset();
            let mon_dt = mon_date.and_hms_opt(0, 0, 0).expect("Valid 00:00");
            let mon_ist =
                DateTime::<FixedOffset>::from_naive_utc_and_offset(mon_dt - offset, offset);
            let is_forming =
                current_minutes < MARKET_CLOSE_MINUTES_TOTAL || date.weekday() != Weekday::Fri;
            return ExpectedCandle {
                timestamp: mon_ist.timestamp(),
                is_forming,
            };
        }

        if tf == Timeframe::Day1 {
            if is_trading && current_minutes >= MARKET_OPEN_MINUTES_TOTAL {
                let slots = Self::session_slots(date, Timeframe::Day1);
                let is_forming = current_minutes < MARKET_CLOSE_MINUTES_TOTAL;
                return ExpectedCandle {
                    timestamp: slots[0],
                    is_forming,
                };
            }
            // Before market open today, or on holiday/weekend: return most recent completed trading day
            let prev_date = if is_trading && current_minutes < MARKET_OPEN_MINUTES_TOTAL {
                Self::previous_trading_day(date)
            } else {
                Self::most_recent_trading_day(date)
            };
            let slots = Self::session_slots(prev_date, Timeframe::Day1);
            return ExpectedCandle {
                timestamp: slots[0],
                is_forming: false,
            };
        }

        // Intraday timeframes (5m, 15m, 30m, 1h)
        if is_trading {
            if (MARKET_OPEN_MINUTES_TOTAL..MARKET_CLOSE_MINUTES_TOTAL).contains(&current_minutes) {
                // Market is currently OPEN: find current in-progress slot
                let now_ts = now.timestamp();
                let aligned = Self::align_timestamp(now_ts, tf);
                return ExpectedCandle {
                    timestamp: aligned,
                    is_forming: true,
                };
            } else if current_minutes >= MARKET_CLOSE_MINUTES_TOTAL {
                // Market has CLOSED today: final slot of today
                let slots = Self::session_slots(date, tf);
                let last_slot = *slots.last().expect("Slots non-empty");
                return ExpectedCandle {
                    timestamp: last_slot,
                    is_forming: false,
                };
            }
        }

        // Before market open or weekend/holiday: get final slot of the previous trading day
        let prev_date = if is_trading && current_minutes < MARKET_OPEN_MINUTES_TOTAL {
            Self::previous_trading_day(date)
        } else {
            Self::most_recent_trading_day(date)
        };
        let slots = Self::session_slots(prev_date, tf);
        let last_slot = *slots.last().expect("Slots non-empty");
        ExpectedCandle {
            timestamp: last_slot,
            is_forming: false,
        }
    }

    /// Enumerate all expected slot timestamps between `start_ts` and `end_ts` (inclusive).
    pub fn expected_slots_between(start_ts: i64, end_ts: i64, tf: Timeframe) -> Vec<i64> {
        if start_ts > end_ts {
            return Vec::new();
        }

        let offset = ist_offset();
        let start_dt = DateTime::from_timestamp(start_ts, 0)
            .unwrap_or_default()
            .with_timezone(&offset);
        let end_dt = DateTime::from_timestamp(end_ts, 0)
            .unwrap_or_default()
            .with_timezone(&offset);

        let mut current_date = start_dt.date_naive();
        let end_date = end_dt.date_naive();

        let mut result = Vec::new();
        while current_date <= end_date {
            if Self::is_trading_day(current_date) {
                let slots = Self::session_slots(current_date, tf);
                for s in slots {
                    if s >= start_ts && s <= end_ts {
                        result.push(s);
                    }
                }
            }
            current_date += Duration::days(1);
        }

        result.sort_unstable();
        result.dedup();
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_weekend_detection() {
        let sat = NaiveDate::from_ymd_opt(2026, 9, 19).unwrap();
        let sun = NaiveDate::from_ymd_opt(2026, 9, 20).unwrap();
        let mon = NaiveDate::from_ymd_opt(2026, 9, 21).unwrap();

        assert!(!MarketCalendar::is_trading_day(sat));
        assert!(!MarketCalendar::is_trading_day(sun));
        assert!(MarketCalendar::is_trading_day(mon));
    }

    #[test]
    fn test_official_holidays_detection() {
        let republic_day_2026 = NaiveDate::from_ymd_opt(2026, 1, 26).unwrap(); // Monday
        let gandhi_jayanti_2026 = NaiveDate::from_ymd_opt(2026, 10, 2).unwrap(); // Friday
        let normal_day = NaiveDate::from_ymd_opt(2026, 9, 23).unwrap(); // Wednesday

        assert!(!MarketCalendar::is_trading_day(republic_day_2026));
        assert!(!MarketCalendar::is_trading_day(gandhi_jayanti_2026));
        assert!(MarketCalendar::is_trading_day(normal_day));
    }

    #[test]
    fn test_session_slots_count() {
        let wednesday = NaiveDate::from_ymd_opt(2026, 9, 23).unwrap();

        let slots_5m = MarketCalendar::session_slots(wednesday, Timeframe::Min5);
        assert_eq!(slots_5m.len(), 75);

        let slots_15m = MarketCalendar::session_slots(wednesday, Timeframe::Min15);
        assert_eq!(slots_15m.len(), 25);

        let slots_30m = MarketCalendar::session_slots(wednesday, Timeframe::Min30);
        assert_eq!(slots_30m.len(), 13);

        let slots_1h = MarketCalendar::session_slots(wednesday, Timeframe::Hour1);
        assert_eq!(slots_1h.len(), 7);

        let slots_1d = MarketCalendar::session_slots(wednesday, Timeframe::Day1);
        assert_eq!(slots_1d.len(), 1);
    }

    #[test]
    fn test_align_timestamp_flooring() {
        let offset = ist_offset();
        // 2026-09-23 11:36:42 IST -> Should floor to 11:30:00 for 15m
        let dt = NaiveDate::from_ymd_opt(2026, 9, 23)
            .unwrap()
            .and_hms_opt(11, 36, 42)
            .unwrap();
        let dt_ist = DateTime::<FixedOffset>::from_naive_utc_and_offset(dt - offset, offset);

        let aligned_15m = MarketCalendar::align_timestamp(dt_ist.timestamp(), Timeframe::Min15);
        let aligned_dt = DateTime::from_timestamp(aligned_15m, 0)
            .unwrap()
            .with_timezone(&offset);

        assert_eq!(aligned_dt.hour(), 11);
        assert_eq!(aligned_dt.minute(), 30);
        assert_eq!(aligned_dt.second(), 0);

        // For 5m -> Should floor to 11:35:00
        let aligned_5m = MarketCalendar::align_timestamp(dt_ist.timestamp(), Timeframe::Min5);
        let aligned_5m_dt = DateTime::from_timestamp(aligned_5m, 0)
            .unwrap()
            .with_timezone(&offset);
        assert_eq!(aligned_5m_dt.hour(), 11);
        assert_eq!(aligned_5m_dt.minute(), 35);
        assert_eq!(aligned_5m_dt.second(), 0);
    }
}
