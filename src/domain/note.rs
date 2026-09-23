use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize};

use crate::domain::Timeframe;

/// Allowed statuses for a stock note.
pub const VALID_NOTE_STATUSES: &[&str] = &["open", "validated", "invalidated", "cancelled"];

/// Check if a status string is valid.
pub fn is_valid_status(status: &str) -> bool {
    VALID_NOTE_STATUSES.contains(&status.trim())
}

/// A trader analysis note / journal entry for a stock setup.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StockNote {
    pub id: i64,
    pub symbol: String,
    pub timeframe: String,
    pub candle_timestamp: Option<i64>,
    pub price_at_note: f64,
    pub title: String,
    pub content: String,
    pub tags: String,
    pub status: String, // "open", "validated", "invalidated", "cancelled"
    pub target_price: Option<f64>,
    pub stop_loss: Option<f64>,
    pub outcome_note: Option<String>,
    pub verified_at: Option<i64>,
    pub reference_note_ids: Vec<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

fn default_timeframe() -> String {
    "15m".to_string()
}

/// Helper deserializer for nullable patch fields.
/// Distinguishes between omitted (None) and explicitly passed null (Some(None)).
fn deserialize_double_option<'de, T, D>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    Deserialize::deserialize(deserializer).map(Some)
}

/// Input payload for creating a new stock analysis note.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateNoteInput {
    pub symbol: String,
    #[serde(default = "default_timeframe")]
    pub timeframe: String,
    pub candle_timestamp: Option<i64>,
    pub price_at_note: f64,
    pub title: String,
    pub content: String,
    #[serde(default)]
    pub tags: String,
    pub target_price: Option<f64>,
    pub stop_loss: Option<f64>,
    #[serde(default)]
    pub reference_note_ids: Vec<i64>,
}

impl CreateNoteInput {
    /// Validates inputs: non-empty strings, valid timeframe, positive finite prices.
    pub fn validate(&self) -> Result<(), String> {
        let sym = self.symbol.trim();
        if sym.is_empty() {
            return Err("Stock symbol cannot be empty".to_string());
        }
        let title = self.title.trim();
        if title.is_empty() {
            return Err("Note title cannot be empty".to_string());
        }
        if title.len() > 200 {
            return Err("Note title exceeds maximum length of 200 characters".to_string());
        }
        let content = self.content.trim();
        if content.is_empty() {
            return Err("Note content cannot be empty".to_string());
        }
        if !self.price_at_note.is_finite() || self.price_at_note <= 0.0 {
            return Err("Price at note must be a positive finite number".to_string());
        }
        if let Some(target) = self.target_price
            && (!target.is_finite() || target <= 0.0)
        {
            return Err("Target price must be a positive finite number".to_string());
        }
        if let Some(stop) = self.stop_loss
            && (!stop.is_finite() || stop <= 0.0)
        {
            return Err("Stop loss must be a positive finite number".to_string());
        }
        if Timeframe::from_str(&self.timeframe).is_err() {
            return Err(format!(
                "Invalid timeframe '{}'. Supported timeframes: 5m, 15m, 30m, 1h, 1d, 1w",
                self.timeframe
            ));
        }
        if let Some(ts) = self.candle_timestamp
            && ts <= 0
        {
            return Err("Candle timestamp must be positive".to_string());
        }
        Ok(())
    }
}

/// Input payload for updating an existing stock analysis note.
/// Uses `Option<Option<T>>` for nullable fields so clients can explicitly clear them with `null`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateNoteInput {
    pub title: Option<String>,
    pub content: Option<String>,
    pub tags: Option<String>,
    pub status: Option<String>,
    #[serde(default, deserialize_with = "deserialize_double_option")]
    pub target_price: Option<Option<f64>>,
    #[serde(default, deserialize_with = "deserialize_double_option")]
    pub stop_loss: Option<Option<f64>>,
    #[serde(default, deserialize_with = "deserialize_double_option")]
    pub outcome_note: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_double_option")]
    pub verified_at: Option<Option<i64>>,
    pub reference_note_ids: Option<Vec<i64>>,
}

impl UpdateNoteInput {
    /// Validates update fields when present.
    pub fn validate(&self, note_id: i64) -> Result<(), String> {
        if let Some(ref title) = self.title {
            let t = title.trim();
            if t.is_empty() {
                return Err("Note title cannot be empty".to_string());
            }
            if t.len() > 200 {
                return Err("Note title exceeds maximum length of 200 characters".to_string());
            }
        }
        if let Some(ref content) = self.content
            && content.trim().is_empty()
        {
            return Err("Note content cannot be empty".to_string());
        }
        if let Some(ref status) = self.status
            && !is_valid_status(status)
        {
            return Err(format!(
                "Invalid status '{}'. Allowed values: open, validated, invalidated, cancelled",
                status
            ));
        }
        if let Some(Some(target)) = self.target_price
            && (!target.is_finite() || target <= 0.0)
        {
            return Err("Target price must be a positive finite number".to_string());
        }
        if let Some(Some(stop)) = self.stop_loss
            && (!stop.is_finite() || stop <= 0.0)
        {
            return Err("Stop loss must be a positive finite number".to_string());
        }
        if let Some(ref refs) = self.reference_note_ids
            && refs.contains(&note_id)
        {
            return Err(format!("Note #{note_id} cannot reference itself"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_note_input_validation() {
        let mut input = CreateNoteInput {
            symbol: "HDFCBANK.NS".to_string(),
            timeframe: "15m".to_string(),
            candle_timestamp: Some(1790000000),
            price_at_note: 1650.0,
            title: "Test Breakout".to_string(),
            content: "Looking strong above 200 EMA".to_string(),
            tags: "breakout".to_string(),
            target_price: Some(1720.0),
            stop_loss: Some(1610.0),
            reference_note_ids: vec![],
        };
        assert!(input.validate().is_ok());

        // Empty symbol
        input.symbol = "   ".to_string();
        assert!(input.validate().is_err());
        input.symbol = "HDFCBANK.NS".to_string();

        // Non-positive price
        input.price_at_note = 0.0;
        assert!(input.validate().is_err());
        input.price_at_note = -10.0;
        assert!(input.validate().is_err());
        input.price_at_note = f64::NAN;
        assert!(input.validate().is_err());
        input.price_at_note = 1650.0;

        // Invalid timeframe
        input.timeframe = "99m".to_string();
        assert!(input.validate().is_err());
        input.timeframe = "15m".to_string();

        // Negative target
        input.target_price = Some(-5.0);
        assert!(input.validate().is_err());
        input.target_price = Some(1720.0);
        assert!(input.validate().is_ok());
    }

    #[test]
    fn test_update_note_input_patch_deserialization() {
        // 1. Omitted fields are None
        let json_omitted = r#"{ "title": "Updated Title" }"#;
        let parsed: UpdateNoteInput = serde_json::from_str(json_omitted).unwrap();
        assert_eq!(parsed.title.as_deref(), Some("Updated Title"));
        assert_eq!(parsed.target_price, None); // omitted
        assert_eq!(parsed.stop_loss, None); // omitted

        // 2. Explicit null fields are Some(None)
        let json_cleared = r#"{ "target_price": null, "stop_loss": null }"#;
        let parsed_cleared: UpdateNoteInput = serde_json::from_str(json_cleared).unwrap();
        assert_eq!(parsed_cleared.target_price, Some(None)); // explicitly cleared!
        assert_eq!(parsed_cleared.stop_loss, Some(None));

        // 3. Concrete value fields are Some(Some(v))
        let json_set = r#"{ "target_price": 1850.50 }"#;
        let parsed_set: UpdateNoteInput = serde_json::from_str(json_set).unwrap();
        assert_eq!(parsed_set.target_price, Some(Some(1850.50)));

        // 4. Status validation
        assert!(is_valid_status("open"));
        assert!(is_valid_status("validated"));
        assert!(is_valid_status("invalidated"));
        assert!(is_valid_status("cancelled"));
        assert!(!is_valid_status("arbitrary_hacked_status"));

        let update_invalid_status = UpdateNoteInput {
            status: Some("<script>alert(1)</script>".to_string()),
            ..Default::default()
        };
        assert!(update_invalid_status.validate(1).is_err());

        // 5. Self-reference check
        let update_self_ref = UpdateNoteInput {
            reference_note_ids: Some(vec![1, 2, 3]),
            ..Default::default()
        };
        assert!(update_self_ref.validate(2).is_err());
        assert!(update_self_ref.validate(5).is_ok());
    }
}
