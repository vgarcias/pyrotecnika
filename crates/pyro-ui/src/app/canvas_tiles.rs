//! Texturas del lienzo partidas en bloques de como mucho 1024×1024: ninguna
//! pasa del máximo de la GPU, y un cambio solo sube el rectángulo tocado.

use eframe::egui::{self, Color32, ColorImage, Painter, Pos2, Rect, TextureHandle, TextureOptions};

/// Lado máximo de un bloque.
const MAX_TILE: u32 = 1024;

/// Qué parte del lienzo hay que volver a subir.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Dirty {
    None,
    /// Píxeles `x0..x1` × `y0..y1`.
    Rect {
        x0: u32,
        y0: u32,
        x1: u32,
        y1: u32,
    },
    All,
}

impl Dirty {
    pub(super) fn union(self, other: Dirty) -> Dirty {
        match (self, other) {
            (Dirty::All, _) | (_, Dirty::All) => Dirty::All,
            (Dirty::None, d) | (d, Dirty::None) => d,
            (Dirty::Rect { x0, y0, x1, y1 }, Dirty::Rect { x0: a0, y0: b0, x1: a1, y1: b1 }) => {
                Dirty::Rect { x0: x0.min(a0), y0: y0.min(b0), x1: x1.max(a1), y1: y1.max(b1) }
            }
        }
    }
}

struct Tile {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    texture: TextureHandle,
}

/// Un juego de texturas que cubre una imagen de `size` píxeles.
pub(super) struct Tiles {
    name: &'static str,
    size: (u32, u32),
    opts: TextureOptions,
    tiles: Vec<Tile>,
    /// Último cambio procesado y cuántos bloques subió (para las pruebas).
    #[cfg(test)]
    pub(super) last_dirty: Dirty,
    #[cfg(test)]
    pub(super) last_uploads: usize,
}

impl Tiles {
    pub(super) fn new(name: &'static str) -> Tiles {
        Tiles {
            name,
            size: (0, 0),
            opts: TextureOptions::NEAREST,
            tiles: Vec::new(),
            #[cfg(test)]
            last_dirty: Dirty::None,
            #[cfg(test)]
            last_uploads: 0,
        }
    }

    pub(super) fn clear(&mut self) {
        self.tiles.clear();
    }

    pub(super) fn is_empty(&self) -> bool {
        self.tiles.is_empty()
    }

    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.tiles.len()
    }

    #[cfg(test)]
    pub(super) fn max_side(&self) -> u32 {
        self.tiles.iter().map(|t| t.w.max(t.h)).max().unwrap_or(0)
    }

    /// Sube lo que indique `dirty`. `source(x0, y0, x1, y1)` da los píxeles
    /// de ese rectángulo. Si cambian las medidas o el filtrado, se rehace todo.
    pub(super) fn update(
        &mut self,
        ctx: &egui::Context,
        size: (u32, u32),
        opts: TextureOptions,
        dirty: Dirty,
        source: impl Fn(u32, u32, u32, u32) -> ColorImage,
    ) {
        let dirty = if size != self.size || opts != self.opts || self.tiles.is_empty() { Dirty::All } else { dirty };
        #[cfg(test)]
        {
            self.last_dirty = dirty;
            self.last_uploads = 0;
        }
        match dirty {
            Dirty::None => {}
            Dirty::All => {
                let side = MAX_TILE.min(ctx.input(|i| i.max_texture_side) as u32).max(1);
                self.size = size;
                self.opts = opts;
                self.tiles.clear();
                for y in (0..size.1).step_by(side as usize) {
                    for x in (0..size.0).step_by(side as usize) {
                        let (w, h) = (side.min(size.0 - x), side.min(size.1 - y));
                        let name = format!("{}-{x}-{y}", self.name);
                        let texture = ctx.load_texture(name, source(x, y, x + w, y + h), opts);
                        self.tiles.push(Tile { x, y, w, h, texture });
                        #[cfg(test)]
                        {
                            self.last_uploads += 1;
                        }
                    }
                }
            }
            Dirty::Rect { x0, y0, x1, y1 } => {
                let (x1, y1) = (x1.min(size.0), y1.min(size.1));
                for t in &mut self.tiles {
                    let (ax, ay) = (x0.max(t.x), y0.max(t.y));
                    let (bx, by) = (x1.min(t.x + t.w), y1.min(t.y + t.h));
                    if ax >= bx || ay >= by {
                        continue;
                    }
                    let pos = [(ax - t.x) as usize, (ay - t.y) as usize];
                    t.texture.set_partial(pos, source(ax, ay, bx, by), opts);
                    #[cfg(test)]
                    {
                        self.last_uploads += 1;
                    }
                }
            }
        }
    }

    /// Dibuja los bloques sobre `canvas` (el rectángulo en pantalla de la
    /// imagen entera).
    pub(super) fn paint(&self, painter: &Painter, canvas: Rect) {
        if self.size.0 == 0 || self.size.1 == 0 {
            return;
        }
        let (sx, sy) = (canvas.width() / self.size.0 as f32, canvas.height() / self.size.1 as f32);
        let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
        for t in &self.tiles {
            let min = canvas.min + egui::vec2(t.x as f32 * sx, t.y as f32 * sy);
            let max = canvas.min + egui::vec2((t.x + t.w) as f32 * sx, (t.y + t.h) as f32 * sy);
            painter.image(t.texture.id(), Rect::from_min_max(min, max), uv, Color32::WHITE);
        }
    }
}
