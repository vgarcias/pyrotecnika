//! Carpetas de la app según la convención de cada SO.

use std::path::PathBuf;

/// Carpeta de `settings.toml` y `keymap.toml`: `%APPDATA%\Pyrotecnika\config`
/// en Windows, `~/Library/Application Support/io.Pyrotecnika` en macOS y
/// `~/.config/pyrotecnika` en Linux. `None` si el SO no da carpeta de usuario.
pub fn config_dir() -> Option<PathBuf> {
    directories::ProjectDirs::from("io", "", "Pyrotecnika").map(|d| d.config_dir().to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_carpeta_de_configuracion_es_de_pyrotecnika() {
        let dir = config_dir().expect("el SO de pruebas tiene carpeta de usuario");
        assert!(dir.to_string_lossy().to_lowercase().contains("pyrotecnika"), "{}", dir.display());
    }
}
