//! Tareas que cambian el documento: abrir (y, en la tarea 8, filtros y
//! exportar) en segundo plano, con progreso y cancelación.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use eframe::egui::{self, Key, Modifiers};
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;

use pyro_core::document::Document;
use pyro_services::jobs::Job;

use super::guard::{Pending, Tone};
use super::scenes::{scene, Scene};
use super::tasks::{TaskKind, TaskOutput};
use super::tools::Tool;
use super::PyroApp;
use crate::design::style::ThemeChoice;

fn main_scene() -> Harness<'static, PyroApp> {
    let mut h = scene(Scene::Main, ThemeChoice::Dark, 1.0, egui::vec2(1280.0, 800.0));
    h.run_steps(2);
    h
}

/// Avanza cuadros hasta que no quede tarea (como mucho 2 s).
pub(super) fn wait_task(h: &mut Harness<'static, PyroApp>) {
    let start = Instant::now();
    while h.state().busy() && start.elapsed() < Duration::from_secs(2) {
        std::thread::sleep(Duration::from_millis(2));
        h.run_steps(1);
    }
    assert!(!h.state().busy(), "la tarea no terminó");
    h.run_steps(1);
}

/// Tarea de abrir que no acaba hasta que se cancela.
fn blocking_task(h: &mut Harness<'static, PyroApp>) {
    let ctx = h.ctx.clone();
    let wake = super::tasks::wake(&ctx);
    let job: Job<TaskOutput> = Job::spawn(wake, |ctx| loop {
        ctx.check()?;
        std::thread::sleep(Duration::from_millis(1));
    });
    h.state_mut().start_task(TaskKind::Open(PathBuf::from("lento.png")), job);
    h.run_steps(2);
}

fn temp_png(dir: &Path, w: u32, h: u32) -> PathBuf {
    let path = dir.join("a.png");
    image::RgbaImage::new(w, h).save(&path).unwrap();
    path
}

#[test]
fn abrir_en_segundo_plano() {
    let dir = tempfile::tempdir().unwrap();
    let path = temp_png(dir.path(), 64, 32);
    let mut h = main_scene();
    let ctx = h.ctx.clone();
    h.state_mut().perform(Pending::Open(path.clone()), &ctx);
    assert!(h.state().busy());
    wait_task(&mut h);
    assert_eq!((h.state().doc.width, h.state().doc.height), (64, 32));
    assert!(h.state().settings.recent.files[0].ends_with("a.png"));
}

#[test]
fn abrir_y_cancelar_deja_el_documento() {
    let dir = tempfile::tempdir().unwrap();
    let path = temp_png(dir.path(), 64, 32);
    let mut h = main_scene();
    let ctx = h.ctx.clone();
    let before = (h.state().doc.width, h.state().doc.height, h.state().history.can_undo());
    h.state_mut().perform(Pending::Open(path), &ctx);
    h.state_mut().cancel_task();
    wait_task(&mut h);
    let app = h.state();
    assert_eq!((app.doc.width, app.doc.height, app.history.can_undo()), before);
    assert!(app.settings.recent.files.is_empty());
    assert_eq!(app.status.text, "Cancelado.");
}

#[test]
fn el_boton_cancelar_detiene_la_tarea() {
    let mut h = main_scene();
    blocking_task(&mut h);
    assert!(h.query_by_label_contains("Abriendo «lento.png»").is_some());
    h.get_by_label("Cancelar").click();
    wait_task(&mut h);
}

#[test]
fn esc_cancela_la_tarea() {
    let mut h = main_scene();
    blocking_task(&mut h);
    h.press_key(Key::Escape);
    wait_task(&mut h);
}

#[test]
fn durante_la_tarea_no_se_edita() {
    let mut h = main_scene();
    h.state_mut().tool = Tool::Brush;
    blocking_task(&mut h);
    h.press_key(Key::G);
    h.run_steps(2);
    assert_eq!(h.state().tool, Tool::Brush);
    let ctx = h.ctx.clone();
    let pixels = h.state().doc.layers[1].pixels.clone();
    h.state_mut().click((300.0, 100.0), Modifiers::NONE, &ctx);
    assert!(*h.state().doc.layers[1].pixels == *pixels);
    assert!(!h.state().available(super::Action::Undo));
    h.press_key_modifiers(Modifiers::COMMAND, Key::K);
    h.run_steps(2);
    assert!(h.state().palette.is_some());
    h.state_mut().cancel_task();
    wait_task(&mut h);
}

#[test]
fn resultado_de_otro_documento_se_descarta() {
    let dir = tempfile::tempdir().unwrap();
    let path = temp_png(dir.path(), 64, 32);
    let mut h = main_scene();
    let ctx = h.ctx.clone();
    h.state_mut().perform(Pending::Open(path), &ctx);
    h.state_mut().reset_document(Document::new(5, 5, None));
    wait_task(&mut h);
    assert_eq!((h.state().doc.width, h.state().doc.height), (5, 5));
}

#[test]
fn imagen_demasiado_grande_avisa() {
    let dir = tempfile::tempdir().unwrap();
    // PNG válido de 1×1 con el alto de la cabecera cambiado a 40000 (y su CRC).
    let path = temp_png(dir.path(), 1, 1);
    let mut bytes = std::fs::read(&path).unwrap();
    bytes[16..20].copy_from_slice(&40_000_u32.to_be_bytes());
    let crc = crc32fast::hash(&bytes[12..29]);
    bytes[29..33].copy_from_slice(&crc.to_be_bytes());
    std::fs::write(&path, bytes).unwrap();
    let mut h = main_scene();
    let ctx = h.ctx.clone();
    h.state_mut().perform(Pending::Open(path), &ctx);
    wait_task(&mut h);
    let status = &h.state().status;
    assert!(matches!(status.tone, Tone::Error));
    assert!(
        status.text.contains(
            "La imagen es demasiado grande (40000×1 px). Pyrotecnika abre imágenes de hasta 32 768 px por lado y 2 GB por capa."
        ),
        "{}",
        status.text
    );
}

#[test]
fn nuevo_respeta_los_limites() {
    let mut h = main_scene();
    let ctx = h.ctx.clone();
    h.state_mut().run(super::Action::New, &ctx);
    h.run_steps(2);
    if let Some(d) = h.state_mut().new_dialog.as_mut() {
        d.width = 32_768;
        d.height = 32_768;
    }
    h.run_steps(2);
    assert!(h.query_by_label_contains("demasiado grande (32768×32768 px)").is_some());
    h.get_by_label("Crear").click();
    h.run_steps(2);
    assert_eq!((h.state().doc.width, h.state().doc.height), (800, 533));
}
