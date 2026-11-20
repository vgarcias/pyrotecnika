//! Lectura de PNG de Fireworks.
//!
//! Un archivo de Fireworks es un PNG normal cuya imagen (IDAT) es la versión
//! aplanada. Las capas, vectores y efectos editables viajan en bloques privados
//! (mkBF, mkBS, mkBT, mkTS, prVW) con un formato propietario sin documentar.
//! Aquí solo los identificamos; la imagen se abre aplanada.

use anyhow::{bail, Result};

pub const PNG_SIGNATURE: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];
const FIREWORKS_CHUNKS: [&str; 5] = ["mkBF", "mkBS", "mkBT", "mkTS", "prVW"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PngChunk {
    pub kind: String,
    pub length: u32,
    pub offset: usize,
    /// El CRC guardado coincide con el contenido.
    pub crc_ok: bool,
}

impl PngChunk {
    /// Bloque privado según la especificación PNG (bit 5 del segundo byte).
    pub fn is_private(&self) -> bool {
        self.kind.as_bytes().get(1).is_some_and(|b| b & 0x20 != 0)
    }

    pub fn is_fireworks(&self) -> bool {
        FIREWORKS_CHUNKS.contains(&self.kind.as_str())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FwInfo {
    pub chunks: Vec<PngChunk>,
}

impl FwInfo {
    pub fn is_fireworks(&self) -> bool {
        self.chunks.iter().any(PngChunk::is_fireworks)
    }

    pub fn private_chunks(&self) -> impl Iterator<Item = &PngChunk> {
        self.chunks.iter().filter(|c| c.is_private())
    }

    pub fn private_bytes(&self) -> u64 {
        self.private_chunks().map(|c| c.length as u64).sum()
    }

    /// Algún bloque privado (los datos de Fireworks) tiene el CRC mal.
    pub fn damaged(&self) -> bool {
        self.private_chunks().any(|c| !c.crc_ok)
    }
}

pub fn inspect_png(bytes: &[u8]) -> Result<FwInfo> {
    if !bytes.starts_with(&PNG_SIGNATURE) {
        bail!("No es un archivo PNG");
    }
    let mut pos = PNG_SIGNATURE.len();
    let mut chunks = Vec::new();
    while pos + 8 <= bytes.len() {
        let length = u32::from_be_bytes([bytes[pos], bytes[pos + 1], bytes[pos + 2], bytes[pos + 3]]);
        let kind = &bytes[pos + 4..pos + 8];
        if !kind.iter().all(u8::is_ascii_alphabetic) {
            bail!("Bloque PNG no válido en el byte {pos}");
        }
        let kind = String::from_utf8_lossy(kind).into_owned();
        let end = pos.checked_add(12).and_then(|v| v.checked_add(length as usize)).filter(|&e| e <= bytes.len());
        let Some(end) = end else { bail!("Bloque {kind} truncado en el byte {pos}") };
        let is_end = kind == "IEND";
        let stored = u32::from_be_bytes([bytes[end - 4], bytes[end - 3], bytes[end - 2], bytes[end - 1]]);
        let crc_ok = crc32fast::hash(&bytes[pos + 4..end - 4]) == stored;
        chunks.push(PngChunk { kind, length, offset: pos, crc_ok });
        pos = end;
        if is_end {
            break;
        }
    }
    Ok(FwInfo { chunks })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use image::{ImageEncoder, Rgba, RgbaImage};

    /// PNG pequeño con un bloque mkBF insertado antes de IEND.
    pub(crate) fn fake_fireworks_png() -> Vec<u8> {
        let img = RgbaImage::from_pixel(3, 2, Rgba([10, 20, 30, 255]));
        let mut png = Vec::new();
        image::codecs::png::PngEncoder::new(&mut png)
            .write_image(img.as_raw(), 3, 2, image::ExtendedColorType::Rgba8)
            .unwrap();
        let iend = png.len() - 12;
        let data = b"datos privados";
        let mut chunk = Vec::new();
        chunk.extend_from_slice(&(data.len() as u32).to_be_bytes());
        chunk.extend_from_slice(b"mkBF");
        chunk.extend_from_slice(data);
        let crc = crc32fast::hash(&chunk[4..]);
        chunk.extend_from_slice(&crc.to_be_bytes());
        png.splice(iend..iend, chunk);
        png
    }

    #[test]
    fn detects_fireworks_chunks() {
        let info = inspect_png(&fake_fireworks_png()).unwrap();
        assert!(info.is_fireworks());
        assert_eq!(info.chunks.first().map(|c| c.kind.as_str()), Some("IHDR"));
        assert_eq!(info.chunks.last().map(|c| c.kind.as_str()), Some("IEND"));
        assert_eq!(info.private_chunks().count(), 1);
        assert_eq!(info.private_bytes(), 14);
    }

    #[test]
    fn plain_png_is_not_fireworks_and_garbage_fails() {
        let mut png = fake_fireworks_png();
        let info = inspect_png(&png).unwrap();
        let idx = info.chunks.iter().position(|c| c.kind == "mkBF").unwrap();
        let c = &info.chunks[idx];
        png.drain(c.offset..c.offset + 12 + c.length as usize);
        assert!(!inspect_png(&png).unwrap().is_fireworks());
        assert!(inspect_png(b"no es png").is_err());
        assert!(inspect_png(&png[..20]).is_err());
    }
}
