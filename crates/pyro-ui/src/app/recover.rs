//! Recuperar al arrancar el trabajo autoguardado de una sesión que no cerró
//! bien: Recuperar, Descartar o Decidir después.

use std::time::SystemTime;

use eframe::egui;

use pyro_formats::ora::decode_ora;
use pyro_services::jobs::{Job, JobError, JobPoll};
use pyro_services::recovery::{Recovery, RecoveryEntry};

use super::tab::Tab;
use super::{guard, tasks, PyroApp};
use crate::design::widgets;

/// «hace 3 min», «hace 2 h», «hace 1 día».
fn ago(when: SystemTime) -> String {
    let secs = SystemTime::now().duration_since(when).map_or(0, |d| d.as_secs());
    match secs {
        0..=59 => "hace un momento".into(),
        60..=3599 => format!("hace {} min", secs / 60),
        3600..=86_399 => format!("hace {} h", secs / 3600),
        _ => match secs / 86_400 {
            1 => "hace 1 día".into(),
            n => format!("hace {n} días"),
        },
    }
}

impl PyroApp {
    /// Lee los autoguardados en segundo plano; cada uno abre una pestaña.
    fn start_recovery(&mut self, entries: Vec<RecoveryEntry>, ctx: &egui::Context) {
        for entry in entries {
            let file = entry.ora_file();
            let job = Job::spawn(tasks::wake(ctx), move |monitor| {
                let bytes = std::fs::read(&file).map_err(|e| JobError::Failed(e.to_string()))?;
                decode_ora(&bytes, monitor).map(|o| o.document).map_err(|e| JobError::Failed(e.to_string()))
            });
            self.recovering.push((entry, job));
        }
    }

    /// Recoge los documentos recuperados y abre su pestaña.
    pub(super) fn poll_recovery(&mut self) {
        let mut done = Vec::new();
        self.recovering.retain_mut(|(entry, job)| match job.poll() {
            JobPoll::Running => true,
            JobPoll::Done(doc) => {
                done.push((entry.clone(), Ok(doc)));
                false
            }
            JobPoll::Failed(e) => {
                done.push((entry.clone(), Err(e)));
                false
            }
            JobPoll::Cancelled => false,
        });
        for (entry, result) in done {
            match result {
                Ok(doc) => self.open_recovered(entry, doc),
                Err(e) => self.fail(format!("No se pudo recuperar «{}»: {e}", entry.title)),
            }
        }
    }

    fn open_recovered(&mut self, entry: RecoveryEntry, doc: pyro_core::document::Document) {
        let file = entry.ora_path.clone().or_else(|| entry.source_path.clone());
        let already_open = file.as_deref().and_then(|p| self.find_tab(p)).is_some();
        let untitled = if file.is_none() { self.next_untitled() } else { 0 };
        let mut tab = Tab::new(doc, untitled);
        tab.source_path = entry.source_path.clone();
        tab.ora_path = entry.ora_path.clone();
        if already_open {
            let name = file.as_deref().map(guard::short_name).unwrap_or_default();
            tab.label = Some(format!("{name} (recuperado)"));
        }
        // Tiene cambios sin guardar, pero no hay paso que deshacer.
        tab.history.mark_unsaved();
        tab.recovered_from = Some(entry);
        self.add_tab(tab);
        self.info("Documento recuperado. Guárdalo para conservarlo.");
    }

    /// Ventana «Recuperar trabajo».
    pub(super) fn recover_window(&mut self, ctx: &egui::Context) {
        let Some(entries) = &self.recover_dialog else { return };
        let rows: Vec<String> = entries.iter().map(|e| format!("{} — {}", e.title, ago(e.saved_at))).collect();
        let (mut recover, mut discard, mut later) = (false, false, false);
        egui::Modal::new(egui::Id::new("recuperar")).show(ctx, |ui| {
            ui.set_max_width(460.0);
            ui.heading("Recuperar trabajo");
            ui.add_space(8.0);
            ui.label("Pyrotecnika se cerró sin guardar estos documentos. Se autoguardaron:");
            for row in &rows {
                ui.label(format!("• {row}"));
            }
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                recover = widgets::primary_button(ui, "Recuperar").clicked();
                discard = widgets::button(ui, "Descartar").clicked();
                later = widgets::button(ui, "Decidir después").clicked();
            });
        });
        if recover {
            if let Some(entries) = self.recover_dialog.take() {
                self.start_recovery(entries, ctx);
            }
        } else if discard {
            for entry in self.recover_dialog.take().into_iter().flatten() {
                Recovery::discard(&entry);
            }
        } else if later {
            self.recover_dialog = None;
        }
    }
}

#[cfg(test)]
pub(super) fn sample_entries() -> Vec<RecoveryEntry> {
    let at = SystemTime::now() - std::time::Duration::from_secs(185);
    vec![
        RecoveryEntry {
            session: "recovery/0".into(),
            uid: 1,
            title: "cenote.png".into(),
            source_path: Some("cenote.png".into()),
            ora_path: None,
            saved_at: at,
        },
        RecoveryEntry {
            session: "recovery/0".into(),
            uid: 2,
            title: "Sin título 2".into(),
            source_path: None,
            ora_path: None,
            saved_at: at,
        },
    ]
}
