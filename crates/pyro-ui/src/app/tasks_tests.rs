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

// ---------- Filtros y exportar (tarea 8) ----------

use pyro_core::filters::Filter;
use pyro_core::task::NoMonitor;

/// Avanza cuadros hasta que `done` se cumpla (como mucho 3 s).
fn wait_until(h: &mut Harness<'static, PyroApp>, done: impl Fn(&PyroApp) -> bool) {
    let start = Instant::now();
    while !done(h.state()) && start.elapsed() < Duration::from_secs(3) {
        std::thread::sleep(Duration::from_millis(2));
        h.run_steps(1);
    }
    assert!(done(h.state()), "no llegó a tiempo");
}

fn active_pixels(app: &PyroApp) -> image::RgbaImage {
    (*app.doc.active_layer().pixels).clone()
}

#[test]
fn vista_previa_solo_la_ultima() {
    let mut h = main_scene();
    let ctx = h.ctx.clone();
    let backup = active_pixels(h.state());
    h.state_mut().run(super::Action::Filter(Filter::GAUSSIAN_BLUR), &ctx);
    h.run_steps(1);
    for i in 0..20 {
        let d = h.state_mut().filter_dialog.as_mut().unwrap();
        d.filter = Filter::GaussianBlur { sigma: 1.0 + i as f32 * 0.5 };
        d.dirty = true;
        h.run_steps(1);
    }
    let last = Filter::GaussianBlur { sigma: 10.5 };
    wait_until(&mut h, |a| a.filter_dialog.as_ref().is_some_and(|d| d.preview.is_none() && d.shown == Some(last)));
    let sel = h.state().doc.selection.clone();
    assert!(active_pixels(h.state()) == last.run(&backup, sel.as_ref(), &NoMonitor).unwrap());
    h.get_by_label("Cancelar").click();
    h.run_steps(2);
    assert!(h.state().filter_dialog.is_none());
    assert!(active_pixels(h.state()) == backup);
}

#[test]
fn aplicar_espera_a_la_vista_previa() {
    let mut h = main_scene();
    let ctx = h.ctx.clone();
    let backup = active_pixels(h.state());
    h.state_mut().run(super::Action::Filter(Filter::GAUSSIAN_BLUR), &ctx);
    h.run_steps(1);
    let wanted = Filter::GaussianBlur { sigma: 6.0 };
    {
        let d = h.state_mut().filter_dialog.as_mut().unwrap();
        d.filter = wanted;
        d.dirty = true;
    }
    h.get_by_label("Aplicar").click();
    wait_until(&mut h, |a| a.filter_dialog.is_none());
    let sel = h.state().doc.selection.clone();
    assert!(active_pixels(h.state()) == wanted.run(&backup, sel.as_ref(), &NoMonitor).unwrap());
    h.state_mut().run(super::Action::Undo, &ctx);
    assert!(active_pixels(h.state()) == backup);
}

#[test]
fn invertir_en_segundo_plano() {
    let mut h = main_scene();
    let ctx = h.ctx.clone();
    let backup = active_pixels(h.state());
    h.state_mut().run(super::Action::Filter(Filter::Invert), &ctx);
    assert!(h.state().busy());
    wait_task(&mut h);
    let sel = h.state().doc.selection.clone();
    assert!(active_pixels(h.state()) == Filter::Invert.run(&backup, sel.as_ref(), &NoMonitor).unwrap());
    assert_eq!(h.state().status.text, "Invertir colores aplicado.");
    h.state_mut().run(super::Action::Undo, &ctx);
    assert!(active_pixels(h.state()) == backup);
}

#[test]
fn exportar_mientras_se_pinta() {
    let mut h = main_scene();
    let ctx = h.ctx.clone();
    let before = h.state().doc.composite();
    h.state_mut().run(super::Action::Export, &ctx);
    h.run_steps(1);
    h.state_mut().tool = Tool::Brush;
    // Dentro de la selección de la escena (120..350 × 90..240).
    h.state_mut().click((200.0, 150.0), Modifiers::NONE, &ctx);
    assert!(h.state().doc.composite() != before, "el pincel no pintó con exportar abierto");
    wait_until(&mut h, |a| a.export_dialog.as_ref().is_some_and(|d| d.bytes.is_some()));
    let dlg = h.state().export_dialog.as_ref().unwrap();
    assert!(*dlg.source == before);
    let decoded = image::load_from_memory(dlg.bytes.as_ref().unwrap()).unwrap().to_rgba8();
    assert!(decoded == before);
}

#[test]
fn cancelar_exportacion() {
    let mut h = main_scene();
    let ctx = h.ctx.clone();
    h.state_mut().run(super::Action::Export, &ctx);
    h.run_steps(1);
    h.state_mut().export_dialog.as_mut().unwrap().cancel_job();
    h.run_steps(3);
    let dlg = h.state().export_dialog.as_ref().expect("el diálogo sigue abierto");
    assert!(dlg.bytes.is_none() && dlg.job.is_none());
    assert!(h.query_by_label_contains("Cálculo cancelado").is_some());
}

// ---------- Arreglos de la revisión final ----------

#[test]
fn exportar_una_imagen_mayor_que_la_textura_maxima() {
    let mut h = main_scene();
    let ctx = h.ctx.clone();
    h.state_mut().reset_document(Document::new(9000, 300, Some([10, 20, 30, 255])));
    h.run_steps(2);
    h.state_mut().run(super::Action::Export, &ctx);
    wait_until(&mut h, |a| a.export_dialog.as_ref().is_some_and(|d| d.bytes.is_some()));
    let max = ctx.input(|i| i.max_texture_side).min(8192);
    let preview = h.state().export_dialog.as_ref().unwrap().preview.as_ref().expect("vista previa").size();
    assert!(preview[0] <= max && preview[1] <= max, "{preview:?} > {max}");
}

/// Tarea de abrir que termina enseguida y avisa por `done` al acabar.
fn quick_open(h: &mut Harness<'static, PyroApp>, path: PathBuf) -> std::sync::mpsc::Receiver<()> {
    let (tx, rx) = std::sync::mpsc::channel();
    let ctx = h.ctx.clone();
    let source = path.clone();
    let job: Job<TaskOutput> = Job::spawn(super::tasks::wake(&ctx), move |_| {
        let opened = pyro_formats::io::open_path(&source, &NoMonitor)
            .map_err(|e| pyro_services::jobs::JobError::Failed(e.to_string()))?;
        let _ = tx.send(());
        Ok(TaskOutput::Opened(opened))
    });
    h.state_mut().start_task(TaskKind::Open(path), job);
    rx
}

#[test]
fn esc_en_el_cuadro_del_resultado_no_lo_aplica() {
    let dir = tempfile::tempdir().unwrap();
    let path = temp_png(dir.path(), 64, 32);
    let mut h = main_scene();
    let done = quick_open(&mut h, path);
    done.recv().unwrap();
    std::thread::sleep(Duration::from_millis(20));
    h.press_key(Key::Escape);
    h.run_steps(2);
    assert_eq!((h.state().doc.width, h.state().doc.height), (800, 533));
    assert!(!h.state().busy());
}

#[test]
fn cancelar_no_espera_al_decodificador() {
    let mut h = main_scene();
    let ctx = h.ctx.clone();
    // Un trabajo que no consulta la cancelación (como un decodificador).
    let job: Job<TaskOutput> = Job::spawn(super::tasks::wake(&ctx), |_| {
        std::thread::sleep(Duration::from_secs(3));
        Err(pyro_services::jobs::JobError::Cancelled)
    });
    h.state_mut().start_task(TaskKind::Open(PathBuf::from("enorme.png")), job);
    h.run_steps(1);
    let start = Instant::now();
    h.state_mut().cancel_task();
    h.run_steps(1);
    assert!(!h.state().busy(), "sigue ocupada tras cancelar");
    assert!(start.elapsed() < Duration::from_millis(100));
    assert_eq!(h.state().status.text, "Cancelado.");
}
