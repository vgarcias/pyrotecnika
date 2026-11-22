//! Varias imágenes en pestañas.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use eframe::egui::{self, Key, Modifiers};
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;
use image::{Rgba, RgbaImage};

use pyro_core::document::Document;
use pyro_core::filters::Filter;
use pyro_core::history::MEMORY_BUDGET;
use pyro_core::task::NoMonitor;

use super::guard::Pending;
use super::scenes::{scene, Scene};
use super::tab::Tab;
use super::tasks_tests::wait_task;
use super::tools::Tool;
use super::{Action, PyroApp};
use crate::design::style::ThemeChoice;

fn app() -> Harness<'static, PyroApp> {
    let mut h = scene(Scene::Main, ThemeChoice::Dark, 1.0, egui::vec2(1280.0, 800.0));
    h.run_steps(2);
    h
}

fn png(dir: &Path, name: &str, w: u32, h: u32) -> PathBuf {
    let path = dir.join(name);
    RgbaImage::from_pixel(w, h, Rgba([200, 200, 200, 255])).save(&path).unwrap();
    path
}

fn open(h: &mut Harness<'static, PyroApp>, path: &Path) {
    let ctx = h.ctx.clone();
    h.state_mut().perform(Pending::Open(path.to_path_buf()), &ctx);
    wait_task(h);
}

/// Avanza hasta que ninguna pestaña tenga tarea.
fn wait_all(h: &mut Harness<'static, PyroApp>) {
    let start = Instant::now();
    while (0..h.state().tab_count()).any(|i| h.state().tab_at(i).task.is_some())
        && start.elapsed() < Duration::from_secs(3)
    {
        std::thread::sleep(Duration::from_millis(2));
        h.run_steps(1);
    }
    h.run_steps(1);
}

fn paint(h: &mut Harness<'static, PyroApp>, x: f32, y: f32) {
    let ctx = h.ctx.clone();
    let app = h.state_mut();
    app.tool = Tool::Brush;
    app.tab.doc.selection = None;
    app.click((x, y), Modifiers::NONE, &ctx);
}

#[test]
fn abrir_dos_archivos_da_dos_pestanas() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = app();
    open(&mut h, &png(dir.path(), "a.png", 40, 30));
    open(&mut h, &png(dir.path(), "b.png", 50, 20));
    // La «Sin título» intacta del arranque se sustituyó por a.png.
    let app = h.state();
    assert_eq!(app.tab_count(), 3, "la escena trae su documento de muestra");
    assert_eq!(app.tab_title(app.active), "b.png");
    assert_eq!(app.tab_title(app.active - 1), "a.png");
}

#[test]
fn pintar_en_una_no_cambia_la_otra() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = app();
    open(&mut h, &png(dir.path(), "a.png", 40, 30));
    open(&mut h, &png(dir.path(), "b.png", 40, 30));
    let a = h.state().active - 1;
    let before = h.state().tab_at(a).doc.composite();
    paint(&mut h, 10.0, 10.0);
    assert!(h.state().tab.history.can_undo());
    h.state_mut().switch_to(a);
    assert!(h.state().tab.doc.composite() == before);
    assert!(!h.state().tab.history.can_undo());
}

#[test]
fn tarea_de_otra_pestana_se_aplica_a_la_suya() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = app();
    open(&mut h, &png(dir.path(), "a.png", 300, 300));
    open(&mut h, &png(dir.path(), "b.png", 300, 300));
    let (a, b) = (h.state().active - 1, h.state().active);
    h.state_mut().switch_to(a);
    let ctx = h.ctx.clone();
    let original = (*h.state().tab.doc.active_layer().pixels).clone();
    let b_before = h.state().tab_at(b).doc.composite();
    h.state_mut().run(Action::Filter(Filter::Invert), &ctx);
    h.state_mut().switch_to(b);
    wait_all(&mut h);
    let app = h.state();
    let want = Filter::Invert.run(&original, None, &NoMonitor).unwrap();
    assert!(*app.tab_at(a).doc.active_layer().pixels == want);
    assert!(app.tab_at(a).history.can_undo());
    assert!(app.tab.doc.composite() == b_before);
    assert_eq!(app.active, b);
}

#[test]
fn abrir_un_archivo_ya_abierto_activa_su_pestana() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = app();
    let a = png(dir.path(), "a.png", 40, 30);
    open(&mut h, &a);
    open(&mut h, &png(dir.path(), "b.png", 40, 30));
    let count = h.state().tab_count();
    open(&mut h, &a);
    assert_eq!(h.state().tab_count(), count);
    assert_eq!(h.state().tab_title(h.state().active), "a.png");
}

#[test]
fn atajos_de_pestanas() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = app();
    open(&mut h, &png(dir.path(), "a.png", 40, 30));
    let start = h.state().active;
    h.press_key_modifiers(Modifiers::COMMAND, Key::Tab);
    h.run_steps(2);
    assert_ne!(h.state().active, start);
    h.press_key_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::Tab);
    h.run_steps(2);
    assert_eq!(h.state().active, start);
    let count = h.state().tab_count();
    h.press_key_modifiers(Modifiers::COMMAND, Key::W);
    h.run_steps(2);
    assert_eq!(h.state().tab_count(), count - 1);
}

#[test]
fn cerrar_con_cambios_pregunta() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = app();
    open(&mut h, &png(dir.path(), "a.png", 40, 30));
    paint(&mut h, 10.0, 10.0);
    let count = h.state().tab_count();
    h.press_key_modifiers(Modifiers::COMMAND, Key::W);
    h.run_steps(2);
    assert!(matches!(h.state().confirm, Some(Pending::CloseTab)));
    h.get_by_label("Cancelar").click();
    h.run_steps(2);
    assert_eq!(h.state().tab_count(), count);
    assert!(h.state().confirm.is_none());
}

#[test]
fn cerrar_la_ultima_deja_sin_titulo() {
    let mut h = egui_kittest::Harness::builder()
        .with_size(egui::vec2(1280.0, 800.0))
        .build_eframe(|cc| PyroApp::with_config(cc, None));
    h.run_steps(2);
    assert_eq!(h.state().tab_title(0), "Sin título 1");
    h.press_key_modifiers(Modifiers::COMMAND, Key::W);
    h.run_steps(2);
    assert_eq!(h.state().tab_count(), 1);
    assert_eq!(h.state().tab_title(0), "Sin título 2");
}

#[test]
fn presupuesto_comun() {
    let mut h = app();
    for _ in 0..2 {
        h.state_mut().add_tab(Tab::new(Document::new(4096, 4096, None), 0));
        for v in 1..=7_u8 {
            let app = h.state_mut();
            app.tab.history.push(&app.tab.doc);
            for p in app.tab.doc.layers[0].image_mut().pixels_mut() {
                *p = Rgba([v, v, v, 255]);
            }
            let doc = &app.tab.doc;
            app.tab.history.seal(doc);
            app.enforce_history_budget();
        }
    }
    let app = h.state();
    let total: usize = (0..app.tab_count()).map(|i| app.tab_at(i).history.bytes()).sum();
    assert!(total <= MEMORY_BUDGET, "{total}");
}

// ---------- Guardar (tarea 4) ----------

/// Avanza hasta que ninguna pestaña esté guardando.
fn wait_saves(h: &mut Harness<'static, PyroApp>) {
    let start = Instant::now();
    while (0..h.state().tab_count()).any(|i| h.state().tab_at(i).save.is_some())
        && start.elapsed() < Duration::from_secs(5)
    {
        std::thread::sleep(Duration::from_millis(2));
        h.run_steps(1);
    }
    h.run_steps(1);
}

fn save_to(h: &mut Harness<'static, PyroApp>, path: Option<PathBuf>) {
    h.state_mut().save_dialog_override = Some(path);
}

#[test]
fn guardar_un_documento_nuevo_pide_ruta() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = app();
    paint(&mut h, 300.0, 100.0);
    let target = dir.path().join("x.ora");
    save_to(&mut h, Some(target.clone()));
    h.press_key_modifiers(Modifiers::COMMAND, Key::S);
    h.run_steps(1);
    wait_saves(&mut h);
    let back = pyro_formats::ora::decode_ora(&std::fs::read(&target).unwrap(), &NoMonitor).unwrap();
    assert_eq!(back.document.layers.len(), h.state().tab.doc.layers.len());
    assert!(!h.state().tab.history.is_modified());
    assert_eq!(h.state().tab_title(h.state().active), "x.ora");
}

#[test]
fn guardar_como_propone_ora_junto_al_origen() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = app();
    open(&mut h, &png(dir.path(), "cenote.png", 40, 30));
    assert_eq!(super::save::suggested_save_path(&h.state().tab), dir.path().join("cenote.ora"));
}

#[test]
fn nunca_sobrescribe_el_png() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = app();
    let original = png(dir.path(), "cenote.png", 40, 30);
    let bytes = std::fs::read(&original).unwrap();
    open(&mut h, &original);
    paint(&mut h, 10.0, 10.0);
    save_to(&mut h, Some(original.clone()));
    h.press_key_modifiers(Modifiers::COMMAND, Key::S);
    h.run_steps(1);
    wait_saves(&mut h);
    assert!(matches!(h.state().status.tone, super::guard::Tone::Error));
    assert_eq!(std::fs::read(&original).unwrap(), bytes);
}

#[test]
fn editar_mientras_se_guarda_mantiene_la_marca() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = app();
    let target = dir.path().join("x.ora");
    let before = h.state().tab.doc.composite();
    paint(&mut h, 200.0, 150.0);
    let saved = h.state().tab.doc.composite();
    save_to(&mut h, Some(target.clone()));
    let ctx = h.ctx.clone();
    h.state_mut().run(Action::Save, &ctx);
    paint(&mut h, 250.0, 180.0);
    wait_saves(&mut h);
    assert!(h.state().tab.history.is_modified());
    let back = pyro_formats::ora::decode_ora(&std::fs::read(&target).unwrap(), &NoMonitor).unwrap();
    assert!(back.document.composite() == saved);
    assert!(saved != before);
}

#[test]
fn exportar_ya_no_quita_la_marca() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = app();
    paint(&mut h, 200.0, 150.0);
    let ctx = h.ctx.clone();
    h.state_mut().run(Action::Export, &ctx);
    let start = Instant::now();
    while h.state().tab.export_dialog.as_ref().is_some_and(|d| d.bytes.is_none())
        && start.elapsed() < Duration::from_secs(3)
    {
        std::thread::sleep(Duration::from_millis(2));
        h.run_steps(1);
    }
    h.state_mut().export_override = Some(dir.path().join("out.png"));
    h.state_mut().save_export();
    assert!(dir.path().join("out.png").exists());
    assert!(h.state().tab.history.is_modified());
    assert_eq!(h.state().history_status_label(), "Cambios sin guardar");
}

#[test]
fn guardar_y_cerrar() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = app();
    open(&mut h, &png(dir.path(), "a.png", 40, 30));
    paint(&mut h, 10.0, 10.0);
    let count = h.state().tab_count();
    save_to(&mut h, Some(dir.path().join("a.ora")));
    h.press_key_modifiers(Modifiers::COMMAND, Key::W);
    h.run_steps(2);
    h.get_by_label("Guardar").click();
    h.run_steps(1);
    wait_saves(&mut h);
    h.run_steps(2);
    assert!(dir.path().join("a.ora").exists());
    assert_eq!(h.state().tab_count(), count - 1);
}

#[test]
fn fallo_al_guardar_avisa_y_no_cierra() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = app();
    open(&mut h, &png(dir.path(), "a.png", 40, 30));
    paint(&mut h, 10.0, 10.0);
    let count = h.state().tab_count();
    save_to(&mut h, Some(dir.path().join("no/existe/a.ora")));
    h.press_key_modifiers(Modifiers::COMMAND, Key::W);
    h.run_steps(2);
    h.get_by_label("Guardar").click();
    h.run_steps(1);
    wait_saves(&mut h);
    assert!(matches!(h.state().status.tone, super::guard::Tone::Error));
    assert_eq!(h.state().tab_count(), count);
}

// ---------- Salir con varias pestañas con cambios (tarea 5) ----------

/// Dos pestañas con cambios: «a.png» (con `a.ora` como destino) y una
/// «Sin título» nueva.
fn two_unsaved(dir: &Path) -> Harness<'static, PyroApp> {
    let mut h = app();
    open(&mut h, &png(dir, "a.png", 40, 30));
    paint(&mut h, 10.0, 10.0);
    h.state_mut().tab.ora_path = Some(dir.join("a.ora"));
    let ctx = h.ctx.clone();
    h.state_mut().perform(Pending::New { width: 40, height: 30, transparent: false }, &ctx);
    paint(&mut h, 10.0, 10.0);
    h
}

#[test]
fn cerrar_con_varias_pregunta_una_vez() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = two_unsaved(dir.path());
    let ctx = h.ctx.clone();
    h.state_mut().request(Pending::Quit, &ctx);
    h.run_steps(2);
    assert!(h.state().quit_window);
    assert!(h.state().confirm.is_none());
    assert!(h.query_by_label_contains("• a.ora").is_some());
    assert!(h.query_by_label_contains("• Sin título 2").is_some());
}

#[test]
fn guardar_todo_y_salir() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = two_unsaved(dir.path());
    let ctx = h.ctx.clone();
    h.state_mut().request(Pending::Quit, &ctx);
    h.run_steps(2);
    save_to(&mut h, Some(dir.path().join("nuevo.ora")));
    h.get_by_label("Guardar todo").click();
    h.run_steps(1);
    wait_saves(&mut h);
    h.run_steps(2);
    assert!(dir.path().join("a.ora").exists());
    assert!(dir.path().join("nuevo.ora").exists());
    assert!(h.state().closing);
}

#[test]
fn cancelar_guardar_como_cancela_el_cierre() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = two_unsaved(dir.path());
    let count = h.state().tab_count();
    let ctx = h.ctx.clone();
    h.state_mut().request(Pending::Quit, &ctx);
    h.run_steps(2);
    save_to(&mut h, None);
    h.get_by_label("Guardar todo").click();
    h.run_steps(1);
    wait_saves(&mut h);
    h.run_steps(2);
    assert!(!h.state().closing);
    assert_eq!(h.state().tab_count(), count);
}

#[test]
fn descartar_todo() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = two_unsaved(dir.path());
    let ctx = h.ctx.clone();
    h.state_mut().request(Pending::Quit, &ctx);
    h.run_steps(2);
    h.get_by_label("Descartar todo").click();
    h.run_steps(2);
    assert!(h.state().closing);
    assert!(!dir.path().join("a.ora").exists());
}
