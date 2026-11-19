//! Colour helpers: `#rgb`, `#rrggbb` and `#rrggbbaa` hex colours, mixing and luminance.

use resvg::tiny_skia::Color;

pub fn hex_to_rgb(hex: &str) -> [f64; 3] {
    let mut h: String = hex.trim().trim_start_matches('#').to_string();
    if h.len() == 3 {
        h = h.chars().flat_map(|c| [c, c]).collect();
    }
    let n = u32::from_str_radix(h.get(0..6).unwrap_or(""), 16).unwrap_or(0);
    [((n >> 16) & 255) as f64, ((n >> 8) & 255) as f64, (n & 255) as f64]
}

pub fn rgb_to_hex(r: f64, g: f64, b: f64) -> String {
    let c = |v: f64| (v.clamp(0.0, 255.0)).round() as u8;
    format!("#{:02x}{:02x}{:02x}", c(r), c(g), c(b))
}

pub fn shade(hex: &str, k: f64) -> String {
    let [r, g, b] = hex_to_rgb(hex);
    rgb_to_hex(r * k, g * k, b * k)
}

pub fn mix(a: &str, b: &str, t: f64) -> String {
    let (x, y) = (hex_to_rgb(a), hex_to_rgb(b));
    rgb_to_hex(x[0] + (y[0] - x[0]) * t, x[1] + (y[1] - x[1]) * t, x[2] + (y[2] - x[2]) * t)
}

pub fn luminance(hex: &str) -> f64 {
    let [r, g, b] = hex_to_rgb(hex);
    (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255.0
}

/// a tiny-skia colour from `#rrggbb` and an alpha
pub fn color(hex: &str, alpha: f64) -> Color {
    let [r, g, b] = hex_to_rgb(hex);
    Color::from_rgba8(r as u8, g as u8, b as u8, (alpha.clamp(0.0, 1.0) * 255.0).round() as u8)
}

pub fn rgba(r: u8, g: u8, b: u8, a: f64) -> Color {
    Color::from_rgba8(r, g, b, (a.clamp(0.0, 1.0) * 255.0).round() as u8)
}

/// alpha of a `#rrggbbaa` colour (1 for `#rrggbb`)
pub fn hex_alpha(hex: &str) -> f64 {
    let h = hex.trim().trim_start_matches('#');
    if h.len() == 8 {
        u8::from_str_radix(&h[6..8], 16).map(|a| a as f64 / 255.0).unwrap_or(1.0)
    } else {
        1.0
    }
}

/// a colour with its alpha, for the drawing backends
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    /// 0…1
    pub a: f32,
}

impl Rgba {
    pub const BLACK: Rgba = Rgba { r: 0, g: 0, b: 0, a: 1.0 };
    pub const WHITE: Rgba = Rgba { r: 255, g: 255, b: 255, a: 1.0 };
    /// `#rrggbb` / `#rrggbbaa` with an extra opacity
    pub fn hex(hex: &str, opacity: f64) -> Rgba {
        let [r, g, b] = hex_to_rgb(hex);
        Rgba { r: r as u8, g: g as u8, b: b as u8, a: (hex_alpha(hex) * opacity).clamp(0.0, 1.0) as f32 }
    }
    pub fn with_alpha(self, a: f32) -> Rgba {
        Rgba { a, ..self }
    }
    pub fn skia(self) -> Color {
        Color::from_rgba8(self.r, self.g, self.b, (self.a.clamp(0.0, 1.0) * 255.0).round() as u8)
    }
}
