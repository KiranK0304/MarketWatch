//! Market scanner module for monitoring movers, running cron jobs, and desktop notifications.

pub mod engine;
pub mod notifier;
pub mod service;
pub mod state;

pub use engine::Scanner;
#[allow(unused_imports)]
pub use notifier::{open_dashboard, send_desktop_notification, show_popup_dialog};
pub use service::ServiceManager;
#[allow(unused_imports)]
pub use state::{now_ist, ScanState, ScheduledSlot};
