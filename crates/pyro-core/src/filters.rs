//! Ajustes y filtros. Todos devuelven una imagen nueva; `apply_masked`
//! mezcla el resultado con el original según la selección.

use image::{imageops, Rgba, RgbaImage};

use crate::document::lerp_rgba;
use crate::selection::Mask;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Filter {
    BrightnessContrast { brightness: f32, contrast: f32 },
    HueSaturation { hue: f32, saturation: f32, lightness: f32 },
    Levels { in_black: f32, in_white: f32, gamma: f32, out_black: f32, out_white: f32 },
    GaussianBlur { sigma: f32 },
    Sharpen { sigma: f32, amount: f32 },
    Invert,
    Grayscale,
}

impl Filter {
    pub const BRIGHTNESS_CONTRAST: Filter = Filter::BrightnessContrast { brightness: 0.0, contrast: 0.0 };
    pub const HUE_SATURATION: Filter = Filter::HueSaturation { hue: 0.0, saturation: 0.0, lightness: 0.0 };
    pub const LEVELS: Filter =
        Filter::Levels { in_black: 0.0, in_white: 255.0, gamma: 1.0, out_black: 0.0, out_white: 255.0 };
    pub const GAUSSIAN_BLUR: Filter = Filter::GaussianBlur { sigma: 2.0 };
    pub const SHARPEN: Filter = Filter::Sharpen { sigma: 1.0, amount: 0.8 };

    pub fn name(&self) -> &'static str {
        match self {
            Filter::BrightnessContrast { .. } => "Brillo y contraste",
            Filter::HueSaturation { .. } => "Tono y saturación",
            Filter::Levels { .. } => "Niveles",
            Filter::GaussianBlur { .. } => "Desenfoque gaussiano",
            Filter::Sharpen { .. } => "Enfocar",
            Filter::Invert => "Invertir colores",
            Filter::Grayscale => "Escala de grises",
        }
    }

    pub fn has_params(&self) -> bool {
        !matches!(self, Filter::Invert | Filter::Grayscale)
    }

    pub fn apply(&self, img: &RgbaImage) -> RgbaImage {
        match *self {
            Filter::BrightnessContrast { brightness, contrast } => brightness_contrast(img, brightness, contrast),
            Filter::HueSaturation { hue, saturation, lightness } => hue_saturation(img, hue, saturation, lightness),
            Filter::Levels { in_black, in_white, gamma, out_black, out_white } => {
                levels(img, in_black, in_white, gamma, out_black, out_white)
            }
            Filter::GaussianBlur { sigma } => gaussian_blur(img, sigma),
            Filter::Sharpen { sigma, amount } => sharpen(img, sigma, amount),
            Filter::Invert => map_rgb(img, |c| 255.0 - c),
            Filter::Grayscale => map_pixels(img, |p| {
                let l = (0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32).round() as u8;
                [l, l, l, p[3]]
            }),
        }
    }
}

/// Mezcla `filtered` sobre `original` usando la selección (sin selección: todo).
pub fn apply_masked(original: &RgbaImage, filtered: &RgbaImage, sel: Option<&Mask>) -> RgbaImage {
    let Some(mask) = sel else {
        return filtered.clone();
    };
    let mut out = original.clone();
    for ((o, f), &m) in out.pixels_mut().zip(filtered.pixels()).zip(mask.data.iter()) {
        if m == 255 {
            *o = *f;
        } else if m > 0 {
            o.0 = lerp_rgba(o.0, f.0, m as f32 / 255.0);
        }
    }
    out
}

fn map_pixels(img: &RgbaImage, f: impl Fn([u8; 4]) -> [u8; 4]) -> RgbaImage {
    let mut out = img.clone();
    for p in out.pixels_mut() {
        p.0 = f(p.0);
    }
    out
}

fn map_rgb(img: &RgbaImage, f: impl Fn(f32) -> f32) -> RgbaImage {
    let lut: Vec<u8> = (0..256).map(|v| f(v as f32).round().clamp(0.0, 255.0) as u8).collect();
    map_pixels(img, |p| [lut[p[0] as usize], lut[p[1] as usize], lut[p[2] as usize], p[3]])
}

/// `brightness` y `contrast` en −100..100.
pub fn brightness_contrast(img: &RgbaImage, brightness: f32, contrast: f32) -> RgbaImage {
    let c = (contrast * 2.55).clamp(-254.0, 254.0);
    let factor = (259.0 * (c + 255.0)) / (255.0 * (259.0 - c));
    let b = brightness * 2.55;
    map_rgb(img, |v| (v - 128.0) * factor + 128.0 + b)
}

/// `hue` en grados (−180..180), `saturation` y `lightness` en −100..100.
pub fn hue_saturation(img: &RgbaImage, hue: f32, saturation: f32, lightness: f32) -> RgbaImage {
    map_pixels(img, |p| {
        let (h, s, l) = rgb_to_hsl(p[0], p[1], p[2]);
        let h = (h + hue / 360.0).rem_euclid(1.0);
        let s = (s * (1.0 + saturation / 100.0)).clamp(0.0, 1.0);
        let l = if lightness >= 0.0 { l + (1.0 - l) * lightness / 100.0 } else { l * (1.0 + lightness / 100.0) };
        let (r, g, b) = hsl_to_rgb(h, s, l.clamp(0.0, 1.0));
        [r, g, b, p[3]]
    })
}

pub fn levels(img: &RgbaImage, in_black: f32, in_white: f32, gamma: f32, out_black: f32, out_white: f32) -> RgbaImage {
    let span = (in_white - in_black).max(1.0);
    let inv_gamma = 1.0 / gamma.max(0.01);
    map_rgb(img, |v| {
        let t = ((v - in_black) / span).clamp(0.0, 1.0).powf(inv_gamma);
        out_black + t * (out_white - out_black)
    })
}

fn premultiply(img: &RgbaImage) -> RgbaImage {
    map_pixels(img, |p| {
        let a = p[3] as u32;
        [(p[0] as u32 * a / 255) as u8, (p[1] as u32 * a / 255) as u8, (p[2] as u32 * a / 255) as u8, p[3]]
    })
}

fn unpremultiply(img: &RgbaImage) -> RgbaImage {
    map_pixels(img, |p| {
        if p[3] == 0 {
            return [0, 0, 0, 0];
        }
        let a = p[3] as u32;
        let f = |c: u8| ((c as u32 * 255 + a / 2) / a).min(255) as u8;
        [f(p[0]), f(p[1]), f(p[2]), p[3]]
    })
}

pub fn gaussian_blur(img: &RgbaImage, sigma: f32) -> RgbaImage {
    if sigma <= 0.0 {
        return img.clone();
    }
    unpremultiply(&imageops::blur(&premultiply(img), sigma))
}

/// Máscara de enfoque: original + amount × (original − desenfocado).
pub fn sharpen(img: &RgbaImage, sigma: f32, amount: f32) -> RgbaImage {
    let blurred = gaussian_blur(img, sigma);
    let mut out = img.clone();
    for (o, b) in out.pixels_mut().zip(blurred.pixels()) {
        for c in 0..3 {
            let v = o.0[c] as f32 + amount * (o.0[c] as f32 - b.0[c] as f32);
            o.0[c] = v.round().clamp(0.0, 255.0) as u8;
        }
    }
    out
}

fn rgb_to_hsl(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    let (r, g, b) = (r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    let d = max - min;
    if d == 0.0 {
        return (0.0, 0.0, l);
    }
    let s = d / (1.0 - (2.0 * l - 1.0).abs());
    let h = if max == r {
        ((g - b) / d).rem_euclid(6.0)
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    } / 6.0;
    (h, s, l)
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (u8, u8, u8) {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = h * 6.0;
    let x = c * (1.0 - (hp.rem_euclid(2.0) - 1.0).abs());
    let (r, g, b) = match hp as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    let to = |v: f32| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    (to(r), to(g), to(b))
}

#[allow(dead_code)]
fn pixel(p: [u8; 4]) -> Rgba<u8> {
    Rgba(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(p: [u8; 4]) -> RgbaImage {
        RgbaImage::from_pixel(1, 1, Rgba(p))
    }

    #[test]
    fn neutral_settings_are_identity() {
        let img = one([12, 130, 250, 200]);
        assert_eq!(Filter::BRIGHTNESS_CONTRAST.apply(&img), img);
        assert_eq!(Filter::LEVELS.apply(&img), img);
        let hs = Filter::HUE_SATURATION.apply(&img).get_pixel(0, 0).0;
        for (out, orig) in hs.iter().zip(img.get_pixel(0, 0).0) {
            assert!((*out as i32 - orig as i32).abs() <= 1);
        }
    }

    #[test]
    fn invert_and_grayscale() {
        assert_eq!(Filter::Invert.apply(&one([0, 100, 255, 7])).get_pixel(0, 0).0, [255, 155, 0, 7]);
        let g = Filter::Grayscale.apply(&one([255, 0, 0, 255])).get_pixel(0, 0).0;
        assert_eq!(g[0], g[1]);
        assert_eq!(g[1], g[2]);
    }

    #[test]
    fn brightness_and_hue_shift() {
        let b = brightness_contrast(&one([100, 100, 100, 255]), 20.0, 0.0).get_pixel(0, 0).0;
        assert!(b[0] > 140);
        let h = hue_saturation(&one([255, 0, 0, 255]), 120.0, 0.0, 0.0).get_pixel(0, 0).0;
        assert_eq!(h, [0, 255, 0, 255]);
    }

    #[test]
    fn blur_keeps_size_and_masked_apply() {
        let mut img = RgbaImage::from_pixel(9, 9, Rgba([0, 0, 0, 255]));
        img.put_pixel(4, 4, Rgba([255, 255, 255, 255]));
        let blurred = gaussian_blur(&img, 1.5);
        assert_eq!(blurred.dimensions(), (9, 9));
        assert!(blurred.get_pixel(4, 4).0[0] < 255);
        assert!(blurred.get_pixel(5, 4).0[0] > 0);
        let sel = Mask::rect(9, 9, (0.0, 0.0), (1.0, 1.0));
        let inverted = Filter::Invert.apply(&img);
        let masked = apply_masked(&img, &inverted, Some(&sel));
        assert_eq!(masked.get_pixel(0, 0).0, [255, 255, 255, 255]);
        assert_eq!(masked.get_pixel(1, 1).0, [0, 0, 0, 255]);
    }

    #[test]
    fn blur_has_no_dark_fringe_on_transparent_edges() {
        let mut img = RgbaImage::from_pixel(9, 1, Rgba([0, 0, 0, 0]));
        for x in 0..4 {
            img.put_pixel(x, 0, Rgba([255, 255, 255, 255]));
        }
        let b = gaussian_blur(&img, 1.0);
        let edge = b.get_pixel(4, 0).0;
        assert!(edge[3] > 0);
        assert!(edge[0] > 240, "borde oscurecido: {edge:?}");
    }
}
