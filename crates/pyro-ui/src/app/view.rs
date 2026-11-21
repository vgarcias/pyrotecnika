//! Vista del lienzo: transformaciones de coordenadas, zoom y texturas.

use eframe::egui::{self, Color32, ColorImage, Pos2, Rect, TextureOptions, Vec2};

use pyro_core::selection::Mask;

use super::canvas_tiles::Dirty;
use super::PyroApp;
use crate::design::tokens::{CHECKER_A, CHECKER_B, OVERLAY_DARK, OVERLAY_LIGHT};

impl PyroApp {
    pub(super) fn origin(&self, rect: Rect) -> Pos2 {
        let size = Vec2::new(self.tab.doc.width as f32, self.tab.doc.height as f32) * self.tab.zoom;
        rect.center() + self.tab.pan - size / 2.0
    }

    pub(super) fn screen_to_img(&self, rect: Rect, p: Pos2) -> (f32, f32) {
        let o = self.origin(rect);
        ((p.x - o.x) / self.tab.zoom, (p.y - o.y) / self.tab.zoom)
    }

    pub(super) fn img_to_screen(&self, rect: Rect, p: (f32, f32)) -> Pos2 {
        self.origin(rect) + Vec2::new(p.0, p.1) * self.tab.zoom
    }

    pub(super) fn zoom_at(&mut self, rect: Rect, anchor: Pos2, factor: f32) {
        let img = self.screen_to_img(rect, anchor);
        self.tab.zoom = (self.tab.zoom * factor).clamp(0.02, 64.0);
        let moved = self.img_to_screen(rect, img);
        self.tab.pan += anchor - moved;
    }

    pub(super) fn fit(&mut self, rect: Rect) {
        let zx = (rect.width() - 40.0).max(10.0) / self.tab.doc.width as f32;
        let zy = (rect.height() - 40.0).max(10.0) / self.tab.doc.height as f32;
        self.tab.zoom = zx.min(zy).clamp(0.02, 1.0);
        self.tab.pan = Vec2::ZERO;
        self.tab.fit_pending = false;
    }

    pub(super) fn refresh_textures(&mut self, ctx: &egui::Context) {
        let opts = if self.tab.zoom >= 1.0 { TextureOptions::NEAREST } else { TextureOptions::LINEAR };
        let size = (self.tab.doc.width, self.tab.doc.height);
        let doc = &self.tab.doc;
        self.tab.canvas.update(
            ctx,
            size,
            opts,
            std::mem::replace(&mut self.tab.dirty, Dirty::None),
            |x0, y0, x1, y1| {
                let part = doc.composite_rect(x0, y0, x1, y1);
                ColorImage::from_rgba_unmultiplied([part.width() as usize, part.height() as usize], part.as_raw())
            },
        );
        if self.tab.sel_dirty {
            match &self.tab.doc.selection {
                Some(m) => {
                    let overlay = selection_overlay(m);
                    self.tab.sel_tiles.update(ctx, size, TextureOptions::NEAREST, Dirty::All, |x0, y0, x1, y1| {
                        crop_color_image(&overlay, x0, y0, x1, y1)
                    });
                }
                None => self.tab.sel_tiles.clear(),
            }
            self.tab.sel_dirty = false;
        }
        if self.checker.is_none() {
            let mut img = ColorImage::new([16, 16], CHECKER_A);
            for y in 0..16 {
                for x in 0..16 {
                    if (x / 8 + y / 8) % 2 == 0 {
                        img.pixels[y * 16 + x] = CHECKER_B;
                    }
                }
            }
            self.checker = Some(ctx.load_texture("cuadricula", img, TextureOptions::NEAREST_REPEAT));
        }
    }
}

/// Rectángulo `x0..x1` × `y0..y1` de una imagen ya en memoria.
fn crop_color_image(img: &ColorImage, x0: u32, y0: u32, x1: u32, y1: u32) -> ColorImage {
    let (w, h) = ((x1 - x0) as usize, (y1 - y0) as usize);
    let stride = img.size[0];
    let mut pixels = Vec::with_capacity(w * h);
    for y in y0 as usize..y1 as usize {
        let start = y * stride + x0 as usize;
        pixels.extend_from_slice(&img.pixels[start..start + w]);
    }
    ColorImage { size: [w, h], pixels }
}

/// Contorno de la selección con «hormigas» blancas y negras.
pub(super) fn selection_overlay(mask: &Mask) -> ColorImage {
    let (w, h) = (mask.width as usize, mask.height as usize);
    let mut img = ColorImage::new([w, h], Color32::TRANSPARENT);
    let on = |i: usize| mask.data[i] >= 128;
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            if !on(i) {
                continue;
            }
            let edge =
                x == 0 || y == 0 || x + 1 == w || y + 1 == h || !on(i - 1) || !on(i + 1) || !on(i - w) || !on(i + w);
            if edge {
                img.pixels[i] = if (x + y) / 4 % 2 == 0 { OVERLAY_DARK } else { OVERLAY_LIGHT };
            }
        }
    }
    img
}
