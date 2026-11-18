//! Pruebas de la app con el catálogo de comandos: teclado, menús,
//! recientes y preferencias persistentes.

use std::path::PathBuf;

use eframe::egui::{self, accesskit::Role, Key, Modifiers};
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;

use super::scenes::{scene, Scene};
use super::tools::Tool;
use super::{Action, PyroApp};
use crate::commands::chord::Chord;
use crate::design::style::ThemeChoice;

fn main_scene() -> Harness<'static, PyroApp> {
    let mut h = scene(Scene::Main, ThemeChoice::Dark, 1.0, egui::vec2(1280.0, 800.0));
    h.run_steps(2);
    h
}

fn ctrl() -> Modifiers {
    Modifiers::COMMAND
}

#[test]
fn exportar_con_el_atajo_de_fireworks() {
    let mut h = main_scene();
    h.press_key_modifiers(ctrl() | Modifiers::SHIFT, Key::R);
    h.run_steps(2);
    assert!(h.state().export_dialog.is_some());
}

#[test]
fn ctrl_e_ya_no_exporta() {
    let mut h = main_scene();
    h.press_key_modifiers(ctrl(), Key::E);
    h.run_steps(2);
    assert!(h.state().export_dialog.is_none());
}

#[test]
fn ctrl_mas_del_teclado_espanol_acerca() {
    let mut h = main_scene();
    let z0 = h.state().zoom;
    h.press_key_modifiers(ctrl(), Key::Plus);
    h.run_steps(2);
    let z1 = h.state().zoom;
    assert!(z1 > z0, "Ctrl++ no acercó: {z0} → {z1}");
    h.press_key_modifiers(ctrl(), Key::Equals);
    h.run_steps(2);
    assert!(h.state().zoom > z1, "Ctrl+= no acercó");
}

#[test]
fn m_alterna_marco_y_elipse() {
    let mut h = main_scene();
    h.state_mut().tool = Tool::Brush;
    let mut seen = Vec::new();
    for _ in 0..3 {
        h.press_key(Key::M);
        h.run_steps(2);
        seen.push(h.state().tool);
    }
    assert_eq!(seen, [Tool::Marquee, Tool::Ellipse, Tool::Marquee]);
}

#[test]
fn ningun_atajo_con_un_campo_de_texto_enfocado() {
    let mut h = main_scene();
    h.state_mut().tool = Tool::Brush;
    h.get_all_by_role(Role::TextInput).next().expect("campo «Nombre» de la capa").focus();
    h.run_steps(2);
    h.press_key_modifiers(ctrl() | Modifiers::ALT, Key::E);
    h.press_key(Key::E);
    h.run_steps(2);
    assert_eq!(h.state().doc.layers.len(), 2);
    assert_eq!(h.state().tool, Tool::Brush);
}

#[test]
fn el_menu_muestra_el_atajo_vigente() {
    let mut h = main_scene();
    assert_eq!(h.state().menu_shortcut("file.export"), "Ctrl+Mayús+R");
    h.state_mut().keymap.assign("file.export", Chord::parse("Ctrl+E").unwrap());
    h.state_mut().keymap.remove("file.export", Chord::parse("Ctrl+Shift+R").unwrap());
    assert_eq!(h.state().menu_shortcut("file.export"), "Ctrl+E");
    h.press_key_modifiers(ctrl(), Key::E);
    h.run_steps(2);
    assert!(h.state().export_dialog.is_some());
}

#[test]
fn reciente_que_ya_no_existe() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = main_scene();
    let ctx = h.ctx.clone();
    let size = (h.state().doc.width, h.state().doc.height);
    {
        let app = h.state_mut();
        app.settings.recent.files = vec![dir.path().join("borrado.png")];
        app.run(Action::OpenRecent(0), &ctx);
    }
    let app = h.state();
    assert!(app.settings.recent.files.is_empty());
    assert!(matches!(app.status.tone, super::Tone::Warn));
    assert_eq!((app.doc.width, app.doc.height), size);
}

#[test]
fn abrir_registra_el_reciente() {
    let dir = tempfile::tempdir().unwrap();
    let path: PathBuf = dir.path().join("a.png");
    image::RgbaImage::new(4, 4).save(&path).unwrap();
    let mut h = main_scene();
    let ctx = h.ctx.clone();
    h.state_mut().open_path(path.clone(), &ctx);
    assert_eq!(h.state().settings.recent.files.len(), 1);
    assert!(h.state().settings.recent.files[0].ends_with("a.png"));
}

#[test]
fn tema_y_escala_se_recuerdan() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().to_path_buf();
    {
        let mut h = Harness::builder().build_eframe(|cc| PyroApp::with_config(cc, Some(p.clone())));
        h.run_steps(2);
        let ctx = h.ctx.clone();
        h.state_mut().run(Action::SetTheme(ThemeChoice::Light), &ctx);
        h.state_mut().run(Action::UiScaleUp, &ctx);
    }
    let mut h = Harness::builder().build_eframe(|cc| PyroApp::with_config(cc, Some(p.clone())));
    h.run_steps(2);
    assert_eq!(h.state().theme, ThemeChoice::Light);
    assert_eq!(h.state().ui_scale, 1.1);
}
