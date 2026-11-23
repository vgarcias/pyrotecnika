//! Varias imágenes en pestañas. La activa vive en `PyroApp::tab` y las demás
//! en `PyroApp::others`; el orden visible es `others[..active]`, la activa y
//! `others[active..]`. Cambiar de pestaña es intercambiarlas, así el resto
//! de la app siempre trabaja con `self.tab`.

use std::path::Path;

use eframe::egui;

use pyro_core::document::Document;
use pyro_core::history::MEMORY_BUDGET;

use super::tab::Tab;
use super::tools::Drag;
use super::{guard, Action, PyroApp};
use crate::commands::store::same_file;
use crate::design::style::palette;
use crate::design::widgets;

/// Largo máximo del nombre en la barra (luego «…»).
const MAX_TITLE_CHARS: usize = 28;

impl PyroApp {
    pub(super) fn tab_count(&self) -> usize {
        self.others.len() + 1
    }

    pub(super) fn tab_at(&self, i: usize) -> &Tab {
        match i.cmp(&self.active) {
            std::cmp::Ordering::Less => &self.others[i],
            std::cmp::Ordering::Equal => &self.tab,
            std::cmp::Ordering::Greater => &self.others[i - 1],
        }
    }

    pub(super) fn tab_at_mut(&mut self, i: usize) -> &mut Tab {
        match i.cmp(&self.active) {
            std::cmp::Ordering::Less => &mut self.others[i],
            std::cmp::Ordering::Equal => &mut self.tab,
            std::cmp::Ordering::Greater => &mut self.others[i - 1],
        }
    }

    /// Saca todas las pestañas en orden visible (deja la activa vacía hasta
    /// [`PyroApp::put_tabs`]).
    fn take_tabs(&mut self) -> Vec<Tab> {
        let mut all = std::mem::take(&mut self.others);
        let active = std::mem::replace(&mut self.tab, Tab::new(Document::new(1, 1, None), 0));
        all.insert(self.active, active);
        all
    }

    fn put_tabs(&mut self, mut all: Vec<Tab>, active: usize) {
        self.tab = all.remove(active);
        self.others = all;
        self.active = active;
        // Cada pestaña conserva sus texturas: cambiar no vuelve a subir nada.
    }

    pub(super) fn switch_to(&mut self, i: usize) {
        if i == self.active || i >= self.tab_count() {
            return;
        }
        self.tab.drag = Drag::None;
        let all = self.take_tabs();
        self.put_tabs(all, i);
    }

    /// Añade una pestaña tras la activa y la activa. Una «Sin título» intacta
    /// (la del arranque) se sustituye en vez de quedar vacía detrás.
    pub(super) fn add_tab(&mut self, tab: Tab) {
        // Una recuperada, con archivo o con cambios nunca es «intacta».
        let pristine = self.tab.untitled > 0
            && !self.tab.history.can_undo()
            && !self.tab.history.can_redo()
            && !self.tab.history.is_modified()
            && self.tab.task.is_none()
            && self.tab.save.is_none()
            && self.tab.recovered_from.is_none()
            && self.tab.ora_path.is_none()
            && self.tab.source_path.is_none();
        let mut all = self.take_tabs();
        let at = if pristine {
            all[self.active] = tab;
            self.active
        } else {
            all.insert(self.active + 1, tab);
            self.active + 1
        };
        self.put_tabs(all, at);
    }

    /// Cierra la activa sin preguntar. La ventana nunca queda sin documento.
    pub(super) fn close_active_tab(&mut self) {
        let mut all = self.take_tabs();
        let closed = all.remove(self.active);
        // Cerrar (guardando o descartando) borra su autoguardado.
        if let Some(recovery) = &self.recovery {
            recovery.remove(closed.uid);
        }
        if let Some(old) = &closed.recovered_from {
            pyro_services::recovery::Recovery::discard(old);
        }
        if all.is_empty() {
            let n = self.next_untitled();
            all.push(Tab::new(Document::new(800, 600, Some([255, 255, 255, 255])), n));
        }
        let at = self.active.min(all.len() - 1);
        self.put_tabs(all, at);
    }

    /// Posición de la pestaña con ese `uid`.
    pub(super) fn tab_index(&self, uid: u64) -> Option<usize> {
        (0..self.tab_count()).find(|&i| self.tab_at(i).uid == uid)
    }

    pub(super) fn next_untitled(&mut self) -> u32 {
        let n = self.untitled_next;
        self.untitled_next += 1;
        n
    }

    /// Pestaña que ya tiene abierto `path`, si la hay.
    pub(super) fn find_tab(&self, path: &Path) -> Option<usize> {
        (0..self.tab_count()).find(|&i| {
            let tab = self.tab_at(i);
            [&tab.source_path, &tab.ora_path].into_iter().flatten().any(|p| same_file(p, path))
        })
    }

    pub(super) fn tab_title(&self, i: usize) -> String {
        let tab = self.tab_at(i);
        if let Some(label) = &tab.label {
            return label.clone();
        }
        match (tab.ora_path.as_ref().or(tab.source_path.as_ref()), tab.untitled) {
            (Some(path), _) => guard::short_name(path),
            (None, n) => format!("Sin título {}", n.max(1)),
        }
    }

    pub(super) fn any_modified(&self) -> bool {
        (0..self.tab_count()).any(|i| self.tab_at(i).history.is_modified())
    }

    /// Presupuesto de historial común: mientras todas las pestañas juntas
    /// pasen de 768 MB, se quita el paso más viejo de la que más ocupa.
    pub(super) fn enforce_history_budget(&mut self) {
        loop {
            let sizes: Vec<usize> = (0..self.tab_count()).map(|i| self.tab_at(i).history.bytes()).collect();
            if sizes.iter().sum::<usize>() <= MEMORY_BUDGET {
                return;
            }
            let mut order: Vec<usize> = (0..sizes.len()).collect();
            order.sort_by_key(|&i| std::cmp::Reverse(sizes[i]));
            if !order.into_iter().any(|i| self.tab_at_mut(i).history.drop_oldest()) {
                return;
            }
        }
    }

    /// Barra de pestañas bajo el menú.
    pub(super) fn tab_bar(&mut self, ctx: &egui::Context) {
        let mut action = None;
        let p = palette(ctx);
        egui::TopBottomPanel::top("pestanas").show(ctx, |ui| {
            egui::ScrollArea::horizontal().show(ui, |ui| {
                ui.horizontal(|ui| {
                    for i in 0..self.tab_count() {
                        let full = self.tab_title(i);
                        let mut title = if full.chars().count() > MAX_TITLE_CHARS {
                            format!("{}…", full.chars().take(MAX_TITLE_CHARS - 1).collect::<String>())
                        } else {
                            full.clone()
                        };
                        if self.tab_at(i).history.is_modified() {
                            title.push_str(" •");
                        }
                        let active = i == self.active;
                        let text = egui::RichText::new(&title).color(if active { p.on_active } else { p.text_primary });
                        let mut button = egui::Button::new(text).selected(active);
                        if active {
                            button = button.fill(p.active);
                        }
                        let tab = ui.add(button).on_hover_text(&full);
                        if tab.clicked() {
                            action = Some(Action::SwitchTab(i));
                        }
                        let close = ui.add(egui::Button::new("×").frame(false)).on_hover_text("Cerrar pestaña");
                        let name = format!("Cerrar «{full}»");
                        close.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &name));
                        if close.clicked() {
                            action = Some(Action::CloseTabAt(i));
                        }
                        ui.add_space(4.0);
                    }
                    if widgets::button(ui, "+").on_hover_text("Nuevo documento").clicked() {
                        action = Some(Action::New);
                    }
                });
            });
        });
        if let Some(a) = action {
            self.run(a, ctx);
        }
    }

    /// Menú Ventana: pestaña siguiente/anterior, cerrar y la lista.
    pub(super) fn window_menu(&self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        self.menu_items(ui, action, &["window.next", "window.prev", "window.close"]);
        ui.separator();
        for i in 0..self.tab_count() {
            if ui.radio(i == self.active, self.tab_title(i)).clicked() {
                *action = Some(Action::SwitchTab(i));
                ui.close_menu();
            }
        }
    }
}
