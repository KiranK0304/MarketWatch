//! CLI argument parsing using clap.
//!
//! If the timeframe is not provided via `-t`, an interactive selector is shown.

use clap::{Parser, Subcommand};
use dialoguer::Select;

use crate::domain::Timeframe;

/// Execution mode for the market terminal.
#[derive(Debug, PartialEq)]
pub enum RunMode {
    /// Native desktop candlestick chart GUI
    NativeGui(Timeframe),
    /// Interactive browser web dashboard
    WebDashboard { port: u16, open_browser: bool },
    /// Run scanner across top stocks universe
    Scan {
        threshold: f64,
        notify: bool,
        catchup: bool,
        popup: bool,
    },
    /// Manage automated systemd user service & timer
    Service { action: String, threshold: f64 },
    /// Run background scanner daemon
    Daemon { threshold: f64 },
}

/// Subcommands for the market CLI.
#[derive(Subcommand, Debug, PartialEq, Clone)]
pub enum Commands {
    /// Scan top 150 stocks for movers exceeding threshold (e.g. ±3%)
    Scan {
        /// Movement threshold percentage (default: 3.0%)
        #[arg(long, default_value = "3.0")]
        threshold: f64,

        /// Send desktop notification on movers found
        #[arg(long)]
        notify: bool,

        /// Only run if 09:30 or 15:30 slot was missed / pending (catch-up mode)
        #[arg(long)]
        catchup: bool,

        /// Show large graphical popup dialog
        #[arg(long)]
        popup: bool,
    },

    /// Manage background cron / systemd user timer with Persistent=true & autostart
    Service {
        /// Action: install, uninstall, status
        #[arg(default_value = "status")]
        action: String,

        /// Threshold percentage for scheduled alerts (default: 3.0%)
        #[arg(long, default_value = "3.0")]
        threshold: f64,
    },

    /// Run continuous scanner daemon (checks 09:30 & 15:30 IST)
    Daemon {
        /// Movement threshold percentage (default: 3.0%)
        #[arg(long, default_value = "3.0")]
        threshold: f64,
    },
}

/// Personal stock market terminal and mover scanner.
#[derive(Parser, Debug)]
#[command(
    name = "market",
    about = "Personal stock market visualization terminal & top movers scanner"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Timeframe for chart data: 5m, 15m, 30m, 1h, 1d, 1w
    #[arg(short = 't', long = "timeframe")]
    pub timeframe: Option<String>,

    /// Stock symbol (e.g. RELIANCE.NS). Defaults to first in config.
    #[arg(short = 's', long = "symbol")]
    pub symbol: Option<String>,

    /// Launch interactive web dashboard in the browser
    #[arg(short = 'w', long = "web")]
    pub web: bool,

    /// Port for web dashboard (default: 3000)
    #[arg(short = 'p', long = "port", default_value = "3000")]
    pub port: u16,

    /// Do not automatically open browser when starting web dashboard
    #[arg(long = "no-browser")]
    pub no_browser: bool,
}

impl Cli {
    /// Resolve application execution mode: Web Dashboard, Native Desktop GUI, Scanner, or Service.
    pub fn resolve_mode(&self) -> anyhow::Result<RunMode> {
        if let Some(ref cmd) = self.command {
            return match cmd {
                Commands::Scan {
                    threshold,
                    notify,
                    catchup,
                    popup,
                } => Ok(RunMode::Scan {
                    threshold: *threshold,
                    notify: *notify,
                    catchup: *catchup,
                    popup: *popup,
                }),
                Commands::Service { action, threshold } => Ok(RunMode::Service {
                    action: action.clone(),
                    threshold: *threshold,
                }),
                Commands::Daemon { threshold } => Ok(RunMode::Daemon {
                    threshold: *threshold,
                }),
            };
        }

        if self.web {
            return Ok(RunMode::WebDashboard {
                port: self.port,
                open_browser: !self.no_browser,
            });
        }

        match &self.timeframe {
            Some(tf_str) => {
                let tf: Timeframe = tf_str.parse().map_err(|e: String| anyhow::anyhow!(e))?;
                Ok(RunMode::NativeGui(tf))
            }
            None => prompt_mode(self.port, !self.no_browser),
        }
    }

    /// Resolve the timeframe: use the CLI arg if present, otherwise prompt interactively.
    #[allow(dead_code)]
    pub fn resolve_timeframe(&self) -> anyhow::Result<Timeframe> {
        match &self.timeframe {
            Some(tf_str) => {
                let tf: Timeframe = tf_str.parse().map_err(|e: String| anyhow::anyhow!(e))?;
                Ok(tf)
            }
            None => prompt_timeframe(),
        }
    }
}

/// Show an interactive terminal prompt to choose mode or timeframe.
fn prompt_mode(port: u16, open_browser: bool) -> anyhow::Result<RunMode> {
    let mut items = vec![
        "🌐 Launch Interactive Web Dashboard (Browser - Movers & Charts)",
        "⚡ Scan Top 150 Stocks Movers (±3% Alert & Notifications)",
    ];
    let tf_labels: Vec<&str> = Timeframe::ALL.iter().map(|t| t.label()).collect();
    for label in &tf_labels {
        items.push(label);
    }

    let selection = Select::new()
        .with_prompt("Choose mode / timeframe")
        .items(&items)
        .default(0)
        .interact()?;

    if selection == 0 {
        Ok(RunMode::WebDashboard { port, open_browser })
    } else if selection == 1 {
        Ok(RunMode::Scan {
            threshold: 3.0,
            notify: true,
            catchup: false,
            popup: false,
        })
    } else {
        Ok(RunMode::NativeGui(Timeframe::ALL[selection - 2]))
    }
}

/// Show an interactive terminal prompt for timeframe selection.
#[allow(dead_code)]
fn prompt_timeframe() -> anyhow::Result<Timeframe> {
    let labels: Vec<&str> = Timeframe::ALL.iter().map(|t| t.label()).collect();

    let selection = Select::new()
        .with_prompt("Choose timeframe")
        .items(&labels)
        .default(1) // Default to 15m
        .interact()?;

    Ok(Timeframe::ALL[selection])
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn parse_with_timeframe() {
        let cli = Cli::parse_from(["market", "-t", "15m"]);
        assert_eq!(cli.timeframe.as_deref(), Some("15m"));
    }

    #[test]
    fn parse_with_long_timeframe() {
        let cli = Cli::parse_from(["market", "--timeframe", "1d"]);
        assert_eq!(cli.timeframe.as_deref(), Some("1d"));
    }

    #[test]
    fn parse_without_timeframe() {
        let cli = Cli::parse_from(["market"]);
        assert!(cli.timeframe.is_none());
        assert!(cli.command.is_none());
    }

    #[test]
    fn parse_with_symbol() {
        let cli = Cli::parse_from(["market", "-t", "15m", "-s", "RELIANCE.NS"]);
        assert_eq!(cli.symbol.as_deref(), Some("RELIANCE.NS"));
    }

    #[test]
    fn resolve_valid_timeframe() {
        let cli = Cli {
            command: None,
            timeframe: Some("5m".to_string()),
            symbol: None,
            web: false,
            port: 3000,
            no_browser: false,
        };
        let tf = cli.resolve_timeframe().unwrap();
        assert_eq!(tf, Timeframe::Min5);
    }

    #[test]
    fn resolve_invalid_timeframe() {
        let cli = Cli {
            command: None,
            timeframe: Some("3m".to_string()),
            symbol: None,
            web: false,
            port: 3000,
            no_browser: false,
        };
        assert!(cli.resolve_timeframe().is_err());
    }

    #[test]
    fn parse_web_flag() {
        let cli = Cli::parse_from(["market", "--web", "--port", "8080", "--no-browser"]);
        assert!(cli.web);
        assert_eq!(cli.port, 8080);
        assert!(cli.no_browser);

        let mode = cli.resolve_mode().unwrap();
        assert_eq!(
            mode,
            RunMode::WebDashboard {
                port: 8080,
                open_browser: false
            }
        );
    }

    #[test]
    fn parse_scan_subcommand() {
        let cli = Cli::parse_from([
            "market",
            "scan",
            "--threshold",
            "2.5",
            "--notify",
            "--catchup",
        ]);
        let mode = cli.resolve_mode().unwrap();
        assert_eq!(
            mode,
            RunMode::Scan {
                threshold: 2.5,
                notify: true,
                catchup: true,
                popup: false,
            }
        );
    }

    #[test]
    fn parse_service_subcommand() {
        let cli = Cli::parse_from(["market", "service", "install", "--threshold", "3.0"]);
        let mode = cli.resolve_mode().unwrap();
        assert_eq!(
            mode,
            RunMode::Service {
                action: "install".to_string(),
                threshold: 3.0,
            }
        );
    }
}
