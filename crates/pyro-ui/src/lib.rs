//! Interfaz de escritorio de Pyrotecnika (egui).

pub(crate) mod app;
// Temporal: los tokens se van usando en las tareas 3–6 del plan de la fase 1.
#[allow(dead_code)]
mod design;

pub use app::PyroApp;
