//! Carpetas de la app según la convención de cada SO.

use std::path::PathBuf;

/// Carpeta de `settings.toml` y `keymap.toml`: `%APPDATA%\Pyrotecnika\config`
/// en Windows, `~/Library/Application Support/io.Pyrotecnika` en macOS y
/// `$XDG_CONFIG_HOME/pyrotecnika` (o `~/.config/pyrotecnika`) en Linux.
/// `None` si el SO no da carpeta de usuario.
///
/// Se calcula a mano en vez de con el crate `directories`, que arrastra
/// `option-ext` (MPL-2.0, rechazada por `deny.toml`).
pub fn config_dir() -> Option<PathBuf> {
    let env = |name: &str| std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from);
    if cfg!(target_os = "windows") {
        env("APPDATA").map(|d| d.join("Pyrotecnika").join("config"))
    } else if cfg!(target_os = "macos") {
        env("HOME").map(|h| h.join("Library/Application Support/io.Pyrotecnika"))
    } else {
        env("XDG_CONFIG_HOME")
            .filter(|p| p.is_absolute())
            .or_else(|| env("HOME").map(|h| h.join(".config")))
            .map(|d| d.join("pyrotecnika"))
    }
}

/// Carpeta de datos de la app (autoguardado en `recovery/`, registro de
/// fallos en `logs/`): `%LOCALAPPDATA%\Pyrotecnika` en Windows,
/// `~/Library/Application Support/io.Pyrotecnika` en macOS y
/// `$XDG_DATA_HOME/pyrotecnika` (o `~/.local/share/pyrotecnika`) en Linux.
pub fn data_dir() -> Option<PathBuf> {
    let env = |name: &str| std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from);
    if cfg!(target_os = "windows") {
        env("LOCALAPPDATA").map(|d| d.join("Pyrotecnika"))
    } else if cfg!(target_os = "macos") {
        env("HOME").map(|h| h.join("Library/Application Support/io.Pyrotecnika"))
    } else {
        env("XDG_DATA_HOME")
            .filter(|p| p.is_absolute())
            .or_else(|| env("HOME").map(|h| h.join(".local/share")))
            .map(|d| d.join("pyrotecnika"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_carpeta_de_datos_es_de_pyrotecnika() {
        let dir = data_dir().expect("el SO de pruebas tiene carpeta de usuario");
        assert!(dir.to_string_lossy().to_lowercase().contains("pyrotecnika"), "{}", dir.display());
    }

    #[test]
    fn la_carpeta_de_configuracion_es_de_pyrotecnika() {
        let dir = config_dir().expect("el SO de pruebas tiene carpeta de usuario");
        assert!(dir.to_string_lossy().to_lowercase().contains("pyrotecnika"), "{}", dir.display());
    }
}
