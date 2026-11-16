//! Menú, barra de estado, barra de herramientas y panel de propiedades.

use eframe::egui;

use pyro_core::document::Document;
use pyro_core::filters::Filter;
use pyro_core::selection::SelectMode;

use super::tools::Tool;
use super::{format_size, guard, icons, updates, Action, PyroApp, Tone};
use crate::design::style::ThemeChoice;
use crate::design::widgets::{self, FocusRing};

impl PyroApp {
    pub(super) fn menu_bar(&mut self, ctx: &egui::Context) {
        let mut action = None;
        let mut theme_pick = None;
        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                let mut item = |ui: &mut egui::Ui, label: &str, shortcut: &str, a: Action, enabled: bool| {
                    let button = egui::Button::new(label).shortcut_text(shortcut);
                    if ui.add_enabled(enabled, button).clicked() {
                        action = Some(a);
                        ui.close_menu();
                    }
                };
                ui.menu_button("Archivo", |ui| {
                    item(ui, "Nuevo…", "Ctrl+N", Action::New, true);
                    item(ui, "Abrir…", "Ctrl+O", Action::Open, true);
                    item(ui, "Exportar…", "Ctrl+E", Action::Export, true);
                    ui.separator();
                    item(ui, "Salir", "", Action::Quit, true);
                });
                let has_sel = self.doc.selection.is_some();
                ui.menu_button("Editar", |ui| {
                    item(ui, "Deshacer", "Ctrl+Z", Action::Undo, self.history.can_undo());
                    item(ui, "Rehacer", "Ctrl+Y", Action::Redo, self.history.can_redo());
                    ui.separator();
                    item(ui, "Borrar selección", "Supr", Action::ClearSelection, has_sel);
                    item(ui, "Recortar a la selección", "", Action::CropToSelection, has_sel);
                });
                ui.menu_button("Selección", |ui| {
                    item(ui, "Seleccionar todo", "Ctrl+A", Action::SelectAll, true);
                    item(ui, "Deseleccionar", "Ctrl+D", Action::Deselect, has_sel);
                    item(ui, "Invertir selección", "Ctrl+Mayús+I", Action::InvertSelection, true);
                });
                ui.menu_button("Filtros", |ui| {
                    ui.label("Ajustes");
                    for f in [Filter::BRIGHTNESS_CONTRAST, Filter::HUE_SATURATION, Filter::LEVELS] {
                        item(ui, &format!("{}…", f.name()), "", Action::Filter(f), true);
                    }
                    item(ui, "Invertir colores", "", Action::Filter(Filter::Invert), true);
                    item(ui, "Escala de grises", "", Action::Filter(Filter::Grayscale), true);
                    ui.separator();
                    ui.label("Filtros");
                    for f in [Filter::GAUSSIAN_BLUR, Filter::SHARPEN] {
                        item(ui, &format!("{}…", f.name()), "", Action::Filter(f), true);
                    }
                });
                ui.menu_button("Ver", |ui| {
                    item(ui, "Acercar", "Ctrl++", Action::ZoomIn, true);
                    item(ui, "Alejar", "Ctrl+-", Action::ZoomOut, true);
                    item(ui, "Ajustar a la ventana", "Ctrl+0", Action::Fit, true);
                    item(ui, "Tamaño real", "Ctrl+1", Action::ActualSize, true);
                    ui.separator();
                    ui.menu_button("Tema", |ui| {
                        for (label, choice) in [
                            ("Seguir al sistema", ThemeChoice::System),
                            ("Claro", ThemeChoice::Light),
                            ("Oscuro", ThemeChoice::Dark),
                            ("Alto contraste", ThemeChoice::HighContrast),
                        ] {
                            if ui.radio(self.theme == choice, label).clicked() {
                                theme_pick = Some(choice);
                                ui.close_menu();
                            }
                        }
                    });
                    ui.menu_button("Tamaño de la interfaz", |ui| {
                        item(ui, "Aumentar", "", Action::UiScaleUp, self.ui_scale < 2.0);
                        item(ui, "Reducir", "", Action::UiScaleDown, self.ui_scale > 0.8);
                        item(ui, "Restablecer (100 %)", "", Action::UiScaleReset, self.ui_scale != 1.0);
                        ui.separator();
                        ui.weak(format!("Actual: {:.0} %", self.ui_scale * 100.0));
                    });
                });
                ui.menu_button("Ayuda", |ui| {
                    item(ui, "Buscar actualizaciones…", "", Action::CheckUpdates, true);
                    ui.separator();
                    ui.weak(format!("Pyrotecnika {}", updates::VERSION));
                });
            });
        });
        if let Some(choice) = theme_pick {
            action = Some(Action::SetTheme(choice));
        }
        if let Some(a) = action {
            self.run(a, ctx);
        }
    }

    pub(super) fn status_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("estado").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.monospace(format!("{}×{} px", self.doc.width, self.doc.height));
                ui.separator();
                ui.monospace(format!("{:.0} %", self.zoom * 100.0));
                if let Some((x, y)) = self.hover_px.and_then(|p| self.inside(p)) {
                    ui.separator();
                    ui.monospace(format!("x {x}, y {y}"));
                }
                ui.separator();
                if self.history.is_modified() {
                    ui.label("Cambios sin exportar");
                    ui.separator();
                }
                self.status_message(ui);
            });
        });
    }

    pub(super) fn tool_panel(&mut self, ctx: &egui::Context) {
        const BUTTON: f32 = 32.0;
        const ICON: f32 = 20.0;
        const GAP: f32 = 4.0;
        const GROUPS: [&[Tool]; 4] = [
            &[Tool::Move],
            &[Tool::Marquee, Tool::Ellipse, Tool::Lasso, Tool::Wand],
            &[Tool::Brush, Tool::Eraser, Tool::Clone, Tool::Blur],
            &[Tool::Bucket, Tool::Eyedropper],
        ];
        let width = 2.0 * BUTTON + GAP + 2.0 * ctx.style().spacing.window_margin.leftf();
        egui::SidePanel::left("herramientas").resizable(false).exact_width(width).show(ctx, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(GAP, GAP);
            ui.add_space(GAP);
            for (i, group) in GROUPS.iter().enumerate() {
                if i > 0 {
                    ui.separator();
                }
                for row in group.chunks(2) {
                    ui.horizontal(|ui| {
                        for &tool in row {
                            self.tool_button(ui, tool);
                        }
                    });
                }
            }
            ui.separator();
            let help = icons::text(icons::CIRCLE_HELP, ICON, ui.visuals().weak_text_color());
            ui.vertical_centered(|ui| {
                ui.add(egui::Label::new(help).sense(egui::Sense::hover()))
                    .on_hover_text(
                        "Espacio + arrastrar o botón central: desplazar.
Rueda: zoom.",
                    )
                    .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, "Ayuda de navegación"));
            });
        });
    }

    /// Botón de herramienta solo con icono. El nombre y el atajo van en el
    /// tooltip; el nombre accesible es solo el nombre.
    fn tool_button(&mut self, ui: &mut egui::Ui, tool: Tool) {
        let selected = self.tool == tool;
        if widgets::tool_button(ui, tool.icon(), selected, tool.label(), tool.shortcut()).clicked() {
            self.tool = tool;
        }
    }

    pub(super) fn side_panel(&mut self, ctx: &egui::Context) {
        egui::SidePanel::right("propiedades").resizable(true).default_width(280.0).show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                self.tool_options(ui);
                ui.separator();
                self.layers_ui(ui);
                if let Some(info) = self.png_info.as_ref().filter(|i| i.is_fireworks()) {
                    ui.separator();
                    egui::CollapsingHeader::new("Archivo de Fireworks").default_open(true).show(ui, |ui| {
                        ui.label(
                            "Se abrió la imagen aplanada. Las capas, vectores y efectos editables de \
                             Fireworks están en bloques privados con un formato propietario sin \
                             documentar y todavía no se leen.",
                        );
                        let mut groups: Vec<(String, usize, usize)> = Vec::new();
                        for c in info.private_chunks() {
                            match groups.iter_mut().find(|g| g.0 == c.kind) {
                                Some(g) => {
                                    g.1 += 1;
                                    g.2 += c.length as usize;
                                }
                                None => groups.push((c.kind.clone(), 1, c.length as usize)),
                            }
                        }
                        for (kind, count, bytes) in groups {
                            ui.monospace(format!("{kind}  ×{count:<4} {}", format_size(bytes)));
                        }
                    });
                }
            });
        });
    }

    pub(super) fn tool_options(&mut self, ui: &mut egui::Ui) {
        ui.strong(self.tool.label());
        if self.tool.edits_pixels() {
            if let Some(reason) = self.layer_block() {
                guard::notice(ui, Tone::Warn, &reason);
            }
        }
        if self.tool.is_brush() {
            let mut size = self.brush.radius * 2.0;
            ui.add(egui::Slider::new(&mut size, 1.0..=500.0).logarithmic(true).text("Tamaño"));
            self.brush.radius = size / 2.0;
            let mut hard = self.brush.hardness * 100.0;
            ui.add(egui::Slider::new(&mut hard, 0.0..=100.0).suffix(" %").text("Dureza"));
            self.brush.hardness = hard / 100.0;
        }
        if self.tool.is_brush() || self.tool == Tool::Bucket {
            let mut op = self.brush.opacity * 100.0;
            ui.add(egui::Slider::new(&mut op, 1.0..=100.0).suffix(" %").text("Opacidad"));
            self.brush.opacity = op / 100.0;
        }
        if matches!(self.tool, Tool::Wand | Tool::Bucket) {
            ui.add(egui::Slider::new(&mut self.tolerance, 0..=255).text("Tolerancia"));
        }
        if self.tool == Tool::Wand {
            ui.checkbox(&mut self.wand_contiguous, "Solo zonas contiguas");
        }
        if self.tool.is_selection() {
            ui.horizontal_wrapped(|ui| {
                ui.radio_value(&mut self.select_mode, SelectMode::Replace, "Nueva");
                ui.radio_value(&mut self.select_mode, SelectMode::Add, "Añadir");
                ui.radio_value(&mut self.select_mode, SelectMode::Subtract, "Restar");
                ui.radio_value(&mut self.select_mode, SelectMode::Intersect, "Intersecar");
            });
            ui.small("Mayús: añadir · Alt: restar");
        }
        if self.tool == Tool::Clone {
            match self.clone_source {
                Some(s) => ui.label(format!("Origen: {:.0}, {:.0} (Alt+clic para cambiarlo)", s.0, s.1)),
                None => ui.label("Alt+clic sobre la imagen para fijar el origen."),
            };
        }
        if self.tool == Tool::Move {
            ui.label("Mueve la selección o, sin selección, toda la capa activa.");
        }
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label("Color");
            ui.color_edit_button_srgba_unmultiplied(&mut self.brush.color);
            let c = self.brush.color;
            ui.monospace(format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2]));
        });
    }

    pub(super) fn layers_ui(&mut self, ui: &mut egui::Ui) {
        ui.strong("Capas");
        let locked = self.filter_dialog.is_some();
        let mut op: Option<fn(&mut Document) -> bool> = None;
        ui.add_enabled_ui(!locked, |ui| {
            ui.horizontal_wrapped(|ui| {
                if widgets::button(ui, "Nueva").clicked() {
                    op = Some(|d| {
                        d.add_layer();
                        true
                    });
                }
                if widgets::button(ui, "Duplicar").clicked() {
                    op = Some(|d| {
                        d.duplicate_layer();
                        true
                    });
                }
                if widgets::button(ui, "Eliminar").clicked() {
                    op = Some(Document::delete_layer);
                }
                if widgets::button(ui, "Subir").clicked() {
                    op = Some(Document::move_layer_up);
                }
                if widgets::button(ui, "Bajar").clicked() {
                    op = Some(Document::move_layer_down);
                }
                if widgets::button(ui, "Combinar abajo").clicked() {
                    op = Some(Document::merge_down);
                }
            });
        });
        if let Some(f) = op {
            let before = self.doc.clone();
            if f(&mut self.doc) {
                self.history.push(&before);
                self.canvas_dirty = true;
            }
        }

        let mut toggle = None;
        let mut select = None;
        for i in (0..self.doc.layers.len()).rev() {
            ui.horizontal(|ui| {
                let mut visible = self.doc.layers[i].visible;
                let name = self.doc.layers[i].name.clone();
                let check = ui.checkbox(&mut visible, "").on_hover_text("Visible").with_focus_ring();
                check.widget_info(|| {
                    egui::WidgetInfo::selected(egui::WidgetType::Checkbox, true, visible, format!("Visible: {name}"))
                });
                if check.changed() {
                    toggle = Some(i);
                }
                if widgets::active_row(ui, i == self.doc.active, &name).clicked() {
                    select = Some(i);
                }
            });
        }
        if let Some(i) = toggle {
            self.history.push(&self.doc);
            self.doc.layers[i].visible = !self.doc.layers[i].visible;
            self.canvas_dirty = true;
        }
        if let Some(i) = select {
            if !locked {
                self.doc.active = i;
            }
        }

        ui.add_space(4.0);
        let active = self.doc.active;
        ui.horizontal(|ui| {
            ui.label("Nombre");
            ui.text_edit_singleline(&mut self.doc.layers[active].name);
        });
        let mut opacity = self.doc.layers[active].opacity * 100.0;
        let r = ui.add(egui::Slider::new(&mut opacity, 0.0..=100.0).suffix(" %").text("Opacidad de capa"));
        if r.drag_started() || (r.changed() && !r.dragged()) {
            self.history.push(&self.doc);
        }
        if r.changed() {
            self.doc.layers[active].opacity = opacity / 100.0;
            self.canvas_dirty = true;
        }
    }
}
