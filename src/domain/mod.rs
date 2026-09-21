//! Domain types: core data structures that are provider-agnostic and UI-agnostic.

pub mod candle;
pub mod mover;
pub mod timeframe;

pub use candle::Candle;
pub use mover::{ScanResult, StockMover};
pub use timeframe::Timeframe;
