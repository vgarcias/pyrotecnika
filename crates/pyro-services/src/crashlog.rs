//! Registro de fallos: un panic hook que apunta la hora, el mensaje, la
//! ubicación y la traza en `logs/pyrotecnika-AAAA-MM-DD.log` (uno por día,
//! se conservan 7 días) y deja que la app termine como siempre. No intenta
//! guardar documentos: el estado puede estar roto; para eso está el
//! autoguardado. Las rutas de la carpeta del usuario se escriben como `~`.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Días conservados.
pub const KEEP_DAYS: u64 = 7;

/// Año, mes y día (UTC) de un número de días desde 1970-01-01.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    // Algoritmo de Howard Hinnant (días civiles), sin dependencias.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

fn secs(t: SystemTime) -> i64 {
    t.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

/// Nombre del log del día de `at`: `pyrotecnika-AAAA-MM-DD.log`.
pub fn log_name(at: SystemTime) -> String {
    let (y, m, d) = civil_from_days(secs(at).div_euclid(86_400));
    format!("pyrotecnika-{y:04}-{m:02}-{d:02}.log")
}

/// Texto de una entrada del log. Cada aparición de `home` se cambia por `~`.
pub fn panic_entry(message: &str, location: &str, backtrace: &str, home: Option<&Path>, at: SystemTime) -> String {
    let s = secs(at);
    let (y, m, d) = civil_from_days(s.div_euclid(86_400));
    let t = s.rem_euclid(86_400);
    let mut text = format!(
        "[{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02} UTC] Pyrotecnika {}: fallo inesperado\n  mensaje: {message}\n  en: {location}\n",
        t / 3600,
        t / 60 % 60,
        t % 60,
        env!("CARGO_PKG_VERSION"),
    );
    if !backtrace.trim().is_empty() {
        text.push_str("  traza:\n");
        for line in backtrace.lines() {
            text.push_str("    ");
            text.push_str(line);
            text.push('\n');
        }
    }
    text.push('\n');
    match home.map(|h| h.to_string_lossy().into_owned()).filter(|h| h.len() > 1) {
        Some(h) => text.replace(&h, "~"),
        None => text,
    }
}

/// Borra los logs de más de `keep_days` días (solo los suyos).
pub fn prune(dir: &Path, now: SystemTime, keep_days: u64) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let keep: Vec<String> =
        (0..keep_days).map(|n| log_name(now - std::time::Duration::from_secs(n * 86_400))).collect();
    for path in entries.flatten().map(|e| e.path()) {
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else { continue };
        if name.starts_with("pyrotecnika-") && name.ends_with(".log") && !keep.iter().any(|k| k == name) {
            let _ = std::fs::remove_file(&path);
        }
    }
}

/// Carpeta del usuario, para no escribirla en el log.
fn home_dir() -> Option<PathBuf> {
    ["USERPROFILE", "HOME"].iter().find_map(|v| std::env::var_os(v).filter(|s| !s.is_empty())).map(PathBuf::from)
}

/// Instala el hook: escribe la entrada y luego llama al hook anterior.
pub fn install_panic_hook(dir: PathBuf) {
    let _ = std::fs::create_dir_all(&dir);
    prune(&dir, SystemTime::now(), KEEP_DAYS);
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let message = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "(sin mensaje)".into());
        let location = info.location().map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column())).unwrap_or_default();
        let backtrace = std::backtrace::Backtrace::force_capture().to_string();
        let now = SystemTime::now();
        let entry = panic_entry(&message, &location, &backtrace, home_dir().as_deref(), now);
        if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(dir.join(log_name(now))) {
            let _ = file.write_all(entry.as_bytes());
        }
        previous(info);
    }));
}
