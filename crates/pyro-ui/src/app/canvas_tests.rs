//! Lienzo por bloques: imágenes mayores que un bloque y subidas parciales.

use eframe::egui::{self, Modifiers};
use egui_kittest::Harness;

use super::canvas_tiles::Dirty;
use super::scenes::{scene, Scene};
use super::tools::Tool;
use super::PyroApp;
use crate::design::style::ThemeChoice;
use pyro_core::document::Document;

fn with_doc(doc: Document) -> Harness<'static, PyroApp> {
    let mut h = scene(Scene::Main, ThemeChoice::Dark, 1.0, egui::vec2(1280.0, 800.0));
    h.state_mut().reset_document(doc);
    h.run_steps(3);
    h
}

#[test]
fn lienzo_mayor_que_un_bloque() {
    let h = with_doc(Document::new(9000, 300, Some([200, 200, 200, 255])));
    let tiles = &h.state().canvas;
    assert_eq!(tiles.len(), 9);
    assert!(tiles.max_side() <= 1024, "{}", tiles.max_side());
}

#[test]
fn un_trazo_solo_sube_su_rectangulo() {
    let mut h = with_doc(Document::new(3000, 3000, Some([255, 255, 255, 255])));
    let ctx = h.ctx.clone();
    h.state_mut().tool = Tool::Brush;
    h.state_mut().brush.radius = 5.0;
    h.state_mut().click((10.0, 10.0), Modifiers::NONE, &ctx);
    h.run_steps(1);
    let tiles = &h.state().canvas;
    match tiles.last_dirty {
        Dirty::Rect { x0, y0, x1, y1 } => assert!(x1 <= 20 && y1 <= 20 && x0 < x1 && y0 < y1, "{x0},{y0}-{x1},{y1}"),
        ref other => panic!("se esperaba un rectángulo, llegó {other:?}"),
    }
    assert_eq!(tiles.last_uploads, 1);
}
