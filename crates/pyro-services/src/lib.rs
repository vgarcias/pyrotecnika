//! Servicios de Pyrotecnika que no dependen de la interfaz ni del sistema
//! operativo. Por ahora, las tareas en segundo plano.

#![forbid(unsafe_code)]

pub mod crashlog;
pub mod jobs;
pub mod recovery;
