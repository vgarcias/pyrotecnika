//! Los pocos widgets que egui no dibuja como pide el sistema de diseño:
//! inversión de lo activo, botón principal, anillo de foco y trazos de
//! superposición sobre la imagen (§4 del spec).

use eframe::egui::{self, Color32, LayerId, Sense, Stroke, StrokeKind, TextWrapMode, WidgetText};

use super::style::palette;
use super::tokens::{OVERLAY_DARK, OVERLAY_LIGHT};
use crate::app::icons;

/// Rectángulo del anillo de foco: 2 px de separación más medio trazo de
/// 2 px, para que se vea también alrededor de un elemento invertido.
pub(crate) fn focus_ring_rect(widget: egui::Rect) -> egui::Rect {
    widget.expand(3.0)
}

/// Dónde va el anillo de foco: capa del widget con foco, rectángulo del
/// anillo y recorte (su zona visible más el anillo). `None` si nada tiene
/// el foco o el widget quedó fuera de la vista.
pub(crate) fn focus_ring_target(ctx: &egui::Context) -> Option<(LayerId, egui::Rect, egui::Rect)> {
    let id = ctx.memory(|m| m.focused())?;
    let response = ctx.read_response(id)?;
    if !response.interact_rect.is_positive() {
        return None;
    }
    Some((response.layer_id, focus_ring_rect(response.rect), response.interact_rect.expand(4.0)))
}

/// Anillo de foco de teclado (2 px en `focus_ring`) alrededor del widget
/// con foco, sea propio o de egui. Se llama una vez al final de cada cuadro;
/// pinta en la capa del widget, así que una ventana encima lo tapa.
pub(crate) fn paint_focus_ring(ctx: &egui::Context) {
    if let Some((layer, ring, clip)) = focus_ring_target(ctx) {
        let color = palette(ctx).focus_ring;
        ctx.layer_painter(layer).with_clip_rect(clip).rect_stroke(
            ring,
            6,
            Stroke::new(2.0_f32, color),
            StrokeKind::Middle,
        );
    }
}

/// Par de trazos de una superposición sobre la imagen: negro ancho debajo y
/// blanco fino encima, legible sobre cualquier foto.
pub(crate) fn overlay_strokes(width: f32) -> [Stroke; 2] {
    [Stroke::new(width + 2.0, OVERLAY_DARK), Stroke::new(width, OVERLAY_LIGHT)]
}

/// Dibuja una forma de superposición (guía, recta, marca del tampón) con
/// el par de trazos de [`overlay_strokes`].
pub(crate) fn overlay_stroke(painter: &egui::Painter, width: f32, shape: impl Fn(Stroke) -> egui::Shape) {
    for stroke in overlay_strokes(width) {
        painter.add(shape(stroke));
    }
}

/// Botón normal con anillo de foco.
pub(crate) fn button(ui: &mut egui::Ui, text: impl Into<WidgetText>) -> egui::Response {
    ui.button(text)
}

/// Botón de la acción principal de un diálogo: invertido.
pub(crate) fn primary_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    let p = palette(ui.ctx());
    let label = egui::RichText::new(text).color(p.on_active);
    ui.add(egui::Button::new(label).fill(p.active).stroke(Stroke::new(p.stroke_width, p.active)))
}

/// Botón de herramienta de 32×32 con icono de 18 px, invertido si está
/// activo. El tooltip muestra «nombre · atajo»; el nombre accesible es solo
/// el nombre.
pub(crate) fn tool_button(
    ui: &mut egui::Ui,
    glyph: char,
    selected: bool,
    name: &str,
    shortcut: &str,
) -> egui::Response {
    let p = palette(ui.ctx());
    let (rect, response) = ui.allocate_exact_size(egui::vec2(32.0, 32.0), Sense::click());
    response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, selected, name));
    if ui.is_rect_visible(rect) {
        let (fill, ink) = if selected {
            (p.active, p.on_active)
        } else if response.hovered() || response.has_focus() {
            (p.bg_hover, p.text_primary)
        } else {
            (Color32::TRANSPARENT, p.text_secondary)
        };
        ui.painter().rect_filled(rect, 4, fill);
        let galley = WidgetText::from(icons::text(glyph, 18.0, ink)).into_galley(
            ui,
            Some(TextWrapMode::Extend),
            f32::INFINITY,
            egui::TextStyle::Body,
        );
        ui.painter().galley(rect.center() - galley.size() / 2.0, galley, ink);
    }
    response.on_hover_text(format!("{name} · {shortcut}"))
}

/// Fila seleccionable de ancho completo, invertida si está activa. El texto
/// se recorta con «…» y el completo queda en el tooltip.
pub(crate) fn active_row(ui: &mut egui::Ui, selected: bool, text: &str) -> egui::Response {
    let p = palette(ui.ctx());
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, 24.0), Sense::click());
    response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, selected, text));
    let galley = WidgetText::from(text).into_galley(
        ui,
        Some(TextWrapMode::Truncate),
        (width - 16.0).max(0.0),
        egui::TextStyle::Body,
    );
    let elided = galley.elided;
    if ui.is_rect_visible(rect) {
        let (fill, ink) = if selected {
            (p.active, p.on_active)
        } else if response.hovered() {
            (p.bg_hover, p.text_primary)
        } else {
            (Color32::TRANSPARENT, p.text_primary)
        };
        ui.painter().rect_filled(rect, 4, fill);
        let pos = egui::pos2(rect.left() + 8.0, rect.center().y - galley.size().y / 2.0);
        ui.painter().galley(pos, galley, ink);
    }
    if elided {
        response.on_hover_text(text)
    } else {
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::accesskit::Toggled;
    use egui::Id;
    use egui_kittest::kittest::Queryable;

    /// Harness de una UI con el sistema de diseño instalado antes del
    /// primer cuadro que dibuja (las fuentes se aplican un cuadro después).
    fn harness<'a>(mut ui_fn: impl FnMut(&mut egui::Ui) + 'a) -> egui_kittest::Harness<'a> {
        let mut ready = false;
        egui_kittest::Harness::new_ui(move |ui| {
            if !ready {
                crate::design::style::install(ui.ctx());
                ready = true;
                ui.ctx().request_repaint();
                return;
            }
            ui_fn(ui);
        })
    }

    #[test]
    fn la_superposicion_es_negro_ancho_bajo_blanco_fino() {
        let [under, over] = overlay_strokes(1.0);
        assert_eq!(under, Stroke::new(3.0_f32, crate::design::tokens::OVERLAY_DARK));
        assert_eq!(over, Stroke::new(1.0_f32, crate::design::tokens::OVERLAY_LIGHT));
    }

    #[test]
    fn el_anillo_de_foco_queda_fuera_del_widget() {
        let r = egui::Rect::from_min_size(egui::pos2(10.0, 10.0), egui::vec2(32.0, 32.0));
        let ring = focus_ring_rect(r);
        assert!(ring.contains_rect(r));
        assert_eq!(ring.min, egui::pos2(7.0, 7.0));
    }

    #[test]
    fn boton_de_herramienta_accesible_sin_atajo() {
        let mut harness = harness(|ui| {
            tool_button(ui, '\u{e2e7}', true, "Pincel", "B");
        });
        harness.run();
        let node = harness.get_by_label("Pincel");
        assert_eq!(node.toggled(), Some(Toggled::True));
    }

    #[test]
    fn fila_activa_accesible_y_seleccionada() {
        let mut harness = harness(|ui| {
            active_row(ui, true, "Retoque cielo");
            active_row(ui, false, "Fondo");
        });
        harness.run();
        assert_eq!(harness.get_by_label("Retoque cielo").toggled(), Some(Toggled::True));
        assert_eq!(harness.get_by_label("Fondo").toggled(), Some(Toggled::False));
    }

    /// Cualquier widget de egui con foco (aquí un campo de texto) recibe el
    /// anillo, por fuera de su rectángulo.
    #[test]
    fn el_anillo_rodea_cualquier_widget_con_foco() {
        let id = Id::new("campo");
        let mut text = String::from("Fondo");
        let mut harness = harness(move |ui| {
            ui.ctx().memory_mut(|m| m.request_focus(id));
            ui.add(egui::TextEdit::singleline(&mut text).id(id));
        });
        harness.run();
        let field = harness.ctx.read_response(id).expect("el campo existe").rect;
        let (_, ring, _) = focus_ring_target(&harness.ctx).expect("hay anillo");
        assert!(ring.contains_rect(field) && ring != field);
    }

    /// Un widget con foco que quedó fuera de su zona visible (desplazado en
    /// un panel) no pinta anillo encima de otra cosa.
    #[test]
    fn sin_anillo_si_el_widget_con_foco_no_se_ve() {
        let id = Id::new("oculto");
        let mut harness = harness(move |ui| {
            ui.ctx().memory_mut(|m| m.request_focus(id));
            ui.set_clip_rect(egui::Rect::from_min_size(egui::pos2(500.0, 500.0), egui::vec2(1.0, 1.0)));
            ui.interact(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(40.0, 20.0)), id, Sense::click());
        });
        harness.run();
        assert!(focus_ring_target(&harness.ctx).is_none());
    }
}
