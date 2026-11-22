//! Guardar y Guardar como en OpenRaster (`.ora`). Se guarda en segundo
//! plano una instantánea (capas compartidas): se puede seguir editando. El
//! archivo original (PNG, Fireworks…) nunca se sobrescribe.

use std::path::{Path, PathBuf};

use eframe::egui;

use pyro_formats::ora::write_ora;
use pyro_services::jobs::{Job, JobError, JobPoll};

use super::guard::{self, Pending, Tone};
use super::tab::Tab;
use super::{tasks, PyroApp};
use crate::commands::store::same_file;

pub(super) struct SaveTask {
    pub(super) job: Job<()>,
    pub(super) path: PathBuf,
    /// Posición del historial de la instantánea guardada.
    pub(super) position: u64,
    /// Qué hacer al terminar bien (cerrar la pestaña, salir…).
    pub(super) then: Option<Pending>,
}

/// Ruta que propone Guardar como: la `.ora` de la pestaña, o el nombre del
/// origen con extensión `.ora` en su misma carpeta.
pub(super) fn suggested_save_path(tab: &Tab) -> PathBuf {
    if let Some(ora) = &tab.ora_path {
        return ora.clone();
    }
    match &tab.source_path {
        Some(source) => source.with_extension("ora"),
        None => PathBuf::from(format!("Sin título {}.ora", tab.untitled.max(1))),
    }
}

impl PyroApp {
    /// Pide dónde guardar (en pruebas, `save_dialog_override`).
    fn pick_save_path(&mut self, suggested: &Path) -> Option<PathBuf> {
        #[cfg(test)]
        if let Some(choice) = self.save_dialog_override.take() {
            return choice;
        }
        let mut dialog = rfd::FileDialog::new().add_filter("OpenRaster (con capas)", &["ora"]);
        if let Some(dir) = suggested.parent().filter(|d| !d.as_os_str().is_empty()) {
            dialog = dialog.set_directory(dir);
        }
        if let Some(name) = suggested.file_name() {
            dialog = dialog.set_file_name(name.to_string_lossy());
        }
        dialog.save_file()
    }

    /// Guardar (`as_new = false`) o Guardar como (`true`) la pestaña activa;
    /// `then` sigue al terminar bien.
    pub(super) fn save(&mut self, as_new: bool, then: Option<Pending>, ctx: &egui::Context) {
        if self.tab.save.is_some() {
            return;
        }
        let path = match (&self.tab.ora_path, as_new) {
            (Some(path), false) => path.clone(),
            _ => {
                let suggested = suggested_save_path(&self.tab);
                let Some(path) = self.pick_save_path(&suggested) else { return };
                path
            }
        };
        let is_ora = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("ora"));
        let is_source = self.tab.source_path.as_deref().is_some_and(|s| same_file(s, &path));
        if !is_ora || (is_source && self.tab.ora_path.is_none()) {
            self.fail(format!(
                "No se guardó en «{}»: Guardar solo escribe archivos .ora y nunca toca el original. Para una \
                 imagen aplanada usa Exportar.",
                guard::short_name(&path)
            ));
            return;
        }
        let doc = self.tab.doc.clone();
        let target = path.clone();
        let job = Job::spawn(tasks::wake(ctx), move |monitor| {
            write_ora(&doc, &target, monitor).map_err(|e| {
                let reason = e
                    .chain()
                    .find_map(|c| c.downcast_ref::<std::io::Error>())
                    .map_or_else(|| e.to_string(), |io| guard::io_reason(io, true));
                JobError::Failed(format!("No se pudo guardar «{}»: {reason}.", guard::short_name(&target)))
            })
        });
        let position = self.tab.history.position();
        self.tab.save = Some(SaveTask { job, path, position, then });
        self.info("Guardando…");
    }

    pub(super) fn any_saving(&self) -> bool {
        (0..self.tab_count()).any(|i| self.tab_at(i).save.is_some())
    }

    /// Recoge los guardados terminados de todas las pestañas.
    pub(super) fn poll_saves(&mut self, ctx: &egui::Context) {
        let mut finished = Vec::new();
        for i in 0..self.tab_count() {
            let tab = self.tab_at_mut(i);
            let Some(save) = &mut tab.save else { continue };
            let result = save.job.poll();
            if matches!(result, JobPoll::Running) {
                continue;
            }
            if let Some(save) = tab.save.take() {
                finished.push((i, tab.uid, save, result));
            }
        }
        for (i, uid, save, result) in finished {
            match result {
                JobPoll::Done(()) => {
                    let tab = self.tab_at_mut(i);
                    tab.history.mark_saved_at(save.position);
                    tab.ora_path = Some(save.path.clone());
                    self.settings.recent.push(save.path.clone());
                    self.save_settings();
                    self.info(format!("Guardado «{}».", guard::short_name(&save.path)));
                    self.after_save(uid);
                    if let Some(then) = save.then {
                        if matches!(then, Pending::CloseTab) {
                            self.switch_to(i);
                        }
                        self.request(then, ctx);
                    }
                }
                JobPoll::Failed(message) => {
                    self.quit_after_saves = false;
                    self.status = guard::Status { tone: Tone::Error, text: message };
                }
                JobPoll::Cancelled | JobPoll::Running => {}
            }
        }
        if self.quit_after_saves && !self.any_saving() {
            self.quit_after_saves = false;
            self.request(Pending::Quit, ctx);
        }
    }

    /// Tras guardar bien una pestaña (la tarea 6 borra aquí su autoguardado).
    pub(super) fn after_save(&mut self, _uid: u64) {}

    /// Texto de la barra de estado cuando hay cambios sin guardar.
    pub(super) fn history_status_label(&self) -> &'static str {
        if self.tab.history.is_modified() {
            "Cambios sin guardar"
        } else {
            ""
        }
    }
}
