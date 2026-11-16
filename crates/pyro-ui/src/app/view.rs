//! Vista del lienzo: transformaciones de coordenadas, zoom y texturas.

use eframe::egui::{self, Color32, ColorImage, Pos2, Rect, TextureOptions, Vec2};

use pyro_core::selection::Mask;

use super::PyroApp;

impl PyroApp {
    pub(super) fn origin(&self, rect: Rect) -> Pos2 {
        let size = Vec2::new(self.doc.width as f32, self.doc.height as f32) * self.zoom;
        rect.center() + self.pan - size / 2.0
    }

    pub(super) fn screen_to_img(&self, rect: Rect, p: Pos2) -> (f32, f32) {
        let o = self.origin(rect);
        ((p.x - o.x) / self.zoom, (p.y - o.y) / self.zoom)
    }

    pub(super) fn img_to_screen(&self, rect: Rect, p: (f32, f32)) -> Pos2 {
        self.origin(rect) + Vec2::new(p.0, p.1) * self.zoom
    }

    pub(super) fn zoom_at(&mut self, rect: Rect, anchor: Pos2, factor: f32) {
        let img = self.screen_to_img(rect, anchor);
        self.zoom = (self.zoom * factor).clamp(0.02, 64.0);
        let moved = self.img_to_screen(rect, img);
        self.pan += anchor - moved;
    }

    pub(super) fn fit(&mut self, rect: Rect) {
        let zx = (rect.width() - 40.0).max(10.0) / self.doc.width as f32;
        let zy = (rect.height() - 40.0).max(10.0) / self.doc.height as f32;
        self.zoom = zx.min(zy).clamp(0.02, 1.0);
        self.pan = Vec2::ZERO;
        self.fit_pending = false;
    }

    pub(super) fn refresh_textures(&mut self, ctx: &egui::Context) {
        let opts = if self.zoom >= 1.0 { TextureOptions::NEAREST } else { TextureOptions::LINEAR };
        if self.canvas_dirty || self.canvas_tex.is_none() || opts != self.canvas_opts {
            let comp = self.doc.composite();
            let img =
                ColorImage::from_rgba_unmultiplied([comp.width() as usize, comp.height() as usize], comp.as_raw());
            match &mut self.canvas_tex {
                Some(t) => t.set(img, opts),
                None => self.canvas_tex = Some(ctx.load_texture("lienzo", img, opts)),
            }
            self.canvas_opts = opts;
            self.canvas_dirty = false;
        }
        if self.sel_dirty {
            self.sel_tex = self
                .doc
                .selection
                .as_ref()
                .map(|m| ctx.load_texture("seleccion", selection_overlay(m), TextureOptions::NEAREST));
            self.sel_dirty = false;
        }
        if self.checker.is_none() {
            let mut img = ColorImage::new([16, 16], Color32::from_gray(204));
            for y in 0..16 {
                for x in 0..16 {
                    if (x / 8 + y / 8) % 2 == 0 {
                        img.pixels[y * 16 + x] = Color32::from_gray(255);
                    }
                }
            }
            self.checker = Some(ctx.load_texture("cuadricula", img, TextureOptions::NEAREST_REPEAT));
        }
    }
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
                img.pixels[i] = if (x + y) / 4 % 2 == 0 { Color32::BLACK } else { Color32::WHITE };
            }
        }
    }
    img
}
