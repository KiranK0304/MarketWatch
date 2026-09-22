//! System service integration for automated background scans and catch-up on Linux.
//!
//! Configures:
//! 1. `marketwatch-scanner.timer` with `Persistent=true` (09:30 AM & 03:30 PM IST)
//! 2. `marketwatch-scanner.service` for background scanning & notification
//! 3. `marketwatch-web.service` so `http://localhost:3000` is always running and ready
//! 4. `marketwatch-catchup.desktop` autostart for instant login/wake catchup

use std::fs;
use std::path::PathBuf;
use std::process::Command;

/// Setup and management of background systemd service and timer.
pub struct ServiceManager;

impl ServiceManager {
    /// Get path to the compiled market executable.
    pub fn executable_path() -> PathBuf {
        if let Ok(exe) = std::env::current_exe() {
            return exe;
        }

        let current_dir = std::env::current_dir().unwrap_or_default();
        let release_exe = current_dir.join("target/release/market");
        if release_exe.exists() {
            return release_exe;
        }

        current_dir.join("target/debug/market")
    }

    /// Install systemd user service & timer with `Persistent=true`, and desktop autostart entry.
    pub fn install(threshold: f64) -> anyhow::Result<()> {
        let home = std::env::var("HOME")
            .map_err(|_| anyhow::anyhow!("HOME environment variable not set"))?;
        let exe_path = Self::executable_path();
        let exe_str = exe_path.to_str().unwrap_or("market");
        let work_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from(&home));
        let work_dir_str = work_dir.to_str().unwrap_or("");

        let systemd_dir = PathBuf::from(&home).join(".config/systemd/user");
        let autostart_dir = PathBuf::from(&home).join(".config/autostart");
        fs::create_dir_all(&systemd_dir)?;
        fs::create_dir_all(&autostart_dir)?;

        // 1. Scanner service unit file
        let service_content = format!(
            r#"[Unit]
Description=MarketWatch 150 Stocks Movers Scanner
After=network-online.target
Wants=network-online.target

[Service]
Type=oneshot
WorkingDirectory={work_dir_str}
ExecStart={exe_str} scan --catchup --notify --threshold {threshold:.1}
Environment=DISPLAY=:0
Environment=WAYLAND_DISPLAY=wayland-0
Environment=DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/%U/bus

[Install]
WantedBy=default.target
"#
        );

        // 2. Timer unit file with Persistent=true
        let timer_content = r#"[Unit]
Description=MarketWatch Scheduled Scanner (09:30 AM & 03:30 PM IST)

[Timer]
OnCalendar=Mon..Fri 09:30:00
OnCalendar=Mon..Fri 15:30:00
Timezone=Asia/Kolkata
Persistent=true

[Install]
WantedBy=timers.target
"#;

        // 3. Persistent Web Server service so localhost:3000 is always alive
        let web_service_content = format!(
            r#"[Unit]
Description=MarketWatch Background Web Terminal (localhost:3000)
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
WorkingDirectory={work_dir_str}
ExecStart={exe_str} --web --port 3000 --no-browser
Restart=always
RestartSec=3

[Install]
WantedBy=default.target
"#
        );

        // 4. Desktop autostart file for login catch-up
        let desktop_autostart = format!(
            r#"[Desktop Entry]
Type=Application
Name=MarketWatch Catchup Scanner
Comment=Checks and runs missed MarketWatch 09:30 and 15:30 scans on startup/wake
Exec={exe_str} scan --catchup --notify --threshold {threshold:.1}
Hidden=false
NoDisplay=false
X-GNOME-Autostart-enabled=true
"#
        );

        let service_file = systemd_dir.join("marketwatch-scanner.service");
        let timer_file = systemd_dir.join("marketwatch-scanner.timer");
        let web_service_file = systemd_dir.join("marketwatch-web.service");
        let autostart_file = autostart_dir.join("marketwatch-catchup.desktop");

        fs::write(&service_file, service_content)?;
        fs::write(&timer_file, timer_content)?;
        fs::write(&web_service_file, web_service_content)?;
        fs::write(&autostart_file, desktop_autostart)?;

        println!("✓ Wrote {}", service_file.display());
        println!("✓ Wrote {}", timer_file.display());
        println!("✓ Wrote {}", web_service_file.display());
        println!("✓ Wrote {}", autostart_file.display());

        // Reload systemd user daemon and enable units
        let daemon_reload = Command::new("systemctl")
            .args(["--user", "daemon-reload"])
            .status()?;
        if !daemon_reload.success() {
            anyhow::bail!("systemctl --user daemon-reload failed");
        }

        let scanner_status = Command::new("systemctl")
            .args(["--user", "enable", "--now", "marketwatch-scanner.timer"])
            .status()?;
        if !scanner_status.success() {
            anyhow::bail!("failed to enable marketwatch-scanner.timer");
        }

        let web_status = Command::new("systemctl")
            .args(["--user", "enable", "--now", "marketwatch-web.service"])
            .status()?;
        if !web_status.success() {
            anyhow::bail!("failed to enable marketwatch-web.service");
        }

        println!("✓ Enabled and started systemd user timer: marketwatch-scanner.timer");
        println!("✓ Enabled and started background web server: marketwatch-web.service (localhost:3000)");
        println!("  Schedule: Mon-Fri at 09:30 AM and 03:30 PM IST");
        println!("  Persistent=true: If laptop is closed/off, scans catch up instantly when turned on!");
        println!("  Clicking the notification will directly open http://localhost:3000!");

        Ok(())
    }

    /// Uninstall systemd user services, timer, and desktop autostart entry.
    pub fn uninstall() -> anyhow::Result<()> {
        let home = std::env::var("HOME").unwrap_or_default();
        let _ = Command::new("systemctl")
            .args(["--user", "disable", "--now", "marketwatch-scanner.timer"])
            .status();

        let _ = Command::new("systemctl")
            .args(["--user", "disable", "--now", "marketwatch-web.service"])
            .status();

        let systemd_dir = PathBuf::from(&home).join(".config/systemd/user");
        let autostart_dir = PathBuf::from(&home).join(".config/autostart");

        let service_file = systemd_dir.join("marketwatch-scanner.service");
        let timer_file = systemd_dir.join("marketwatch-scanner.timer");
        let web_service_file = systemd_dir.join("marketwatch-web.service");
        let autostart_file = autostart_dir.join("marketwatch-catchup.desktop");

        let _ = fs::remove_file(service_file);
        let _ = fs::remove_file(timer_file);
        let _ = fs::remove_file(web_service_file);
        let _ = fs::remove_file(autostart_file);

        let _ = Command::new("systemctl")
            .args(["--user", "daemon-reload"])
            .status();

        println!("✓ Uninstalled MarketWatch background services, timer, and autostart entry.");
        Ok(())
    }

    /// Show status of systemd timer, web service, and state.
    pub fn status() {
        println!("\n=== MarketWatch Scheduled Timer ===");
        let _ = Command::new("systemctl")
            .args(["--user", "status", "marketwatch-scanner.timer", "--no-pager"])
            .status();

        println!("\n=== MarketWatch Web Server (localhost:3000) ===");
        let _ = Command::new("systemctl")
            .args(["--user", "status", "marketwatch-web.service", "--no-pager"])
            .status();
    }
}
