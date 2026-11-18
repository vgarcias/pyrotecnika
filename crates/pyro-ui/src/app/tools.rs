//! Herramientas del lienzo: selección, pintura, relleno y cuentagotas.

use eframe::egui::{self, Modifiers};
use image::RgbaImage;

use pyro_core::document::shift_pixels;
use pyro_core::paint::{self, Stroke, StrokeKind};
use pyro_core::selection::{Mask, SelectMode};

use super::{icons, Action, PyroApp};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Tool {
    Move,
    Marquee,
    Ellipse,
    Lasso,
    Wand,
    Brush,
    Eraser,
    Clone,
    Blur,
    Bucket,
    Eyedropper,
}

pub(crate) const TOOLS: [(Tool, &str); 11] = [
    (Tool::Move, "Mover"),
    (Tool::Marquee, "Marco"),
    (Tool::Ellipse, "Elipse"),
    (Tool::Lasso, "Lazo"),
    (Tool::Wand, "Varita mágica"),
    (Tool::Brush, "Pincel"),
    (Tool::Eraser, "Borrador"),
    (Tool::Clone, "Tampón de clonar"),
    (Tool::Blur, "Desenfocar"),
    (Tool::Bucket, "Bote de pintura"),
    (Tool::Eyedropper, "Cuentagotas"),
];

impl Tool {
    pub(super) fn is_brush(self) -> bool {
        matches!(self, Tool::Brush | Tool::Eraser | Tool::Clone | Tool::Blur)
    }

    /// Herramientas que cambian los píxeles de la capa activa.
    pub(super) fn edits_pixels(self) -> bool {
        self.is_brush() || matches!(self, Tool::Move | Tool::Bucket)
    }

    pub(super) fn is_selection(self) -> bool {
        matches!(self, Tool::Marquee | Tool::Ellipse | Tool::Lasso | Tool::Wand)
    }

    pub(super) fn stroke_kind(self) -> StrokeKind {
        match self {
            Tool::Eraser => StrokeKind::Erase,
            Tool::Clone => StrokeKind::Clone,
            Tool::Blur => StrokeKind::Blur,
            _ => StrokeKind::Paint,
        }
    }

    /// Glifo de la caja de herramientas (fuente de iconos Lucide).
    pub(super) fn icon(self) -> char {
        match self {
            Tool::Move => icons::MOVE,
            Tool::Marquee => icons::SQUARE_DASHED,
            Tool::Ellipse => icons::CIRCLE_DASHED,
            Tool::Lasso => icons::LASSO,
            Tool::Wand => icons::WAND_SPARKLES,
            Tool::Brush => icons::PAINTBRUSH,
            Tool::Eraser => icons::ERASER,
            Tool::Clone => icons::STAMP,
            Tool::Blur => icons::DROPLET,
            Tool::Bucket => icons::PAINT_BUCKET,
            Tool::Eyedropper => icons::PIPETTE,
        }
    }

    /// Nombre de la herramienta, sin atajo (es también su nombre accesible).
    pub(super) fn label(self) -> &'static str {
        TOOLS.iter().find(|(t, _)| *t == self).map_or("", |(_, l)| l)
    }
}

pub(super) enum Drag {
    None,
    Move { base: RgbaImage, base_sel: Option<Mask>, start: (f32, f32), last: (i32, i32) },
    Shape { start: (f32, f32), current: (f32, f32) },
    Lasso { points: Vec<(f32, f32)> },
    Paint { stroke: Stroke, last: (f32, f32) },
}

impl PyroApp {
    pub(super) fn apply_selection(&mut self, mask: Mask, modifiers: Modifiers) {
        let mode = if modifiers.shift {
            SelectMode::Add
        } else if modifiers.alt {
            SelectMode::Subtract
        } else {
            self.select_mode
        };
        self.history.push(&self.doc);
        let result = match (mode, self.doc.selection.take()) {
            (SelectMode::Replace, _) | (SelectMode::Add, None) => Some(mask),
            (_, None) => None,
            (mode, Some(mut current)) => {
                current.combine(&mask, mode);
                Some(current)
            }
        };
        self.doc.selection = result.filter(|m| !m.is_empty());
        self.sel_dirty = true;
    }

    // ---------- Herramientas sobre el lienzo ----------

    pub(super) fn inside(&self, p: (f32, f32)) -> Option<(u32, u32)> {
        (p.0 >= 0.0 && p.1 >= 0.0 && p.0 < self.doc.width as f32 && p.1 < self.doc.height as f32)
            .then_some((p.0 as u32, p.1 as u32))
    }

    pub(super) fn begin_drag(&mut self, p: (f32, f32)) {
        if !matches!(self.drag, Drag::None) {
            return;
        }
        if self.tool.edits_pixels() {
            if let Some(reason) = self.layer_block() {
                self.warn(reason);
                return;
            }
        }
        self.drag = match self.tool {
            Tool::Move => {
                self.history.push(&self.doc);
                Drag::Move {
                    base: self.doc.active_layer().pixels.clone(),
                    base_sel: self.doc.selection.clone(),
                    start: p,
                    last: (0, 0),
                }
            }
            Tool::Marquee | Tool::Ellipse => Drag::Shape { start: p, current: p },
            Tool::Lasso => Drag::Lasso { points: vec![p] },
            t if t.is_brush() => {
                let offset = if t == Tool::Clone {
                    match self.clone_source {
                        Some(src) => (src.0 - p.0, src.1 - p.1),
                        None => {
                            self.warn("Tampón: primero Alt+clic para fijar el punto de origen.");
                            return;
                        }
                    }
                } else {
                    (0.0, 0.0)
                };
                self.history.push(&self.doc);
                self.clone_stroke_offset = (t == Tool::Clone).then_some(offset);
                let mut stroke = Stroke::new(t.stroke_kind(), self.doc.active_layer().pixels.clone(), offset);
                let active = self.doc.active;
                stroke.dab(&mut self.doc.layers[active].pixels, self.doc.selection.as_ref(), p.0, p.1, &self.brush);
                self.canvas_dirty = true;
                Drag::Paint { stroke, last: p }
            }
            _ => Drag::None,
        };
    }

    pub(super) fn continue_drag(&mut self, p: (f32, f32)) {
        match &mut self.drag {
            Drag::None => {}
            Drag::Move { base, base_sel, start, last } => {
                let d = ((p.0 - start.0).round() as i32, (p.1 - start.1).round() as i32);
                if d != *last {
                    *last = d;
                    let active = self.doc.active;
                    self.doc.layers[active].pixels = shift_pixels(base, base_sel.as_ref(), d.0, d.1);
                    self.doc.selection = base_sel.as_ref().map(|m| m.shifted(d.0, d.1));
                    self.canvas_dirty = true;
                    self.sel_dirty = true;
                }
            }
            Drag::Shape { current, .. } => *current = p,
            Drag::Lasso { points } => {
                let far = points.last().is_none_or(|l| (l.0 - p.0).abs() + (l.1 - p.1).abs() >= 1.0);
                if far {
                    points.push(p);
                }
            }
            Drag::Paint { stroke, last } => {
                let spacing = (self.brush.radius * 0.25).max(1.0);
                let active = self.doc.active;
                for pt in paint::stroke_points(*last, p, spacing) {
                    stroke.dab(
                        &mut self.doc.layers[active].pixels,
                        self.doc.selection.as_ref(),
                        pt.0,
                        pt.1,
                        &self.brush,
                    );
                    *last = pt;
                }
                self.canvas_dirty = true;
            }
        }
    }

    pub(super) fn end_drag(&mut self, modifiers: Modifiers) {
        let (w, h) = (self.doc.width, self.doc.height);
        match std::mem::replace(&mut self.drag, Drag::None) {
            Drag::Shape { start, current } => {
                let tiny = (start.0 - current.0).abs() < 1.0 && (start.1 - current.1).abs() < 1.0;
                if tiny {
                    return;
                }
                let mask = if self.tool == Tool::Ellipse {
                    Mask::ellipse(w, h, start, current)
                } else {
                    Mask::rect(w, h, start, current)
                };
                self.apply_selection(mask, modifiers);
            }
            Drag::Lasso { points } if points.len() >= 3 => {
                self.apply_selection(Mask::polygon(w, h, &points), modifiers);
            }
            _ => {}
        }
    }

    pub(super) fn click(&mut self, p: (f32, f32), modifiers: Modifiers, ctx: &egui::Context) {
        match self.tool {
            Tool::Clone if modifiers.alt => {
                self.clone_source = Some(p);
                self.info(format!("Origen del tampón: {:.0}, {:.0}", p.0, p.1));
            }
            t if t.is_brush() => {
                self.begin_drag(p);
                self.end_drag(modifiers);
            }
            Tool::Wand => {
                if let Some((x, y)) = self.inside(p) {
                    let mask =
                        Mask::magic_wand(&self.doc.active_layer().pixels, x, y, self.tolerance, self.wand_contiguous);
                    self.apply_selection(mask, modifiers);
                }
            }
            Tool::Bucket => {
                if let Some(reason) = self.layer_block() {
                    self.warn(reason);
                    return;
                }
                if let Some((x, y)) = self.inside(p) {
                    self.history.push(&self.doc);
                    let active = self.doc.active;
                    paint::flood_fill(
                        &mut self.doc.layers[active].pixels,
                        self.doc.selection.as_ref(),
                        x,
                        y,
                        self.tolerance,
                        self.brush.color,
                        self.brush.opacity,
                    );
                    self.canvas_dirty = true;
                }
            }
            Tool::Eyedropper => {
                if let Some((x, y)) = self.inside(p) {
                    let c = self.doc.composite().get_pixel(x, y).0;
                    if c[3] > 0 {
                        self.brush.color = [c[0], c[1], c[2], 255];
                    }
                }
            }
            Tool::Marquee | Tool::Ellipse | Tool::Lasso if !modifiers.shift && !modifiers.alt => {
                self.run(Action::Deselect, ctx);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nombre_y_atajo_separados() {
        assert_eq!(Tool::Brush.label(), "Pincel");
        assert!(TOOLS.iter().all(|(_, l)| !l.contains('(')));
    }
}
