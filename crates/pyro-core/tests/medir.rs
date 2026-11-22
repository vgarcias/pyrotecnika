//! Mediciones de RNF-P3/P4 sobre un documento de 4096×4096 con 3 capas.
//! Se corren a mano, de una en una para que no compitan por los núcleos:
//! `cargo test --release -- --ignored medir --test-threads=1`. Siempre
//! imprimen los tiempos; solo fallan por tiempo con `PYRO_MEDIR_ESTRICTO=1`.

use std::sync::Arc;
use std::time::{Duration, Instant};

use image::{Rgba, RgbaImage};
use pyro_core::document::Document;
use pyro_core::history::History;
use pyro_core::paint::{self, BrushParams, Stroke, StrokeKind};
use pyro_core::selection::Mask;

const SIDE: u32 = 4096;

fn strict() -> bool {
    std::env::var("PYRO_MEDIR_ESTRICTO").is_ok_and(|v| v == "1")
}

fn report(name: &str, took: Duration, limit_ms: f64) {
    let ms = took.as_secs_f64() * 1000.0;
    println!("medir {name}: {ms:.1} ms (límite {limit_ms} ms)");
    if strict() {
        assert!(ms < limit_ms, "{name}: {ms:.1} ms ≥ {limit_ms} ms");
    }
}

/// Mediana de 5 ejecuciones; cada una parte de una copia barata de `doc`.
fn median(doc: &Document, mut op: impl FnMut(&mut Document)) -> Duration {
    let mut times: Vec<Duration> = (0..5)
        .map(|_| {
            let mut d = doc.clone();
            let t = Instant::now();
            op(&mut d);
            t.elapsed()
        })
        .collect();
    times.sort();
    times[2]
}

/// Fondo en degradado, una capa de manchas semitransparentes y otra con
/// formas opacas sobre transparente.
fn big_document() -> Document {
    let mut doc = Document::new(SIDE, SIDE, None);
    *doc.layers[0].image_mut() =
        RgbaImage::from_fn(SIDE, SIDE, |x, y| Rgba([(x / 16) as u8, (y / 16) as u8, 120, 255]));
    doc.add_layer();
    *doc.layers[1].image_mut() =
        RgbaImage::from_fn(SIDE, SIDE, |x, y| Rgba([200, 90, 40, if (x / 64 + y / 64) % 3 == 0 { 140 } else { 0 }]));
    doc.layers[1].opacity = 0.8;
    doc.add_layer();
    *doc.layers[2].image_mut() = RgbaImage::from_fn(SIDE, SIDE, |x, y| {
        Rgba([20, 20, 20, if (x / 300) % 2 == 0 && (y / 300) % 2 == 0 { 255 } else { 0 }])
    });
    doc.selection = Some(Mask::rect(SIDE, SIDE, (500.0, 500.0), (3000.0, 2800.0)));
    doc
}

fn brush() -> BrushParams {
    BrushParams { radius: 24.0, ..BrushParams::default() }
}

/// Un trazo de `n` puntos en diagonal desde `(x, y)`.
fn stroke(doc: &mut Document, x: f32, y: f32, n: usize) {
    let active = doc.active;
    let mut s = Stroke::new(StrokeKind::Paint, Arc::clone(&doc.layers[active].pixels), (0.0, 0.0));
    let b = brush();
    let mut last = (x, y);
    for i in 1..n {
        let p = (x + i as f32 * 6.0, y + i as f32 * 3.0);
        for pt in paint::stroke_points(last, p, (b.radius * 0.25).max(1.0)) {
            s.dab(doc.layers[active].image_mut(), None, pt.0, pt.1, &b);
        }
        last = p;
    }
}

#[test]
#[ignore = "medición: cargo test --release -- --ignored medir"]
fn medir_sellar_historial_tras_trazo() {
    let doc = big_document();
    // Solo cuenta el sellado; el trazo queda fuera del tiempo.
    let mut times = Vec::new();
    for _ in 0..5 {
        let mut d = doc.clone();
        let mut h = History::default();
        h.push(&d, "Cambio");
        stroke(&mut d, 600.0, 600.0, 100);
        let t = Instant::now();
        h.seal(&d);
        times.push(t.elapsed());
    }
    times.sort();
    report("sellar_historial_tras_trazo", times[2], 50.0);
}

#[test]
#[ignore = "medición: cargo test --release -- --ignored medir"]
fn medir_operaciones_en_el_hilo_de_la_ui() {
    let doc = big_document();
    type Op = Box<dyn Fn(&mut Document)>;
    let ops: Vec<(&str, Op)> = vec![
        ("primera_pincelada_copia_la_capa", Box::new(|d| stroke(d, 100.0, 100.0, 2))),
        (
            "varita_contigua",
            Box::new(|d| {
                d.selection = Some(Mask::magic_wand(&d.layers[0].pixels, 10, 10, 32, true));
            }),
        ),
        (
            "varita_no_contigua",
            Box::new(|d| {
                d.selection = Some(Mask::magic_wand(&d.layers[0].pixels, 10, 10, 32, false));
            }),
        ),
        (
            "bote",
            Box::new(|d| {
                let sel = d.selection.clone();
                paint::flood_fill(d.layers[0].image_mut(), sel.as_ref(), 600, 600, 32, [255, 0, 0, 255], 1.0);
            }),
        ),
        ("recortar", Box::new(|d| d.crop(100, 100, 4000, 3900))),
        (
            "combinar_abajo",
            Box::new(|d| {
                d.merge_down();
            }),
        ),
        ("duplicar_capa", Box::new(|d| d.duplicate_layer())),
        (
            "invertir_seleccion",
            Box::new(|d| {
                if let Some(m) = d.selection.as_mut() {
                    m.invert();
                }
            }),
        ),
        (
            "borrar_seleccion",
            Box::new(|d| {
                d.clear_selected();
            }),
        ),
        (
            "componer_todo",
            Box::new(|d| {
                let _ = d.composite();
            }),
        ),
        (
            "color_de_un_pixel",
            Box::new(|d| {
                let _ = d.composite_pixel(2000, 2000);
            }),
        ),
    ];
    for (name, op) in &ops {
        report(name, median(&doc, op), 50.0);
    }

    // Deshacer y rehacer tras un trazo (el trazo no cuenta).
    let mut undo = Vec::new();
    let mut redo = Vec::new();
    for _ in 0..5 {
        let mut d = doc.clone();
        let mut h = History::default();
        h.push(&d, "Cambio");
        stroke(&mut d, 600.0, 600.0, 100);
        h.seal(&d);
        let t = Instant::now();
        h.undo(&mut d);
        undo.push(t.elapsed());
        let t = Instant::now();
        h.redo(&mut d);
        redo.push(t.elapsed());
    }
    undo.sort();
    redo.sort();
    report("deshacer", undo[2], 50.0);
    report("rehacer", redo[2], 50.0);
}

#[test]
#[ignore = "medición: cargo test --release -- --ignored medir"]
fn medir_memoria_del_historial() {
    let mut d = big_document();
    let mut h = History::default();
    for i in 0..100 {
        h.push(&d, "Cambio");
        stroke(&mut d, 100.0 + (i % 10) as f32 * 350.0, 100.0 + (i / 10) as f32 * 350.0, 20);
    }
    h.seal(&d);
    let mb = h.bytes() as f64 / (1024.0 * 1024.0);
    println!("medir memoria_del_historial: {mb:.1} MB tras 100 trazos (límite 64 MB)");
    if strict() {
        assert!(mb < 64.0, "{mb:.1} MB");
    }
}
