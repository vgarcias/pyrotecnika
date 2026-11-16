//! Integración de Pyrotecnika con el sistema operativo.
//!
//! Todo el código específico de un SO (`#[cfg(target_os = ...)]`) vive en
//! este crate, dentro de `os/`. El resto de la aplicación solo usa la API
//! pública, que es la misma en Windows, macOS y Linux.

pub mod open_files;
pub mod updates;

#[cfg(target_os = "macos")]
mod os {
    pub mod macos;
}
