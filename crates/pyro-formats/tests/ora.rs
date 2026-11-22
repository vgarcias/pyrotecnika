//! OpenRaster (.ora): ida y vuelta, archivos de GIMP y Krita, y archivos
//! dañados o malintencionados.

use std::io::{Cursor, Read, Write};

use image::{Rgba, RgbaImage};
use pyro_core::document::{Document, Layer};
use pyro_core::task::NoMonitor;
use pyro_formats::ora::{decode_ora, encode_ora, write_ora};
use zip::write::SimpleFileOptions;

fn sample() -> Document {
    let (w, h) = (300, 200);
    let mut doc = Document::from_image(RgbaImage::from_fn(w, h, |x, y| Rgba([x as u8, y as u8, 90, 255])), "Fondo");
    let mut mid =
        Layer::from_image("Retoque ñandú", RgbaImage::from_fn(w, h, |x, _| Rgba([200, 10, 10, (x % 256) as u8])));
    mid.opacity = 0.4;
    doc.layers.push(mid);
    let mut top = Layer::from_image("Oculta", RgbaImage::from_pixel(w, h, Rgba([0, 0, 255, 255])));
    top.visible = false;
    doc.layers.push(top);
    doc.active = 1;
    doc
}

fn png(img: &RgbaImage) -> Vec<u8> {
    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).unwrap();
    out.into_inner()
}

/// Zip OpenRaster hecho a mano: `stack` como stack.xml y las capas dadas.
fn ora_zip(stack: &str, layers: &[(&str, &RgbaImage)]) -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let stored = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    zip.start_file("mimetype", stored).unwrap();
    zip.write_all(b"image/openraster").unwrap();
    zip.start_file("stack.xml", SimpleFileOptions::default()).unwrap();
    zip.write_all(stack.as_bytes()).unwrap();
    for (name, img) in layers {
        zip.start_file(*name, SimpleFileOptions::default()).unwrap();
        zip.write_all(&png(img)).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

#[test]
fn ida_y_vuelta() {
    let doc = sample();
    let bytes = encode_ora(&doc, &NoMonitor).unwrap();
    let back = decode_ora(&bytes, &NoMonitor).unwrap();
    assert!(back.warnings.is_empty(), "{:?}", back.warnings);
    let d = back.document;
    assert_eq!((d.width, d.height, d.active, d.layers.len()), (300, 200, 1, 3));
    for (a, b) in doc.layers.iter().zip(&d.layers) {
        assert_eq!(a.name, b.name);
        assert!((a.opacity - b.opacity).abs() < 0.001, "{} {}", a.opacity, b.opacity);
        assert_eq!(a.visible, b.visible);
        assert!(*a.pixels == *b.pixels, "píxeles de «{}»", a.name);
    }
    // mimetype primero y sin comprimir; mergedimage = composición.
    let mut zip = zip::ZipArchive::new(Cursor::new(&bytes)).unwrap();
    {
        let first = zip.by_index(0).unwrap();
        assert_eq!(first.name().unwrap(), "mimetype");
        assert_eq!(first.compression(), zip::CompressionMethod::Stored);
    }
    let mut merged = Vec::new();
    zip.by_name("mergedimage.png").unwrap().read_to_end(&mut merged).unwrap();
    assert!(image::load_from_memory(&merged).unwrap().to_rgba8() == doc.composite());
    let mut thumb = Vec::new();
    zip.by_name("Thumbnails/thumbnail.png").unwrap().read_to_end(&mut thumb).unwrap();
    let t = image::load_from_memory(&thumb).unwrap();
    assert!(t.width() <= 256 && t.height() <= 256);
}

#[test]
fn estilo_gimp_y_krita() {
    let a = RgbaImage::from_pixel(64, 48, Rgba([10, 20, 30, 255]));
    let b = RgbaImage::from_pixel(20, 10, Rgba([200, 0, 0, 255]));
    let gimp = r#"<?xml version="1.0" encoding="UTF-8"?>
<image version="0.0.3" w="64" h="48" xres="72" yres="72">
 <stack>
  <layer name="Capa roja" src="data/b.png" x="5" y="6" opacity="1.000" visibility="visible" composite-op="svg:src-over" selected="true"/>
  <layer name="Fondo" src="data/a.png" x="0" y="0" opacity="1.000" visibility="visible" composite-op="svg:src-over"/>
 </stack>
</image>"#;
    let g = decode_ora(&ora_zip(gimp, &[("data/a.png", &a), ("data/b.png", &b)]), &NoMonitor).unwrap();
    assert_eq!(g.document.layers.len(), 2);
    assert_eq!(g.document.layers[1].name, "Capa roja");
    assert_eq!(g.document.layers[1].pixels.get_pixel(5, 6).0, [200, 0, 0, 255]);
    assert_eq!(g.document.layers[1].pixels.get_pixel(4, 6).0[3], 0);
    assert!(g.warnings.is_empty(), "{:?}", g.warnings);

    let krita = r#"<?xml version="1.0" encoding="UTF-8"?>
<image w="64" h="48" version="0.0.1">
 <stack>
  <stack name="Grupo" opacity="1" visibility="visible" x="0" y="0">
   <layer name="Dentro" src="data/b.png" x="0" y="0" opacity="0.5" visibility="visible"/>
  </stack>
  <layer name="Fondo" src="data/a.png" x="0" y="0" opacity="1" visibility="visible"/>
 </stack>
</image>"#;
    let k = decode_ora(&ora_zip(krita, &[("data/a.png", &a), ("data/b.png", &b)]), &NoMonitor).unwrap();
    assert_eq!(k.document.layers.len(), 2);
    assert_eq!(k.document.layers[1].name, "Dentro");
    assert_eq!(k.warnings.len(), 1, "{:?}", k.warnings);
}

#[test]
fn capas_desplazadas_y_rutas_raras() {
    let a = RgbaImage::from_pixel(40, 30, Rgba([1, 2, 3, 255]));
    let big = RgbaImage::from_pixel(100, 10, Rgba([9, 9, 9, 255]));
    let shifted = r#"<image w="40" h="30"><stack>
      <layer name="Ancha" src="data/big.png" x="-50" y="0"/>
      <layer name="Fondo" src="data/a.png"/>
    </stack></image>"#;
    let s = decode_ora(&ora_zip(shifted, &[("data/a.png", &a), ("data/big.png", &big)]), &NoMonitor).unwrap();
    assert_eq!(s.document.layers[1].pixels.dimensions(), (40, 30));
    assert_eq!(s.document.layers[1].pixels.get_pixel(0, 0).0, [9, 9, 9, 255]);
    assert_eq!(s.warnings, vec!["Parte de alguna capa quedaba fuera del lienzo y se recortó.".to_string()]);
    for src in ["../x.png", "/etc/x.png", "data/no-existe.png"] {
        let stack = format!(r#"<image w="40" h="30"><stack><layer name="x" src="{src}"/></stack></image>"#);
        let err = decode_ora(&ora_zip(&stack, &[("data/a.png", &a)]), &NoMonitor).err().expect(src).to_string();
        assert!(err.contains("El archivo .ora está dañado"), "{src}: {err}");
    }
}

#[test]
fn danado_nunca_cae() {
    let good = encode_ora(&sample(), &NoMonitor).unwrap();
    let mut cases: Vec<Vec<u8>> = (1..15).map(|i| good[..good.len() * i / 15].to_vec()).collect();
    let a = RgbaImage::from_pixel(4, 4, Rgba([1, 2, 3, 255]));
    cases.push(ora_zip("<image w=", &[]));
    cases
        .push(ora_zip(r#"<image w="4" h="4"><stack><layer src="data/a.png"/></stack></image>"#, &[("data/a.png", &a)]));
    cases.push(ora_zip(r#"<image w="40000" h="10"><stack/></image>"#, &[]));
    cases.push(ora_zip(r#"<image w="4" h="4"><stack><layer src="data/a.png"/></stack></image>"#, &[]));
    let mut corrupt =
        ora_zip(r#"<image w="4" h="4"><stack><layer src="data/a.png"/></stack></image>"#, &[("data/a.png", &a)]);
    let n = corrupt.len();
    corrupt[n / 2] ^= 0xff;
    cases.push(corrupt);
    let many: String = (0..1001).map(|i| format!(r#"<layer name="{i}" src="data/a.png"/>"#)).collect();
    cases.push(ora_zip(&format!(r#"<image w="4" h="4"><stack>{many}</stack></image>"#), &[("data/a.png", &a)]));
    for bytes in &cases {
        let _ = decode_ora(bytes, &NoMonitor);
    }
    let too_many = decode_ora(cases.last().unwrap(), &NoMonitor).err().unwrap().to_string();
    assert!(too_many.contains("demasiado grande"), "{too_many}");
}

#[test]
fn escritura_atomica() {
    let dir = tempfile::tempdir().unwrap();
    assert!(write_ora(&sample(), &dir.path().join("no/existe/x.ora"), &NoMonitor).is_err());
    assert!(!dir.path().join("no").exists());
    let path = dir.path().join("x.ora");
    std::fs::write(&path, b"viejo").unwrap();
    write_ora(&sample(), &path, &NoMonitor).unwrap();
    let back = decode_ora(&std::fs::read(&path).unwrap(), &NoMonitor).unwrap();
    assert_eq!(back.document.layers.len(), 3);
    assert!(!dir.path().join("x.ora.tmp").exists());
}
