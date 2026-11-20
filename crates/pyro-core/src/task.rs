//! Lo que un trabajo largo del núcleo necesita de quien lo lanza: saber si
//! se canceló e informar de su avance. Sin hilos ni interfaz.

/// El trabajo se detuvo porque se pidió cancelarlo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cancelled;

impl std::fmt::Display for Cancelled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Cancelado")
    }
}

impl std::error::Error for Cancelled {}

/// Vigila un trabajo largo. Se consulta desde varios hilos a la vez.
pub trait Monitor: Sync {
    /// Se pidió cancelar: el trabajo debe parar en cuanto pueda.
    fn cancelled(&self) -> bool;
    /// Fracción hecha, de 0 a 1.
    fn progress(&self, fraction: f32);
}

/// Monitor que nunca cancela ni escucha el progreso.
pub struct NoMonitor;

impl Monitor for NoMonitor {
    fn cancelled(&self) -> bool {
        false
    }

    fn progress(&self, _fraction: f32) {}
}
