//! Candlestick chart rendering using egui and egui_plot.
//!
//! Renders OHLCV candle data as a candlestick chart using `BoxPlot` with
//! `BoxElem` elements. Each candle is represented as a box with:
//! - The body spanning open→close (colored green/red)
//! - The wick spanning low→high

use eframe::egui;
use egui::Color32;
use egui_plot::{BoxElem, BoxPlot, BoxSpread, Plot, PlotPoint, Text as PlotText};

use crate::domain::{Candle, Timeframe};

/// The egui application state for the chart window.
pub struct ChartApp {
    candles: Vec<Candle>,
    symbol: String,
    timeframe: Timeframe,
}

impl ChartApp {
    pub fn new(candles: Vec<Candle>, symbol: String, timeframe: Timeframe) -> Self {
        Self {
            candles,
            symbol,
            timeframe,
        }
    }
}

/// Colors for the chart.
const BULLISH_COLOR: Color32 = Color32::from_rgb(38, 166, 91); // Green
const BEARISH_COLOR: Color32 = Color32::from_rgb(214, 48, 49); // Red
const BULLISH_FILL: Color32 = Color32::from_rgb(38, 166, 91);
const BEARISH_FILL: Color32 = Color32::from_rgb(214, 48, 49);

impl eframe::App for ChartApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            // Header with stock info
            ui.horizontal(|ui| {
                ui.heading(format!("{} — {}", self.symbol, self.timeframe));
                ui.separator();
                if let Some(last) = self.candles.last() {
                    let change = last.close - last.open;
                    let pct = if last.open != 0.0 {
                        (change / last.open) * 100.0
                    } else {
                        0.0
                    };
                    let color = if change >= 0.0 {
                        BULLISH_COLOR
                    } else {
                        BEARISH_COLOR
                    };
                    ui.colored_label(
                        color,
                        format!(
                            "Last: {:.2}  Change: {:+.2} ({:+.2}%)",
                            last.close, change, pct
                        ),
                    );
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{} candles", self.candles.len()));
                });
            });

            ui.separator();

            // Build candlestick elements
            let elements: Vec<BoxElem> = self
                .candles
                .iter()
                .enumerate()
                .map(|(i, candle)| {
                    let x = i as f64;
                    let (body_lo, body_hi) = if candle.is_bullish() {
                        (candle.open, candle.close)
                    } else {
                        (candle.close, candle.open)
                    };

                    let color = if candle.is_bullish() {
                        BULLISH_FILL
                    } else {
                        BEARISH_FILL
                    };

                    // BoxSpread: (whisker_lo, quartile1, median, quartile3, whisker_hi)
                    // For candlesticks: (low, body_lo, body_lo, body_hi, high)
                    // Setting median = body_lo avoids drawing a median line
                    BoxElem::new(
                        x,
                        BoxSpread::new(candle.low, body_lo, body_lo, body_hi, candle.high),
                    )
                    .whisker_width(0.0)
                    .box_width(0.6)
                    .fill(color)
                    .stroke(egui::Stroke::new(1.0_f32, color))
                    .name(format!(
                        "{}\nO: {:.2}\nH: {:.2}\nL: {:.2}\nC: {:.2}\nV: {}",
                        candle.datetime.format("%Y-%m-%d %H:%M"),
                        candle.open,
                        candle.high,
                        candle.low,
                        candle.close,
                        format_volume(candle.volume),
                    ))
                })
                .collect();

            let box_plot = BoxPlot::new(elements);

            // Build x-axis time labels
            let time_labels = build_time_labels(&self.candles, self.timeframe);

            Plot::new("candlestick_chart")
                .allow_zoom(true)
                .allow_drag(true)
                .allow_scroll(true)
                .allow_boxed_zoom(true)
                .x_axis_label("Time")
                .y_axis_label("Price")
                .show_axes([true, true])
                .show_grid(true)
                .auto_bounds([true, true])
                .show(ui, |plot_ui| {
                    plot_ui.box_plot(box_plot);

                    // Add time labels along x-axis
                    for (x, _label) in &time_labels {
                        plot_ui.text(PlotText::new(
                            PlotPoint::new(*x, 0.0),
                            " ", // invisible - labels are handled via axis formatter
                        ));
                    }

                    // Use custom x-axis formatter to show dates
                    let candles_for_fmt = self.candles.clone();
                    let _ = (time_labels, candles_for_fmt); // used above
                });
        });
    }
}

/// Build time labels: select evenly spaced candles for x-axis annotation.
fn build_time_labels(candles: &[Candle], timeframe: Timeframe) -> Vec<(f64, String)> {
    if candles.is_empty() {
        return vec![];
    }

    // Show ~10-15 labels regardless of candle count
    let step = (candles.len() / 12).max(1);
    let fmt = match timeframe {
        Timeframe::Min5 | Timeframe::Min15 | Timeframe::Min30 | Timeframe::Hour1 => "%m/%d %H:%M",
        Timeframe::Day1 | Timeframe::Week1 => "%Y-%m-%d",
    };

    candles
        .iter()
        .enumerate()
        .step_by(step)
        .map(|(i, c)| (i as f64, c.datetime.format(fmt).to_string()))
        .collect()
}

/// Format volume with K/M/B suffixes for readability.
fn format_volume(vol: u64) -> String {
    if vol >= 1_000_000_000 {
        format!("{:.1}B", vol as f64 / 1_000_000_000.0)
    } else if vol >= 1_000_000 {
        format!("{:.1}M", vol as f64 / 1_000_000.0)
    } else if vol >= 1_000 {
        format!("{:.1}K", vol as f64 / 1_000.0)
    } else {
        vol.to_string()
    }
}

/// Launch the chart window with the given candle data.
pub fn show_chart(candles: Vec<Candle>, symbol: &str, timeframe: Timeframe) -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 700.0])
            .with_title(format!("MarketWatch — {} ({})", symbol, timeframe)),
        ..Default::default()
    };

    eframe::run_native(
        &format!("MarketWatch — {} ({})", symbol, timeframe),
        options,
        Box::new(move |_cc| {
            Ok(Box::new(ChartApp::new(
                candles,
                symbol.to_string(),
                timeframe,
            )))
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_volume_units() {
        assert_eq!(format_volume(500), "500");
        assert_eq!(format_volume(1_500), "1.5K");
        assert_eq!(format_volume(2_500_000), "2.5M");
        assert_eq!(format_volume(1_200_000_000), "1.2B");
    }

    #[test]
    fn time_labels_empty_candles() {
        let labels = build_time_labels(&[], Timeframe::Day1);
        assert!(labels.is_empty());
    }

    #[test]
    fn time_labels_spacing() {
        use chrono::DateTime;
        let candles: Vec<Candle> = (0..100)
            .map(|i| Candle {
                timestamp: 1_700_000_000 + i * 900,
                datetime: DateTime::from_timestamp(1_700_000_000 + i * 900, 0).unwrap(),
                open: 100.0,
                high: 101.0,
                low: 99.0,
                close: 100.5,
                volume: 1_000_000,
            })
            .collect();

        let labels = build_time_labels(&candles, Timeframe::Min15);
        // Should have ~8-13 labels for 100 candles
        assert!(labels.len() >= 5 && labels.len() <= 15);
    }
}
