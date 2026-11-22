//! Autoguardado por sesión y recuperación de las sesiones que no cerraron
//! bien.

use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use pyro_core::document::Document;
use pyro_core::task::NoMonitor;
use pyro_services::recovery::Recovery;

fn doc(w: u32, h: u32) -> Document {
    Document::new(w, h, Some([10, 20, 30, 255]))
}

#[test]
fn autoguardar_y_listar_huerfanos() {
    let base = tempfile::tempdir().unwrap();
    let a = Recovery::start(base.path()).unwrap();
    a.autosave(1, "cenote.png", Some(&PathBuf::from("C:/fotos/cenote.png")), None, &doc(40, 30), &NoMonitor).unwrap();
    a.autosave(2, "Sin título 2", None, Some(&PathBuf::from("C:/x/logo.ora")), &doc(8, 8), &NoMonitor).unwrap();
    let now = SystemTime::now();
    a.write_heartbeat(now - Duration::from_secs(600)).unwrap();
    let b = Recovery::start(base.path()).unwrap();
    let mut found = Recovery::orphans(base.path(), now, Some(b.session_dir()));
    found.sort_by_key(|e| e.uid);
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].title, "cenote.png");
    assert_eq!(found[0].source_path, Some(PathBuf::from("C:/fotos/cenote.png")));
    assert_eq!(found[1].ora_path, Some(PathBuf::from("C:/x/logo.ora")));
    let bytes = std::fs::read(found[0].ora_file()).unwrap();
    let back = pyro_formats::ora::decode_ora(&bytes, &NoMonitor).unwrap();
    assert_eq!((back.document.width, back.document.height), (40, 30));
}

#[test]
fn sesion_viva_no_se_ofrece() {
    let base = tempfile::tempdir().unwrap();
    let a = Recovery::start(base.path()).unwrap();
    a.autosave(1, "a", None, None, &doc(4, 4), &NoMonitor).unwrap();
    a.heartbeat().unwrap();
    let b = Recovery::start(base.path()).unwrap();
    assert!(Recovery::orphans(base.path(), SystemTime::now(), Some(b.session_dir())).is_empty());
}

#[test]
fn cierre_normal_no_deja_nada() {
    let base = tempfile::tempdir().unwrap();
    let a = Recovery::start(base.path()).unwrap();
    a.autosave(1, "a", None, None, &doc(4, 4), &NoMonitor).unwrap();
    let dir = a.session_dir().to_path_buf();
    a.finish();
    assert!(!dir.exists());
}

#[test]
fn descartar() {
    let base = tempfile::tempdir().unwrap();
    let a = Recovery::start(base.path()).unwrap();
    a.autosave(7, "a", None, None, &doc(4, 4), &NoMonitor).unwrap();
    let now = SystemTime::now();
    a.write_heartbeat(now - Duration::from_secs(600)).unwrap();
    let entry = Recovery::orphans(base.path(), now, None).pop().unwrap();
    Recovery::discard(&entry);
    assert!(!entry.ora_file().exists());
    assert!(Recovery::orphans(base.path(), now, None).is_empty());
}

#[test]
fn remove_tras_guardar() {
    let base = tempfile::tempdir().unwrap();
    let a = Recovery::start(base.path()).unwrap();
    a.autosave(3, "a", None, None, &doc(4, 4), &NoMonitor).unwrap();
    assert!(a.session_dir().join("3.ora").exists());
    a.remove(3);
    assert!(!a.session_dir().join("3.ora").exists());
    assert!(!a.session_dir().join("3.toml").exists());
}
