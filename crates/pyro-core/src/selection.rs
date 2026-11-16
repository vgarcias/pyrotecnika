//! Máscaras de selección (0 = fuera, 255 = dentro).

use image::RgbaImage;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectMode {
    Replace,
    Add,
    Subtract,
    Intersect,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mask {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

impl Mask {
    pub fn empty(width: u32, height: u32) -> Self {
        Self { width, height, data: vec![0; (width * height) as usize] }
    }

    pub fn full(width: u32, height: u32) -> Self {
        Self { width, height, data: vec![255; (width * height) as usize] }
    }

    #[inline]
    pub fn get(&self, x: u32, y: u32) -> u8 {
        self.data[(y * self.width + x) as usize]
    }

    /// Rectángulo entre dos esquinas (en coordenadas de imagen, puede salirse).
    pub fn rect(width: u32, height: u32, a: (f32, f32), b: (f32, f32)) -> Self {
        let mut m = Self::empty(width, height);
        let (x0, x1) = clamp_span(a.0.min(b.0), a.0.max(b.0), width);
        let (y0, y1) = clamp_span(a.1.min(b.1), a.1.max(b.1), height);
        for y in y0..y1 {
            let row = (y * width) as usize;
            m.data[row + x0 as usize..row + x1 as usize].fill(255);
        }
        m
    }

    /// Elipse inscrita en el rectángulo entre dos esquinas.
    pub fn ellipse(width: u32, height: u32, a: (f32, f32), b: (f32, f32)) -> Self {
        let mut m = Self::empty(width, height);
        let (l, r) = (a.0.min(b.0), a.0.max(b.0));
        let (t, bt) = (a.1.min(b.1), a.1.max(b.1));
        let (rx, ry) = ((r - l) / 2.0, (bt - t) / 2.0);
        if rx <= 0.0 || ry <= 0.0 {
            return m;
        }
        let (cx, cy) = (l + rx, t + ry);
        let (x0, x1) = clamp_span(l, r, width);
        let (y0, y1) = clamp_span(t, bt, height);
        for y in y0..y1 {
            for x in x0..x1 {
                let dx = (x as f32 + 0.5 - cx) / rx;
                let dy = (y as f32 + 0.5 - cy) / ry;
                if dx * dx + dy * dy <= 1.0 {
                    m.data[(y * width + x) as usize] = 255;
                }
            }
        }
        m
    }

    /// Polígono (lazo) con regla par-impar, muestreando centros de píxel.
    pub fn polygon(width: u32, height: u32, points: &[(f32, f32)]) -> Self {
        let mut m = Self::empty(width, height);
        if points.len() < 3 {
            return m;
        }
        let mut xs: Vec<f32> = Vec::new();
        for y in 0..height {
            let cy = y as f32 + 0.5;
            xs.clear();
            for i in 0..points.len() {
                let (x1, y1) = points[i];
                let (x2, y2) = points[(i + 1) % points.len()];
                if (y1 <= cy && cy < y2) || (y2 <= cy && cy < y1) {
                    xs.push(x1 + (cy - y1) * (x2 - x1) / (y2 - y1));
                }
            }
            xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            for pair in xs.chunks_exact(2) {
                let start = (pair[0] - 0.5).ceil().max(0.0) as u32;
                let end = ((pair[1] - 0.5).ceil().max(0.0) as u32).min(width);
                let row = (y * width) as usize;
                if start < end {
                    m.data[row + start as usize..row + end as usize].fill(255);
                }
            }
        }
        m
    }

    /// Varita mágica: píxeles cuyo color difiere del de (x, y) como mucho
    /// `tolerance` en cada canal RGBA.
    pub fn magic_wand(img: &RgbaImage, x: u32, y: u32, tolerance: u8, contiguous: bool) -> Self {
        let (w, h) = img.dimensions();
        let mut m = Self::empty(w, h);
        if x >= w || y >= h {
            return m;
        }
        let seed = img.get_pixel(x, y).0;
        let similar = |p: [u8; 4]| p.iter().zip(seed.iter()).all(|(a, b)| a.abs_diff(*b) <= tolerance);
        if !contiguous {
            for (i, p) in img.pixels().enumerate() {
                if similar(p.0) {
                    m.data[i] = 255;
                }
            }
            return m;
        }
        let mut stack = vec![(x, y)];
        while let Some((px, py)) = stack.pop() {
            let idx = (py * w + px) as usize;
            if m.data[idx] != 0 || !similar(img.get_pixel(px, py).0) {
                continue;
            }
            m.data[idx] = 255;
            if px > 0 {
                stack.push((px - 1, py));
            }
            if px + 1 < w {
                stack.push((px + 1, py));
            }
            if py > 0 {
                stack.push((px, py - 1));
            }
            if py + 1 < h {
                stack.push((px, py + 1));
            }
        }
        m
    }

    pub fn combine(&mut self, other: &Mask, mode: SelectMode) {
        for (a, &b) in self.data.iter_mut().zip(other.data.iter()) {
            *a = match mode {
                SelectMode::Replace => b,
                SelectMode::Add => (*a).max(b),
                SelectMode::Subtract => a.saturating_sub(b),
                SelectMode::Intersect => (*a).min(b),
            };
        }
    }

    pub fn invert(&mut self) {
        for v in &mut self.data {
            *v = 255 - *v;
        }
    }

    pub fn is_empty(&self) -> bool {
        self.data.iter().all(|&v| v == 0)
    }

    /// Caja envolvente (x0, y0, x1, y1), con x1/y1 exclusivos.
    pub fn bounds(&self) -> Option<(u32, u32, u32, u32)> {
        let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0, 0);
        for y in 0..self.height {
            for x in 0..self.width {
                if self.get(x, y) > 0 {
                    x0 = x0.min(x);
                    y0 = y0.min(y);
                    x1 = x1.max(x + 1);
                    y1 = y1.max(y + 1);
                }
            }
        }
        (x0 != u32::MAX).then_some((x0, y0, x1, y1))
    }

    pub fn shifted(&self, dx: i32, dy: i32) -> Mask {
        let mut out = Mask::empty(self.width, self.height);
        for y in 0..self.height as i32 {
            let sy = y - dy;
            if sy < 0 || sy >= self.height as i32 {
                continue;
            }
            for x in 0..self.width as i32 {
                let sx = x - dx;
                if sx >= 0 && sx < self.width as i32 {
                    out.data[(y as u32 * self.width + x as u32) as usize] = self.get(sx as u32, sy as u32);
                }
            }
        }
        out
    }

    /// Recorta la máscara a un rectángulo (x1/y1 exclusivos).
    pub fn cropped(&self, x0: u32, y0: u32, x1: u32, y1: u32) -> Mask {
        let mut out = Mask::empty(x1 - x0, y1 - y0);
        for y in y0..y1 {
            for x in x0..x1 {
                out.data[((y - y0) * out.width + (x - x0)) as usize] = self.get(x, y);
            }
        }
        out
    }
}

/// Convierte un intervalo continuo [a, b) a índices de píxel cuyos centros caen dentro.
fn clamp_span(a: f32, b: f32, max: u32) -> (u32, u32) {
    let start = (a - 0.5).ceil().clamp(0.0, max as f32) as u32;
    let end = (b - 0.5).ceil().clamp(0.0, max as f32) as u32;
    (start, end.max(start))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    fn count(m: &Mask) -> usize {
        m.data.iter().filter(|&&v| v > 0).count()
    }

    #[test]
    fn rect_selects_expected_pixels_and_clips() {
        let m = Mask::rect(10, 10, (2.0, 3.0), (5.0, 7.0));
        assert_eq!(count(&m), 3 * 4);
        assert_eq!(m.bounds(), Some((2, 3, 5, 7)));
        let clipped = Mask::rect(10, 10, (-5.0, -5.0), (20.0, 2.0));
        assert_eq!(count(&clipped), 20);
    }

    #[test]
    fn ellipse_is_inside_its_box() {
        let m = Mask::ellipse(20, 20, (0.0, 0.0), (20.0, 20.0));
        assert_eq!(m.get(10, 10), 255);
        assert_eq!(m.get(0, 0), 0);
        let area = count(&m) as f32;
        assert!((area - std::f32::consts::PI * 100.0).abs() < 20.0, "área {area}");
    }

    #[test]
    fn polygon_triangle() {
        let m = Mask::polygon(10, 10, &[(0.0, 0.0), (10.0, 0.0), (0.0, 10.0)]);
        assert_eq!(m.get(1, 1), 255);
        assert_eq!(m.get(8, 8), 0);
        assert!((count(&m) as i32 - 50).abs() <= 6);
    }

    #[test]
    fn wand_contiguous_vs_global() {
        let mut img = RgbaImage::from_pixel(6, 1, Rgba([255, 0, 0, 255]));
        img.put_pixel(3, 0, Rgba([0, 0, 255, 255]));
        let c = Mask::magic_wand(&img, 0, 0, 10, true);
        assert_eq!(count(&c), 3);
        let g = Mask::magic_wand(&img, 0, 0, 10, false);
        assert_eq!(count(&g), 5);
    }

    #[test]
    fn combine_modes() {
        let a = Mask::rect(4, 1, (0.0, 0.0), (2.0, 1.0));
        let b = Mask::rect(4, 1, (1.0, 0.0), (3.0, 1.0));
        let mut add = a.clone();
        add.combine(&b, SelectMode::Add);
        assert_eq!(add.data, vec![255, 255, 255, 0]);
        let mut sub = a.clone();
        sub.combine(&b, SelectMode::Subtract);
        assert_eq!(sub.data, vec![255, 0, 0, 0]);
        let mut int = a.clone();
        int.combine(&b, SelectMode::Intersect);
        assert_eq!(int.data, vec![0, 255, 0, 0]);
    }

    #[test]
    fn shift_and_crop() {
        let m = Mask::rect(4, 4, (0.0, 0.0), (1.0, 1.0));
        let s = m.shifted(2, 1);
        assert_eq!(s.get(2, 1), 255);
        assert_eq!(s.get(0, 0), 0);
        let c = s.cropped(2, 1, 4, 3);
        assert_eq!((c.width, c.height), (2, 2));
        assert_eq!(c.get(0, 0), 255);
    }
}
