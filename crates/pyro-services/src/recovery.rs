//! Autoguardado y recuperación tras un cierre inesperado.
//!
//! Cada sesión (cada ventana de Pyrotecnika) escribe en su propia carpeta
//! `recovery/<inicio-ms>-<pid>/`: un `session.toml` con un latido que se
//! renueva cada 30 s, y por cada pestaña con cambios un `<uid>.ora` y un
//! `<uid>.toml` con su nombre y rutas. Al cerrar bien se borra la carpeta.
//! Una sesión cuyo latido tiene más de 3 min no está viva: sus documentos
//! se ofrecen para recuperar.

use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use pyro_core::document::Document;
use pyro_core::task::Monitor;
use pyro_formats::ora::write_ora;

/// Latido de más de esto: la sesión ya no está viva.
pub const STALE_AFTER: Duration = Duration::from_secs(180);

/// Un documento autoguardado de una sesión que no cerró bien.
#[derive(Clone, Debug, PartialEq)]
pub struct RecoveryEntry {
    pub session: PathBuf,
    pub uid: u64,
    pub title: String,
    pub source_path: Option<PathBuf>,
    pub ora_path: Option<PathBuf>,
    pub saved_at: SystemTime,
}

impl RecoveryEntry {
    /// El `.ora` autoguardado.
    pub fn ora_file(&self) -> PathBuf {
        self.session.join(format!("{}.ora", self.uid))
    }

    fn meta_file(&self) -> PathBuf {
        self.session.join(format!("{}.toml", self.uid))
    }
}

#[derive(Serialize, Deserialize)]
struct SessionFile {
    pid: u32,
    started: u64,
    heartbeat: u64,
}

#[derive(Serialize, Deserialize)]
struct EntryFile {
    title: String,
    source_path: Option<PathBuf>,
    ora_path: Option<PathBuf>,
    saved_at: u64,
}

fn secs(t: SystemTime) -> u64 {
    t.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn from_secs(s: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(s)
}

/// Cada cuánto se renueva el latido.
pub const HEARTBEAT_EVERY: Duration = Duration::from_secs(30);

/// Carpeta de autoguardado de esta sesión. Barata de clonar (rutas y la
/// señal de parada del latido), para pasarla a la tarea que autoguarda.
#[derive(Clone, Debug)]
pub struct Recovery {
    dir: PathBuf,
    started: u64,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl Recovery {
    /// Crea la carpeta de la sesión dentro de `base/recovery` y arranca el
    /// latido en su propio hilo (sigue aunque la ventana no se dibuje:
    /// minimizada o con un diálogo del SO abierto).
    pub fn start(base: &Path) -> io::Result<Recovery> {
        Self::start_with_interval(base, HEARTBEAT_EVERY)
    }

    /// Como [`Recovery::start`], con otro intervalo de latido (pruebas).
    pub fn start_with_interval(base: &Path, every: Duration) -> io::Result<Recovery> {
        let now = SystemTime::now();
        let ms = now.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis());
        // El contador distingue dos sesiones del mismo proceso en el mismo
        // milisegundo (pruebas).
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = base.join("recovery").join(format!("{ms}-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir)?;
        let r = Recovery { dir, started: secs(now), stop: Default::default() };
        r.write_heartbeat(now)?;
        let beat = r.clone();
        std::thread::spawn(move || loop {
            std::thread::sleep(every);
            if beat.stop.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }
            let _ = beat.heartbeat();
        });
        Ok(r)
    }

    pub fn session_dir(&self) -> &Path {
        &self.dir
    }

    /// Renueva el latido (la sesión sigue viva).
    pub fn heartbeat(&self) -> io::Result<()> {
        self.write_heartbeat(SystemTime::now())
    }

    /// Escribe el latido con la hora `at` (las pruebas simulan sesiones viejas).
    pub fn write_heartbeat(&self, at: SystemTime) -> io::Result<()> {
        let file = SessionFile { pid: std::process::id(), started: self.started, heartbeat: secs(at) };
        let text = toml::to_string(&file).map_err(io::Error::other)?;
        std::fs::write(self.dir.join("session.toml"), text)
    }

    /// Guarda la pestaña `uid` (escritura atómica del `.ora`, luego sus datos).
    pub fn autosave(
        &self,
        uid: u64,
        title: &str,
        source: Option<&PathBuf>,
        ora: Option<&PathBuf>,
        doc: &Document,
        m: &dyn Monitor,
    ) -> Result<()> {
        // Si la carpeta desapareció (p. ej. otra instancia la tomó por muerta),
        // se recrea.
        std::fs::create_dir_all(&self.dir).context("No se pudo crear la carpeta de autoguardado")?;
        let _ = self.heartbeat();
        write_ora(doc, &self.dir.join(format!("{uid}.ora")), m)?;
        let meta = EntryFile {
            title: title.to_string(),
            source_path: source.cloned(),
            ora_path: ora.cloned(),
            saved_at: secs(SystemTime::now()),
        };
        std::fs::write(self.dir.join(format!("{uid}.toml")), toml::to_string(&meta)?)
            .context("No se pudieron guardar los datos del autoguardado")
    }

    /// Borra el autoguardado de la pestaña `uid` (se guardó o se cerró).
    pub fn remove(&self, uid: u64) {
        let _ = std::fs::remove_file(self.dir.join(format!("{uid}.ora")));
        let _ = std::fs::remove_file(self.dir.join(format!("{uid}.toml")));
    }

    /// Cierre normal: no queda nada que recuperar.
    pub fn finish(self) {
        self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
        let _ = std::fs::remove_dir_all(&self.dir);
    }

    /// Documentos de sesiones que no están vivas (latido de hace más de
    /// [`STALE_AFTER`]), sin contar `own`. Las sesiones muertas sin
    /// documentos se borran.
    pub fn orphans(base: &Path, now: SystemTime, own: Option<&Path>) -> Vec<RecoveryEntry> {
        let Ok(sessions) = std::fs::read_dir(base.join("recovery")) else { return Vec::new() };
        let mut found = Vec::new();
        for session in sessions.flatten().map(|e| e.path()).filter(|p| p.is_dir()) {
            if own.is_some_and(|o| o == session) {
                continue;
            }
            let beat = std::fs::read_to_string(session.join("session.toml"))
                .ok()
                .and_then(|t| toml::from_str::<SessionFile>(&t).ok())
                .map_or(UNIX_EPOCH, |s| from_secs(s.heartbeat));
            if now.duration_since(beat).unwrap_or(Duration::ZERO) <= STALE_AFTER {
                continue;
            }
            let before = found.len();
            for file in std::fs::read_dir(&session).into_iter().flatten().flatten().map(|e| e.path()) {
                let Some(uid) = file.file_stem().and_then(|s| s.to_str()).and_then(|s| s.parse::<u64>().ok()) else {
                    continue;
                };
                if file.extension().is_none_or(|e| e != "toml") || !session.join(format!("{uid}.ora")).exists() {
                    continue;
                }
                let Some(meta) = std::fs::read_to_string(&file).ok().and_then(|t| toml::from_str::<EntryFile>(&t).ok())
                else {
                    continue;
                };
                found.push(RecoveryEntry {
                    session: session.clone(),
                    uid,
                    title: meta.title,
                    source_path: meta.source_path,
                    ora_path: meta.ora_path,
                    saved_at: from_secs(meta.saved_at),
                });
            }
            if found.len() == before {
                let _ = std::fs::remove_dir_all(&session);
            }
        }
        found
    }

    /// Descarta un documento recuperable (y su sesión si queda vacía).
    pub fn discard(entry: &RecoveryEntry) {
        let _ = std::fs::remove_file(entry.ora_file());
        let _ = std::fs::remove_file(entry.meta_file());
        let empty = std::fs::read_dir(&entry.session)
            .map(|d| d.flatten().all(|e| e.file_name() == "session.toml"))
            .unwrap_or(false);
        if empty {
            let _ = std::fs::remove_dir_all(&entry.session);
        }
    }
}
