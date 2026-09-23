//! Application orchestration.
//!
//! Ties together config loading, CLI parsing, data fetching, scanner, and chart display.
//! This is the central coordination point — it knows about all layers.

use std::path::PathBuf;
use std::sync::Arc;

use crate::config::{self, StockConfig};
use crate::domain::{Timeframe, mover::validate_threshold};
use crate::provider::MarketDataProvider;
use crate::provider::yahoo::YahooProvider;
use crate::scanner::{
    ScanState, Scanner, ServiceManager, now_ist, send_desktop_notification, show_popup_dialog,
};
use crate::ui::chart;

/// Run the full application flow for native GUI:
/// 1. Load stock config
/// 2. Select stock (first in list for MVP, or from CLI arg)
/// 3. Fetch market data
/// 4. Display chart
pub async fn run(timeframe: Timeframe, symbol_arg: Option<&str>) -> anyhow::Result<()> {
    // Load stock universe
    let config_path = find_config_path()?;
    let stock_config = config::load_stock_config(&config_path)?;

    // Resolve which stock to display
    let stock = resolve_stock(&stock_config, symbol_arg)?;
    println!("Fetching {} data for {}...", timeframe, stock.symbol);

    // Fetch market data with SQLite caching
    let db = crate::storage::MarketDb::open_default()?;
    db.sync_tickers(&stock_config.stocks)?;
    let provider = crate::provider::CachedProvider::new(YahooProvider::new()?, db);
    let candles = provider.fetch_candles(&stock.symbol, timeframe).await?;
    println!(
        "Received {} candles for {} ({})",
        candles.len(),
        stock.name,
        stock.symbol
    );

    // Display chart
    chart::show_chart(candles, &stock.symbol, timeframe)
        .map_err(|e| anyhow::anyhow!("Failed to launch chart window: {e}"))?;

    Ok(())
}

/// Run scanner across stock universe.
pub async fn run_scan(
    threshold: f64,
    notify: bool,
    catchup: bool,
    popup: bool,
) -> anyhow::Result<()> {
    let threshold = validate_threshold(threshold).map_err(|e| anyhow::anyhow!("{e}"))?;
    let state_path = ScanState::default_path();
    let mut state = ScanState::load(&state_path);
    let ist_now = now_ist();

    let pending_slots = if catchup {
        let slots = state.determine_pending_slots(ist_now);
        if slots.is_empty() {
            println!("✓ No missed scan slots. Last scans are up to date.");
            return Ok(());
        }
        slots
    } else {
        vec![]
    };

    let config_path = find_config_path()?;
    let stock_config = config::load_stock_config(&config_path)?;

    let slots = if catchup {
        pending_slots.into_iter().map(Some).collect()
    } else {
        vec![None]
    };

    for slot in slots {
        if let Some(slot) = slot {
            println!("[Catchup] Missed scheduled slot detected: {}", slot.label());
        }

        println!(
            "Scanning {} stocks for moves exceeding ±{:.1}%...",
            stock_config.stocks.len(),
            threshold
        );

        let provider = Arc::new(YahooProvider::new()?);
        let scanner = Scanner::new(provider);
        // A fully-failed sweep is an error (engine) and never recorded;
        // partial failures still record, with the count visible below.
        let result = scanner.scan(&stock_config.stocks, threshold).await?;

        print_scan_result(&result);

        if let Some(slot) = slot {
            let today_str = ist_now.format("%Y-%m-%d").to_string();
            state.record_slot_completion(slot, &today_str, result.clone());
        } else {
            state.last_scan_result = Some(result.clone());
        }
        state.save(&state_path)?;

        if notify {
            send_desktop_notification(&result, slot.map(|s| s.label()));
        }

        if popup {
            show_popup_dialog(&result, slot.map(|s| s.label()));
        }
    }

    Ok(())
}

fn print_scan_result(result: &crate::domain::ScanResult) {
    let threshold = result.threshold_percent;
    println!("\n╔════════════════════════════════════════════════════════════════════════╗");
    println!("║                 MarketWatch Stock Universe Scan Result                 ║");
    println!("╠════════════════════════════════════════════════════════════════════════╣");
    println!("║ Time:      {:<59} ║", result.scan_time);
    println!("║ Threshold: ±{:<58.1}% ║", threshold);
    println!(
        "║ Scanned:   {:<59} ║",
        format!(
            "{} stocks ({} movers, {} gainers, {} losers{})",
            result.total_scanned,
            result.movers_count,
            result.gainers_count,
            result.losers_count,
            if result.failed_count > 0 {
                format!(", {} failed", result.failed_count)
            } else {
                String::new()
            }
        )
    );
    println!("╚════════════════════════════════════════════════════════════════════════╝\n");

    if result.movers.is_empty() {
        println!("No stocks moved beyond ±{:.1}% today.\n", threshold);
    } else {
        println!(
            "{:<15} {:<32} {:<12} {:<12} {:<12}",
            "SYMBOL", "NAME", "LTP (₹)", "CHANGE", "VOLUME"
        );
        println!("{}", "-".repeat(88));
        for m in &result.movers {
            let chg_sign = if m.change_percent >= 0.0 { "+" } else { "" };
            let chg_str = format!("{}{:.2}%", chg_sign, m.change_percent);
            let name_disp = if m.name.chars().count() > 30 {
                format!("{}...", m.name.chars().take(27).collect::<String>())
            } else {
                m.name.clone()
            };
            println!(
                "{:<15} {:<32} {:<12.2} {:<12} {:<12}",
                m.symbol, name_disp, m.price, chg_str, m.volume
            );
        }
        println!();
    }
}

/// Run continuous background daemon.
pub async fn run_daemon(threshold: f64) -> anyhow::Result<()> {
    let threshold = validate_threshold(threshold).map_err(|e| anyhow::anyhow!("{e}"))?;
    println!("\n╔════════════════════════════════════════════════════════════════════════╗");
    println!("║       MarketWatch Background Daemon is Running!                        ║");
    println!("╠════════════════════════════════════════════════════════════════════════╣");
    println!("║ Target Slots: 09:30 AM IST (Morning) & 03:30 PM IST (Evening)          ║");
    println!("║ Trading Days: Monday – Friday                                          ║");
    println!("║ Threshold:    ±{:<55.1}% ║", threshold);
    println!("║ Catch-up:     Automatic immediately on boot/wake                       ║");
    println!("╚════════════════════════════════════════════════════════════════════════╝\n");
    println!("Press Ctrl+C to stop.\n");

    let state_path = ScanState::default_path();

    loop {
        let mut state = ScanState::load(&state_path);
        let ist_now = now_ist();

        if let Some(slot) = state.determine_pending_slot(ist_now) {
            println!(
                "\n[{}] Triggering scheduled scan for {}...",
                ist_now.format("%H:%M:%S IST"),
                slot.label()
            );

            // Per-iteration failures must not kill the daemon: log, back off, retry.
            let config_path = match find_config_path() {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("Daemon config error (retrying in 60s): {e:#}");
                    tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                    continue;
                }
            };
            let stock_config = match config::load_stock_config(&config_path) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Daemon config error (retrying in 60s): {e:#}");
                    tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                    continue;
                }
            };
            let provider = match YahooProvider::new() {
                Ok(p) => Arc::new(p),
                Err(e) => {
                    eprintln!("Daemon provider error (retrying in 60s): {e:#}");
                    tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                    continue;
                }
            };
            let scanner = Scanner::new(provider);

            match scanner.scan(&stock_config.stocks, threshold).await {
                Ok(result) => {
                    println!(
                        "[{}] Scan complete! Found {} movers (> ±{:.1}%).",
                        ist_now.format("%H:%M:%S IST"),
                        result.movers_count,
                        threshold
                    );
                    let today_str = ist_now.format("%Y-%m-%d").to_string();
                    state.record_slot_completion(slot, &today_str, result.clone());
                    if let Err(e) = state.save(&state_path) {
                        eprintln!("Scan completed but state persistence failed: {e}");
                    }
                    send_desktop_notification(&result, Some(slot.label()));
                }
                Err(e) => {
                    // Fully-failed sweeps are not recorded (see engine); back off
                    // before the next attempt instead of hot-looping every 30s.
                    eprintln!("Scan error (slot not recorded, retrying in 60s): {e:#}");
                    tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                }
            }
        }

        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
    }
}

/// Run service command.
pub fn run_service(action: &str, threshold: f64) -> anyhow::Result<()> {
    match action {
        "install" => {
            let threshold = validate_threshold(threshold).map_err(|e| anyhow::anyhow!("{e}"))?;
            ServiceManager::install(threshold)
        }
        "uninstall" => ServiceManager::uninstall(),
        "status" => {
            ServiceManager::status();
            Ok(())
        }
        other => anyhow::bail!(
            "Unknown service action '{other}'. Valid actions: install, uninstall, status."
        ),
    }
}

/// Find the config directory. Checks:
/// 1. ./config/stocks.toml (relative to CWD — typical for development)
/// 2. Adjacent to the executable
pub fn find_config_path() -> anyhow::Result<PathBuf> {
    // Try relative to current directory first
    let cwd_path = PathBuf::from("config/stocks.toml");
    if cwd_path.exists() {
        return Ok(cwd_path);
    }

    // Try relative to executable
    if let Ok(exe) = std::env::current_exe()
        && let Some(exe_dir) = exe.parent()
    {
        let exe_path = exe_dir.join("config/stocks.toml");
        if exe_path.exists() {
            return Ok(exe_path);
        }
    }

    anyhow::bail!(
        "Cannot find config/stocks.toml. \
         Run from the project root directory, or place the config \
         directory next to the executable."
    )
}

/// Resolve which stock to use.
/// If a symbol is given via CLI, find it in the config.
/// Otherwise, use the first stock in the list.
fn resolve_stock<'a>(
    config: &'a StockConfig,
    symbol_arg: Option<&str>,
) -> anyhow::Result<&'a config::StockEntry> {
    match symbol_arg {
        Some(sym) => {
            let upper = sym.trim().to_uppercase();
            if upper.is_empty() {
                anyhow::bail!("Empty --symbol provided. Omit -s to use the first stock.");
            }
            config
                .stocks
                .iter()
                .find(|s| s.symbol.to_uppercase() == upper)
                .ok_or_else(|| {
                    let available: Vec<&str> =
                        config.stocks.iter().map(|s| s.symbol.as_str()).collect();
                    anyhow::anyhow!(
                        "Symbol '{}' not found in stock universe.\nAvailable: {:?}",
                        sym.trim(),
                        &available[..available.len().min(10)]
                    )
                })
        }
        None => config.stocks.first().ok_or_else(|| {
            anyhow::anyhow!("Stock universe is empty. Add at least one [[stocks]] entry.")
        }),
    }
}
