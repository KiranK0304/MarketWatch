//! Desktop notification dispatcher using native Linux notifications (`notify-send`).
//!
//! Includes action handling so clicking the notification automatically starts the
//! background web server (if not already running) and opens `http://localhost:3000`
//! in the browser.

use std::net::TcpStream;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use crate::domain::ScanResult;
use crate::scanner::ServiceManager;

/// Check if the local MarketWatch web server is listening on the port.
pub fn is_server_listening(port: u16) -> bool {
    TcpStream::connect_timeout(
        &std::net::SocketAddr::from(([127, 0, 0, 1], port)),
        Duration::from_millis(300),
    )
    .is_ok()
}

/// Ensure MarketWatch web server is running and open browser to `http://localhost:{port}`.
pub fn open_dashboard(port: u16) {
    if !is_server_listening(port) {
        println!(
            "MarketWatch web server is not running on port {port}. Starting it in background..."
        );
        let exe = ServiceManager::executable_path();
        let _ = Command::new(exe)
            .args(["--web", "--port", &port.to_string(), "--no-browser"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();

        // Wait up to 2 seconds for server to bind
        for _ in 0..10 {
            thread::sleep(Duration::from_millis(200));
            if is_server_listening(port) {
                break;
            }
        }
    }

    let url = format!("http://localhost:{port}");
    println!("Opening {url} in your default browser...");
    let _ = webbrowser::open(&url);
}

/// Dispatches a formatted, prominent desktop notification with click-to-open action.
pub fn send_desktop_notification(result: &ScanResult, slot_label: Option<&str>) -> bool {
    let title = match slot_label {
        Some(label) => format!(
            "📈 MarketWatch Movers — {label} (±{:.1}%)",
            result.threshold_percent
        ),
        None => format!(
            "📈 MarketWatch Movers Alert (±{:.1}%)",
            result.threshold_percent
        ),
    };

    let mut body = String::new();

    if result.movers.is_empty() {
        body.push_str(&format!(
            "Scanned {} stocks at {}.\nNo stocks moved beyond ±{:.1}% threshold.",
            result.total_scanned, result.scan_time, result.threshold_percent
        ));
    } else {
        // Gainers section
        let gainers: Vec<_> = result.movers.iter().filter(|m| m.is_gainer()).collect();
        if !gainers.is_empty() {
            body.push_str("<b>🟢 Top Gainers:</b>\n");
            for g in gainers.iter().take(4) {
                body.push_str(&format!(
                    "  • <b>{:<12}</b> {:>+6.2}%   ₹{:.2}\n",
                    g.symbol.replace(".NS", ""),
                    g.change_percent,
                    g.price
                ));
            }
        }

        // Losers section
        let losers: Vec<_> = result.movers.iter().filter(|m| !m.is_gainer()).collect();
        if !losers.is_empty() {
            if !gainers.is_empty() {
                body.push('\n');
            }
            body.push_str("<b>🔴 Top Losers:</b>\n");
            for l in losers.iter().take(4) {
                body.push_str(&format!(
                    "  • <b>{:<12}</b> {:>6.2}%   ₹{:.2}\n",
                    l.symbol.replace(".NS", ""),
                    l.change_percent,
                    l.price
                ));
            }
        }

        body.push_str(&format!(
            "\n📊 <b>{}/{}</b> stocks moved > {:.1}%\n🕒 {}\n👉 <i>Click to view charts & screener</i>",
            result.movers_count, result.total_scanned, result.threshold_percent, result.scan_time
        ));
    }

    // Spawn a background thread to send the notification with click action support
    thread::spawn(move || {
        let mut cmd = Command::new("notify-send");
        cmd.arg("-a")
            .arg("MarketWatch")
            .arg("-u")
            .arg("critical") // Critical stays visible and doesn't disappear in 3 seconds!
            .arg("-t")
            .arg("20000") // 20 seconds timeout if not dismissed
            .arg("-i")
            .arg("utilities-system-monitor")
            .arg("-A")
            .arg("default=Open Dashboard")
            .arg("-A")
            .arg("open=🚀 Open Web Dashboard & Charts")
            .arg(&title)
            .arg(&body);

        match cmd.output() {
            Ok(output) => {
                let action = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if action == "open" || action == "default" || action == "0" || action == "1" {
                    println!("\n[Notification] Clicked action: '{action}'. Launching dashboard...");
                    open_dashboard(3000);
                }
            }
            Err(e) => {
                eprintln!("Warning: notify-send execution error: {e}");
            }
        }
    });

    println!(
        "✓ Prominent desktop notification dispatched. Click it anytime to open localhost:3000!"
    );
    true
}

/// Display a full-sized graphical popup dialog using Zenity if available.
pub fn show_popup_dialog(result: &ScanResult, slot_label: Option<&str>) {
    let title = match slot_label {
        Some(label) => format!("MarketWatch Movers Alert — {label}"),
        None => format!(
            "MarketWatch Movers Alert (±{:.1}%)",
            result.threshold_percent
        ),
    };

    let mut text = format!(
        "<big><b>MarketWatch Movers Alert (±{:.1}%)</b></big>\n\n\
         Time: <b>{}</b>\n\
         Total Scanned: <b>{} stocks</b> | Movers Found: <b>{}</b>\n\n",
        result.threshold_percent, result.scan_time, result.total_scanned, result.movers_count
    );

    let gainers: Vec<_> = result.movers.iter().filter(|m| m.is_gainer()).collect();
    if !gainers.is_empty() {
        text.push_str("<b>🟢 Top Gainers:</b>\n");
        for g in gainers.iter().take(5) {
            text.push_str(&format!(
                "  • {:<12} {:>+6.2}%  (LTP: ₹{:.2})\n",
                g.symbol.replace(".NS", ""),
                g.change_percent,
                g.price
            ));
        }
        text.push('\n');
    }

    let losers: Vec<_> = result.movers.iter().filter(|m| !m.is_gainer()).collect();
    if !losers.is_empty() {
        text.push_str("<b>🔴 Top Losers:</b>\n");
        for l in losers.iter().take(5) {
            text.push_str(&format!(
                "  • {:<12} {:>6.2}%  (LTP: ₹{:.2})\n",
                l.symbol.replace(".NS", ""),
                l.change_percent,
                l.price
            ));
        }
        text.push('\n');
    }

    text.push_str("Would you like to open the MarketWatch interactive charts dashboard?");

    let mut cmd = Command::new("zenity");
    cmd.args([
        "--question",
        &format!("--title={title}"),
        &format!("--text={text}"),
        "--ok-label=🚀 Open Dashboard",
        "--cancel-label=Dismiss",
        "--width=480",
    ]);

    if let Ok(status) = cmd.status() {
        if status.success() {
            open_dashboard(3000);
        }
    }
}
