//! Lienzo: interacción con el puntero y dibujo de la imagen y las guías.

use eframe::egui::{
    self, Color32, CursorIcon, Key, PointerButton, Pos2, Rect, Sense, Stroke as UiStroke, StrokeKind as UiStrokeKind,
    Vec2,
};

use pyro_core::selection::Mask;

use super::tools::{Drag, Tool};
use super::PyroApp;
use crate::design::style::palette;
use crate::design::tokens::{OVERLAY_DARK, OVERLAY_LIGHT};
use crate::design::widgets::overlay_stroke;

impl PyroApp {
    pub(super) fn canvas_ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let (rect, resp) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, "Lienzo"));
        if self.tab.fit_pending && rect.width() > 50.0 {
            self.fit(rect);
        }

        if let Some(hover) = resp.hover_pos() {
            let scroll = ui.input(|i| i.raw_scroll_delta.y);
            if scroll != 0.0 {
                self.zoom_at(rect, hover, (scroll * 0.0015).exp());
            }
        }
        let space = ui.input(|i| i.key_down(Key::Space));
        let modifiers = ui.input(|i| i.modifiers);
        let panning = resp.dragged_by(PointerButton::Middle)
            || resp.dragged_by(PointerButton::Secondary)
            || (space && resp.dragged_by(PointerButton::Primary));
        if panning {
            self.tab.pan += resp.drag_delta();
        }

        if self.tab.filter_dialog.is_none() && !space {
            if resp.drag_started_by(PointerButton::Primary) {
                if let Some(start) = ui.input(|i| i.pointer.press_origin()).or(resp.interact_pointer_pos()) {
                    let p = self.screen_to_img(rect, start);
                    self.begin_drag(p);
                }
            }
            if resp.dragged_by(PointerButton::Primary) {
                if let Some(pos) = resp.interact_pointer_pos() {
                    let p = self.screen_to_img(rect, pos);
                    self.continue_drag(p);
                }
            }
            if resp.drag_stopped_by(PointerButton::Primary) {
                self.end_drag(modifiers);
            }
            if resp.clicked_by(PointerButton::Primary) {
                if let Some(pos) = resp.interact_pointer_pos() {
                    let p = self.screen_to_img(rect, pos);
                    self.click(p, modifiers, &ctx);
                }
            }
        } else if resp.drag_stopped_by(PointerButton::Primary) {
            self.end_drag(modifiers);
        }

        self.refresh_textures(&ctx);

        let painter = ui.painter_at(rect);
        let pal = palette(&ctx);
        painter.rect_filled(rect, 0.0, pal.bg_canvas);
        let canvas = Rect::from_min_size(
            self.origin(rect),
            Vec2::new(self.tab.doc.width as f32, self.tab.doc.height as f32) * self.tab.zoom,
        );
        if let Some(ch) = &self.checker {
            let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(canvas.width() / 16.0, canvas.height() / 16.0));
            painter.image(ch.id(), canvas, uv, Color32::WHITE);
        }
        self.tab.canvas.paint(&painter, canvas);
        painter.rect_stroke(canvas, 0.0, UiStroke::new(pal.stroke_width, pal.border_subtle), UiStrokeKind::Outside);
        if !self.tab.sel_tiles.is_empty() {
            self.tab.sel_tiles.paint(&painter, canvas);
            if self.tab.zoom < 1.0 {
                if let Some((x0, y0, x1, y1)) = self.tab.doc.selection.as_ref().and_then(Mask::bounds) {
                    let r = Rect::from_min_max(
                        self.img_to_screen(rect, (x0 as f32, y0 as f32)),
                        self.img_to_screen(rect, (x1 as f32, y1 as f32)),
                    );
                    overlay_stroke(&painter, 1.0, |s| egui::Shape::rect_stroke(r, 0.0, s, UiStrokeKind::Middle));
                }
            }
        }

        match &self.tab.drag {
            Drag::Shape { start, current } => {
                let r = Rect::from_two_pos(self.img_to_screen(rect, *start), self.img_to_screen(rect, *current));
                if self.tool == Tool::Ellipse {
                    let pts: Vec<Pos2> = (0..72)
                        .map(|i| {
                            let a = i as f32 / 72.0 * std::f32::consts::TAU;
                            r.center() + Vec2::new(a.cos() * r.width() / 2.0, a.sin() * r.height() / 2.0)
                        })
                        .collect();
                    overlay_stroke(&painter, 1.0, |s| egui::Shape::closed_line(pts.clone(), s));
                } else {
                    overlay_stroke(&painter, 1.0, |s| egui::Shape::rect_stroke(r, 0.0, s, UiStrokeKind::Middle));
                }
            }
            Drag::Lasso { points } => {
                let pts: Vec<Pos2> = points.iter().map(|p| self.img_to_screen(rect, *p)).collect();
                overlay_stroke(&painter, 1.0, |s| egui::Shape::line(pts.clone(), s));
            }
            _ => {}
        }

        self.hover_px = resp.hover_pos().map(|h| self.screen_to_img(rect, h));
        // El anillo del pincel sustituye al cursor solo si se ve: con un pincel
        // pequeño y poco zoom mediría menos de un píxel y el puntero se perdería.
        const MIN_RING: f32 = 4.0;
        let ring = self.brush.radius * self.tab.zoom;
        let blocked = self.tool.edits_pixels() && self.tab.filter_dialog.is_none() && self.layer_block().is_some();
        let show_ring = self.tool.is_brush() && !space && !blocked && ring >= MIN_RING;
        if let (true, Some(h)) = (show_ring, resp.hover_pos()) {
            painter.circle_stroke(h, ring, UiStroke::new(1.0_f32, OVERLAY_LIGHT));
            painter.circle_stroke(h, ring + 1.0, UiStroke::new(1.0_f32, OVERLAY_DARK));
        }
        if self.tool == Tool::Clone {
            if let Some(src) = self.tab.clone_source {
                // Durante el trazo, la marca de origen sigue al pincel.
                let following = match (&self.tab.drag, self.tab.clone_stroke_offset, resp.hover_pos()) {
                    (Drag::Paint { .. }, Some(o), Some(h)) => {
                        let cur = self.screen_to_img(rect, h);
                        Some((cur.0 + o.0, cur.1 + o.1))
                    }
                    _ => None,
                };
                let c = self.img_to_screen(rect, following.unwrap_or(src));
                for (a, b) in [(c - Vec2::X * 8.0, c + Vec2::X * 8.0), (c - Vec2::Y * 8.0, c + Vec2::Y * 8.0)] {
                    overlay_stroke(&painter, 1.5, |s| egui::Shape::line_segment([a, b], s));
                }
            }
        }

        let cursor = if panning || space {
            CursorIcon::Grab
        } else {
            match self.tool {
                _ if blocked => CursorIcon::NotAllowed,
                Tool::Move => CursorIcon::Move,
                _ if show_ring => CursorIcon::None,
                _ => CursorIcon::Crosshair,
            }
        };
        if resp.hovered() {
            ctx.set_cursor_icon(cursor);
        }
    }
}
