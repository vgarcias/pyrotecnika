//! Protección del trabajo: mensajes con severidad, confirmación antes de
//! descartar cambios, título de la ventana y bloqueo de capas que no se ven.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use eframe::egui;

use pyro_core::document::Document;

use super::{icons, PyroApp};
use crate::design::widgets;

/// Severidad de un mensaje de la barra de estado.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Tone {
    Info,
    Warn,
    Error,
}

pub(super) struct Status {
    pub(super) tone: Tone,
    pub(super) text: String,
}

/// Acción que sustituye o cierra el documento y espera confirmación.
pub(super) enum Pending {
    Open(PathBuf),
    New {
        width: u32,
        height: u32,
        transparent: bool,
    },
    Quit,
    /// Cerrar la pestaña con ese `uid`.
    CloseTab(u64),
    /// Instalar la actualización encontrada (en Windows cierra la app).
    InstallUpdate,
    /// Abrir la versión recién instalada y cerrar esta.
    Restart,
}

enum Choice {
    Save,
    Discard,
    Cancel,
}

/// Nombre de archivo para mostrar, acortado si es muy largo.
pub(super) fn short_name(path: &Path) -> String {
    const MAX: usize = 40;
    let name = path.file_name().map_or_else(|| path.to_string_lossy(), |n| n.to_string_lossy());
    if name.chars().count() > MAX {
        let head: String = name.chars().take(MAX - 1).collect();
        format!("{head}…")
    } else {
        name.into_owned()
    }
}

/// Causa de un error de E/S en lenguaje llano, sin códigos del sistema.
pub(super) fn io_reason(e: &std::io::Error, writing: bool) -> String {
    match (e.kind(), writing) {
        (ErrorKind::NotFound, false) => "el archivo no existe o se movió".into(),
        (ErrorKind::NotFound, true) => "la carpeta de destino ya no existe".into(),
        (ErrorKind::PermissionDenied, false) => "no tienes permiso para leerlo".into(),
        (ErrorKind::PermissionDenied, true) => {
            "no tienes permiso para escribir ahí, o el archivo está abierto en otro programa".into()
        }
        _ => {
            let text = e.to_string();
            text.split(" (os error").next().unwrap_or(&text).trim_end_matches('.').to_string()
        }
    }
}

/// Fila de aviso o error: icono y texto en semibold, para que la severidad
/// no dependa del color.
pub(super) fn notice(ui: &mut egui::Ui, tone: Tone, text: &str) {
    ui.horizontal_wrapped(|ui| {
        if let Some(icon) = tone_icon(ui, tone) {
            ui.label(icon);
        }
        ui.label(tone_text(tone, text));
    });
}

/// Avisos y errores en semibold: con el icono, la severidad no depende del
/// color (la paleta es neutra).
fn tone_text(tone: Tone, text: &str) -> egui::RichText {
    let rich = egui::RichText::new(text);
    match tone {
        Tone::Info => rich,
        Tone::Warn | Tone::Error => rich.family(egui::FontFamily::Name(crate::design::fonts::SEMIBOLD.into())),
    }
}

fn tone_icon(ui: &egui::Ui, tone: Tone) -> Option<egui::RichText> {
    let size = ui.text_style_height(&egui::TextStyle::Body);
    match tone {
        Tone::Info => None,
        Tone::Warn => Some(icons::text(icons::TRIANGLE_ALERT, size, ui.visuals().warn_fg_color)),
        Tone::Error => Some(icons::text(icons::CIRCLE_X, size, ui.visuals().error_fg_color)),
    }
}

impl PyroApp {
    pub(super) fn info(&mut self, text: impl Into<String>) {
        self.status = Status { tone: Tone::Info, text: text.into() };
    }

    pub(super) fn warn(&mut self, text: impl Into<String>) {
        self.status = Status { tone: Tone::Warn, text: text.into() };
    }

    pub(super) fn fail(&mut self, text: impl Into<String>) {
        self.status = Status { tone: Tone::Error, text: text.into() };
    }

    /// Mensaje de la barra de estado: una línea que se recorta si no cabe
    /// (el texto completo queda en el tooltip).
    pub(super) fn status_message(&self, ui: &mut egui::Ui) {
        let Status { tone, text } = &self.status;
        if let Some(icon) = tone_icon(ui, *tone) {
            ui.label(icon);
        }
        let rich = tone_text(*tone, text);
        ui.add(egui::Label::new(rich).truncate()).on_hover_text(text);
    }

    /// Motivo por el que no se puede editar la capa activa, si lo hay:
    /// pintar o filtrar lo que no se ve parece no hacer nada.
    pub(super) fn layer_block(&self) -> Option<String> {
        let layer = self.tab.doc.active_layer();
        if !layer.visible {
            Some(format!("La capa «{}» está oculta. Muéstrala para editarla.", layer.name))
        } else if layer.opacity <= 0.0 {
            Some(format!("La capa «{}» tiene opacidad 0 %. Súbela para ver lo que editas.", layer.name))
        } else {
            None
        }
    }

    // ---------- Cambios sin exportar ----------

    /// Ejecuta la acción o, si hay cambios sin exportar, pide confirmación.
    pub(super) fn request(&mut self, pending: Pending, ctx: &egui::Context) {
        let modified: Vec<usize> = (0..self.tab_count()).filter(|&i| self.tab_at(i).history.is_modified()).collect();
        let closes_app = matches!(pending, Pending::Quit | Pending::InstallUpdate | Pending::Restart);
        let at_risk = match pending {
            // Abrir y Nuevo crean una pestaña: no se pierde nada.
            Pending::Open(_) | Pending::New { .. } => false,
            Pending::CloseTab(uid) => match self.tab_index(uid) {
                Some(i) => {
                    // La confirmación habla de la pestaña que se cierra.
                    self.switch_to(i);
                    self.tab.history.is_modified()
                }
                None => return,
            },
            Pending::Quit | Pending::InstallUpdate | Pending::Restart => !modified.is_empty(),
        };
        if closes_app && modified.len() > 1 {
            // Varias pestañas con cambios: una sola ventana para todas.
            self.quit_window = Some(pending);
            self.confirm_focus = true;
        } else if at_risk {
            if closes_app {
                // Una sola con cambios: se muestra esa, aunque no sea la activa.
                self.switch_to(modified[0]);
            }
            self.confirm = Some(pending);
            self.confirm_focus = true;
        } else {
            self.perform(pending, ctx);
        }
    }

    pub(super) fn perform(&mut self, pending: Pending, ctx: &egui::Context) {
        match pending {
            Pending::Open(path) => self.open_path(path, ctx),
            Pending::New { width, height, transparent } => {
                let bg = (!transparent).then_some([255, 255, 255, 255]);
                let n = self.next_untitled();
                self.add_tab(super::tab::Tab::new(Document::new(width, height, bg), n));
                self.info("Documento nuevo.");
            }
            Pending::CloseTab(uid) => {
                let Some(i) = self.tab_index(uid) else { return };
                self.switch_to(i);
                match &mut self.tab.save {
                    // Cerrar espera a que termine el guardado de la pestaña.
                    Some(save) => {
                        save.then = Some(Pending::CloseTab(uid));
                        self.info("Guardando…");
                    }
                    None => self.close_active_tab(),
                }
            }
            Pending::Quit if self.any_saving() => {
                self.after_saves = Some(Pending::Quit);
                self.info("Guardando…");
            }
            Pending::Quit => {
                self.closing = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            Pending::InstallUpdate => self.install_update(),
            Pending::Restart => self.restart_after_update(ctx),
        }
    }

    /// Intercepta el cierre de la ventana (botón, Alt+F4, Archivo › Salir).
    pub(super) fn guard_close(&mut self, ctx: &egui::Context) {
        let requested = ctx.input(|i| i.viewport().close_requested());
        if requested && !self.closing && self.any_saving() {
            // Salir espera a los guardados en curso.
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.after_saves = Some(Pending::Quit);
            self.info("Guardando…");
        } else if requested && !self.closing && self.any_modified() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.request(Pending::Quit, ctx);
        }
    }

    /// Título de la ventana: archivo abierto y marca de cambios sin exportar.
    pub(super) fn sync_title(&mut self, ctx: &egui::Context) {
        let name = self.tab_title(self.active);
        let mark = if self.tab.history.is_modified() { "• " } else { "" };
        let title = format!("{mark}{name} - Pyrotecnika");
        if title != self.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }
    }

    pub(super) fn confirm_window(&mut self, ctx: &egui::Context) {
        let Some(pending) = &self.confirm else {
            return;
        };
        let (title, consequence) = match pending {
            Pending::Open(path) => (
                format!("¿Abrir «{}» y descartar los cambios?", short_name(path)),
                "Si abres otro archivo, esos cambios se pierden y no se pueden deshacer.",
            ),
            Pending::New { .. } => (
                "¿Crear un documento nuevo y descartar los cambios?".to_string(),
                "Si creas uno nuevo, esos cambios se pierden y no se pueden deshacer.",
            ),
            Pending::Quit => {
                ("¿Salir y descartar los cambios?".to_string(), "Si sales ahora, esos cambios se pierden.")
            }
            Pending::CloseTab(_) => (
                format!("¿Cerrar «{}» y descartar los cambios?", self.tab_title(self.active)),
                "Si la cierras, esos cambios se pierden y no se pueden deshacer.",
            ),
            Pending::InstallUpdate => (
                "¿Actualizar y descartar los cambios?".to_string(),
                "Para actualizar, Pyrotecnika se cierra y esos cambios se pierden.",
            ),
            Pending::Restart => {
                ("¿Reiniciar y descartar los cambios?".to_string(), "Si reinicias ahora, esos cambios se pierden.")
            }
        };
        let name = self.tab_title(self.active);
        let focus = std::mem::take(&mut self.confirm_focus);
        let mut choice = None;
        let modal = egui::Modal::new(egui::Id::new("confirmar-descartar")).show(ctx, |ui| {
            ui.set_max_width(440.0);
            ui.heading(title);
            ui.add_space(8.0);
            ui.label(format!("«{name}» tiene cambios sin guardar. {consequence}"));
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if widgets::button(ui, "Guardar").clicked() {
                    choice = Some(Choice::Save);
                }
                if widgets::button(ui, "Descartar").clicked() {
                    choice = Some(Choice::Discard);
                }
                // Cancelar recibe el foco: Intro o Espacio nunca descartan.
                let cancel = widgets::button(ui, "Cancelar");
                if focus {
                    cancel.request_focus();
                }
                if cancel.clicked() {
                    choice = Some(Choice::Cancel);
                }
            });
        });
        if choice.is_none() && modal.should_close() {
            choice = Some(Choice::Cancel);
        }
        match choice {
            None => {}
            Some(Choice::Cancel) => self.confirm = None,
            Some(Choice::Discard) => {
                if let Some(pending) = self.confirm.take() {
                    self.perform(pending, ctx);
                }
            }
            Some(Choice::Save) => {
                let pending = self.confirm.take();
                self.save(false, pending, ctx);
            }
        }
    }

    /// Salir con varias pestañas con cambios: lista y Guardar todo /
    /// Descartar todo / Cancelar (foco en Cancelar).
    pub(super) fn quit_window(&mut self, ctx: &egui::Context) {
        let Some(pending) = &self.quit_window else {
            return;
        };
        let heading = match pending {
            Pending::InstallUpdate => "¿Actualizar con cambios sin guardar?",
            Pending::Restart => "¿Reiniciar con cambios sin guardar?",
            _ => "¿Salir con cambios sin guardar?",
        };
        let titles: Vec<String> = (0..self.tab_count())
            .filter(|&i| self.tab_at(i).history.is_modified())
            .map(|i| self.tab_title(i))
            .collect();
        let focus = std::mem::take(&mut self.confirm_focus);
        let mut choice = None;
        let modal = egui::Modal::new(egui::Id::new("salir-varias")).show(ctx, |ui| {
            ui.set_max_width(440.0);
            ui.heading(heading);
            ui.add_space(8.0);
            ui.label(format!("{} documentos tienen cambios sin guardar:", titles.len()));
            for t in &titles {
                ui.label(format!("• {t}"));
            }
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if widgets::button(ui, "Guardar todo").clicked() {
                    choice = Some(Choice::Save);
                }
                if widgets::button(ui, "Descartar todo").clicked() {
                    choice = Some(Choice::Discard);
                }
                // Cancelar recibe el foco: Intro o Espacio nunca descartan.
                let cancel = widgets::button(ui, "Cancelar");
                if focus {
                    cancel.request_focus();
                }
                if cancel.clicked() {
                    choice = Some(Choice::Cancel);
                }
            });
        });
        if choice.is_none() && modal.should_close() {
            choice = Some(Choice::Cancel);
        }
        match choice {
            None => {}
            Some(Choice::Cancel) => self.quit_window = None,
            Some(Choice::Discard) => {
                if let Some(pending) = self.quit_window.take() {
                    self.perform(pending, ctx);
                }
            }
            Some(Choice::Save) => {
                if let Some(pending) = self.quit_window.take() {
                    self.save_all_and_then(pending, ctx);
                }
            }
        }
    }

    /// Guarda todas las pestañas con cambios (las que no tienen archivo piden
    /// ruta una a una) y sale al terminar. Si se cancela un diálogo o un
    /// guardado no arranca, no se sale (lo ya lanzado se guarda igual).
    fn save_all_and_then(&mut self, then: Pending, ctx: &egui::Context) {
        let original = self.active;
        let pending: Vec<usize> = (0..self.tab_count()).filter(|&i| self.tab_at(i).history.is_modified()).collect();
        for i in pending {
            self.switch_to(i);
            if self.tab.save.is_none() {
                self.save(false, None, ctx);
            }
            if self.tab.save.is_none() {
                self.after_saves = None;
                self.switch_to(original.min(self.tab_count() - 1));
                return;
            }
        }
        self.switch_to(original.min(self.tab_count() - 1));
        self.after_saves = Some(then);
    }
}
