#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui;

fn main() -> eframe::Result {
    pyro_platform::open_files::install();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Pyrotecnika")
            .with_app_id("pyrotecnika")
            .with_inner_size([1200.0, 760.0])
            .with_maximized(true)
            .with_min_inner_size([900.0, 560.0])
            .with_drag_and_drop(true),
        ..Default::default()
    };
    eframe::run_native("Pyrotecnika", options, Box::new(|cc| Ok(Box::new(pyro_ui::PyroApp::new(cc)))))
}
