#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod theme;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_app_id("Hexaris")
            .with_inner_size([1240.0, 790.0])
            .with_min_inner_size([880.0, 560.0])
            .with_icon(theme::window_icon()),
        ..Default::default()
    };
    let paths = std::env::args_os()
        .skip(1)
        .map(std::path::PathBuf::from)
        .collect();
    eframe::run_native(
        "Hexaris",
        options,
        Box::new(|cc| Ok(Box::new(app::App::new(cc, paths)))),
    )
}
