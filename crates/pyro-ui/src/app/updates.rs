//! Ayuda › Buscar actualizaciones…: ventana con el estado de la búsqueda y
//! de la instalación. Solo se conecta cuando el usuario lo pide.

use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::time::Duration;

use eframe::egui;

use pyro_platform::updates::{self, Available};

use super::{guard, Pending, PyroApp, Tone};
use crate::design::widgets;

pub(super) const VERSION: &str = env!("CARGO_PKG_VERSION");

pub(super) enum UpdateState {
    Checking(Receiver<Result<Option<Available>, String>>),
    UpToDate,
    Found(Available),
    Installing { version: String, job: Receiver<Result<(), String>> },
    Installed { version: String },
    Failed(String),
}

enum Click {
    Install,
    Restart,
    OpenDownloads,
    Close,
}

impl PyroApp {
    pub(super) fn check_updates(&mut self) {
        if matches!(self.update, Some(UpdateState::Checking(_) | UpdateState::Installing { .. })) {
            return;
        }
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let _ = tx.send(updates::check(VERSION));
        });
        self.update = Some(UpdateState::Checking(rx));
    }

    /// Lo llama `perform` cuando ya no quedan cambios sin exportar.
    pub(super) fn install_update(&mut self) {
        let Some(UpdateState::Found(available)) = self.update.take() else {
            return;
        };
        let version = available.version.clone();
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let _ = tx.send(available.install());
        });
        self.update = Some(UpdateState::Installing { version, job: rx });
    }

    /// Lo llama `perform`: abre la versión nueva y cierra esta.
    pub(super) fn restart_after_update(&mut self, ctx: &egui::Context) {
        match updates::relaunch() {
            Ok(()) => {
                self.closing = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            Err(e) => self.update = Some(UpdateState::Failed(e)),
        }
    }

    fn poll_update_jobs(&mut self, ctx: &egui::Context) {
        let next = match &self.update {
            Some(UpdateState::Checking(rx)) => match rx.try_recv() {
                Ok(Ok(None)) => Some(UpdateState::UpToDate),
                Ok(Ok(Some(available))) => Some(UpdateState::Found(available)),
                Ok(Err(e)) => Some(UpdateState::Failed(e)),
                Err(TryRecvError::Disconnected) => Some(UpdateState::Failed("La búsqueda se interrumpió.".into())),
                Err(TryRecvError::Empty) => None,
            },
            Some(UpdateState::Installing { version, job }) => match job.try_recv() {
                Ok(Ok(())) => Some(UpdateState::Installed { version: version.clone() }),
                Ok(Err(e)) => Some(UpdateState::Failed(e)),
                Err(TryRecvError::Disconnected) => Some(UpdateState::Failed("La instalación se interrumpió.".into())),
                Err(TryRecvError::Empty) => None,
            },
            _ => None,
        };
        if let Some(state) = next {
            self.update = Some(state);
        }
        if matches!(self.update, Some(UpdateState::Checking(_) | UpdateState::Installing { .. })) {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }

    pub(super) fn update_window(&mut self, ctx: &egui::Context) {
        self.poll_update_jobs(ctx);
        // El diálogo de cambios sin exportar va primero; luego se vuelve aquí.
        if self.confirm.is_some() || self.export_dialog.is_some() {
            return;
        }
        let Some(state) = &self.update else {
            return;
        };
        let busy = matches!(state, UpdateState::Checking(_) | UpdateState::Installing { .. });
        let mut click = None;
        let modal = egui::Modal::new(egui::Id::new("actualizaciones")).show(ctx, |ui| {
            ui.set_max_width(460.0);
            ui.heading("Actualizaciones");
            ui.add_space(8.0);
            match state {
                UpdateState::Checking(_) => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Buscando una versión nueva…");
                    });
                }
                UpdateState::UpToDate => {
                    ui.label(format!("Tienes la versión más reciente ({VERSION})."));
                }
                UpdateState::Found(available) => {
                    ui.label(format!("Hay una versión nueva: {} (tienes la {VERSION}).", available.version));
                    if let Some(notes) = &available.notes {
                        ui.add_space(6.0);
                        egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                            ui.label(notes.as_str());
                        });
                    }
                    ui.add_space(6.0);
                    if available.can_install {
                        ui.label(
                            "Se descarga, se comprueba su firma y se instala. Después hay que reiniciar Pyrotecnika.",
                        );
                    } else {
                        guard::notice(
                            ui,
                            Tone::Info,
                            "Esta copia no se actualiza sola: se instaló con MSI, PKG, DEB o RPM (o es una \
                             compilación de desarrollo). Instala la versión nueva con el mismo método.",
                        );
                    }
                }
                UpdateState::Installing { version, .. } => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(format!("Descargando e instalando la versión {version}…"));
                    });
                }
                UpdateState::Installed { version } => {
                    ui.label(format!("La versión {version} está instalada. Reinicia Pyrotecnika para usarla."));
                }
                UpdateState::Failed(e) => guard::notice(ui, Tone::Error, e),
            }
            ui.add_space(12.0);
            ui.horizontal(|ui| match state {
                UpdateState::Found(available) if available.can_install => {
                    if widgets::primary_button(ui, "Descargar e instalar").clicked() {
                        click = Some(Click::Install);
                    }
                    if widgets::button(ui, "Más tarde").clicked() {
                        click = Some(Click::Close);
                    }
                }
                UpdateState::Found(_) | UpdateState::Failed(_) => {
                    if widgets::button(ui, "Abrir página de descargas").clicked() {
                        click = Some(Click::OpenDownloads);
                    }
                    if widgets::button(ui, "Cerrar").clicked() {
                        click = Some(Click::Close);
                    }
                }
                UpdateState::Installed { .. } => {
                    if widgets::primary_button(ui, "Reiniciar ahora").clicked() {
                        click = Some(Click::Restart);
                    }
                    if widgets::button(ui, "Más tarde").clicked() {
                        click = Some(Click::Close);
                    }
                }
                UpdateState::UpToDate => {
                    if widgets::button(ui, "Cerrar").clicked() {
                        click = Some(Click::Close);
                    }
                }
                UpdateState::Checking(_) | UpdateState::Installing { .. } => {}
            });
        });
        // Escape o clic fuera cierran, salvo mientras se trabaja.
        if click.is_none() && !busy && modal.should_close() {
            click = Some(Click::Close);
        }
        match click {
            None => {}
            Some(Click::Close) => self.update = None,
            Some(Click::OpenDownloads) => {
                ctx.open_url(egui::OpenUrl::new_tab(updates::RELEASES_URL));
            }
            Some(Click::Install) => self.request(Pending::InstallUpdate, ctx),
            Some(Click::Restart) => self.request(Pending::Restart, ctx),
        }
    }
}
