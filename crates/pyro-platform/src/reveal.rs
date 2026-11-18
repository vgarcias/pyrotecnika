//! Mostrar un archivo en el gestor de archivos del SO (Archivo › Mostrar en
//! carpeta).

use std::ffi::OsString;
use std::path::Path;
use std::process::Command;

/// Programas a probar, en orden, para mostrar `path` seleccionado.
pub(crate) fn reveal_commands(path: &Path) -> Vec<(String, Vec<OsString>)> {
    if cfg!(target_os = "windows") {
        // explorer quiere la ruta entre comillas tras la coma; se pasa tal
        // cual (sin el entrecomillado de Rust) en `run`.
        let arg = format!("/select,\"{}\"", path.display());
        vec![("explorer".into(), vec![arg.into()])]
    } else if cfg!(target_os = "macos") {
        vec![("open".into(), vec!["-R".into(), path.as_os_str().to_owned()])]
    } else {
        let folder = path.parent().unwrap_or(path).as_os_str().to_owned();
        vec![
            (
                "dbus-send".into(),
                [
                    "--session",
                    "--dest=org.freedesktop.FileManager1",
                    "--type=method_call",
                    "/org/freedesktop/FileManager1",
                    "org.freedesktop.FileManager1.ShowItems",
                ]
                .iter()
                .map(OsString::from)
                .chain([format!("array:string:{}", file_uri(path)).into(), "string:".into()])
                .collect(),
            ),
            ("xdg-open".into(), vec![folder]),
        ]
    }
}

/// `file://` con la ruta codificada (espacios, tildes…), como pide D-Bus.
pub(crate) fn file_uri(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    let mut out = String::from("file://");
    if !text.starts_with('/') {
        out.push('/');
    }
    for b in text.bytes() {
        if b.is_ascii_alphanumeric() || b"/-_.~:".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

#[cfg(target_os = "windows")]
fn run(program: &str, args: &[OsString]) -> std::io::Result<bool> {
    use std::os::windows::process::CommandExt;
    let mut cmd = Command::new(program);
    for a in args {
        cmd.raw_arg(a);
    }
    // explorer devuelve 1 aunque funcione: basta con que arranque.
    cmd.spawn().map(|_| true)
}

#[cfg(not(target_os = "windows"))]
fn run(program: &str, args: &[OsString]) -> std::io::Result<bool> {
    Command::new(program).args(args).status().map(|s| s.success())
}

/// Abre el gestor de archivos con `path` seleccionado (o su carpeta).
pub fn reveal(path: &Path) -> Result<(), String> {
    let mut last = String::from("no hay gestor de archivos");
    for (program, args) in reveal_commands(path) {
        match run(&program, &args) {
            Ok(true) => return Ok(()),
            Ok(false) => last = format!("{program} terminó con error"),
            Err(e) => last = format!("{program}: {e}"),
        }
    }
    Err(last)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_para_este_so() {
        let p = std::env::temp_dir().join("a b.png");
        let cmds = reveal_commands(&p);
        assert!(!cmds.is_empty());
        let all: String = cmds.iter().flat_map(|(_, a)| a.iter().map(|s| s.to_string_lossy().into_owned())).collect();
        assert!(all.contains("a b.png") || all.contains("a%20b.png"), "{all}");
    }

    #[test]
    fn uri_de_archivo_codificada() {
        assert_eq!(file_uri(std::path::Path::new("/tmp/año 1.png")), "file:///tmp/a%C3%B1o%201.png");
    }
}
