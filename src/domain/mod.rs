//! Domain types: core data structures that are provider-agnostic and UI-agnostic.

pub mod calendar;
pub mod candle;
pub mod mover;
pub mod note;
pub mod timeframe;

#[allow(unused_imports)]
pub use calendar::{ExpectedCandle, MarketCalendar};
pub use candle::Candle;
pub use mover::{ScanResult, StockMover};
pub use note::{CreateNoteInput, StockNote, UpdateNoteInput};
pub use timeframe::Timeframe;
