//! Tareas que cambian el documento y corren en segundo plano (abrir; en la
//! fase 3 también los filtros). Mientras corren, la barra de estado muestra
//! el progreso y «Cancelar», y la edición se bloquea; la vista sigue viva.

use std::path::PathBuf;
use std::sync::Arc;

use eframe::egui;
use image::RgbaImage;

use pyro_core::filters::Filter;

use pyro_core::task::Cancelled;
use pyro_formats::io::{self, Opened, TooLarge};
use pyro_services::jobs::{Job, JobError, JobPoll, Wake};

use super::{guard, Action, PyroApp};
use crate::design::widgets;

pub(super) enum TaskKind {
    Open(PathBuf),
    /// Filtro sin parámetros sobre la capa activa.
    Filter(Filter),
}

pub(super) enum TaskOutput {
    Opened(Opened),
    Filtered(RgbaImage),
}

pub(super) struct DocTask {
    pub(super) kind: TaskKind,
    pub(super) job: Job<TaskOutput>,
    /// Documento sobre el que se lanzó (ver [`PyroApp::generation`]).
    pub(super) generation: u64,
}

impl TaskKind {
    fn label(&self) -> String {
        match self {
            TaskKind::Open(path) => format!("Abriendo «{}»…", guard::short_name(path)),
            TaskKind::Filter(filter) => format!("Aplicando «{}»…", filter.name()),
        }
    }
}

/// Despierta a la interfaz cuando la tarea avanza o termina.
pub(super) fn wake(ctx: &egui::Context) -> Wake {
    let ctx = ctx.clone();
    Arc::new(move || ctx.request_repaint())
}

impl Action {
    /// Acciones que no cambian el documento: siguen disponibles durante una
    /// tarea (y Salir, que la cancela al cerrar).
    pub(super) fn is_view_only(self) -> bool {
        matches!(
            self,
            Action::ZoomIn
                | Action::ZoomOut
                | Action::Fit
                | Action::ActualSize
                | Action::SetTheme(_)
                | Action::UiScaleUp
                | Action::UiScaleDown
                | Action::UiScaleReset
                | Action::OpenPalette
                | Action::OpenPreferences
                | Action::Quit
        )
    }
}

/// Mensaje de un archivo que no se pudo abrir.
fn open_error(path: &std::path::Path, e: &anyhow::Error) -> String {
    if let Some(t) = e.downcast_ref::<TooLarge>() {
        return t.to_string();
    }
    let reason = e
        .chain()
        .find_map(|cause| cause.downcast_ref::<std::io::Error>())
        .map_or_else(|| e.to_string(), |io| guard::io_reason(io, false));
    format!("No se pudo abrir «{}»: {reason}.", guard::short_name(path))
}

impl PyroApp {
    pub(super) fn busy(&self) -> bool {
        self.task.is_some()
    }

    pub(super) fn start_task(&mut self, kind: TaskKind, job: Job<TaskOutput>) {
        if let Some(old) = self.task.take() {
            old.job.cancel();
        }
        self.task = Some(DocTask { kind, job, generation: self.generation });
    }

    /// Cancela al momento: se suelta la tarea (su resultado, si llega, se
    /// descarta) sin esperar a que el hilo termine, que puede estar dentro de
    /// un decodificador que no se interrumpe.
    pub(super) fn cancel_task(&mut self) {
        let Some(task) = self.task.take() else { return };
        task.job.cancel();
        if matches!(task.kind, TaskKind::Filter(_)) {
            self.history.discard_last();
        }
        self.info("Cancelado.");
    }

    /// Lee y decodifica en segundo plano; el documento actual no se toca
    /// hasta que termina bien.
    pub(super) fn start_open(&mut self, path: PathBuf, ctx: &egui::Context) {
        let source = path.clone();
        let job = Job::spawn(wake(ctx), move |monitor| match io::open_path(&source, monitor) {
            Ok(opened) => Ok(TaskOutput::Opened(opened)),
            Err(e) if e.downcast_ref::<Cancelled>().is_some() => Err(JobError::Cancelled),
            Err(e) => Err(JobError::Failed(open_error(&source, &e))),
        });
        self.start_task(TaskKind::Open(path), job);
    }

    /// Aplica un filtro sin parámetros a la capa activa en segundo plano
    /// (el paso de historial ya está puesto; se quita si se cancela).
    pub(super) fn start_filter(&mut self, filter: Filter, source: Arc<RgbaImage>, ctx: &egui::Context) {
        let sel = self.doc.selection.clone();
        let job = Job::spawn(wake(ctx), move |monitor| {
            Ok(TaskOutput::Filtered(filter.run(&source, sel.as_ref(), monitor)?))
        });
        self.start_task(TaskKind::Filter(filter), job);
    }

    /// Recoge el resultado de la tarea, si terminó. Se descarta si entretanto
    /// se abrió o creó otro documento.
    pub(super) fn poll_task(&mut self) {
        let Some(task) = &mut self.task else { return };
        let result = task.job.poll();
        if matches!(result, JobPoll::Running) {
            return;
        }
        let Some(task) = self.task.take() else { return };
        if task.generation != self.generation {
            return;
        }
        let filtering = matches!(task.kind, TaskKind::Filter(_));
        match (result, task.kind) {
            (JobPoll::Done(TaskOutput::Opened(opened)), TaskKind::Open(path)) => self.finish_open(path, opened),
            (JobPoll::Done(TaskOutput::Filtered(img)), TaskKind::Filter(filter)) => {
                self.doc.active_layer_mut().pixels = Arc::new(img);
                self.history.seal(&self.doc);
                self.mark_dirty();
                self.info(format!("{} aplicado.", filter.name()));
            }
            (JobPoll::Done(_), _) | (JobPoll::Running, _) => {}
            (JobPoll::Failed(message), _) => {
                if filtering {
                    self.history.discard_last();
                }
                self.fail(message);
            }
            (JobPoll::Cancelled, _) => {
                if filtering {
                    self.history.discard_last();
                }
                self.info("Cancelado.");
            }
        }
    }

    /// Tarea en curso en la barra de estado: qué hace, cuánto lleva y
    /// «Cancelar».
    pub(super) fn task_status(&mut self, ui: &mut egui::Ui) {
        let Some(task) = &self.task else { return };
        ui.label(task.kind.label());
        let progress = task.job.progress();
        ui.add(egui::ProgressBar::new(progress.unwrap_or(0.0)).desired_width(160.0).animate(progress.is_none()));
        if widgets::button(ui, "Cancelar").clicked() {
            self.cancel_task();
        }
        ui.separator();
    }
}
