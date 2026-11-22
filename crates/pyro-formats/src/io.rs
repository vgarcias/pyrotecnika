//! Apertura de archivos de imagen.

use std::io::Cursor;
use std::path::Path;

use anyhow::{Context, Result};
use image::{ImageReader, Limits};

use crate::fwpng::{self, FwInfo, PNG_SIGNATURE};
use pyro_core::document::Document;
use pyro_core::task::{Cancelled, Monitor};

/// Lado máximo de una imagen (RNF-S1).
pub const MAX_SIDE: u32 = 32_768;
/// Memoria máxima de una capa RGBA de 8 bits, y de lo que reserve el
/// decodificador.
pub const MAX_LAYER_BYTES: u64 = 2 << 30;

/// La imagen pasa de los límites: no se decodifica.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TooLarge {
    pub width: u32,
    pub height: u32,
}

impl std::fmt::Display for TooLarge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "La imagen es demasiado grande ({}×{} px). Pyrotecnika abre imágenes de hasta 32 768 px por lado y 2 GB \
             por capa.",
            self.width, self.height
        )
    }
}

impl std::error::Error for TooLarge {}

/// Comprueba que una imagen de `width`×`height` cabe en los límites (abrir y
/// «Nuevo»).
pub fn check_size(width: u32, height: u32) -> Result<(), TooLarge> {
    let bytes = u64::from(width) * u64::from(height) * 4;
    if width > MAX_SIDE || height > MAX_SIDE || bytes > MAX_LAYER_BYTES {
        Err(TooLarge { width, height })
    } else {
        Ok(())
    }
}

pub struct Opened {
    pub document: Document,
    /// Presente si el archivo es PNG (con o sin bloques de Fireworks).
    pub png_info: Option<FwInfo>,
    /// Avisos de lo que se abrió aproximado (`.ora` de otros programas).
    pub warnings: Vec<String>,
    /// El archivo es OpenRaster: Guardar escribe en él.
    pub is_ora: bool,
}

pub const OPEN_EXTENSIONS: [&str; 9] = ["ora", "png", "jpg", "jpeg", "gif", "webp", "bmp", "tif", "tiff"];

pub fn open_path(path: &Path, m: &dyn Monitor) -> Result<Opened> {
    let bytes = std::fs::read(path).with_context(|| format!("No se pudo leer {}", path.display()))?;
    open_bytes(&bytes, m)
}

/// Decodifica una imagen comprobando antes sus medidas: una cabecera que
/// pide demasiado se rechaza con [`TooLarge`] sin reservar memoria.
pub fn open_bytes(bytes: &[u8], m: &dyn Monitor) -> Result<Opened> {
    if m.cancelled() {
        return Err(Cancelled.into());
    }
    if crate::ora::is_ora(bytes) {
        let ora = crate::ora::decode_ora(bytes, m)?;
        return Ok(Opened { document: ora.document, png_info: None, warnings: ora.warnings, is_ora: true });
    }
    let png_info = bytes.starts_with(&PNG_SIGNATURE).then(|| fwpng::inspect_png(bytes).ok()).flatten();
    let img = decode_limited(bytes)?;
    if m.cancelled() {
        return Err(Cancelled.into());
    }
    let name = if png_info.as_ref().is_some_and(FwInfo::is_fireworks) { "Fireworks (aplanado)" } else { "Fondo" };
    Ok(Opened { document: Document::from_image(img, name), png_info, warnings: Vec::new(), is_ora: false })
}

/// Decodifica una imagen a RGBA 8 bits con los límites de RNF-S1: medidas
/// comprobadas en la cabecera antes de reservar memoria.
pub(crate) fn decode_limited(bytes: &[u8]) -> Result<image::RgbaImage> {
    const UNKNOWN: &str = "Formato de imagen no reconocido o archivo dañado";
    let reader = || ImageReader::new(Cursor::new(bytes)).with_guessed_format().context(UNKNOWN);
    let (width, height) = reader()?.into_dimensions().context(UNKNOWN)?;
    check_size(width, height)?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_SIDE);
    limits.max_image_height = Some(MAX_SIDE);
    limits.max_alloc = Some(MAX_LAYER_BYTES);
    let mut decoder = reader()?;
    decoder.limits(limits);
    Ok(decoder.decode().context(UNKNOWN)?.into_rgba8())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pyro_core::task::NoMonitor;

    #[test]
    fn opens_fireworks_png_as_flattened_layer() {
        let opened = open_bytes(&crate::fwpng::tests::fake_fireworks_png(), &NoMonitor).unwrap();
        assert!(opened.png_info.unwrap().is_fireworks());
        assert_eq!((opened.document.width, opened.document.height), (3, 2));
        assert_eq!(opened.document.layers[0].name, "Fireworks (aplanado)");
        assert_eq!(opened.document.layers[0].pixels.get_pixel(0, 0).0, [10, 20, 30, 255]);
    }

    #[test]
    fn rejects_garbage() {
        assert!(open_bytes(b"hola", &NoMonitor).is_err());
    }
}
