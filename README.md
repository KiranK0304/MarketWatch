# 📈 MarketWatch

> **High-performance financial terminal, movers screener, and multi-chart monitoring system for top Indian equities (NSE).** Built with Rust, Axum, Lightweight Charts, and Tokio async parallel scanning.

---

## ✨ Features

- **🌐 Interactive Web Terminal**: Single-page app with Lightweight Charts, White (default) and Black themes, OHLC inspector, and instant keyboard navigation.
- **⚡ Top 150 Movers Screener**: Live scanner for Nifty 50, Nifty Next 50, and Nifty Midcap 50 equities with configurable percentage thresholds (**1%, 2%, 3%, 5%, custom**).
- **🔲 Multi-Chart Grid View**: View interactive candlestick charts for **all qualifying movers simultaneously** side-by-side.
- **🔔 Automated Scheduled Alerts**: Runs automatically on trading days at:
  - **09:30 AM IST** (15 minutes after market open)
  - **03:30 PM IST** (market close)
- **⚡ Offline Catch-up System**: Powered by `systemd.timer` with `Persistent=true` and desktop autostart. If your laptop was closed/off during the scheduled scan time, it **automatically runs the scan and notifies you immediately upon boot or login**.
- **🖥️ Linux Desktop Notifications**: Rich, prominent notifications (`notify-send`) with clickable actions (`[🚀 Open Dashboard]`) that auto-launch the server and open the browser.
- **🎨 Cool SVG Favicon**: Dark squircle badge with dual glowing red & green candlesticks and neon momentum sparkline.

---

## 🚀 Quick Start

### 1. Run the Web Dashboard
```bash
cargo run --release -- --web
```
Or open: **[http://localhost:3000](http://localhost:3000)**

### 2. Manual Movers Scan
Scan all 150 stocks for moves exceeding ±3.0% and receive a desktop notification:
```bash
cargo run --release -- scan --threshold 3.0 --notify
```

To display a large graphical dialog window:
```bash
cargo run --release -- scan --threshold 3.0 --popup
```

### 3. Automated Systemd Background Service & Timers
Install and enable the background web server and scheduled catch-up timer:
```bash
cargo run --release -- service install --threshold 3.0
```
Check timer status:
```bash
systemctl --user list-timers marketwatch-scanner.timer
systemctl --user status marketwatch-web.service
```

---

## ⌨️ Keyboard Shortcuts

| Key | Action |
|---|---|
| `↑` / `↓` or `K` / `J` | Navigate up/down through stocks list |
| `1` – `6` | Switch timeframes (`5m`, `15m`, `30m`, `1h`, `1d`, `1w`) |
| `M` | Toggle between Universe and Movers Screener |
| `G` | Toggle Single Chart focus vs Multi-Chart Grid |
| `R` | Refresh current market data / re-run scan |
| `T` | Toggle White / Black Theme |
| `/` | Focus search filter |
| `ESC` | Clear search / close modal |

---

## 🛠️ Architecture

- **Backend**: Rust 2024 edition, `tokio` (multi-threaded async runtime with Semaphore for rate-limiting), `axum` (REST API & embedded web server), `reqwest`, `serde`, `chrono`.
- **Data Provider**: Yahoo Finance v8 chart API (public, resilient endpoint).
- **Frontend**: Single-page app with TradingView's Lightweight Charts 4.2.1, embedded directly inside the Rust binary.
- **Persistence**: `~/.config/marketwatch/scanner_state.json` tracks scan history, slots, and catch-up dates.

---

## 📜 License
MIT License
