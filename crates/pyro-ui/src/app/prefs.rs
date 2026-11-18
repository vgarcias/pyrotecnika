//! Editar › Preferencias…: apariencia y atajos de teclado.

use eframe::egui;

use super::{Action, PyroApp};
use crate::commands::chord::Chord;
use crate::design::style::ThemeChoice;
use crate::design::widgets;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Section {
    Appearance,
    Shortcuts,
}

/// Estado de la ventana de Preferencias.
pub(super) struct PrefsDialog {
    pub(super) section: Section,
    pub(super) search: String,
    /// Comando elegido en la lista de atajos.
    pub(super) selected: Option<&'static str>,
    /// Esperando la combinación de «+ Añadir».
    pub(super) waiting: bool,
    /// Combinación que ya usa otro comando, a la espera de Reasignar o Cancelar.
    pub(super) pending: Option<(&'static str, Chord)>,
    /// Aviso de la última operación (tecla no permitida, importación…).
    pub(super) message: Option<String>,
}

impl PrefsDialog {
    pub(super) fn new() -> PrefsDialog {
        PrefsDialog {
            section: Section::Appearance,
            search: String::new(),
            selected: None,
            waiting: false,
            pending: None,
            message: None,
        }
    }

    #[cfg(test)]
    pub(super) fn capturing(&self) -> bool {
        self.waiting
    }
}

impl PyroApp {
    pub(super) fn prefs_window(&mut self, ctx: &egui::Context) {
        let Some(section) = self.prefs.as_ref().map(|p| p.section) else { return };
        // La captura se queda con la tecla antes que nadie (Esc incluida).
        self.capture_chord(ctx);
        let mut action = None;
        let mut close = false;
        let mut tab = None;
        let modal = egui::Modal::new(egui::Id::new("preferencias")).show(ctx, |ui| {
            ui.set_width(640.0);
            ui.heading("Preferencias");
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                for (label, s) in [("Apariencia", Section::Appearance), ("Atajos", Section::Shortcuts)] {
                    let clicked = ui
                        .allocate_ui(egui::vec2(140.0, 24.0), |ui| widgets::active_row(ui, section == s, label))
                        .inner
                        .clicked();
                    if clicked {
                        tab = Some(s);
                    }
                }
            });
            ui.separator();
            match section {
                Section::Appearance => action = self.appearance_ui(ui),
                Section::Shortcuts => self.shortcuts_ui(ui),
            }
            ui.add_space(8.0);
            if widgets::primary_button(ui, "Cerrar").clicked() {
                close = true;
            }
        });
        if let (Some(s), Some(prefs)) = (tab, self.prefs.as_mut()) {
            prefs.section = s;
            prefs.waiting = false;
            prefs.pending = None;
        }
        let busy = self.prefs.as_ref().is_some_and(|p| p.waiting || p.pending.is_some());
        if close || (modal.should_close() && !busy) {
            self.prefs = None;
        }
        if let Some(a) = action {
            self.run(a, ctx);
        }
    }

    fn appearance_ui(&self, ui: &mut egui::Ui) -> Option<Action> {
        let mut action = None;
        ui.label("Tema");
        for (label, choice) in [
            ("Seguir al sistema", ThemeChoice::System),
            ("Claro", ThemeChoice::Light),
            ("Oscuro", ThemeChoice::Dark),
            ("Alto contraste", ThemeChoice::HighContrast),
        ] {
            if ui.radio(self.theme == choice, label).clicked() {
                action = Some(Action::SetTheme(choice));
            }
        }
        ui.add_space(8.0);
        ui.label("Tamaño de la interfaz");
        ui.horizontal(|ui| {
            for (label, a) in [("Reducir", Action::UiScaleDown), ("Aumentar", Action::UiScaleUp)] {
                if ui.add_enabled(self.available(a), egui::Button::new(label)).clicked() {
                    action = Some(a);
                }
            }
            ui.monospace(format!("{:.0} %", self.ui_scale * 100.0));
            if ui.add_enabled(self.available(Action::UiScaleReset), egui::Button::new("Restablecer (100 %)")).clicked()
            {
                action = Some(Action::UiScaleReset);
            }
        });
        action
    }
}
