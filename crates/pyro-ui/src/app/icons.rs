//! Iconos de la interfaz: subconjunto de la fuente de Lucide (licencia ISC),
//! en `assets/icons`. Al ser una fuente, egui los dibuja con su propio motor
//! de texto: se ven nítidos a cualquier escala y se colorean como el texto.
//! La fuente se registra junto con las demás en `design::fonts`.

use eframe::egui::{Color32, FontFamily, RichText};

pub(crate) const FONT: &[u8] = include_bytes!("../../assets/icons/lucide-subset.ttf");
pub(crate) const FAMILY: &str = "iconos";

pub(super) const MOVE: char = '\u{e125}';
pub(super) const SQUARE_DASHED: char = '\u{e1cb}';
pub(super) const CIRCLE_DASHED: char = '\u{e4b5}';
pub(super) const LASSO: char = '\u{e1ce}';
pub(super) const WAND_SPARKLES: char = '\u{e35b}';
pub(super) const PAINTBRUSH: char = '\u{e2e7}';
pub(super) const ERASER: char = '\u{e28f}';
pub(super) const STAMP: char = '\u{e3bf}';
pub(super) const DROPLET: char = '\u{e0b8}';
pub(super) const PAINT_BUCKET: char = '\u{e2e6}';
pub(super) const PIPETTE: char = '\u{e13f}';
pub(super) const CIRCLE_HELP: char = '\u{e082}';
pub(super) const TRIANGLE_ALERT: char = '\u{e193}';
pub(super) const CIRCLE_X: char = '\u{e088}';

/// Texto con un solo icono, del tamaño y color indicados.
pub(super) fn text(glyph: char, size: f32, color: Color32) -> RichText {
    RichText::new(glyph.to_string()).family(FontFamily::Name(FAMILY.into())).size(size).color(color)
}
