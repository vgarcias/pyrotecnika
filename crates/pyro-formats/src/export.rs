//! Exportación optimizada a PNG-24, PNG-8, JPEG, GIF y WebP.

use std::io::Cursor;

use anyhow::{anyhow, Context, Result};
use image::codecs::gif::GifEncoder;
use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::{Delay, ExtendedColorType, Frame, ImageEncoder, RgbaImage};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ExportFormat {
    Png24,
    /// PNG con paleta (2–256 colores) y transparencia por color.
    Png8 {
        colors: u16,
    },
    /// Calidad 1–100. El fondo transparente se rellena de blanco.
    Jpeg {
        quality: u8,
    },
    Gif,
    /// Calidad 0–100 (ignorada si `lossless`).
    WebP {
        lossless: bool,
        quality: f32,
    },
}

impl ExportFormat {
    pub fn extension(&self) -> &'static str {
        match self {
            ExportFormat::Png24 | ExportFormat::Png8 { .. } => "png",
            ExportFormat::Jpeg { .. } => "jpg",
            ExportFormat::Gif => "gif",
            ExportFormat::WebP { .. } => "webp",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            ExportFormat::Png24 => "PNG-24",
            ExportFormat::Png8 { .. } => "PNG-8",
            ExportFormat::Jpeg { .. } => "JPEG",
            ExportFormat::Gif => "GIF",
            ExportFormat::WebP { .. } => "WebP",
        }
    }
}

pub fn encode(img: &RgbaImage, format: ExportFormat) -> Result<Vec<u8>> {
    let (w, h) = img.dimensions();
    let mut buf = Vec::new();
    match format {
        ExportFormat::Png24 => {
            let enc = PngEncoder::new_with_quality(&mut buf, CompressionType::Best, FilterType::Adaptive);
            if img.pixels().all(|p| p.0[3] == 255) {
                let rgb = flatten(img, [255, 255, 255]);
                enc.write_image(&rgb, w, h, ExtendedColorType::Rgb8)?;
            } else {
                enc.write_image(img.as_raw(), w, h, ExtendedColorType::Rgba8)?;
            }
        }
        ExportFormat::Png8 { colors } => encode_png8(img, colors.clamp(2, 256), &mut buf)?,
        ExportFormat::Jpeg { quality } => {
            let rgb = flatten(img, [255, 255, 255]);
            JpegEncoder::new_with_quality(&mut buf, quality.clamp(1, 100)).write_image(
                &rgb,
                w,
                h,
                ExtendedColorType::Rgb8,
            )?;
        }
        ExportFormat::Gif => {
            let mut enc = GifEncoder::new_with_speed(&mut buf, 10);
            enc.encode_frame(Frame::from_parts(img.clone(), 0, 0, Delay::from_numer_denom_ms(0, 1)))?;
        }
        ExportFormat::WebP { lossless, quality } => {
            let enc = webp::Encoder::from_rgba(img.as_raw(), w, h);
            let mem = enc
                .encode_simple(lossless, quality.clamp(0.0, 100.0))
                .map_err(|e| anyhow!("Error al codificar WebP: {e:?}"))?;
            buf.extend_from_slice(&mem);
        }
    }
    Ok(buf)
}

/// Compone la imagen sobre un color de fondo y devuelve RGB.
pub fn flatten(img: &RgbaImage, bg: [u8; 3]) -> Vec<u8> {
    let mut out = Vec::with_capacity((img.width() * img.height() * 3) as usize);
    for p in img.pixels() {
        let a = p.0[3] as u32;
        for (src, back) in p.0.iter().zip(bg) {
            out.push(((*src as u32 * a + back as u32 * (255 - a) + 127) / 255) as u8);
        }
    }
    out
}

fn encode_png8(img: &RgbaImage, colors: u16, buf: &mut Vec<u8>) -> Result<()> {
    let pixels = img.as_raw();
    let quant = color_quant::NeuQuant::new(10, colors as usize, pixels);
    let map = quant.color_map_rgba();
    let indices: Vec<u8> = pixels.chunks_exact(4).map(|p| quant.index_of(p) as u8).collect();
    let palette: Vec<u8> = map.chunks_exact(4).flat_map(|c| [c[0], c[1], c[2]]).collect();
    let alpha: Vec<u8> = map.chunks_exact(4).map(|c| c[3]).collect();

    let mut enc = png::Encoder::new(Cursor::new(buf), img.width(), img.height());
    enc.set_color(png::ColorType::Indexed);
    enc.set_depth(png::BitDepth::Eight);
    enc.set_palette(palette);
    if alpha.iter().any(|&a| a < 255) {
        enc.set_trns(alpha);
    }
    enc.set_compression(png::Compression::Best);
    let mut writer = enc.write_header().context("PNG-8: cabecera")?;
    writer.write_image_data(&indices).context("PNG-8: datos")?;
    writer.finish().context("PNG-8: cierre")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    fn sample() -> RgbaImage {
        RgbaImage::from_fn(32, 24, |x, y| {
            let a = if x < 4 { 0 } else { 255 };
            Rgba([(x * 8) as u8, (y * 10) as u8, 120, a])
        })
    }

    fn roundtrip(format: ExportFormat) -> RgbaImage {
        let bytes = encode(&sample(), format).expect("encode");
        assert!(!bytes.is_empty());
        image::load_from_memory(&bytes).expect("decode").to_rgba8()
    }

    #[test]
    fn png24_is_lossless() {
        assert_eq!(roundtrip(ExportFormat::Png24), sample());
    }

    #[test]
    fn png8_keeps_size_and_transparency() {
        let out = roundtrip(ExportFormat::Png8 { colors: 64 });
        assert_eq!(out.dimensions(), (32, 24));
        assert!(out.get_pixel(0, 0).0[3] < 128);
        assert_eq!(out.get_pixel(20, 10).0[3], 255);
    }

    #[test]
    fn jpeg_gif_webp_decode() {
        assert_eq!(roundtrip(ExportFormat::Jpeg { quality: 80 }).dimensions(), (32, 24));
        assert_eq!(roundtrip(ExportFormat::Gif).dimensions(), (32, 24));
        assert_eq!(roundtrip(ExportFormat::WebP { lossless: false, quality: 75.0 }).dimensions(), (32, 24));
        // WebP sin pérdida descarta el color de los píxeles 100 % transparentes.
        let lossless = roundtrip(ExportFormat::WebP { lossless: true, quality: 100.0 });
        for (a, b) in lossless.pixels().zip(sample().pixels()) {
            assert_eq!(a.0[3], b.0[3]);
            if b.0[3] > 0 {
                assert_eq!(a.0, b.0);
            }
        }
    }

    #[test]
    fn lower_jpeg_quality_is_smaller() {
        let img = sample();
        let hi = encode(&img, ExportFormat::Jpeg { quality: 95 }).unwrap().len();
        let lo = encode(&img, ExportFormat::Jpeg { quality: 20 }).unwrap().len();
        assert!(lo < hi);
    }
}
