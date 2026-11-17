//! Interfaz de escritorio de Pyrotecnika (egui).

pub(crate) mod app;
#[allow(dead_code)] // Temporal: se usa desde la tarea 5 del plan de la fase 2.
mod commands;
mod design;

pub use app::PyroApp;
