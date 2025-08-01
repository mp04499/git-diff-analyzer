//! Graphical UI for analyzing GitHub pull requests.

use eframe::NativeOptions;

mod ui;
use ui::DiffAnalyzerApp;

fn main() -> eframe::Result<()> {
    let options = NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([800.0, 600.0])
            .with_title("Git Diff Analyzer")
            .with_icon(eframe::icon_data::from_png_bytes(&[]).unwrap_or_default()),
        ..Default::default()
    };
    eframe::run_native(
        "Git Diff Analyzer",
        options,
        Box::new(|_cc| Ok(Box::<DiffAnalyzerApp>::default())),
    )
}