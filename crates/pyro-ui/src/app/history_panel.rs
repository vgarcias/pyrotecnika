//! Panel de historial: los pasos de la pestaña activa con su nombre; clic
//! (o Intro) en uno deshace o rehace hasta él; ↑/↓ recorren la lista.

use eframe::egui::{self, Color32, Sense, TextWrapMode, WidgetText};

use super::PyroApp;
use crate::design::style::palette;

impl PyroApp {
    pub(super) fn history_ui(&mut self, ui: &mut egui::Ui) {
        let steps = self.tab.history.steps();
        let p = palette(ui.ctx());
        let mut target = None;
        egui::CollapsingHeader::new("Historial").default_open(true).show(ui, |ui| {
            if steps.trimmed {
                ui.weak("Pasos anteriores descartados por memoria");
            }
            let mut rows = Vec::with_capacity(steps.labels.len());
            egui::ScrollArea::vertical().max_height(200.0).stick_to_bottom(true).show(ui, |ui| {
                for (i, label) in steps.labels.iter().enumerate() {
                    let current = i == steps.current;
                    let width = ui.available_width();
                    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, 22.0), Sense::click());
                    let name = format!("Paso {i}: {label}");
                    response.widget_info(|| {
                        egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, current, &name)
                    });
                    if ui.is_rect_visible(rect) {
                        let (fill, ink) = if current {
                            (p.active, p.on_active)
                        } else if response.hovered() || response.has_focus() {
                            (p.bg_hover, if i > steps.current { p.text_disabled } else { p.text_primary })
                        } else if i > steps.current {
                            // Pasos que se pueden rehacer.
                            (Color32::TRANSPARENT, p.text_disabled)
                        } else {
                            (Color32::TRANSPARENT, p.text_primary)
                        };
                        ui.painter().rect_filled(rect, 4, fill);
                        let galley = WidgetText::from(*label).into_galley(
                            ui,
                            Some(TextWrapMode::Truncate),
                            (width - 16.0).max(0.0),
                            egui::TextStyle::Body,
                        );
                        let pos = egui::pos2(rect.left() + 8.0, rect.center().y - galley.size().y / 2.0);
                        ui.painter().galley(pos, galley, ink);
                    }
                    if response.clicked() {
                        target = Some(i);
                    }
                    rows.push(response);
                }
            });
            // ↑/↓ mueven el foco por la lista (Intro sobre una fila la elige).
            if let Some(focused) = rows.iter().position(|r| r.has_focus()) {
                let (up, down) = ui.input(|i| (i.key_pressed(egui::Key::ArrowUp), i.key_pressed(egui::Key::ArrowDown)));
                if up && focused > 0 {
                    rows[focused - 1].request_focus();
                } else if down && focused + 1 < rows.len() {
                    rows[focused + 1].request_focus();
                }
            }
        });
        if let Some(i) = target {
            // Por run(): no se salta con una tarea o el diálogo de filtro abiertos.
            self.run(super::Action::GoToStep(i), ui.ctx());
        }
    }
}
