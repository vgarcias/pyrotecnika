//! Prueba de concepto (fase 0): comprueba que los archivos que el sistema
//! pide abrir llegan a la app a través del bucle de eventos de winit/eframe.
//!
//! En macOS se ejecuta empaquetada como `.app` y se le envían archivos con
//! `open -a` (ver `packaging/macos/poc-open-files.sh`). Cada ruta recibida se
//! añade a `$HOME/pyro-open-files-probe.txt`. Se cierra sola a los 20 s.

use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use eframe::egui;

fn main() -> eframe::Result {
    pyro_platform::open_files::install();
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).expect("sin carpeta personal");
    let out = PathBuf::from(home).join("pyro-open-files-probe.txt");
    let start = Instant::now();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([320.0, 120.0]),
        ..Default::default()
    };
    eframe::run_simple_native("Pyrotecnika probe", options, move |ctx, _| {
        let paths = pyro_platform::open_files::take();
        if !paths.is_empty() {
            let text: String = paths.iter().map(|p| format!("{}\n", p.display())).collect();
            let written = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&out)
                .and_then(|mut f| f.write_all(text.as_bytes()));
            if let Err(e) = written {
                eprintln!("no se pudo escribir {}: {e}", out.display());
            }
        }
        if start.elapsed() > Duration::from_secs(20) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        egui::CentralPanel::default().show(ctx, |ui| ui.label("Esperando archivos…"));
        ctx.request_repaint_after(Duration::from_millis(200));
    })
}
