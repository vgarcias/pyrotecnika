//! Tokens de color de los tres temas y constantes de las superposiciones
//! sobre la imagen (docs/superpowers/specs/2026-11-16-fase1-sistema-diseno-design.md §1).

use eframe::egui::Color32;

/// Colores semánticos de un tema. Todos son grises: lo activo se marca con
/// `active`/`on_active` (inversión), nunca con un tono.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Palette {
    pub(crate) bg_app: Color32,
    pub(crate) bg_panel: Color32,
    pub(crate) bg_raised: Color32,
    pub(crate) bg_hover: Color32,
    pub(crate) bg_pressed: Color32,
    pub(crate) bg_canvas: Color32,
    pub(crate) border_subtle: Color32,
    pub(crate) border_control: Color32,
    pub(crate) text_primary: Color32,
    pub(crate) text_secondary: Color32,
    pub(crate) text_disabled: Color32,
    pub(crate) active: Color32,
    pub(crate) on_active: Color32,
    pub(crate) focus_ring: Color32,
    /// Grosor de bordes y divisores: 2 px en alto contraste.
    pub(crate) stroke_width: f32,
}

const fn gray(v: u8) -> Color32 {
    Color32::from_rgb(v, v, v)
}

pub(crate) const DARK: Palette = Palette {
    bg_app: gray(0x16),
    bg_panel: gray(0x1F),
    bg_raised: gray(0x2A),
    bg_hover: gray(0x30),
    bg_pressed: gray(0x33),
    bg_canvas: gray(0x12),
    border_subtle: gray(0x33),
    border_control: gray(0x7A),
    text_primary: gray(0xED),
    text_secondary: gray(0xA6),
    text_disabled: gray(0x6B),
    active: gray(0xED),
    on_active: gray(0x12),
    focus_ring: gray(0xFF),
    stroke_width: 1.0,
};

pub(crate) const LIGHT: Palette = Palette {
    bg_app: gray(0xEA),
    bg_panel: gray(0xF7),
    bg_raised: gray(0xFF),
    bg_hover: gray(0xED),
    bg_pressed: gray(0xDC),
    bg_canvas: gray(0xD6),
    border_subtle: gray(0xDC),
    border_control: gray(0x85),
    text_primary: gray(0x18),
    text_secondary: gray(0x57),
    text_disabled: gray(0x9A),
    active: gray(0x18),
    on_active: gray(0xFF),
    focus_ring: gray(0x00),
    stroke_width: 1.0,
};

pub(crate) const HIGH_CONTRAST: Palette = Palette {
    bg_app: gray(0x00),
    bg_panel: gray(0x00),
    bg_raised: gray(0x00),
    bg_hover: gray(0x1A),
    bg_pressed: gray(0x33),
    bg_canvas: gray(0x00),
    border_subtle: gray(0xFF),
    border_control: gray(0xFF),
    text_primary: gray(0xFF),
    text_secondary: gray(0xFF),
    text_disabled: gray(0x8C),
    active: gray(0xFF),
    on_active: gray(0x00),
    focus_ring: gray(0xFF),
    stroke_width: 2.0,
};

/// Superposiciones sobre la imagen: siempre en par blanco + negro para que
/// se vean sobre cualquier foto. No dependen del tema.
pub(crate) const OVERLAY_LIGHT: Color32 = Color32::WHITE;
pub(crate) const OVERLAY_DARK: Color32 = Color32::BLACK;
/// Tablero de transparencia.
pub(crate) const CHECKER_A: Color32 = gray(204);
pub(crate) const CHECKER_B: Color32 = gray(255);

/// Razón de contraste WCAG 2.x entre dos colores (1 a 21).
pub(crate) fn contrast(a: Color32, b: Color32) -> f32 {
    fn luminance(c: Color32) -> f32 {
        let ch = |v: u8| {
            let v = f32::from(v) / 255.0;
            if v <= 0.039_28 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * ch(c.r()) + 0.7152 * ch(c.g()) + 0.0722 * ch(c.b())
    }
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contraste_de_referencia() {
        assert!((contrast(Color32::WHITE, Color32::BLACK) - 21.0).abs() < 0.01);
        assert!(
            (contrast(Color32::from_rgb(0xED, 0xED, 0xED), Color32::from_rgb(0x1F, 0x1F, 0x1F)) - 14.08).abs() < 0.05
        );
    }

    #[test]
    fn las_tres_paletas_cumplen_wcag() {
        for (name, p) in [("oscuro", DARK), ("claro", LIGHT), ("alto contraste", HIGH_CONTRAST)] {
            for (par, ratio, min) in [
                ("texto/panel", contrast(p.text_primary, p.bg_panel), 4.5),
                ("secundario/panel", contrast(p.text_secondary, p.bg_panel), 4.5),
                ("secundario/raised", contrast(p.text_secondary, p.bg_raised), 4.5),
                ("on_active/active", contrast(p.on_active, p.active), 4.5),
                ("active/panel", contrast(p.active, p.bg_panel), 3.0),
                ("borde/raised", contrast(p.border_control, p.bg_raised), 3.0),
                ("foco/panel", contrast(p.focus_ring, p.bg_panel), 3.0),
            ] {
                assert!(ratio >= min, "{name}: {par} = {ratio:.2} < {min}");
            }
        }
    }

    #[test]
    fn la_interfaz_es_neutra() {
        for p in [DARK, LIGHT, HIGH_CONTRAST] {
            for c in [
                p.bg_app,
                p.bg_panel,
                p.bg_raised,
                p.bg_hover,
                p.bg_pressed,
                p.bg_canvas,
                p.border_subtle,
                p.border_control,
                p.text_primary,
                p.text_secondary,
                p.text_disabled,
                p.active,
                p.on_active,
                p.focus_ring,
            ] {
                assert!(c.r() == c.g() && c.g() == c.b(), "{c:?} no es gris");
            }
        }
    }
}
