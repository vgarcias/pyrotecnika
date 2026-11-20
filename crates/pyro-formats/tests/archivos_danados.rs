//! Archivos dañados, enormes o maliciosos: abren con error o con aviso,
//! nunca cierran la app ni reservan memoria de más.

use std::time::{Duration, Instant};

use image::{ImageEncoder, Rgba, RgbaImage};
use pyro_core::task::NoMonitor;
use pyro_formats::io::{check_size, open_bytes, TooLarge};

fn chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut c = Vec::new();
    c.extend_from_slice(&(data.len() as u32).to_be_bytes());
    c.extend_from_slice(kind);
    c.extend_from_slice(data);
    let crc = crc32fast::hash(&c[4..]);
    c.extend_from_slice(&crc.to_be_bytes());
    c
}

/// PNG con la cabecera que se pida y un IDAT mínimo (el decodificador no
/// debería llegar a leerlo).
fn png_with_header(width: u32, height: u32) -> Vec<u8> {
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]); // 8 bits, RGBA
    let mut png = vec![137, 80, 78, 71, 13, 10, 26, 10];
    png.extend(chunk(b"IHDR", &ihdr));
    png.extend(chunk(b"IDAT", &[0x78, 0x9c, 0x03, 0x00, 0x00, 0x00, 0x00, 0x01]));
    png.extend(chunk(b"IEND", &[]));
    png
}

fn valid_png() -> Vec<u8> {
    let img = RgbaImage::from_fn(40, 30, |x, y| Rgba([x as u8 * 6, y as u8 * 8, 100, 255]));
    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new(&mut png)
        .write_image(img.as_raw(), 40, 30, image::ExtendedColorType::Rgba8)
        .unwrap();
    png
}

fn encoded(format: image::ImageFormat) -> Vec<u8> {
    let img = image::DynamicImage::ImageRgba8(RgbaImage::from_pixel(20, 20, Rgba([1, 2, 3, 255])));
    let img = if format == image::ImageFormat::Jpeg { image::DynamicImage::ImageRgb8(img.to_rgb8()) } else { img };
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, format).unwrap();
    out.into_inner()
}

fn too_large(bytes: &[u8]) -> Option<(u32, u32)> {
    let err = open_bytes(bytes, &NoMonitor).err()?;
    err.downcast_ref::<TooLarge>().map(|t| (t.width, t.height))
}

#[test]
fn lado_demasiado_grande() {
    let start = Instant::now();
    assert_eq!(too_large(&png_with_header(40_000, 10)), Some((40_000, 10)));
    assert!(start.elapsed() < Duration::from_millis(100), "{:?}", start.elapsed());
    let message = TooLarge { width: 40_000, height: 10 }.to_string();
    assert_eq!(
        message,
        "La imagen es demasiado grande (40000×10 px). Pyrotecnika abre imágenes de hasta 32 768 px por lado y 2 GB por capa."
    );
}

#[test]
fn capa_de_mas_de_2_gb() {
    let start = Instant::now();
    assert_eq!(too_large(&png_with_header(30_000, 30_000)), Some((30_000, 30_000)));
    assert!(start.elapsed() < Duration::from_millis(100), "{:?}", start.elapsed());
    assert!(check_size(32_768, 16_384).is_ok());
    assert!(check_size(32_768, 16_385).is_err());
    assert!(check_size(32_769, 1).is_err());
}

#[test]
fn ninguno_cierra_la_app() {
    let mut cases: Vec<Vec<u8>> = Vec::new();
    let png = valid_png();
    for i in 0..20 {
        cases.push(png[..png.len() * i / 20].to_vec());
    }
    // IHDR con una longitud imposible.
    let mut bad_len = png.clone();
    bad_len[8..12].copy_from_slice(&u32::MAX.to_be_bytes());
    cases.push(bad_len);
    // CRC del IHDR roto.
    let mut bad_crc = png.clone();
    bad_crc[29] ^= 0xff;
    cases.push(bad_crc);
    // Cabeceras absurdas.
    cases.push(png_with_header(0, 0));
    cases.push(png_with_header(u32::MAX, u32::MAX));
    for format in [
        image::ImageFormat::Gif,
        image::ImageFormat::WebP,
        image::ImageFormat::Bmp,
        image::ImageFormat::Tiff,
        image::ImageFormat::Jpeg,
    ] {
        let bytes = encoded(format);
        for i in 1..8 {
            cases.push(bytes[..bytes.len() * i / 8].to_vec());
        }
    }
    let mut seed = 99_u64;
    for _ in 0..200 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let len = (seed >> 54) as usize;
        let mut bytes: Vec<u8> = (0..len)
            .map(|_| {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                (seed >> 56) as u8
            })
            .collect();
        if seed % 2 == 0 && bytes.len() > 8 {
            bytes[..8].copy_from_slice(&[137, 80, 78, 71, 13, 10, 26, 10]);
        }
        cases.push(bytes);
    }
    for bytes in &cases {
        let _ = open_bytes(bytes, &NoMonitor);
    }
}

#[test]
fn fireworks_con_crc_roto_abre_aplanado() {
    let mut png = valid_png();
    let iend = png.len() - 12;
    let mut private = chunk(b"mkBF", b"datos privados de Fireworks");
    let n = private.len();
    private[n - 1] ^= 0xff; // CRC roto
    png.splice(iend..iend, private);
    let opened = open_bytes(&png, &NoMonitor).expect("abre aplanado");
    let info = opened.png_info.expect("es PNG");
    assert!(info.is_fireworks());
    assert!(info.damaged());
    assert_eq!((opened.document.width, opened.document.height), (40, 30));
}
