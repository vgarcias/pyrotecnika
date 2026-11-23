//! Autoguardado: cada 2 min, cada pestaña con cambios desde su último
//! autoguardado se escribe en la carpeta de recuperación de la sesión, en
//! segundo plano. El latido de la sesión se renueva cada 30 s.

use std::time::{Duration, Instant};

use eframe::egui;

use pyro_services::jobs::{Job, JobError, JobPoll};

use super::{tasks, PyroApp};

pub(super) const AUTOSAVE_EVERY: Duration = Duration::from_secs(120);
/// Cada cuánto se despierta la interfaz para mirar si toca autoguardar.
pub(super) const HEARTBEAT_EVERY: Duration = Duration::from_secs(30);

/// Autoguardado en curso de una pestaña y la posición del historial que
/// escribe.
pub(super) struct AutosaveTask {
    pub(super) job: Job<()>,
    pub(super) position: u64,
}

impl PyroApp {
    /// Hora para el autoguardado (en pruebas se puede adelantar).
    pub(super) fn now(&self) -> Instant {
        #[cfg(test)]
        return Instant::now() + self.clock_offset;
        #[cfg(not(test))]
        Instant::now()
    }

    /// Latido, autoguardado periódico y recogida de los terminados. Se llama
    /// cada cuadro; pide repintar para que el reloj avance sin eventos.
    pub(super) fn tick_autosave(&mut self, ctx: &egui::Context) {
        let Some(recovery) = self.recovery.clone() else { return };
        // El latido va en su propio hilo (Recovery::start); aquí solo hace
        // falta que el reloj del autoguardado avance sin eventos.
        ctx.request_repaint_after(HEARTBEAT_EVERY);
        let now = self.now();
        let due = now.duration_since(self.last_autosave) >= AUTOSAVE_EVERY;
        let mut waiting = false;
        let mut failed = None;
        for i in 0..self.tab_count() {
            let title = self.tab_title(i);
            let tab = self.tab_at_mut(i);
            if let Some(task) = &mut tab.autosave {
                match task.job.poll() {
                    JobPoll::Running => continue,
                    JobPoll::Done(()) => {
                        tab.autosaved_position = Some(task.position);
                        // El autoguardado de una sesión vieja ya no hace falta.
                        if let Some(old) = tab.recovered_from.take() {
                            pyro_services::recovery::Recovery::discard(&old);
                        }
                    }
                    JobPoll::Failed(e) => failed = Some(e),
                    JobPoll::Cancelled => {}
                }
                tab.autosave = None;
            }
            let state = tab.history.state_id();
            let needed = tab.history.is_modified() && tab.autosaved_position != Some(state);
            if !(needed && (due || tab.recovered_from.is_some())) {
                continue;
            }
            // Solo estados completos: ni a mitad de un filtro ni de un trazo.
            if tab.task.is_some() || !matches!(tab.drag, super::tools::Drag::None) {
                waiting = true;
                continue;
            }
            let (uid, doc, source, ora) = (tab.uid, tab.doc.clone(), tab.source_path.clone(), tab.ora_path.clone());
            let target = recovery.clone();
            let job = Job::spawn(tasks::wake(ctx), move |monitor| {
                target
                    .autosave(uid, &title, source.as_ref(), ora.as_ref(), &doc, monitor)
                    .map_err(|e| JobError::Failed(e.to_string()))
            });
            tab.autosave = Some(AutosaveTask { job, position: state });
        }
        // Las que tuvieron que esperar lo intentan en el próximo cuadro.
        if due && !waiting {
            self.last_autosave = now;
        }
        if let Some(e) = failed {
            if !self.autosave_warned {
                self.autosave_warned = true;
                self.warn(format!("No se pudo autoguardar: {e}"));
            }
        }
    }

    /// Borra el autoguardado de una pestaña (se guardó o se cerró).
    pub(super) fn forget_autosave(&mut self, uid: u64) {
        if let Some(recovery) = &self.recovery {
            recovery.remove(uid);
        }
    }
}
