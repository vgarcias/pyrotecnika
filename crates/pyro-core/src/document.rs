//! Documento con capas de bitmap del mismo tamaño que el lienzo.
//! La capa 0 es la del fondo (la más baja).

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use image::{imageops, Rgba, RgbaImage};
use rayon::prelude::*;

use crate::selection::Mask;

/// Identidad estable de una capa: la conservan las copias del documento
/// (instantáneas, historial) y cambia al duplicarla.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct LayerId(pub u64);

impl LayerId {
    fn fresh() -> LayerId {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        LayerId(NEXT.fetch_add(1, Ordering::Relaxed))
    }
}

/// Capa de bitmap. Los píxeles se comparten entre copias del documento y se
/// copian solo al escribir ([`Layer::image_mut`]).
#[derive(Clone, Debug)]
pub struct Layer {
    pub id: LayerId,
    pub name: String,
    pub visible: bool,
    pub opacity: f32,
    pub pixels: Arc<RgbaImage>,
}

impl Layer {
    pub fn new(name: impl Into<String>, width: u32, height: u32) -> Self {
        Self::from_image(name, RgbaImage::new(width, height))
    }

    pub fn from_image(name: impl Into<String>, pixels: RgbaImage) -> Self {
        Self { id: LayerId::fresh(), name: name.into(), visible: true, opacity: 1.0, pixels: Arc::new(pixels) }
    }

    /// Píxeles para escribir; si otra copia los comparte, se copian antes.
    pub fn image_mut(&mut self) -> &mut RgbaImage {
        Arc::make_mut(&mut self.pixels)
    }
}

/// Compone `src` sobre `dst` (alfa no premultiplicado) con una opacidad extra.
#[inline]
pub fn blend_over(dst: [u8; 4], src: [u8; 4], opacity: f32) -> [u8; 4] {
    let sa = src[3] as f32 / 255.0 * opacity.clamp(0.0, 1.0);
    if sa <= 0.0 {
        return dst;
    }
    let da = dst[3] as f32 / 255.0;
    let oa = sa + da * (1.0 - sa);
    let mut out = [0u8; 4];
    for c in 0..3 {
        let v = (src[c] as f32 * sa + dst[c] as f32 * da * (1.0 - sa)) / oa;
        out[c] = v.round().clamp(0.0, 255.0) as u8;
    }
    out[3] = (oa * 255.0).round() as u8;
    out
}

/// Interpola dos píxeles en espacio premultiplicado (evita halos oscuros).
#[inline]
pub fn lerp_rgba(a: [u8; 4], b: [u8; 4], t: f32) -> [u8; 4] {
    let t = t.clamp(0.0, 1.0);
    let aa = a[3] as f32 / 255.0;
    let ba = b[3] as f32 / 255.0;
    let oa = aa + (ba - aa) * t;
    if oa <= 0.0 {
        return [0, 0, 0, 0];
    }
    let mut out = [0u8; 4];
    for c in 0..3 {
        let v = (a[c] as f32 * aa * (1.0 - t) + b[c] as f32 * ba * t) / oa;
        out[c] = v.round().clamp(0.0, 255.0) as u8;
    }
    out[3] = (oa * 255.0).round() as u8;
    out
}

#[derive(Clone, Debug)]
pub struct Document {
    pub width: u32,
    pub height: u32,
    pub layers: Vec<Layer>,
    pub active: usize,
    pub selection: Option<Mask>,
}

impl Document {
    pub fn new(width: u32, height: u32, background: Option<[u8; 4]>) -> Self {
        let mut layer = Layer::new("Fondo", width, height);
        if let Some(c) = background {
            for p in layer.image_mut().pixels_mut() {
                *p = Rgba(c);
            }
        }
        Self { width, height, layers: vec![layer], active: 0, selection: None }
    }

    pub fn from_image(pixels: RgbaImage, layer_name: impl Into<String>) -> Self {
        let (width, height) = pixels.dimensions();
        Self { width, height, layers: vec![Layer::from_image(layer_name, pixels)], active: 0, selection: None }
    }

    /// Imagen final con todas las capas visibles.
    pub fn composite(&self) -> RgbaImage {
        self.composite_rect(0, 0, self.width, self.height)
    }

    /// Imagen final del rectángulo `x0..x1` × `y0..y1` (recortado al
    /// lienzo), compuesta en paralelo por filas.
    pub fn composite_rect(&self, x0: u32, y0: u32, x1: u32, y1: u32) -> RgbaImage {
        let (x1, y1) = (x1.min(self.width), y1.min(self.height));
        let (x0, y0) = (x0.min(x1), y0.min(y1));
        let (w, h) = (x1 - x0, y1 - y0);
        let mut out = RgbaImage::new(w, h);
        if w == 0 || h == 0 {
            return out;
        }
        let layers: Vec<&Layer> = self.layers.iter().filter(|l| l.visible && l.opacity > 0.0).collect();
        let row_bytes = w as usize * 4;
        let src_stride = self.width as usize * 4;
        out.par_chunks_mut(row_bytes).enumerate().for_each(|(row, dst)| {
            let y = y0 as usize + row;
            for layer in &layers {
                let start = y * src_stride + x0 as usize * 4;
                let src = &layer.pixels.as_raw()[start..start + row_bytes];
                let opaque = layer.opacity >= 1.0;
                for (d, s) in dst.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
                    match s[3] {
                        0 => {}
                        255 if opaque => d.copy_from_slice(s),
                        _ => {
                            let out = blend_over([d[0], d[1], d[2], d[3]], [s[0], s[1], s[2], s[3]], layer.opacity);
                            d.copy_from_slice(&out);
                        }
                    }
                }
            }
        });
        out
    }

    /// Color final de un píxel, sin componer el documento entero.
    pub fn composite_pixel(&self, x: u32, y: u32) -> [u8; 4] {
        self.layers
            .iter()
            .filter(|l| l.visible && l.opacity > 0.0)
            .fold([0, 0, 0, 0], |d, l| blend_over(d, l.pixels.get_pixel(x, y).0, l.opacity))
    }

    pub fn active_layer(&self) -> &Layer {
        &self.layers[self.active]
    }

    pub fn active_layer_mut(&mut self) -> &mut Layer {
        &mut self.layers[self.active]
    }

    pub fn add_layer(&mut self) {
        let name = format!("Capa {}", self.layers.len() + 1);
        self.layers.insert(self.active + 1, Layer::new(name, self.width, self.height));
        self.active += 1;
    }

    pub fn duplicate_layer(&mut self) {
        let mut copy = self.layers[self.active].clone();
        copy.id = LayerId::fresh();
        copy.name = format!("{} copia", copy.name);
        self.layers.insert(self.active + 1, copy);
        self.active += 1;
    }

    pub fn delete_layer(&mut self) -> bool {
        if self.layers.len() <= 1 {
            return false;
        }
        self.layers.remove(self.active);
        self.active = self.active.min(self.layers.len() - 1);
        true
    }

    pub fn move_layer_up(&mut self) -> bool {
        if self.active + 1 >= self.layers.len() {
            return false;
        }
        self.layers.swap(self.active, self.active + 1);
        self.active += 1;
        true
    }

    pub fn move_layer_down(&mut self) -> bool {
        if self.active == 0 {
            return false;
        }
        self.layers.swap(self.active, self.active - 1);
        self.active -= 1;
        true
    }

    /// Combina la capa activa con la de abajo.
    pub fn merge_down(&mut self) -> bool {
        if self.active == 0 {
            return false;
        }
        let top = self.layers.remove(self.active);
        self.active -= 1;
        if top.visible {
            let below = self.layers[self.active].image_mut();
            for (d, s) in below.pixels_mut().zip(top.pixels.pixels()) {
                d.0 = blend_over(d.0, s.0, top.opacity);
            }
        }
        true
    }

    /// Recorta todas las capas (x1/y1 exclusivos).
    pub fn crop(&mut self, x0: u32, y0: u32, x1: u32, y1: u32) {
        let (w, h) = (x1 - x0, y1 - y0);
        for layer in &mut self.layers {
            layer.pixels = Arc::new(imageops::crop_imm(&*layer.pixels, x0, y0, w, h).to_image());
        }
        self.width = w;
        self.height = h;
        self.selection = None;
    }

    pub fn crop_to_selection(&mut self) -> bool {
        match self.selection.as_ref().and_then(Mask::bounds) {
            Some((x0, y0, x1, y1)) => {
                self.crop(x0, y0, x1, y1);
                true
            }
            None => false,
        }
    }

    /// Borra los píxeles seleccionados de la capa activa.
    pub fn clear_selected(&mut self) -> bool {
        let Some(sel) = &self.selection else {
            return false;
        };
        let layer = self.layers[self.active].image_mut();
        for (p, &m) in layer.pixels_mut().zip(sel.data.iter()) {
            if m > 0 {
                p.0[3] = (p.0[3] as f32 * (1.0 - m as f32 / 255.0)).round() as u8;
            }
        }
        true
    }
}

/// Desplaza una capa. Con selección, solo se mueven los píxeles seleccionados
/// y el hueco que dejan queda transparente.
pub fn shift_pixels(base: &RgbaImage, sel: Option<&Mask>, dx: i32, dy: i32) -> RgbaImage {
    let (w, h) = base.dimensions();
    let source = |x: u32, y: u32| -> Option<(u32, u32)> {
        let (sx, sy) = (x as i64 - dx as i64, y as i64 - dy as i64);
        (sx >= 0 && sy >= 0 && sx < w as i64 && sy < h as i64).then_some((sx as u32, sy as u32))
    };
    match sel {
        None => RgbaImage::from_fn(w, h, |x, y| match source(x, y) {
            Some((sx, sy)) => *base.get_pixel(sx, sy),
            None => Rgba([0, 0, 0, 0]),
        }),
        Some(mask) => {
            let mut out = base.clone();
            for (p, &m) in out.pixels_mut().zip(mask.data.iter()) {
                if m > 0 {
                    p.0[3] = (p.0[3] as f32 * (1.0 - m as f32 / 255.0)).round() as u8;
                }
            }
            for y in 0..h {
                for x in 0..w {
                    let Some((sx, sy)) = source(x, y) else {
                        continue;
                    };
                    let m = mask.get(sx, sy);
                    if m == 0 {
                        continue;
                    }
                    let mut src = base.get_pixel(sx, sy).0;
                    src[3] = (src[3] as u32 * m as u32 / 255) as u8;
                    let d = out.get_pixel(x, y).0;
                    out.put_pixel(x, y, Rgba(blend_over(d, src, 1.0)));
                }
            }
            out
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blend_over_opaque_and_half() {
        assert_eq!(blend_over([0, 0, 0, 255], [255, 255, 255, 255], 1.0), [255, 255, 255, 255]);
        let half = blend_over([0, 0, 0, 255], [255, 255, 255, 255], 0.5);
        assert_eq!(half[3], 255);
        assert!((half[0] as i32 - 128).abs() <= 1);
        assert_eq!(blend_over([0, 0, 0, 0], [10, 20, 30, 0], 1.0), [0, 0, 0, 0]);
    }

    #[test]
    fn lerp_with_transparent_keeps_color() {
        let out = lerp_rgba([0, 0, 0, 0], [200, 100, 50, 255], 0.5);
        assert_eq!(&out[..3], &[200, 100, 50]);
        assert!((out[3] as i32 - 128).abs() <= 1);
    }

    #[test]
    fn composite_respects_visibility_and_order() {
        let mut doc = Document::new(2, 2, Some([255, 0, 0, 255]));
        doc.add_layer();
        for p in doc.active_layer_mut().image_mut().pixels_mut() {
            *p = Rgba([0, 0, 255, 255]);
        }
        assert_eq!(doc.composite().get_pixel(0, 0).0, [0, 0, 255, 255]);
        doc.layers[1].visible = false;
        assert_eq!(doc.composite().get_pixel(0, 0).0, [255, 0, 0, 255]);
    }

    #[test]
    fn layer_management() {
        let mut doc = Document::new(2, 2, None);
        doc.add_layer();
        doc.duplicate_layer();
        assert_eq!(doc.layers.len(), 3);
        assert_eq!(doc.active, 2);
        assert!(doc.move_layer_down());
        assert_eq!(doc.active, 1);
        assert!(doc.merge_down());
        assert_eq!(doc.layers.len(), 2);
        assert!(doc.delete_layer());
        assert!(!doc.delete_layer());
    }

    #[test]
    fn crop_to_selection_resizes() {
        let mut doc = Document::new(10, 10, Some([1, 2, 3, 255]));
        doc.selection = Some(Mask::rect(10, 10, (2.0, 2.0), (6.0, 5.0)));
        assert!(doc.crop_to_selection());
        assert_eq!((doc.width, doc.height), (4, 3));
        assert_eq!(doc.layers[0].pixels.dimensions(), (4, 3));
        assert!(doc.selection.is_none());
    }

    #[test]
    fn shift_selected_pixels_only() {
        let mut base = RgbaImage::from_pixel(4, 1, Rgba([0, 255, 0, 255]));
        base.put_pixel(0, 0, Rgba([255, 0, 0, 255]));
        let sel = Mask::rect(4, 1, (0.0, 0.0), (1.0, 1.0));
        let out = shift_pixels(&base, Some(&sel), 2, 0);
        assert_eq!(out.get_pixel(0, 0).0[3], 0);
        assert_eq!(out.get_pixel(2, 0).0, [255, 0, 0, 255]);
        assert_eq!(out.get_pixel(1, 0).0, [0, 255, 0, 255]);
        let whole = shift_pixels(&base, None, 1, 0);
        assert_eq!(whole.get_pixel(1, 0).0, [255, 0, 0, 255]);
        assert_eq!(whole.get_pixel(0, 0).0[3], 0);
    }
    /// Tres capas deterministas: fondo opaco en gradiente, una con alfa
    /// variable y opacidad 0.5, y otra con opacidad 0.8.
    pub(crate) fn doc_de_prueba(w: u32, h: u32) -> Document {
        let mut doc = Document::new(w, h, None);
        *doc.layers[0].image_mut() = RgbaImage::from_fn(w, h, |x, y| Rgba([(x % 256) as u8, (y % 256) as u8, 90, 255]));
        doc.add_layer();
        *doc.layers[1].image_mut() =
            RgbaImage::from_fn(w, h, |x, y| Rgba([200, (x * 3 % 256) as u8, 40, ((x + y) * 7 % 256) as u8]));
        doc.layers[1].opacity = 0.5;
        doc.add_layer();
        *doc.layers[2].image_mut() = RgbaImage::from_fn(w, h, |x, y| {
            Rgba([10, 20, (y * 5 % 256) as u8, if (x / 13) % 2 == 0 { 0 } else { 255 }])
        });
        doc.layers[2].opacity = 0.8;
        doc
    }

    #[test]
    fn clonar_el_documento_no_copia_pixeles() {
        let doc = Document::new(64, 64, Some([1, 2, 3, 255]));
        let snap = doc.clone();
        assert!(Arc::ptr_eq(&doc.layers[0].pixels, &snap.layers[0].pixels));
        let mut doc = doc;
        doc.layers[0].image_mut().put_pixel(0, 0, Rgba([9, 9, 9, 255]));
        assert_eq!(snap.layers[0].pixels.get_pixel(0, 0).0, [1, 2, 3, 255]);
    }

    #[test]
    fn duplicar_da_un_id_nuevo() {
        let mut doc = Document::new(8, 8, None);
        doc.duplicate_layer();
        assert_ne!(doc.layers[0].id, doc.layers[1].id);
        assert_eq!(doc.clone().layers[1].id, doc.layers[1].id);
    }

    #[test]
    fn componer_un_rectangulo_igual_que_recortar_el_total() {
        let doc = doc_de_prueba(300, 200);
        let full = doc.composite();
        let part = doc.composite_rect(37, 11, 250, 199);
        assert_eq!(part, imageops::crop_imm(&full, 37, 11, 213, 188).to_image());
        assert_eq!(doc.composite_pixel(100, 50), full.get_pixel(100, 50).0);
        // Fuera del lienzo se recorta.
        assert_eq!(doc.composite_rect(290, 190, 400, 400).dimensions(), (10, 10));
    }
}
