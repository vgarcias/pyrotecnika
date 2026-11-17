//! Comandos de la app: combinaciones de teclas, catálogo, mapa de atajos y
//! almacenamiento de preferencias (fase 2).

pub(crate) mod chord;

/// ¿Se ejecuta en macOS? (Ctrl se muestra como ⌘).
pub(crate) fn is_mac() -> bool {
    pyro_platform::IS_MAC
}
