//! Núcleo de edición de Pyrotecnika: documento, capas, selección, pintura,
//! filtros e historial. Es puro: no lee archivos ni depende de la interfaz
//! ni del sistema operativo. Reparte el trabajo de píxeles entre núcleos
//! con rayon.

#![forbid(unsafe_code)]

pub mod document;
pub mod filters;
pub mod history;
pub mod paint;
pub mod selection;
