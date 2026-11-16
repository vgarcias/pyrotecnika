#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui;

/// Ícono de la ventana y de la barra de tareas (generado por
/// packaging/icons/make_icons.py).
const ICON_PNG: &[u8] = include_bytes!("../../../packaging/icons/256x256.png");

fn main() -> eframe::Result {
    pyro_platform::open_files::install();
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Pyrotecnika")
        .with_app_id("pyrotecnika")
        .with_inner_size([1200.0, 760.0])
        .with_maximized(true)
        .with_min_inner_size([900.0, 560.0])
        .with_drag_and_drop(true);
    if let Ok(icon) = eframe::icon_data::from_png_bytes(ICON_PNG) {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions { viewport, ..Default::default() };
    eframe::run_native("Pyrotecnika", options, Box::new(|cc| Ok(Box::new(pyro_ui::PyroApp::new(cc)))))
}
