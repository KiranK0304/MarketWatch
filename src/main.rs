//! MarketWatch — Personal stock market visualization terminal & top movers scanner.
//!
//! Entry point: parse CLI arguments, resolve execution mode, and run.

mod app;
mod cli;
mod config;
mod domain;
mod error;
mod provider;
mod scanner;
mod storage;
mod ui;
mod web;

use clap::Parser;

use crate::cli::{Cli, RunMode};

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // Resolve execution mode: Web Dashboard, Native GUI, Scanner, Service, or Daemon
    let mode = match cli.resolve_mode() {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    };

    match mode {
        RunMode::WebDashboard { port, open_browser } => {
            let config_path = match app::find_config_path() {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            };

            if let Err(e) = web::start_server(config_path, port, open_browser).await {
                eprintln!("Web server error: {e:#}");
                std::process::exit(1);
            }
        }
        RunMode::NativeGui(timeframe) => {
            if let Err(e) = app::run(timeframe, cli.symbol.as_deref()).await {
                eprintln!("Error: {e:#}");
                std::process::exit(1);
            }
        }
        RunMode::Scan {
            threshold,
            notify,
            catchup,
            popup,
        } => {
            if let Err(e) = app::run_scan(threshold, notify, catchup, popup).await {
                eprintln!("Scan error: {e:#}");
                std::process::exit(1);
            }
        }
        RunMode::Daemon { threshold } => {
            if let Err(e) = app::run_daemon(threshold).await {
                eprintln!("Daemon error: {e:#}");
                std::process::exit(1);
            }
        }
        RunMode::Service { action, threshold } => {
            if let Err(e) = app::run_service(&action, threshold) {
                eprintln!("Service error: {e:#}");
                std::process::exit(1);
            }
        }
    }
}
