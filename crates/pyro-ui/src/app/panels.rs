//! Menú, barra de estado, barra de herramientas y panel de propiedades.

use eframe::egui;

use pyro_core::selection::SelectMode;

use super::canvas_tiles::Dirty;
use super::tools::Tool;
use super::{format_size, guard, icons, updates, Action, PyroApp, Tone};
use crate::commands::catalog;
use crate::design::fonts::title_style;
use crate::design::style::{self, ThemeChoice};
use crate::design::widgets;

impl PyroApp {
    pub(super) fn menu_bar(&mut self, ctx: &egui::Context) {
        let mut action = None;
        let mut theme_pick = None;
        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("Archivo", |ui| {
                    self.menu_items(ui, &mut action, &["file.new", "file.open"]);
                    ui.menu_button("Abrir reciente", |ui| self.recent_menu(ui, &mut action));
                    self.menu_items(ui, &mut action, &["file.export", "file.reveal"]);
                    ui.separator();
                    self.menu_items(ui, &mut action, &["file.quit"]);
                });
                ui.menu_button("Editar", |ui| {
                    self.menu_items(ui, &mut action, &["edit.undo", "edit.redo"]);
                    ui.separator();
                    self.menu_items(ui, &mut action, &["edit.clear", "edit.crop"]);
                    ui.separator();
                    self.menu_items(ui, &mut action, &["edit.palette", "edit.preferences"]);
                });
                ui.menu_button("Selección", |ui| {
                    self.menu_items(ui, &mut action, &["select.all", "select.none", "select.invert"]);
                });
                ui.menu_button("Capas", |ui| {
                    self.menu_items(ui, &mut action, &["layer.new", "layer.duplicate", "layer.delete"]);
                    ui.separator();
                    self.menu_items(ui, &mut action, &["layer.up", "layer.down"]);
                    ui.separator();
                    self.menu_items(ui, &mut action, &["layer.merge_down"]);
                });
                ui.menu_button("Filtros", |ui| {
                    ui.label("Ajustes");
                    self.menu_items(
                        ui,
                        &mut action,
                        &[
                            "filter.brightness_contrast",
                            "filter.hue_saturation",
                            "filter.levels",
                            "filter.invert",
                            "filter.grayscale",
                        ],
                    );
                    ui.separator();
                    ui.label("Filtros");
                    self.menu_items(ui, &mut action, &["filter.gaussian_blur", "filter.sharpen"]);
                });
                ui.menu_button("Ver", |ui| {
                    self.menu_items(
                        ui,
                        &mut action,
                        &["view.zoom_in", "view.zoom_out", "view.fit", "view.actual_size"],
                    );
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
                        self.menu_item(ui, &mut action, "view.ui_scale_up", Some("Aumentar"));
                        self.menu_item(ui, &mut action, "view.ui_scale_down", Some("Reducir"));
                        self.menu_item(ui, &mut action, "view.ui_scale_reset", Some("Restablecer (100 %)"));
                        ui.separator();
                        ui.weak(format!("Actual: {:.0} %", self.ui_scale * 100.0));
                    });
                });
                ui.menu_button("Ayuda", |ui| {
                    self.menu_items(ui, &mut action, &["help.updates"]);
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

    /// Entrada de menú de un comando del catálogo: su nombre (o `label`), su
    /// atajo vigente y deshabilitada si no tiene sentido ahora.
    fn menu_item(&self, ui: &mut egui::Ui, action: &mut Option<Action>, id: &str, label: Option<&str>) {
        let Some(def) = catalog::find(id) else { return };
        let button = egui::Button::new(label.unwrap_or(def.name)).shortcut_text(self.menu_shortcut(id));
        if ui.add_enabled(self.available(def.action), button).clicked() {
            *action = Some(def.action);
            ui.close_menu();
        }
    }

    fn menu_items(&self, ui: &mut egui::Ui, action: &mut Option<Action>, ids: &[&str]) {
        for id in ids {
            self.menu_item(ui, action, id, None);
        }
    }

    /// Archivo › Abrir reciente: nombre del archivo, ruta completa en el
    /// tooltip y «Limpiar lista» al final.
    fn recent_menu(&self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        if self.settings.recent.files.is_empty() {
            ui.weak("Sin archivos recientes");
        }
        for (i, path) in self.settings.recent.files.iter().enumerate() {
            if ui.button(guard::short_name(path)).on_hover_text(path.display().to_string()).clicked() {
                *action = Some(Action::OpenRecent(i));
                ui.close_menu();
            }
        }
        ui.separator();
        self.menu_item(ui, action, "file.clear_recent", Some("Limpiar lista"));
    }

    pub(super) fn status_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("estado").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.monospace(format!("{}×{} px", self.tab.doc.width, self.tab.doc.height));
                ui.separator();
                ui.monospace(format!("{:.0} %", self.tab.zoom * 100.0));
                if let Some((x, y)) = self.hover_px.and_then(|p| self.inside(p)) {
                    ui.separator();
                    ui.monospace(format!("x {x}, y {y}"));
                }
                ui.separator();
                if self.tab.history.is_modified() {
                    ui.label("Cambios sin exportar");
                    ui.separator();
                }
                self.task_status(ui);
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
        let width = 2.0 * BUTTON + GAP + 2.0 * f32::from(style::PANEL_MARGIN);
        let frame = style::side_panel_frame(&ctx.style());
        egui::SidePanel::left("herramientas").resizable(false).exact_width(width).frame(frame).show(ctx, |ui| {
            // Al 200 % en la ventana mínima la columna no cabe: se desplaza.
            egui::ScrollArea::vertical().show(ui, |ui| {
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
                        .widget_info(|| {
                            egui::WidgetInfo::labeled(egui::WidgetType::Label, true, "Ayuda de navegación")
                        });
                });
            });
        });
    }

    /// Botón de herramienta solo con icono. El nombre y el atajo van en el
    /// tooltip; el nombre accesible es solo el nombre.
    fn tool_button(&mut self, ui: &mut egui::Ui, tool: Tool) {
        let selected = self.tool == tool;
        let shortcut =
            catalog::by_action(Action::SelectTool(tool)).map(|d| self.menu_shortcut(d.id)).unwrap_or_default();
        if widgets::tool_button(ui, tool.icon(), selected, tool.label(), &shortcut).clicked() {
            self.tool = tool;
        }
    }

    pub(super) fn side_panel(&mut self, ctx: &egui::Context) {
        let frame = style::side_panel_frame(&ctx.style());
        egui::SidePanel::right("propiedades").resizable(true).default_width(280.0).frame(frame).show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                // Durante una tarea que cambia el documento, el panel no edita.
                ui.add_enabled_ui(!self.busy(), |ui| {
                    self.tool_options(ui);
                    ui.separator();
                    self.layers_ui(ui);
                });
                if let Some(info) = self.tab.png_info.as_ref().filter(|i| i.is_fireworks()) {
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
        ui.label(egui::RichText::new(self.tool.label()).text_style(title_style()));
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
            match self.tab.clone_source {
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
        ui.label(egui::RichText::new("Capas").text_style(title_style()));
        let locked = self.tab.filter_dialog.is_some();
        let mut clicked = None;
        ui.add_enabled_ui(!locked, |ui| {
            ui.horizontal_wrapped(|ui| {
                for (label, id) in [
                    ("Nueva", "layer.new"),
                    ("Duplicar", "layer.duplicate"),
                    ("Eliminar", "layer.delete"),
                    ("Subir", "layer.up"),
                    ("Bajar", "layer.down"),
                    ("Combinar abajo", "layer.merge_down"),
                ] {
                    let Some(def) = catalog::find(id) else { continue };
                    let shortcut = self.menu_shortcut(id);
                    let tip =
                        if shortcut.is_empty() { def.name.to_string() } else { format!("{} · {shortcut}", def.name) };
                    if widgets::button(ui, label).on_hover_text(tip).clicked() {
                        clicked = Some(def.action);
                    }
                }
            });
        });
        if let Some(action) = clicked {
            let ctx = ui.ctx().clone();
            self.run(action, &ctx);
        }

        let mut toggle = None;
        let mut select = None;
        for i in (0..self.tab.doc.layers.len()).rev() {
            ui.horizontal(|ui| {
                let mut visible = self.tab.doc.layers[i].visible;
                let name = self.tab.doc.layers[i].name.clone();
                let check = ui.checkbox(&mut visible, "").on_hover_text("Visible");
                check.widget_info(|| {
                    egui::WidgetInfo::selected(egui::WidgetType::Checkbox, true, visible, format!("Visible: {name}"))
                });
                if check.changed() {
                    toggle = Some(i);
                }
                if widgets::active_row(ui, i == self.tab.doc.active, &name).clicked() {
                    select = Some(i);
                }
            });
        }
        if let Some(i) = toggle {
            self.tab.history.push(&self.tab.doc);
            self.tab.doc.layers[i].visible = !self.tab.doc.layers[i].visible;
            self.tab.dirty = Dirty::All;
        }
        if let Some(i) = select {
            if !locked {
                self.tab.doc.active = i;
            }
        }

        ui.add_space(4.0);
        let active = self.tab.doc.active;
        ui.horizontal(|ui| {
            ui.label("Nombre");
            ui.text_edit_singleline(&mut self.tab.doc.layers[active].name);
        });
        let mut opacity = self.tab.doc.layers[active].opacity * 100.0;
        let r = ui.add(egui::Slider::new(&mut opacity, 0.0..=100.0).suffix(" %").text("Opacidad de capa"));
        if r.drag_started() || (r.changed() && !r.dragged()) {
            self.tab.history.push(&self.tab.doc);
        }
        if r.changed() {
            self.tab.doc.layers[active].opacity = opacity / 100.0;
            self.tab.dirty = Dirty::All;
        }
    }
}
