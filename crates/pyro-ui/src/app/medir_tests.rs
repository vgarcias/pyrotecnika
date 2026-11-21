//! Medición de RNF-P3: trabajo de CPU por fotograma durante un trazo en un
//! lienzo de 4096×4096 con 3 capas (pincelada + componer el rectángulo
//! sucio + preparar la imagen del bloque). `cargo test --release -p pyro-ui
//! -- --ignored medir`; solo falla por tiempo con `PYRO_MEDIR_ESTRICTO=1`.

use std::sync::Arc;
use std::time::Instant;

use eframe::egui::ColorImage;
use image::{Rgba, RgbaImage};

use pyro_core::document::Document;
use pyro_core::paint::{self, BrushParams, Stroke, StrokeKind};

#[test]
#[ignore = "medición: cargo test --release -- --ignored medir"]
fn medir_trazo_por_fotograma() {
    const SIDE: u32 = 4096;
    let mut doc = Document::new(SIDE, SIDE, None);
    *doc.layers[0].image_mut() =
        RgbaImage::from_fn(SIDE, SIDE, |x, y| Rgba([(x / 16) as u8, (y / 16) as u8, 120, 255]));
    doc.add_layer();
    *doc.layers[1].image_mut() =
        RgbaImage::from_fn(SIDE, SIDE, |x, y| Rgba([200, 90, 40, if (x / 64 + y / 64) % 3 == 0 { 140 } else { 0 }]));
    doc.layers[1].opacity = 0.8;
    doc.add_layer();
    let brush = BrushParams { radius: 24.0, ..BrushParams::default() };
    let active = doc.active;
    let mut stroke = Stroke::new(StrokeKind::Paint, Arc::clone(&doc.layers[active].pixels), (0.0, 0.0));
    // El ratón avanza unos 12 px por fotograma.
    let mut last = (300.0_f32, 300.0_f32);
    let mut frames = Vec::with_capacity(200);
    for i in 1..=200 {
        let p = (300.0 + i as f32 * 10.0, 300.0 + i as f32 * 7.0);
        let t = Instant::now();
        for pt in paint::stroke_points(last, p, (brush.radius * 0.25).max(1.0)) {
            stroke.dab(doc.layers[active].image_mut(), None, pt.0, pt.1, &brush);
        }
        let r = brush.radius + 1.0;
        let (x0, y0) = ((last.0.min(p.0) - r) as u32, (last.1.min(p.1) - r) as u32);
        let (x1, y1) = ((last.0.max(p.0) + r) as u32, (last.1.max(p.1) + r) as u32);
        let part = doc.composite_rect(x0, y0, x1, y1);
        let img = ColorImage::from_rgba_unmultiplied([part.width() as usize, part.height() as usize], part.as_raw());
        std::hint::black_box(img);
        frames.push(t.elapsed());
        last = p;
    }
    frames.sort();
    let p95 = frames[frames.len() * 95 / 100];
    let ms = p95.as_secs_f64() * 1000.0;
    println!("medir trazo_por_fotograma: {ms:.2} ms p95 (límite 16 ms)");
    if std::env::var("PYRO_MEDIR_ESTRICTO").is_ok_and(|v| v == "1") {
        assert!(ms < 16.0, "{ms:.2} ms");
    }
}
