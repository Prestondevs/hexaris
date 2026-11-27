#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod theme;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1240.0, 790.0])
            .with_min_inner_size([880.0, 560.0])
            .with_icon(theme::window_icon()),
        ..Default::default()
    };
    let input = std::env::args_os().nth(1).map(std::path::PathBuf::from);
    eframe::run_native(
        "Hexaris",
        options,
        Box::new(|cc| Ok(Box::new(app::App::new(cc, input)))),
    )
}
