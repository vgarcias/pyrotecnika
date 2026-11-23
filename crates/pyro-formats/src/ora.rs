//! OpenRaster (`.ora`), el formato abierto con capas de GIMP y Krita: un zip
//! con `mimetype`, `stack.xml`, una PNG por capa, la imagen compuesta y una
//! miniatura. Especificación 0.0.6.

use std::io::{Cursor, Read, Write};
use std::path::Path;

use anyhow::{anyhow, bail, Context, Result};
use image::{imageops, Rgba, RgbaImage};
use quick_xml::events::{BytesStart, Event};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::io::{check_size, decode_limited};
use pyro_core::document::{Document, Layer};
use pyro_core::task::{Cancelled, Monitor};

const MIMETYPE: &[u8] = b"image/openraster";
/// Espacio de nombres de los atributos propios (capa activa).
const NAMESPACE: &str = "https://pyrotecnika.app/ora";
const MAX_LAYERS: usize = 1000;
/// Tamaño total descomprimido máximo del zip.
const MAX_TOTAL: u64 = 8 << 30;
/// Tamaño máximo de `stack.xml`.
const MAX_STACK_XML: u64 = 16 << 20;
const THUMBNAIL: u32 = 256;

pub const UNSUPPORTED: &str = "Este archivo usa funciones que Pyrotecnika aún no admite; se abrió aproximado.";
pub const CROPPED: &str = "Parte de alguna capa quedaba fuera del lienzo y se recortó.";
pub const FLATTENED: &str = "Este archivo no tiene la lista de capas (stack.xml); se abrió aplanado.";
const TOO_LARGE: &str = "El archivo .ora es demasiado grande.";

/// Memoria total máxima de las capas de un `.ora` (cada capa es un lienzo
/// entero de `w`×`h`).
const MAX_CANVAS_TOTAL: u64 = 8 << 30;

/// Las `layers` capas de `w`×`h` caben en el presupuesto de memoria.
pub fn canvas_budget_ok(w: u32, h: u32, layers: usize) -> bool {
    u64::from(w).saturating_mul(u64::from(h)).saturating_mul(4).saturating_mul(layers.max(1) as u64) <= MAX_CANVAS_TOTAL
}

pub struct OraOpened {
    pub document: Document,
    pub warnings: Vec<String>,
}

fn png_bytes(img: &RgbaImage) -> Result<Vec<u8>> {
    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).context("No se pudo codificar una capa")?;
    Ok(out.into_inner())
}

fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c => out.push(c),
        }
    }
    out
}

/// Codifica el documento como `.ora`.
pub fn encode_ora(doc: &Document, m: &dyn Monitor) -> Result<Vec<u8>> {
    let mut stack = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<image version=\"0.0.6\" w=\"{}\" h=\"{}\" \
         xmlns:pyrotecnika=\"{NAMESPACE}\">\n <stack>\n",
        doc.width, doc.height
    );
    // stack.xml va de arriba abajo; las capas del documento, de abajo arriba.
    for (i, layer) in doc.layers.iter().enumerate().rev() {
        stack.push_str(&format!(
            "  <layer name=\"{}\" src=\"data/layer{i}.png\" x=\"0\" y=\"0\" opacity=\"{:.3}\" visibility=\"{}\" \
             composite-op=\"svg:src-over\"{}/>\n",
            escape(&layer.name),
            layer.opacity.clamp(0.0, 1.0),
            if layer.visible { "visible" } else { "hidden" },
            if i == doc.active { " pyrotecnika:selected=\"true\"" } else { "" },
        ));
    }
    stack.push_str(" </stack>\n</image>\n");

    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    // Las PNG ya van comprimidas: se guardan tal cual.
    let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    let deflated = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    zip.start_file("mimetype", stored)?;
    zip.write_all(MIMETYPE)?;
    zip.start_file("stack.xml", deflated)?;
    zip.write_all(stack.as_bytes())?;
    let total = doc.layers.len() as f32 + 2.0;
    for (i, layer) in doc.layers.iter().enumerate() {
        if m.cancelled() {
            return Err(Cancelled.into());
        }
        zip.start_file(format!("data/layer{i}.png"), stored)?;
        zip.write_all(&png_bytes(&layer.pixels)?)?;
        m.progress((i + 1) as f32 / total);
    }
    let merged = doc.composite();
    zip.start_file("mergedimage.png", stored)?;
    zip.write_all(&png_bytes(&merged)?)?;
    let (w, h) = merged.dimensions();
    let scale = (THUMBNAIL as f32 / w.max(h) as f32).min(1.0);
    let thumb = imageops::thumbnail(&merged, ((w as f32 * scale) as u32).max(1), ((h as f32 * scale) as u32).max(1));
    zip.start_file("Thumbnails/thumbnail.png", stored)?;
    zip.write_all(&png_bytes(&thumb)?)?;
    m.progress(1.0);
    Ok(zip.finish()?.into_inner())
}

/// Escribe el `.ora` sin dejar nunca un archivo a medias: primero un
/// temporal en la misma carpeta y luego se renombra.
pub fn write_ora(doc: &Document, path: &Path, m: &dyn Monitor) -> Result<()> {
    let bytes = encode_ora(doc, m)?;
    // Temporal único: dos guardados a la vez en el mismo archivo no se pisan.
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(format!(".{}-{n}.tmp", std::process::id()));
    let tmp = std::path::PathBuf::from(tmp);
    std::fs::write(&tmp, bytes).with_context(|| format!("No se pudo escribir {}", path.display()))?;
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e).with_context(|| format!("No se pudo guardar {}", path.display()));
    }
    Ok(())
}

/// Los bytes son un zip OpenRaster (su `mimetype` lo dice).
pub fn is_ora(bytes: &[u8]) -> bool {
    if !bytes.starts_with(b"PK\x03\x04") {
        return false;
    }
    let Ok(mut zip) = ZipArchive::new(Cursor::new(bytes)) else { return false };
    let Ok(mut entry) = zip.by_name("mimetype") else { return false };
    let mut text = Vec::new();
    entry.by_ref().take(64).read_to_end(&mut text).is_ok() && text.trim_ascii() == MIMETYPE
}

fn damaged(what: &str) -> anyhow::Error {
    anyhow!("El archivo .ora está dañado: {what}.")
}

struct LayerSpec {
    name: Option<String>,
    src: String,
    x: i64,
    y: i64,
    opacity: f32,
    visible: bool,
    selected: bool,
}

fn attr(e: &BytesStart, key: &str) -> Option<String> {
    e.attributes()
        .flatten()
        .find(|a| a.key.as_ref() == key.as_bytes())
        .and_then(|a| a.unescape_value().ok())
        .map(|v| v.into_owned())
}

/// Lee `stack.xml`: medidas, capas de arriba abajo y si usa cosas que no
/// se admiten (grupos, modos de fusión, texto, filtros).
fn parse_stack(xml: &str) -> Result<(u32, u32, Vec<LayerSpec>, bool)> {
    let mut reader = quick_xml::Reader::from_str(xml);
    let (mut w, mut h) = (None, None);
    let mut layers = Vec::new();
    let mut depth = 0usize;
    let mut unsupported = false;
    loop {
        let event = reader.read_event().map_err(|_| damaged("stack.xml no se puede leer"))?;
        let (e, empty) = match &event {
            Event::Start(e) => (e, false),
            Event::Empty(e) => (e, true),
            Event::End(e) if e.name().as_ref() == b"stack" => {
                depth = depth.saturating_sub(1);
                continue;
            }
            Event::Eof => break,
            _ => continue,
        };
        match e.name().as_ref() {
            b"image" => {
                w = attr(e, "w").and_then(|v| v.trim().parse::<u32>().ok());
                h = attr(e, "h").and_then(|v| v.trim().parse::<u32>().ok());
            }
            b"stack" => {
                if !empty {
                    depth += 1;
                }
                if depth > 1 {
                    unsupported = true;
                }
            }
            b"layer" => {
                if layers.len() >= MAX_LAYERS {
                    bail!(TOO_LARGE);
                }
                let op = attr(e, "composite-op");
                if op.as_deref().is_some_and(|o| o != "svg:src-over") {
                    unsupported = true;
                }
                let flag = |k: &str| attr(e, k).is_some_and(|v| v == "true");
                layers.push(LayerSpec {
                    name: attr(e, "name"),
                    src: attr(e, "src").ok_or_else(|| damaged("una capa no dice su archivo"))?,
                    x: attr(e, "x").and_then(|v| v.trim().parse().ok()).unwrap_or(0),
                    y: attr(e, "y").and_then(|v| v.trim().parse().ok()).unwrap_or(0),
                    opacity: attr(e, "opacity").and_then(|v| v.trim().parse().ok()).unwrap_or(1.0_f32).clamp(0.0, 1.0),
                    visible: attr(e, "visibility").is_none_or(|v| v != "hidden"),
                    selected: flag("pyrotecnika:selected") || flag("selected"),
                });
            }
            b"text" | b"filter" => unsupported = true,
            _ => {}
        }
    }
    let (Some(w), Some(h)) = (w, h) else { return Err(damaged("faltan las medidas de la imagen")) };
    Ok((w, h, layers, unsupported))
}

fn read_entry(zip: &mut ZipArchive<Cursor<&[u8]>>, name: &str, limit: u64) -> Option<Vec<u8>> {
    let entry = zip.by_name(name).ok()?;
    let mut data = Vec::new();
    entry.take(limit).read_to_end(&mut data).ok()?;
    Some(data)
}

/// `src` dentro del zip, sin salir de él.
fn safe_src(src: &str) -> bool {
    !src.is_empty()
        && !src.starts_with('/')
        && !src.starts_with('\\')
        && !src.contains(':')
        && !src.split(['/', '\\']).any(|part| part == "..")
}

/// Lee un `.ora`. Lo que no se admite se aproxima y se avisa; lo que no se
/// puede leer es un error, nunca un `panic`.
pub fn decode_ora(bytes: &[u8], m: &dyn Monitor) -> Result<OraOpened> {
    let mut zip = ZipArchive::new(Cursor::new(bytes)).map_err(|_| damaged("no es un zip válido"))?;
    let mut total = 0u64;
    for i in 0..zip.len() {
        total = total.saturating_add(zip.by_index_raw(i).map_err(|_| damaged("índice roto"))?.size());
    }
    if total > MAX_TOTAL {
        bail!(TOO_LARGE);
    }
    let Some(xml) = read_entry(&mut zip, "stack.xml", MAX_STACK_XML) else {
        let merged = read_entry(&mut zip, "mergedimage.png", u64::MAX).ok_or_else(|| damaged("no tiene capas"))?;
        let img = decode_limited(&merged)?;
        return Ok(OraOpened { document: Document::from_image(img, "Fondo"), warnings: vec![FLATTENED.into()] });
    };
    let xml = String::from_utf8(xml).map_err(|_| damaged("stack.xml no es texto"))?;
    let (w, h, specs, unsupported) = parse_stack(&xml)?;
    check_size(w, h)?;
    if !canvas_budget_ok(w, h, specs.len()) {
        bail!(TOO_LARGE);
    }
    let mut warnings = Vec::new();
    if unsupported {
        warnings.push(UNSUPPORTED.to_string());
    }
    let mut cropped = false;
    let count = specs.len().max(1) as f32;
    let mut layers = Vec::with_capacity(specs.len());
    let mut active = None;
    // De abajo arriba, como el documento.
    for (n, spec) in specs.into_iter().rev().enumerate() {
        if m.cancelled() {
            return Err(Cancelled.into());
        }
        if !safe_src(&spec.src) {
            return Err(damaged(&format!("falta la capa «{}»", spec.src)));
        }
        let data = read_entry(&mut zip, &spec.src, u64::MAX)
            .ok_or_else(|| damaged(&format!("falta la capa «{}»", spec.src)))?;
        let img = decode_limited(&data)?;
        let mut canvas = RgbaImage::from_pixel(w, h, Rgba([0, 0, 0, 0]));
        let (lw, lh) = (i64::from(img.width()), i64::from(img.height()));
        if spec.x < 0 || spec.y < 0 || spec.x + lw > i64::from(w) || spec.y + lh > i64::from(h) {
            cropped = true;
        }
        imageops::replace(&mut canvas, &img, spec.x, spec.y);
        let mut layer = Layer::from_image(spec.name.unwrap_or_else(|| format!("Capa {}", n + 1)), canvas);
        layer.opacity = spec.opacity;
        layer.visible = spec.visible;
        if spec.selected {
            // Se recorre de abajo arriba: gana la más alta marcada.
            active = Some(n);
        }
        layers.push(layer);
        m.progress((n + 1) as f32 / count);
    }
    if cropped {
        warnings.push(CROPPED.to_string());
    }
    if layers.is_empty() {
        layers.push(Layer::new("Fondo", w, h));
    }
    let top = layers.len() - 1;
    let mut document = Document::from_image(RgbaImage::new(1, 1), "");
    document.width = w;
    document.height = h;
    document.layers = layers;
    // La marcada como activa; si no hay ninguna, la de arriba. Si hay
    // varias, la primera de stack.xml (la más alta).
    document.active = active.unwrap_or(top);
    Ok(OraOpened { document, warnings })
}
