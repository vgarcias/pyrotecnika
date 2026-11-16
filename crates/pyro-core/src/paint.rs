//! Pinceles: pintar, borrar, clonar y desenfocar, más el bote de pintura.
//!
//! Un trazo guarda la capa tal como estaba al empezar y la cobertura máxima de
//! cada píxel, así la opacidad no se acumula al pasar dos veces por el mismo
//! sitio dentro del mismo trazo.

use image::{Rgba, RgbaImage};

use crate::document::{blend_over, lerp_rgba};
use crate::selection::{Mask, SelectMode};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BrushParams {
    pub radius: f32,
    /// 0 = borde muy suave, 1 = borde duro.
    pub hardness: f32,
    pub opacity: f32,
    pub color: [u8; 4],
}

impl Default for BrushParams {
    fn default() -> Self {
        Self { radius: 12.0, hardness: 0.7, opacity: 1.0, color: [0, 0, 0, 255] }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StrokeKind {
    Paint,
    Erase,
    Clone,
    Blur,
}

pub fn dab_weight(dist: f32, radius: f32, hardness: f32) -> f32 {
    if radius <= 0.0 || dist >= radius {
        return 0.0;
    }
    let inner = radius * hardness.clamp(0.0, 1.0);
    if dist <= inner {
        return 1.0;
    }
    let s = 1.0 - (dist - inner) / (radius - inner).max(1e-6);
    s * s * (3.0 - 2.0 * s)
}

pub struct Stroke {
    kind: StrokeKind,
    base: RgbaImage,
    coverage: Vec<f32>,
    clone_offset: (f32, f32),
}

impl Stroke {
    /// `clone_offset` = origen − punto inicial (solo para el tampón).
    pub fn new(kind: StrokeKind, base: RgbaImage, clone_offset: (f32, f32)) -> Self {
        let len = (base.width() * base.height()) as usize;
        Self { kind, base, coverage: vec![0.0; len], clone_offset }
    }

    pub fn dab(&mut self, target: &mut RgbaImage, sel: Option<&Mask>, cx: f32, cy: f32, p: &BrushParams) {
        let (w, h) = target.dimensions();
        let r = p.radius.max(0.5);
        let x0 = (cx - r).floor().clamp(0.0, w as f32) as u32;
        let x1 = (cx + r).ceil().clamp(0.0, w as f32) as u32;
        let y0 = (cy - r).floor().clamp(0.0, h as f32) as u32;
        let y1 = (cy + r).ceil().clamp(0.0, h as f32) as u32;
        for y in y0..y1 {
            for x in x0..x1 {
                let d = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt();
                let wgt = dab_weight(d, r, p.hardness);
                let idx = (y * w + x) as usize;
                if wgt <= self.coverage[idx] {
                    continue;
                }
                let base = self.base.get_pixel(x, y).0;
                let sample = match self.kind {
                    StrokeKind::Clone => {
                        let sx = (x as f32 + self.clone_offset.0).round() as i64;
                        let sy = (y as f32 + self.clone_offset.1).round() as i64;
                        if sx < 0 || sy < 0 || sx >= w as i64 || sy >= h as i64 {
                            continue;
                        }
                        Some(self.base.get_pixel(sx as u32, sy as u32).0)
                    }
                    StrokeKind::Blur => Some(box_average(&self.base, x, y, 2)),
                    _ => None,
                };
                self.coverage[idx] = wgt;
                let m = sel.map_or(1.0, |s| s.data[idx] as f32 / 255.0);
                let amount = wgt * p.opacity * m;
                let out = match self.kind {
                    StrokeKind::Paint => blend_over(base, p.color, amount),
                    StrokeKind::Erase => {
                        let mut b = base;
                        b[3] = (b[3] as f32 * (1.0 - amount)).round() as u8;
                        b
                    }
                    StrokeKind::Clone | StrokeKind::Blur => lerp_rgba(base, sample.unwrap_or(base), amount),
                };
                target.put_pixel(x, y, Rgba(out));
            }
        }
    }
}

/// Promedio (premultiplicado) de un cuadrado de lado 2r+1.
fn box_average(img: &RgbaImage, x: u32, y: u32, r: u32) -> [u8; 4] {
    let (w, h) = img.dimensions();
    let mut acc = [0f32; 4];
    let mut n = 0f32;
    for yy in y.saturating_sub(r)..(y + r + 1).min(h) {
        for xx in x.saturating_sub(r)..(x + r + 1).min(w) {
            let p = img.get_pixel(xx, yy).0;
            let a = p[3] as f32 / 255.0;
            for c in 0..3 {
                acc[c] += p[c] as f32 * a;
            }
            acc[3] += a;
            n += 1.0;
        }
    }
    if acc[3] <= 0.0 {
        return [0, 0, 0, 0];
    }
    [
        (acc[0] / acc[3]).round() as u8,
        (acc[1] / acc[3]).round() as u8,
        (acc[2] / acc[3]).round() as u8,
        (acc[3] / n * 255.0).round() as u8,
    ]
}

/// Puntos a lo largo de un segmento separados `spacing`, sin incluir `from`.
pub fn stroke_points(from: (f32, f32), to: (f32, f32), spacing: f32) -> Vec<(f32, f32)> {
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let dist = (dx * dx + dy * dy).sqrt();
    let spacing = spacing.max(0.5);
    let n = (dist / spacing).floor() as usize;
    (1..=n)
        .map(|i| {
            let t = i as f32 * spacing / dist;
            (from.0 + dx * t, from.1 + dy * t)
        })
        .collect()
}

/// Bote de pintura: rellena la zona contigua parecida al píxel (x, y).
pub fn flood_fill(
    layer: &mut RgbaImage,
    sel: Option<&Mask>,
    x: u32,
    y: u32,
    tolerance: u8,
    color: [u8; 4],
    opacity: f32,
) -> bool {
    if x >= layer.width() || y >= layer.height() {
        return false;
    }
    let mut region = Mask::magic_wand(layer, x, y, tolerance, true);
    if let Some(s) = sel {
        region.combine(s, SelectMode::Intersect);
    }
    for (p, &m) in layer.pixels_mut().zip(region.data.iter()) {
        if m > 0 {
            p.0 = blend_over(p.0, color, opacity * m as f32 / 255.0);
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weight_profile() {
        assert_eq!(dab_weight(0.0, 10.0, 0.5), 1.0);
        assert_eq!(dab_weight(10.0, 10.0, 0.5), 0.0);
        let mid = dab_weight(7.5, 10.0, 0.5);
        assert!(mid > 0.0 && mid < 1.0);
    }

    #[test]
    fn paint_dab_colors_center_not_outside() {
        let base = RgbaImage::from_pixel(20, 20, Rgba([255, 255, 255, 255]));
        let mut target = base.clone();
        let mut s = Stroke::new(StrokeKind::Paint, base, (0.0, 0.0));
        let p = BrushParams { radius: 4.0, hardness: 1.0, opacity: 1.0, color: [255, 0, 0, 255] };
        s.dab(&mut target, None, 10.0, 10.0, &p);
        assert_eq!(target.get_pixel(10, 10).0, [255, 0, 0, 255]);
        assert_eq!(target.get_pixel(0, 0).0, [255, 255, 255, 255]);
    }

    #[test]
    fn opacity_does_not_accumulate_within_stroke() {
        let base = RgbaImage::from_pixel(10, 10, Rgba([255, 255, 255, 255]));
        let mut target = base.clone();
        let mut s = Stroke::new(StrokeKind::Paint, base, (0.0, 0.0));
        let p = BrushParams { radius: 3.0, hardness: 1.0, opacity: 0.5, color: [0, 0, 0, 255] };
        s.dab(&mut target, None, 5.0, 5.0, &p);
        let once = target.get_pixel(5, 5).0;
        s.dab(&mut target, None, 5.0, 5.0, &p);
        assert_eq!(target.get_pixel(5, 5).0, once);
    }

    #[test]
    fn selection_limits_painting_and_erase_works() {
        let base = RgbaImage::from_pixel(10, 10, Rgba([255, 255, 255, 255]));
        let sel = Mask::rect(10, 10, (0.0, 0.0), (5.0, 10.0));
        let mut target = base.clone();
        let mut s = Stroke::new(StrokeKind::Erase, base, (0.0, 0.0));
        let p = BrushParams { radius: 10.0, hardness: 1.0, ..Default::default() };
        s.dab(&mut target, Some(&sel), 5.0, 5.0, &p);
        assert_eq!(target.get_pixel(2, 5).0[3], 0);
        assert_eq!(target.get_pixel(7, 5).0[3], 255);
    }

    #[test]
    fn clone_copies_from_offset() {
        let mut base = RgbaImage::from_pixel(20, 10, Rgba([0, 0, 0, 255]));
        for y in 0..10 {
            for x in 0..5 {
                base.put_pixel(x, y, Rgba([0, 255, 0, 255]));
            }
        }
        let mut target = base.clone();
        let mut s = Stroke::new(StrokeKind::Clone, base, (-12.0, 0.0));
        let p = BrushParams { radius: 2.0, hardness: 1.0, ..Default::default() };
        s.dab(&mut target, None, 14.5, 5.5, &p);
        assert_eq!(target.get_pixel(14, 5).0, [0, 255, 0, 255]);
    }

    #[test]
    fn spacing_points() {
        let pts = stroke_points((0.0, 0.0), (10.0, 0.0), 2.5);
        assert_eq!(pts.len(), 4);
        assert_eq!(pts.last().copied(), Some((10.0, 0.0)));
        assert!(stroke_points((0.0, 0.0), (1.0, 0.0), 2.0).is_empty());
    }

    #[test]
    fn bucket_fills_region() {
        let mut img = RgbaImage::from_pixel(5, 1, Rgba([255, 255, 255, 255]));
        img.put_pixel(2, 0, Rgba([0, 0, 0, 255]));
        assert!(flood_fill(&mut img, None, 0, 0, 0, [255, 0, 0, 255], 1.0));
        assert_eq!(img.get_pixel(1, 0).0, [255, 0, 0, 255]);
        assert_eq!(img.get_pixel(3, 0).0, [255, 255, 255, 255]);
    }
}
