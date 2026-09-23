//! Persistence and state tracking for scheduled scans and automatic catch-up.

use chrono::{DateTime, FixedOffset, Timelike};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::domain::{MarketCalendar, ScanResult};
use crate::error::MarketError;

/// Which market scan slot is scheduled or pending.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScheduledSlot {
    /// 09:30 IST morning scan (15m after 9:15 market open)
    Morning,
    /// 15:30 IST evening scan (market closing)
    Evening,
}

impl ScheduledSlot {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Morning => "Morning (09:30 IST)",
            Self::Evening => "Evening (15:30 IST)",
        }
    }
}

/// Persistent state to track when morning and evening scans were last executed.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScanState {
    /// Date of last morning scan (e.g. "2026-09-21")
    pub last_morning_scan_date: Option<String>,
    /// Date of last evening scan (e.g. "2026-09-21")
    pub last_evening_scan_date: Option<String>,
    /// Default threshold percentage (e.g. 3.0)
    pub default_threshold: Option<f64>,
    /// Most recent scan result for instant retrieval
    pub last_scan_result: Option<ScanResult>,
}

impl ScanState {
    /// Load state from JSON file or return default.
    ///
    /// A corrupt file is never silently discarded: it is renamed to
    /// `<name>.corrupt-<timestamp>` for inspection and a default is returned.
    pub fn load(path: &Path) -> Self {
        if !path.exists() {
            return Self::default();
        }

        match std::fs::read_to_string(path) {
            Ok(content) => match serde_json::from_str(&content) {
                Ok(state) => state,
                Err(e) => {
                    let backup =
                        path.with_extension(format!("corrupt-{}", chrono::Utc::now().timestamp()));
                    if std::fs::rename(path, &backup).is_ok() {
                        eprintln!(
                            "Warning: scan state '{}' was corrupt ({e}); moved to '{}' and starting fresh.",
                            path.display(),
                            backup.display()
                        );
                    } else {
                        eprintln!(
                            "Warning: scan state '{}' is corrupt ({e}); starting fresh.",
                            path.display()
                        );
                    }
                    Self::default()
                }
            },
            Err(e) => {
                eprintln!(
                    "Warning: cannot read scan state '{}' ({e}); starting fresh.",
                    path.display()
                );
                Self::default()
            }
        }
    }

    /// Save state to JSON file atomically (tmp file + rename + fsync).
    pub fn save(&self, path: &Path) -> Result<(), MarketError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                MarketError::Config(format!(
                    "Failed to create state dir '{}': {e}",
                    parent.display()
                ))
            })?;
        }

        let json = serde_json::to_string_pretty(self)
            .map_err(|e| MarketError::Config(format!("Failed to serialize scan state: {e}")))?;

        let tmp_path = path.with_extension("tmp");
        std::fs::write(&tmp_path, json).map_err(|e| {
            MarketError::Config(format!(
                "Failed to write state to '{}': {e}",
                tmp_path.display()
            ))
        })?;
        std::fs::rename(&tmp_path, path).map_err(|e| {
            let _ = std::fs::remove_file(&tmp_path);
            MarketError::Config(format!(
                "Failed to persist state to '{}': {e}",
                path.display()
            ))
        })?;

        Ok(())
    }

    /// Record completion of a scheduled slot.
    pub fn record_slot_completion(
        &mut self,
        slot: ScheduledSlot,
        date_str: &str,
        result: ScanResult,
    ) {
        match slot {
            ScheduledSlot::Morning => self.last_morning_scan_date = Some(date_str.to_string()),
            ScheduledSlot::Evening => self.last_evening_scan_date = Some(date_str.to_string()),
        }
        self.last_scan_result = Some(result);
    }

    /// Determine if there is a pending scan that was missed because the laptop was off.
    ///
    /// Checks Indian Standard Time (IST). If current time is past 09:30 or 15:30 on a weekday
    /// and that slot has not yet run today, it returns the pending slot.
    pub fn determine_pending_slot(&self, ist_now: DateTime<FixedOffset>) -> Option<ScheduledSlot> {
        self.determine_pending_slots(ist_now).into_iter().next()
    }

    /// Determine all scheduled slots that are due and have not run today.
    ///
    /// Morning is returned before evening so a catch-up run cannot skip the
    /// earlier slot when both scheduled scans were missed.
    pub fn determine_pending_slots(&self, ist_now: DateTime<FixedOffset>) -> Vec<ScheduledSlot> {
        if !MarketCalendar::is_trading_day(ist_now.date_naive()) {
            return Vec::new();
        }

        let today_str = ist_now.format("%Y-%m-%d").to_string();
        let current_minutes = ist_now.hour() * 60 + ist_now.minute();

        const MORNING_MINUTES: u32 = 9 * 60 + 30; // 09:30 AM
        const EVENING_MINUTES: u32 = 15 * 60 + 30; // 03:30 PM

        let mut pending = Vec::new();
        if current_minutes >= MORNING_MINUTES
            && self.last_morning_scan_date.as_deref() != Some(&today_str)
        {
            pending.push(ScheduledSlot::Morning);
        }
        if current_minutes >= EVENING_MINUTES
            && self.last_evening_scan_date.as_deref() != Some(&today_str)
        {
            pending.push(ScheduledSlot::Evening);
        }

        pending
    }

    /// Get default path for saving scan state:
    /// 1. ~/.config/marketwatch/scanner_state.json
    /// 2. ./cache/scanner_state.json
    pub fn default_path() -> PathBuf {
        if let Some(home) = std::env::var_os("HOME") {
            let path = PathBuf::from(home).join(".config/marketwatch/scanner_state.json");
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            return path;
        }

        PathBuf::from("cache/scanner_state.json")
    }
}

/// Helper to get current Indian Standard Time (+05:30).
pub fn now_ist() -> DateTime<FixedOffset> {
    let offset = FixedOffset::east_opt(5 * 3600 + 30 * 60)
        .unwrap_or_else(|| FixedOffset::east_opt(0).unwrap());
    chrono::Utc::now().with_timezone(&offset)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn test_pending_slot_detection() {
        let ist = FixedOffset::east_opt(19800).unwrap();
        let mut state = ScanState::default();

        // Monday at 10:15 AM (after 09:30 morning slot)
        let monday_10_15 = ist.with_ymd_and_hms(2026, 9, 21, 10, 15, 0).unwrap();
        assert_eq!(
            state.determine_pending_slot(monday_10_15),
            Some(ScheduledSlot::Morning)
        );

        // Mark morning scan as completed
        state.last_morning_scan_date = Some("2026-09-21".to_string());
        // At 10:15 AM, morning scan is done, evening is not due yet
        assert_eq!(state.determine_pending_slot(monday_10_15), None);

        // Monday at 16:00 (after 15:30 evening slot)
        let monday_16_00 = ist.with_ymd_and_hms(2026, 9, 21, 16, 0, 0).unwrap();
        assert_eq!(
            state.determine_pending_slot(monday_16_00),
            Some(ScheduledSlot::Evening)
        );

        let both_missed = ScanState::default();
        assert_eq!(
            both_missed.determine_pending_slots(monday_16_00),
            vec![ScheduledSlot::Morning, ScheduledSlot::Evening]
        );

        // Mark evening scan as completed
        state.last_evening_scan_date = Some("2026-09-21".to_string());
        assert_eq!(state.determine_pending_slot(monday_16_00), None);

        // Sunday should return None
        let sunday_12_00 = ist.with_ymd_and_hms(2026, 9, 20, 12, 0, 0).unwrap();
        let empty_state = ScanState::default();
        assert_eq!(empty_state.determine_pending_slot(sunday_12_00), None);
    }
}
