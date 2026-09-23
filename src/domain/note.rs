use serde::{Deserialize, Serialize};

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

/// Input payload for updating an existing stock analysis note.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateNoteInput {
    pub title: Option<String>,
    pub content: Option<String>,
    pub tags: Option<String>,
    pub status: Option<String>,
    pub target_price: Option<f64>,
    pub stop_loss: Option<f64>,
    pub outcome_note: Option<String>,
    pub verified_at: Option<i64>,
    pub reference_note_ids: Option<Vec<i64>>,
}
