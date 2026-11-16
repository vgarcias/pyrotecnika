//! Archivos que el sistema pide abrir: argumentos de línea de comandos en
//! Windows y Linux, y el evento Apple «abrir documentos» en macOS (doble
//! clic en el Finder, «Abrir con», soltar sobre el icono del Dock).
//!
//! Las rutas se acumulan en una cola global y la interfaz las recoge en cada
//! cuadro con [`take`]. Así un archivo que llega antes de que exista la
//! ventana (al arrancar) no se pierde.

use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

type Waker = Box<dyn Fn() + Send + Sync>;

static QUEUE: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());
static WAKER: OnceLock<Waker> = OnceLock::new();

/// Prepara la recepción de archivos. Debe llamarse en el hilo principal
/// antes de crear el bucle de eventos.
pub fn install() {
    queue(std::env::args_os().skip(1).map(PathBuf::from));
    #[cfg(target_os = "macos")]
    crate::os::macos::install_open_documents_handler();
}

/// Función que despierta la interfaz cuando llega un archivo (por ejemplo,
/// `ctx.request_repaint()`). Solo se tiene en cuenta la primera llamada.
pub fn set_waker(waker: impl Fn() + Send + Sync + 'static) {
    let _ = WAKER.set(Box::new(waker));
}

/// Añade rutas a la cola y despierta la interfaz.
pub fn queue(paths: impl IntoIterator<Item = PathBuf>) {
    let mut q = QUEUE.lock().unwrap_or_else(|e| e.into_inner());
    let before = q.len();
    q.extend(paths);
    let added = q.len() > before;
    drop(q);
    if added {
        if let Some(wake) = WAKER.get() {
            wake();
        }
    }
}

/// Saca todas las rutas pendientes.
pub fn take() -> Vec<PathBuf> {
    std::mem::take(&mut *QUEUE.lock().unwrap_or_else(|e| e.into_inner()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn queue_take_and_wake() {
        static WOKEN: AtomicUsize = AtomicUsize::new(0);
        set_waker(|| {
            WOKEN.fetch_add(1, Ordering::SeqCst);
        });
        queue([PathBuf::from("a.png"), PathBuf::from("b.ora")]);
        queue(std::iter::empty());
        assert_eq!(take(), [PathBuf::from("a.png"), PathBuf::from("b.ora")]);
        assert!(take().is_empty());
        assert_eq!(WOKEN.load(Ordering::SeqCst), 1);
    }
}
