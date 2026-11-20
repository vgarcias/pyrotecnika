//! Tareas en segundo plano: corren en su propio hilo, informan del avance,
//! se pueden cancelar y avisan a la interfaz al cambiar (`Wake`).

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::sync::Arc;
use std::time::Instant;

use pyro_core::task::{Cancelled, Monitor};

/// Cómo despertar a quien espera la tarea (la interfaz pasa
/// `ctx.request_repaint()`).
pub type Wake = Arc<dyn Fn() + Send + Sync>;

/// Por qué una tarea no dio resultado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobError {
    Cancelled,
    Failed(String),
}

impl From<Cancelled> for JobError {
    fn from(_: Cancelled) -> Self {
        JobError::Cancelled
    }
}

/// Lo que ve la interfaz en cada cuadro.
#[derive(Debug)]
pub enum JobPoll<T> {
    Running,
    Done(T),
    Failed(String),
    Cancelled,
}

/// Sin porcentaje todavía.
const NO_PROGRESS: u32 = u32::MAX;
/// Como mucho un aviso de progreso cada tanto (ms).
const WAKE_EVERY_MS: u64 = 50;

struct Shared {
    cancelled: AtomicBool,
    progress: AtomicU32,
    start: Instant,
    /// Milisegundos desde `start` del último aviso de progreso.
    last_wake: AtomicU64,
}

/// Lo que el trabajo recibe: cancelación y progreso.
pub struct JobCtx {
    shared: Arc<Shared>,
    wake: Wake,
}

impl JobCtx {
    /// `Err(Cancelled)` si se pidió cancelar: para usar con `?`.
    pub fn check(&self) -> Result<(), JobError> {
        if self.cancelled() {
            Err(JobError::Cancelled)
        } else {
            Ok(())
        }
    }
}

impl Monitor for JobCtx {
    fn cancelled(&self) -> bool {
        self.shared.cancelled.load(Ordering::Relaxed)
    }

    fn progress(&self, fraction: f32) {
        self.shared.progress.store(fraction.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
        let now = self.shared.start.elapsed().as_millis() as u64;
        let last = self.shared.last_wake.load(Ordering::Relaxed);
        if now.saturating_sub(last) >= WAKE_EVERY_MS
            && self.shared.last_wake.compare_exchange(last, now, Ordering::Relaxed, Ordering::Relaxed).is_ok()
        {
            (self.wake)();
        }
    }
}

/// Avisa al terminar el hilo, también si el trabajo hace `panic`.
struct WakeOnExit(Wake);

impl Drop for WakeOnExit {
    fn drop(&mut self) {
        (self.0)();
    }
}

/// Una tarea en marcha. Al soltarla se cancela.
pub struct Job<T> {
    shared: Arc<Shared>,
    rx: Receiver<Result<T, JobError>>,
    delivered: bool,
}

impl<T: Send + 'static> Job<T> {
    pub fn spawn(wake: Wake, work: impl FnOnce(&JobCtx) -> Result<T, JobError> + Send + 'static) -> Job<T> {
        let shared = Arc::new(Shared {
            cancelled: AtomicBool::new(false),
            progress: AtomicU32::new(NO_PROGRESS),
            start: Instant::now(),
            last_wake: AtomicU64::new(0),
        });
        let (tx, rx) = channel();
        let ctx = JobCtx { shared: Arc::clone(&shared), wake: Arc::clone(&wake) };
        std::thread::spawn(move || {
            let _wake = WakeOnExit(wake);
            let _ = tx.send(work(&ctx));
        });
        Job { shared, rx, delivered: false }
    }
}

impl<T> Job<T> {
    /// Fracción hecha, o `None` si la tarea aún no la ha dicho.
    pub fn progress(&self) -> Option<f32> {
        let bits = self.shared.progress.load(Ordering::Relaxed);
        (bits != NO_PROGRESS).then(|| f32::from_bits(bits))
    }

    pub fn cancel(&self) {
        self.shared.cancelled.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.shared.cancelled.load(Ordering::Relaxed)
    }

    /// Estado de la tarea. Si se pidió cancelar, nunca da `Done` aunque el
    /// trabajo ya hubiera terminado. Tras entregar un resultado, `Cancelled`.
    pub fn poll(&mut self) -> JobPoll<T> {
        if self.delivered {
            return JobPoll::Cancelled;
        }
        let result = match self.rx.try_recv() {
            Ok(r) => r,
            Err(TryRecvError::Empty) => return JobPoll::Running,
            Err(TryRecvError::Disconnected) => Err(JobError::Failed("La tarea terminó de forma inesperada.".into())),
        };
        self.delivered = true;
        if self.is_cancelled() {
            return JobPoll::Cancelled;
        }
        match result {
            Ok(v) => JobPoll::Done(v),
            Err(JobError::Cancelled) => JobPoll::Cancelled,
            Err(JobError::Failed(e)) => JobPoll::Failed(e),
        }
    }
}

impl<T> Drop for Job<T> {
    fn drop(&mut self) {
        self.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    fn counter() -> (Arc<AtomicUsize>, Wake) {
        let n = Arc::new(AtomicUsize::new(0));
        let c = Arc::clone(&n);
        (
            n,
            Arc::new(move || {
                c.fetch_add(1, Ordering::SeqCst);
            }),
        )
    }

    fn wait<T>(job: &mut Job<T>) -> JobPoll<T> {
        let start = Instant::now();
        loop {
            match job.poll() {
                JobPoll::Running if start.elapsed() < Duration::from_secs(2) => {
                    std::thread::sleep(Duration::from_millis(1))
                }
                other => return other,
            }
        }
    }

    #[test]
    fn termina_y_avisa() {
        let (n, wake) = counter();
        let mut job = Job::spawn(wake, |_| Ok(42));
        assert!(matches!(wait(&mut job), JobPoll::Done(42)));
        // El aviso llega al salir el hilo, justo después del resultado.
        let start = Instant::now();
        while n.load(Ordering::SeqCst) == 0 && start.elapsed() < Duration::from_secs(2) {
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(n.load(Ordering::SeqCst) >= 1);
    }

    #[test]
    fn informa_progreso() {
        let (_, wake) = counter();
        let (tx, rx) = mpsc::channel::<()>();
        let job = Job::spawn(wake, move |ctx| {
            ctx.progress(0.5);
            let _ = rx.recv();
            Ok(())
        });
        let start = Instant::now();
        while job.progress().is_none() && start.elapsed() < Duration::from_secs(2) {
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(job.progress(), Some(0.5));
        tx.send(()).unwrap();
    }

    #[test]
    fn cancelar_responde() {
        let (_, wake) = counter();
        let mut job: Job<()> = Job::spawn(wake, |ctx| loop {
            ctx.check()?;
            std::thread::sleep(Duration::from_millis(1));
        });
        std::thread::sleep(Duration::from_millis(10));
        let start = Instant::now();
        job.cancel();
        assert!(job.is_cancelled());
        assert!(matches!(wait(&mut job), JobPoll::Cancelled));
        assert!(start.elapsed() < Duration::from_millis(100), "{:?}", start.elapsed());
    }

    #[test]
    fn cancelar_gana_al_resultado() {
        let (n, wake) = counter();
        let mut job = Job::spawn(wake, |_| Ok(1));
        let start = Instant::now();
        while n.load(Ordering::SeqCst) == 0 && start.elapsed() < Duration::from_secs(2) {
            std::thread::sleep(Duration::from_millis(1));
        }
        job.cancel();
        assert!(matches!(job.poll(), JobPoll::Cancelled));
    }

    #[test]
    fn error() {
        let (_, wake) = counter();
        let mut job: Job<()> = Job::spawn(wake, |_| Err(JobError::Failed("x".into())));
        assert!(matches!(wait(&mut job), JobPoll::Failed(e) if e == "x"));
    }

    #[test]
    fn un_panic_es_un_error() {
        let (_, wake) = counter();
        let mut job: Job<()> = Job::spawn(wake, |_| panic!("roto"));
        assert!(matches!(wait(&mut job), JobPoll::Failed(_)));
    }
}
