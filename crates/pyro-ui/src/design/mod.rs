//! Sistema de diseño de Pyrotecnika (docs/ARQUITECTURA.md §4).
//!
//! Es el único sitio del código con colores literales: el resto de la
//! interfaz lee los tokens de aquí. La paleta es neutra (grises, blanco y
//! negro) y lo activo se marca por inversión.

pub(crate) mod fonts;
pub(crate) mod style;
pub(crate) mod tokens;
pub(crate) mod widgets;
