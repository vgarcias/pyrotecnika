//! Estilo de egui generado desde los tokens, y cambio de tema (§3–§5 del spec).

use eframe::egui::{self, Color32, CornerRadius, Shadow, Stroke, Theme, ThemePreference};

use super::fonts::text_styles;
use super::tokens::{Palette, DARK, HIGH_CONTRAST, LIGHT};

/// Tema elegido en Ver › Tema.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ThemeChoice {
    System,
    Light,
    Dark,
    HighContrast,
}

fn is_dark(p: &Palette) -> bool {
    p.text_primary.r() > p.bg_panel.r()
}

/// Color que, mezclado al 50 % con `primary` (como hace egui con el texto
/// «débil»), da exactamente `secondary`.
fn weak_target(primary: Color32, secondary: Color32) -> Color32 {
    let ch = |p: u8, s: u8| (2 * i32::from(s) - i32::from(p)).clamp(0, 255) as u8;
    Color32::from_rgb(ch(primary.r(), secondary.r()), ch(primary.g(), secondary.g()), ch(primary.b(), secondary.b()))
}

/// Estilo completo de egui para una paleta.
pub(crate) fn style(p: &Palette) -> egui::Style {
    let dark = is_dark(p);
    let mut s = egui::Style {
        visuals: if dark { egui::Visuals::dark() } else { egui::Visuals::light() },
        text_styles: text_styles(),
        animation_time: 0.12,
        drag_value_text_style: egui::TextStyle::Monospace,
        ..Default::default()
    };

    let sp = &mut s.spacing;
    sp.item_spacing = egui::vec2(8.0, 4.0);
    sp.button_padding = egui::vec2(12.0, 4.0);
    sp.interact_size.y = 24.0;
    sp.window_margin = egui::Margin::same(16);
    sp.menu_margin = egui::Margin::same(6);

    let v = &mut s.visuals;
    let w = p.stroke_width;
    let radius = CornerRadius::same(4);
    v.dark_mode = dark;
    v.panel_fill = p.bg_panel;
    v.window_fill = p.bg_raised;
    v.window_stroke = Stroke::new(w, p.border_subtle);
    v.extreme_bg_color = p.bg_raised;
    v.faint_bg_color = p.bg_hover;
    v.code_bg_color = p.bg_raised;
    v.hyperlink_color = p.text_primary;
    v.warn_fg_color = p.text_primary;
    v.error_fg_color = p.text_primary;
    v.window_corner_radius = CornerRadius::same(8);
    v.menu_corner_radius = CornerRadius::same(6);
    let shadow = Shadow { offset: [0, 4], blur: 14, spread: 0, color: Color32::from_black_alpha(90) };
    v.window_shadow = shadow;
    v.popup_shadow = shadow;
    v.selection.bg_fill = p.active.gamma_multiply(0.35);
    v.selection.stroke = Stroke::new(1.0, p.on_active);
    v.text_cursor.stroke = Stroke::new(2.0, p.text_primary);

    let wd = &mut v.widgets;
    wd.noninteractive.bg_fill = p.bg_panel;
    wd.noninteractive.weak_bg_fill = weak_target(p.text_primary, p.text_secondary);
    wd.noninteractive.bg_stroke = Stroke::new(w, p.border_subtle);
    wd.noninteractive.fg_stroke = Stroke::new(1.0, p.text_primary);
    wd.noninteractive.corner_radius = radius;
    for (state, fill) in
        [(&mut wd.inactive, p.bg_raised), (&mut wd.hovered, p.bg_hover), (&mut wd.active, p.bg_pressed)]
    {
        state.bg_fill = fill;
        state.weak_bg_fill = fill;
        state.bg_stroke = Stroke::new(w, p.border_control);
        state.fg_stroke = Stroke::new(1.0, p.text_primary);
        state.corner_radius = radius;
        state.expansion = 0.0;
    }
    wd.open = wd.hovered;
    s
}

/// Aplica la elección de Ver › Tema. El alto contraste es un único tema
/// fijo (blanco sobre negro): se registra como estilo oscuro y se fija la
/// preferencia en oscuro. El resto vuelve a registrar las paletas normales.
pub(crate) fn apply_theme(ctx: &egui::Context, choice: ThemeChoice) {
    let dark = if choice == ThemeChoice::HighContrast { &HIGH_CONTRAST } else { &DARK };
    ctx.set_style_of(Theme::Dark, style(dark));
    ctx.set_style_of(Theme::Light, style(&LIGHT));
    ctx.options_mut(|o| o.fallback_theme = Theme::Dark);
    ctx.set_theme(match choice {
        ThemeChoice::System => ThemePreference::System,
        ThemeChoice::Light => ThemePreference::Light,
        ThemeChoice::Dark | ThemeChoice::HighContrast => ThemePreference::Dark,
    });
}

/// Fuentes y tema del sistema: lo primero que hace la app al arrancar.
pub(crate) fn install(ctx: &egui::Context) {
    super::fonts::install_fonts(ctx);
    apply_theme(ctx, ThemeChoice::System);
}

/// Paleta del estilo vigente, para los widgets propios y el lienzo.
pub(crate) fn palette(ctx: &egui::Context) -> &'static Palette {
    let panel = ctx.style().visuals.panel_fill;
    [&HIGH_CONTRAST, &LIGHT, &DARK].into_iter().find(|p| p.bg_panel == panel).unwrap_or(&DARK)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estados_desde_los_tokens() {
        let s = style(&DARK);
        let w = &s.visuals.widgets;
        assert_eq!(w.inactive.bg_fill, DARK.bg_raised);
        assert_eq!(w.hovered.bg_fill, DARK.bg_hover);
        assert_eq!(w.active.bg_fill, DARK.bg_pressed);
        assert_eq!(w.inactive.bg_stroke.color, DARK.border_control);
        assert_eq!(s.visuals.panel_fill, DARK.bg_panel);
        assert_eq!(s.visuals.window_fill, DARK.bg_raised);
        assert_eq!(s.visuals.warn_fg_color, DARK.text_primary);
        assert_eq!(s.visuals.error_fg_color, DARK.text_primary);
        assert_eq!(s.visuals.selection.stroke.color, DARK.on_active);
        assert!(s.visuals.dark_mode);
        assert!(!style(&LIGHT).visuals.dark_mode);
        assert_eq!(style(&HIGH_CONTRAST).visuals.widgets.inactive.bg_stroke.width, 2.0);
    }

    #[test]
    fn el_texto_debil_de_egui_coincide_con_el_secundario() {
        for p in [DARK, LIGHT, HIGH_CONTRAST] {
            let weak = style(&p).visuals.weak_text_color();
            assert!((i32::from(weak.r()) - i32::from(p.text_secondary.r())).abs() <= 1, "{weak:?}");
        }
    }

    #[test]
    fn espaciado_y_forma() {
        let s = style(&DARK);
        assert_eq!(s.spacing.item_spacing, egui::vec2(8.0, 4.0));
        assert_eq!(s.spacing.button_padding, egui::vec2(12.0, 4.0));
        assert_eq!(s.visuals.widgets.inactive.corner_radius, egui::CornerRadius::same(4));
        assert_eq!(s.visuals.window_corner_radius, egui::CornerRadius::same(8));
        assert_eq!(s.visuals.menu_corner_radius, egui::CornerRadius::same(6));
        assert_eq!(s.animation_time, 0.12);
        assert_eq!(s.drag_value_text_style, egui::TextStyle::Monospace);
    }

    #[test]
    fn alto_contraste_y_vuelta_al_sistema() {
        let ctx = egui::Context::default();
        apply_theme(&ctx, ThemeChoice::HighContrast);
        assert_eq!(ctx.options(|o| o.theme_preference), egui::ThemePreference::Dark);
        assert_eq!(ctx.style_of(egui::Theme::Dark).visuals.panel_fill, HIGH_CONTRAST.bg_panel);
        apply_theme(&ctx, ThemeChoice::System);
        assert_eq!(ctx.options(|o| o.theme_preference), egui::ThemePreference::System);
        assert_eq!(ctx.style_of(egui::Theme::Dark).visuals.panel_fill, DARK.bg_panel);
        assert_eq!(ctx.style_of(egui::Theme::Light).visuals.panel_fill, LIGHT.bg_panel);
    }

    #[test]
    fn sin_tema_del_sistema_usa_el_oscuro() {
        let ctx = egui::Context::default();
        install(&ctx);
        assert_eq!(ctx.options(|o| o.fallback_theme), egui::Theme::Dark);
        let _ = ctx.run(Default::default(), |_| {});
        assert_eq!(ctx.style().visuals.panel_fill, DARK.bg_panel);
        assert_eq!(*palette(&ctx), DARK);
    }
}
