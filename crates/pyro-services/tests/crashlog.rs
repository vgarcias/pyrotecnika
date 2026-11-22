//! Registro de fallos (panic hook). Binario de pruebas propio: el hook es
//! global al proceso.

use std::path::Path;
use std::time::{Duration, SystemTime};

use pyro_services::crashlog::{install_panic_hook, log_name, panic_entry, prune};

#[test]
fn un_panic_escribe_en_el_log() {
    let dir = tempfile::tempdir().unwrap();
    install_panic_hook(dir.path().to_path_buf());
    let _ = std::thread::spawn(|| panic!("prueba de fallo")).join();
    let log = dir.path().join(log_name(SystemTime::now()));
    let text = std::fs::read_to_string(&log).unwrap_or_else(|_| panic!("no se escribió {}", log.display()));
    assert!(text.contains("prueba de fallo"), "{text}");
    assert!(text.contains("crashlog.rs"), "{text}");
}

#[test]
fn sin_rutas_del_usuario() {
    let entry = panic_entry(
        r"no se pudo abrir C:\Users\ana\fotos\x.png",
        r"src\main.rs:1:1",
        "",
        Some(Path::new(r"C:\Users\ana")),
        SystemTime::now(),
    );
    assert!(entry.contains(r"~\fotos\x.png"), "{entry}");
    assert!(!entry.contains(r"C:\Users\ana"), "{entry}");
}

#[test]
fn poda_de_siete_dias() {
    let dir = tempfile::tempdir().unwrap();
    let now = SystemTime::now();
    let day = Duration::from_secs(86_400);
    let recent = dir.path().join(log_name(now - day * 3));
    let old = dir.path().join(log_name(now - day * 10));
    std::fs::write(&recent, "a").unwrap();
    std::fs::write(&old, "b").unwrap();
    std::fs::write(dir.path().join("otro.txt"), "c").unwrap();
    prune(dir.path(), now, 7);
    assert!(recent.exists());
    assert!(!old.exists());
    assert!(dir.path().join("otro.txt").exists());
    assert_eq!(log_name(SystemTime::UNIX_EPOCH + day * 19_682), "pyrotecnika-2023-11-21.log");
}
