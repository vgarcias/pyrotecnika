//! Protección del trabajo: mensajes con severidad, confirmación antes de
//! descartar cambios, título de la ventana y bloqueo de capas que no se ven.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use eframe::egui;

use pyro_core::document::Document;

use super::{icons, Action, PyroApp};
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
    /// Instalar la actualización encontrada (en Windows cierra la app).
    InstallUpdate,
    /// Abrir la versión recién instalada y cerrar esta.
    Restart,
}

enum Choice {
    Export,
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
        if self.tab.history.is_modified() {
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
                self.reset_document(Document::new(width, height, bg));
                self.info("Documento nuevo.");
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
        if requested && !self.closing && self.tab.history.is_modified() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.confirm = Some(Pending::Quit);
            self.confirm_focus = true;
        }
    }

    /// Título de la ventana: archivo abierto y marca de cambios sin exportar.
    pub(super) fn sync_title(&mut self, ctx: &egui::Context) {
        let name = self.tab.source_path.as_deref().map_or_else(|| "Sin título".to_string(), short_name);
        let mark = if self.tab.history.is_modified() { "• " } else { "" };
        let title = format!("{mark}{name} - Pyrotecnika");
        if title != self.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }
    }

    /// Qué ocurrirá después de guardar la exportación, si hay algo pendiente.
    pub(super) fn after_export_hint(&self) -> Option<String> {
        Some(match self.tab.after_export.as_ref()? {
            Pending::Open(path) => format!("Después de guardar se abrirá «{}».", short_name(path)),
            Pending::New { .. } => "Después de guardar se creará el documento nuevo.".into(),
            Pending::Quit => "Después de guardar se cerrará Pyrotecnika.".into(),
            Pending::InstallUpdate => "Después de guardar se instalará la actualización.".into(),
            Pending::Restart => "Después de guardar se reiniciará Pyrotecnika.".into(),
        })
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
            Pending::InstallUpdate => (
                "¿Actualizar y descartar los cambios?".to_string(),
                "Para actualizar, Pyrotecnika se cierra y esos cambios se pierden.",
            ),
            Pending::Restart => {
                ("¿Reiniciar y descartar los cambios?".to_string(), "Si reinicias ahora, esos cambios se pierden.")
            }
        };
        let layers = self.tab.doc.layers.len();
        let focus = std::mem::take(&mut self.confirm_focus);
        let mut choice = None;
        let modal = egui::Modal::new(egui::Id::new("confirmar-descartar")).show(ctx, |ui| {
            ui.set_max_width(440.0);
            ui.heading(title);
            ui.add_space(8.0);
            ui.label(format!("El documento actual tiene cambios sin exportar. {consequence}"));
            if layers > 1 {
                ui.add_space(4.0);
                ui.label(format!(
                    "Exportar guarda la imagen aplanada: las {layers} capas no se conservan en el archivo."
                ));
            }
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if widgets::button(ui, "Exportar primero…").clicked() {
                    choice = Some(Choice::Export);
                }
                if widgets::button(ui, "Descartar cambios").clicked() {
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
            Some(Choice::Export) => {
                let pending = self.confirm.take();
                self.run(Action::Export, ctx);
                if self.tab.export_dialog.is_some() {
                    self.tab.after_export = pending;
                }
            }
        }
    }
}
