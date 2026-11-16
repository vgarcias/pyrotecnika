//! Apertura de archivos de imagen.

use std::path::Path;

use anyhow::{Context, Result};

use crate::fwpng::{self, FwInfo, PNG_SIGNATURE};
use pyro_core::document::Document;

pub struct Opened {
    pub document: Document,
    /// Presente si el archivo es PNG (con o sin bloques de Fireworks).
    pub png_info: Option<FwInfo>,
}

pub const OPEN_EXTENSIONS: [&str; 8] = ["png", "jpg", "jpeg", "gif", "webp", "bmp", "tif", "tiff"];

pub fn open_path(path: &Path) -> Result<Opened> {
    let bytes = std::fs::read(path).with_context(|| format!("No se pudo leer {}", path.display()))?;
    open_bytes(&bytes)
}

pub fn open_bytes(bytes: &[u8]) -> Result<Opened> {
    let png_info = bytes.starts_with(&PNG_SIGNATURE).then(|| fwpng::inspect_png(bytes).ok()).flatten();
    let img = image::load_from_memory(bytes).context("Formato de imagen no reconocido o archivo dañado")?.to_rgba8();
    let name = if png_info.as_ref().is_some_and(FwInfo::is_fireworks) { "Fireworks (aplanado)" } else { "Fondo" };
    Ok(Opened { document: Document::from_image(img, name), png_info })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opens_fireworks_png_as_flattened_layer() {
        let opened = open_bytes(&crate::fwpng::tests::fake_fireworks_png()).unwrap();
        assert!(opened.png_info.unwrap().is_fireworks());
        assert_eq!((opened.document.width, opened.document.height), (3, 2));
        assert_eq!(opened.document.layers[0].name, "Fireworks (aplanado)");
        assert_eq!(opened.document.layers[0].pixels.get_pixel(0, 0).0, [10, 20, 30, 255]);
    }

    #[test]
    fn rejects_garbage() {
        assert!(open_bytes(b"hola").is_err());
    }
}
