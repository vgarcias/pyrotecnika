//! Deshacer/rehacer por diferencias. Cada paso guarda el estado anterior
//! como *delta inverso* respecto del siguiente: de cada capa, solo los
//! bloques de 256×256 que cambiaron (las capas nuevas, borradas o de otro
//! tamaño, enteras). Limitado por memoria y por número de pasos.

use std::collections::HashMap;
use std::sync::Arc;

use image::RgbaImage;
use rayon::prelude::*;

use crate::document::{Document, Layer, LayerId};
use crate::selection::Mask;

/// Lado de los bloques que se comparan y guardan.
pub const TILE: u32 = 256;
/// Memoria máxima de los pasos guardados.
pub const MEMORY_BUDGET: usize = 768 << 20;
/// Pasos de deshacer como mucho.
pub const MAX_STEPS: usize = 200;

/// Rectángulo de píxeles de una capa (bytes RGBA fila a fila).
struct Tile {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    data: Vec<u8>,
}

enum Content {
    /// La misma capa del otro estado, salvo estos bloques.
    Patch(Vec<Tile>),
    /// Capa entera (no existe en el otro estado o cambió de tamaño).
    Full(Arc<RgbaImage>),
}

struct LayerDelta {
    id: LayerId,
    name: String,
    visible: bool,
    opacity: f32,
    content: Content,
}

enum SelDelta {
    Same,
    Set(Option<Mask>),
}

/// Cómo era un estado respecto de otro.
struct Delta {
    width: u32,
    height: u32,
    active: usize,
    selection: SelDelta,
    layers: Vec<LayerDelta>,
    /// Memoria que ocupa (calculada una vez al crearlo).
    size: usize,
}

impl Delta {
    /// Calcula y guarda su tamaño.
    fn sized(mut self) -> Delta {
        self.size = self.measure();
        self
    }

    fn measure(&self) -> usize {
        let layers: usize = self
            .layers
            .iter()
            .map(|l| {
                64 + l.name.len()
                    + match &l.content {
                        Content::Patch(tiles) => tiles.iter().map(|t| t.data.len() + 16).sum(),
                        Content::Full(img) => img.as_raw().len(),
                    }
            })
            .sum();
        let sel = match &self.selection {
            SelDelta::Set(Some(m)) => m.data.len(),
            _ => 0,
        };
        64 + layers + sel
    }
}

fn read_tile(img: &RgbaImage, x: u32, y: u32, w: u32, h: u32) -> Vec<u8> {
    let stride = img.width() as usize * 4;
    let raw = img.as_raw();
    let mut data = Vec::with_capacity((w * h * 4) as usize);
    for row in y..y + h {
        let start = row as usize * stride + x as usize * 4;
        data.extend_from_slice(&raw[start..start + w as usize * 4]);
    }
    data
}

fn write_tile(img: &mut RgbaImage, t: &Tile) {
    let stride = img.width() as usize * 4;
    let row_bytes = t.w as usize * 4;
    let raw: &mut [u8] = img;
    for (i, row) in (t.y..t.y + t.h).enumerate() {
        let start = row as usize * stride + t.x as usize * 4;
        raw[start..start + row_bytes].copy_from_slice(&t.data[i * row_bytes..(i + 1) * row_bytes]);
    }
}

/// Bloques de `before` distintos de `after` (mismas medidas).
fn diff_tiles(before: &RgbaImage, after: &RgbaImage) -> Vec<Tile> {
    let (w, h) = before.dimensions();
    let stride = w as usize * 4;
    let (a, b) = (before.as_raw(), after.as_raw());
    let rows: Vec<u32> = (0..h).step_by(TILE as usize).collect();
    rows.par_iter()
        .flat_map_iter(|&ty| {
            let th = TILE.min(h - ty);
            (0..w).step_by(TILE as usize).filter_map(move |tx| {
                let tw = TILE.min(w - tx);
                let differs = (ty..ty + th).any(|row| {
                    let start = row as usize * stride + tx as usize * 4;
                    let end = start + tw as usize * 4;
                    a[start..end] != b[start..end]
                });
                differs.then(|| Tile { x: tx, y: ty, w: tw, h: th, data: read_tile(before, tx, ty, tw, th) })
            })
        })
        .collect()
}

/// Describe `before` respecto de `after`.
fn delta(before: &Document, after: &Document) -> Delta {
    let layers = before
        .layers
        .iter()
        .map(|l| {
            let content = match after.layers.iter().find(|o| o.id == l.id) {
                Some(o) if Arc::ptr_eq(&o.pixels, &l.pixels) => Content::Patch(Vec::new()),
                Some(o) if o.pixels.dimensions() == l.pixels.dimensions() => {
                    Content::Patch(diff_tiles(&l.pixels, &o.pixels))
                }
                _ => Content::Full(Arc::clone(&l.pixels)),
            };
            LayerDelta { id: l.id, name: l.name.clone(), visible: l.visible, opacity: l.opacity, content }
        })
        .collect();
    let selection =
        if before.selection == after.selection { SelDelta::Same } else { SelDelta::Set(before.selection.clone()) };
    Delta { width: before.width, height: before.height, active: before.active, selection, layers, size: 0 }.sized()
}

/// Convierte `doc` en el estado que describe `d` y devuelve el delta que lo
/// devuelve a como estaba (el estado de `doc` respecto del nuevo).
fn apply(d: Delta, doc: &mut Document) -> Delta {
    let meta: Vec<(LayerId, String, bool, f32)> =
        doc.layers.iter().map(|l| (l.id, l.name.clone(), l.visible, l.opacity)).collect();
    let mut old: HashMap<LayerId, Layer> = std::mem::take(&mut doc.layers).into_iter().map(|l| (l.id, l)).collect();
    let mut undo_tiles: HashMap<LayerId, Vec<Tile>> = HashMap::new();
    let mut layers = Vec::with_capacity(d.layers.len());
    for ld in d.layers {
        let pixels = match ld.content {
            Content::Patch(tiles) => {
                let mut layer = old.remove(&ld.id).expect("el delta parcial apunta a una capa del estado actual");
                let mut undo = Vec::with_capacity(tiles.len());
                if !tiles.is_empty() {
                    let img = layer.image_mut();
                    for t in tiles {
                        let previous = read_tile(img, t.x, t.y, t.w, t.h);
                        write_tile(img, &t);
                        undo.push(Tile { data: previous, ..t });
                    }
                }
                undo_tiles.insert(ld.id, undo);
                layer.pixels
            }
            Content::Full(pixels) => pixels,
        };
        layers.push(Layer { id: ld.id, name: ld.name, visible: ld.visible, opacity: ld.opacity, pixels });
    }
    let inverse_layers = meta
        .into_iter()
        .map(|(id, name, visible, opacity)| {
            let content = match undo_tiles.remove(&id) {
                Some(tiles) => Content::Patch(tiles),
                None => Content::Full(old.remove(&id).expect("capa del estado actual").pixels),
            };
            LayerDelta { id, name, visible, opacity, content }
        })
        .collect();
    let selection = match d.selection {
        SelDelta::Same => SelDelta::Same,
        SelDelta::Set(s) => SelDelta::Set(std::mem::replace(&mut doc.selection, s)),
    };
    let inverse =
        Delta { width: doc.width, height: doc.height, active: doc.active, selection, layers: inverse_layers, size: 0 }
            .sized();
    doc.layers = layers;
    doc.width = d.width;
    doc.height = d.height;
    doc.active = d.active;
    inverse
}

pub struct History {
    undo: Vec<Delta>,
    redo: Vec<Delta>,
    /// Estado anterior al último cambio, aún sin comparar con el siguiente
    /// (comparte los píxeles con el documento: no cuesta memoria).
    pending: Option<Document>,
    /// Posición en la línea de cambios: sube al modificar o rehacer y baja
    /// al deshacer. No depende de cuántos pasos siguen en memoria.
    position: u64,
    /// Posición del último guardado, o `None` si ya no se puede volver a él.
    saved: Option<u64>,
}

impl Default for History {
    fn default() -> Self {
        Self { undo: Vec::new(), redo: Vec::new(), pending: None, position: 0, saved: Some(0) }
    }
}

impl History {
    /// Guarda el estado actual antes de modificarlo.
    pub fn push(&mut self, doc: &Document) {
        self.seal(doc);
        self.pending = Some(doc.clone());
        self.redo.clear();
        // El guardado quedaba en la rama de rehacer, que se acaba de perder.
        if self.saved.is_some_and(|s| s > self.position) {
            self.saved = None;
        }
        self.position += 1;
        self.trim();
    }

    /// Compara el paso pendiente con `current` y se queda solo con lo que
    /// cambió. Se llama solo; la interfaz lo adelanta al soltar el pincel.
    pub fn seal(&mut self, current: &Document) {
        if let Some(before) = self.pending.take() {
            self.undo.push(delta(&before, current));
            self.trim();
        }
    }

    pub fn undo(&mut self, doc: &mut Document) -> bool {
        self.seal(doc);
        match self.undo.pop() {
            Some(d) => {
                let inverse = apply(d, doc);
                self.redo.push(inverse);
                self.position -= 1;
                true
            }
            None => false,
        }
    }

    pub fn redo(&mut self, doc: &mut Document) -> bool {
        self.seal(doc);
        match self.redo.pop() {
            Some(d) => {
                let inverse = apply(d, doc);
                self.undo.push(inverse);
                self.position += 1;
                true
            }
            None => false,
        }
    }

    /// Descarta el último estado guardado (p. ej. al cancelar un filtro).
    pub fn discard_last(&mut self) {
        if self.pending.take().is_some() || self.undo.pop().is_some() {
            self.position -= 1;
        }
    }

    /// Vacía el historial y da el documento por guardado (recién abierto o creado).
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Memoria que ocupan los pasos guardados.
    pub fn bytes(&self) -> usize {
        self.undo.iter().chain(&self.redo).map(|d| d.size).sum()
    }

    /// Descarta el paso más viejo (para el presupuesto común de varias
    /// pestañas). Siempre deja al menos uno; `false` si no pudo.
    pub fn drop_oldest(&mut self) -> bool {
        if self.undo.len() > 1 {
            self.undo.remove(0);
            true
        } else {
            false
        }
    }

    /// Descarta los pasos más viejos por encima del tope o del presupuesto
    /// (siempre queda al menos uno).
    fn trim(&mut self) {
        let pending = usize::from(self.pending.is_some());
        let mut excess = (self.undo.len() + pending).saturating_sub(MAX_STEPS);
        let mut total = self.bytes();
        while self.undo.len() > 1 && (excess > 0 || total > MEMORY_BUDGET) {
            total -= self.undo[0].size;
            self.undo.remove(0);
            excess = excess.saturating_sub(1);
        }
    }

    /// Posición actual en la línea de cambios (para guardar una instantánea
    /// y marcarla luego con [`History::mark_saved_at`]).
    pub fn position(&self) -> u64 {
        self.position
    }

    /// Marca como guardada la posición `position` (la de la instantánea que
    /// se escribió); si se editó entretanto, el documento sigue modificado.
    pub fn mark_saved_at(&mut self, position: u64) {
        self.saved = Some(position);
    }

    /// Da el documento por no guardado sin añadir pasos (un documento
    /// recuperado tras un fallo).
    pub fn mark_unsaved(&mut self) {
        self.saved = None;
    }

    /// Marca el estado actual como guardado.
    pub fn mark_saved(&mut self) {
        self.saved = Some(self.position);
    }

    /// Indica si hay cambios desde el último guardado.
    pub fn is_modified(&self) -> bool {
        self.saved != Some(self.position)
    }

    pub fn can_undo(&self) -> bool {
        self.pending.is_some() || !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undo_redo_roundtrip() {
        let mut h = History::default();
        let mut doc = Document::new(2, 2, None);
        h.push(&doc);
        doc.add_layer();
        assert!(h.undo(&mut doc));
        assert_eq!(doc.layers.len(), 1);
        assert!(h.redo(&mut doc));
        assert_eq!(doc.layers.len(), 2);
        assert!(!h.redo(&mut doc));
        h.push(&doc);
        assert!(!h.can_redo());
        h.discard_last();
        assert!(h.can_undo());
    }

    #[test]
    fn modified_follows_the_saved_position() {
        let mut h = History::default();
        let mut doc = Document::new(2, 2, None);
        assert!(!h.is_modified());
        h.push(&doc);
        doc.add_layer();
        assert!(h.is_modified());
        // Deshacer hasta el estado guardado deja el documento limpio.
        h.undo(&mut doc);
        assert!(!h.is_modified());
        h.redo(&mut doc);
        h.mark_saved();
        assert!(!h.is_modified());
        // Cancelar un filtro (push + discard) no cuenta como cambio.
        h.push(&doc);
        assert!(h.is_modified());
        h.discard_last();
        assert!(!h.is_modified());
    }

    #[test]
    fn saved_state_in_a_lost_redo_branch_stays_modified() {
        let mut h = History::default();
        let mut doc = Document::new(2, 2, None);
        h.push(&doc);
        doc.add_layer();
        h.mark_saved();
        h.undo(&mut doc);
        h.push(&doc);
        doc.add_layer();
        assert!(h.is_modified());
        h.clear();
        assert!(!h.is_modified());
    }
    use crate::selection::Mask;
    use image::Rgba;

    /// Generador congruencial: secuencias al azar pero repetibles.
    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self, n: u32) -> u32 {
            self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((self.0 >> 33) % n as u64) as u32
        }
    }

    fn assert_same(a: &Document, b: &Document, what: &str) {
        assert_eq!((a.width, a.height, a.active), (b.width, b.height, b.active), "{what}: medidas o capa activa");
        assert_eq!(a.selection, b.selection, "{what}: selección");
        assert_eq!(a.layers.len(), b.layers.len(), "{what}: número de capas");
        for (la, lb) in a.layers.iter().zip(&b.layers) {
            assert_eq!(
                (la.id, &la.name, la.visible, la.opacity),
                (lb.id, &lb.name, lb.visible, lb.opacity),
                "{what}: datos de capa"
            );
            assert!(*la.pixels == *lb.pixels, "{what}: píxeles de «{}»", la.name);
        }
    }

    fn random_op(doc: &mut Document, r: &mut Lcg) {
        match r.next(12) {
            0..=3 => {
                let (w, h) = (doc.width, doc.height);
                let (x0, y0) = (r.next(w), r.next(h));
                let (x1, y1) = ((x0 + 1 + r.next(300)).min(w), (y0 + 1 + r.next(300)).min(h));
                let c = Rgba([r.next(256) as u8, r.next(256) as u8, r.next(256) as u8, r.next(256) as u8]);
                let img = doc.active_layer_mut().image_mut();
                for y in y0..y1 {
                    for x in x0..x1 {
                        img.put_pixel(x, y, c);
                    }
                }
            }
            4 => doc.add_layer(),
            5 => doc.duplicate_layer(),
            6 => {
                doc.delete_layer();
            }
            7 => {
                if r.next(2) == 0 {
                    doc.move_layer_up();
                } else {
                    doc.move_layer_down();
                }
            }
            8 => {
                doc.merge_down();
            }
            9 => {
                if doc.width > 120 && doc.height > 120 {
                    let (x0, y0) = (r.next(40), r.next(40));
                    doc.crop(x0, y0, doc.width - r.next(40), doc.height - r.next(40));
                }
            }
            10 => {
                let n = doc.layers.len() as u32;
                let (i, opacity, visible, name) =
                    (r.next(n) as usize, r.next(101) as f32 / 100.0, r.next(3) != 0, format!("n{}", r.next(1000)));
                let l = &mut doc.layers[i];
                l.opacity = opacity;
                l.visible = visible;
                l.name = name;
                doc.active = r.next(n) as usize;
            }
            _ => {
                doc.selection = if r.next(2) == 0 {
                    None
                } else {
                    let (w, h) = (doc.width, doc.height);
                    Some(Mask::rect(w, h, (r.next(w) as f32, r.next(h) as f32), (w as f32, h as f32)))
                };
            }
        }
    }

    #[test]
    fn deshacer_y_rehacer_secuencias_al_azar() {
        for seed in [1_u64, 7, 42] {
            let mut r = Lcg(seed);
            let mut doc = Document::new(600, 400, Some([255, 255, 255, 255]));
            let mut h = History::default();
            let mut states = vec![doc.clone()];
            for _ in 0..150 {
                h.push(&doc);
                random_op(&mut doc, &mut r);
                if r.next(3) == 0 {
                    h.seal(&doc);
                }
                states.push(doc.clone());
            }
            for k in (0..150).rev() {
                assert!(h.undo(&mut doc), "semilla {seed}: deshacer {k}");
                assert_same(&doc, &states[k], &format!("semilla {seed}, deshacer hasta {k}"));
            }
            assert!(!h.can_undo());
            for (k, state) in states.iter().enumerate().skip(1) {
                assert!(h.redo(&mut doc));
                assert_same(&doc, state, &format!("semilla {seed}, rehacer hasta {k}"));
            }
            assert!(!h.can_redo());
        }
    }

    #[test]
    fn un_trazo_pequeno_cuesta_un_bloque() {
        let mut doc = Document::new(4096, 4096, Some([255, 255, 255, 255]));
        let mut h = History::default();
        h.push(&doc);
        doc.layers[0].image_mut().put_pixel(10, 10, Rgba([0, 0, 0, 255]));
        h.seal(&doc);
        assert!(h.bytes() <= 256 * 256 * 4 + 4096, "{}", h.bytes());
    }

    #[test]
    fn doscientos_pasos() {
        let mut doc = Document::new(512, 512, None);
        let mut h = History::default();
        for i in 0..250_u32 {
            h.push(&doc);
            doc.layers[0].image_mut().put_pixel(i, i, Rgba([1, 2, 3, 255]));
        }
        let mut n = 0;
        while h.undo(&mut doc) {
            n += 1;
        }
        assert_eq!(n, MAX_STEPS);
    }

    #[test]
    fn presupuesto_de_memoria() {
        // Capa de 8192×8192 (256 MB) cambiada entera cinco veces: 4 pasos
        // sellados pasarían de 768 MB.
        let mut doc = Document::new(8192, 8192, None);
        let mut h = History::default();
        for v in 1..=5_u8 {
            h.push(&doc);
            for p in doc.layers[0].image_mut().pixels_mut() {
                *p = Rgba([v, v, v, 255]);
            }
        }
        h.seal(&doc);
        assert!(h.bytes() <= MEMORY_BUDGET, "{}", h.bytes());
        assert!(h.can_undo());
        assert!(h.undo(&mut doc));
        assert_eq!(doc.layers[0].pixels.get_pixel(0, 0).0, [4, 4, 4, 255]);
    }
}
