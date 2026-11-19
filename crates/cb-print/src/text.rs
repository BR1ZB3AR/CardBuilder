//! Text: the bundled fonts, shaped with rustybuzz (kerning, ligatures) and turned into outlines.
//!
//! A text layer has a font size in points and a box width. Text is laid out at BASE units per
//! em, then scaled to its size. With wrapping on, lines break between words at the box width;
//! with wrapping off, a line wider than the box shrinks the whole text to fit (names from a
//! spreadsheet never run off the card). Weights a font doesn't have are synthesised like a
//! browser does (outline emboldened, or slanted for italic).

use cb_core::doc::{Align, Layer};
use resvg::tiny_skia::{Path, PathBuilder};
use rustybuzz::{ttf_parser, Face, UnicodeBuffer, Variation};

/// units per em text is laid out at before scaling to its size
pub const BASE: f64 = 100.0;

macro_rules! font {
    ($f:literal) => {
        include_bytes!(concat!("../../cardbuilder/assets/fonts/", $f))
    };
}

pub const INTER_REGULAR: &[u8] = font!("Inter-Regular.ttf");
pub const INTER_SEMIBOLD: &[u8] = font!("Inter-SemiBold.ttf");
const INTER_VAR: &[u8] = font!("Inter-Variable.ttf");
const MONTSERRAT: &[u8] = font!("Montserrat-Variable.ttf");
const SPACE_GROTESK: &[u8] = font!("SpaceGrotesk-Variable.ttf");
const OSWALD: &[u8] = font!("Oswald-Variable.ttf");
const PLAYFAIR: &[u8] = font!("PlayfairDisplay-Variable.ttf");
const PLAYFAIR_ITALIC: &[u8] = font!("PlayfairDisplay-Italic-Variable.ttf");
const LORA: &[u8] = font!("Lora-Variable.ttf");
const CORMORANT: &[u8] = font!("CormorantGaramond-Variable.ttf");
const GREAT_VIBES: &[u8] = font!("GreatVibes-Regular.ttf");
const DANCING: &[u8] = font!("DancingScript-Variable.ttf");
const PACIFICO: &[u8] = font!("Pacifico-Regular.ttf");
const BEBAS: &[u8] = font!("BebasNeue-Regular.ttf");
const ANTON: &[u8] = font!("Anton-Regular.ttf");
const BANGERS: &[u8] = font!("Bangers-Regular.ttf");
const PERMANENT_MARKER: &[u8] = font!("PermanentMarker-Regular.ttf");
const SPACE_MONO: &[u8] = font!("SpaceMono-Regular.ttf");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FontKind {
    Sans,
    Serif,
    Script,
    Display,
    Mono,
}

impl FontKind {
    pub fn name(self) -> &'static str {
        match self {
            FontKind::Sans => "Sans serif",
            FontKind::Serif => "Serif",
            FontKind::Script => "Script",
            FontKind::Display => "Display",
            FontKind::Mono => "Monospace",
        }
    }
}

/// the fonts offered in the editor, by kind
pub const FONTS: [(&str, FontKind); 15] = [
    ("Inter", FontKind::Sans),
    ("Montserrat", FontKind::Sans),
    ("Space Grotesk", FontKind::Sans),
    ("Oswald", FontKind::Sans),
    ("Playfair Display", FontKind::Serif),
    ("Lora", FontKind::Serif),
    ("Cormorant Garamond", FontKind::Serif),
    ("Great Vibes", FontKind::Script),
    ("Dancing Script", FontKind::Script),
    ("Pacifico", FontKind::Script),
    ("Bebas Neue", FontKind::Display),
    ("Anton", FontKind::Display),
    ("Bangers", FontKind::Display),
    ("Permanent Marker", FontKind::Display),
    ("Space Mono", FontKind::Mono),
];

/// raw font file for a family (UI previews use these too)
pub fn font_bytes(family: &str, italic: bool) -> &'static [u8] {
    match family {
        "Montserrat" => MONTSERRAT,
        "Space Grotesk" => SPACE_GROTESK,
        "Oswald" => OSWALD,
        "Playfair Display" if italic => PLAYFAIR_ITALIC,
        "Playfair Display" => PLAYFAIR,
        "Lora" => LORA,
        "Cormorant Garamond" => CORMORANT,
        "Great Vibes" => GREAT_VIBES,
        "Dancing Script" => DANCING,
        "Pacifico" => PACIFICO,
        "Bebas Neue" => BEBAS,
        "Anton" => ANTON,
        "Bangers" => BANGERS,
        "Permanent Marker" => PERMANENT_MARKER,
        "Space Mono" => SPACE_MONO,
        _ => INTER_VAR,
    }
}

/// a face at a weight, with what has to be synthesised
pub struct FontPick {
    pub face: Face<'static>,
    pub fake_bold: bool,
    pub fake_italic: bool,
}

pub fn pick(family: &str, weight: f64, italic: bool) -> FontPick {
    let bytes = font_bytes(family, italic);
    let mut face = Face::from_slice(bytes, 0).unwrap_or_else(|| Face::from_slice(INTER_VAR, 0).expect("bundled font"));
    let wght = ttf_parser::Tag::from_bytes(b"wght");
    let axis = face.variation_axes().into_iter().find(|a| a.tag == wght);
    if let Some(a) = axis {
        face.set_variations(&[Variation { tag: wght, value: (weight as f32).clamp(a.min_value, a.max_value) }]);
    }
    // a variable font too light to reach the weight (e.g. Cormorant tops out at 700) is fine; a
    // single-weight font is emboldened from semi-bold up
    FontPick { face, fake_bold: axis.is_none() && weight >= 600.0, fake_italic: italic && family != "Playfair Display" }
}

struct Glyph {
    /// 0 = primary face, 1 = Inter fallback
    face: usize,
    id: u16,
    x: f64,
}

pub struct Line {
    pub width: f64,
    glyphs: Vec<Glyph>,
}

/// a laid-out text block, in BASE units
pub struct TextBlock {
    /// widest line
    pub w: f64,
    pub h: f64,
    pub lines: Vec<Line>,
    pub line_h: f64,
    /// from the middle of a line down to its baseline
    pub mid_to_base: f64,
    pick: FontPick,
    fallback: Face<'static>,
    scale: f64,
    fallback_scale: f64,
}

/// shape one line; glyphs missing from the font come from Inter (like a browser's fallback)
fn shape_line(text: &str, f: &FontPick, fallback: &Face<'static>, scale: f64, fscale: f64, spacing: f64) -> Line {
    let mut buf = UnicodeBuffer::new();
    buf.push_str(text);
    let out = rustybuzz::shape(&f.face, &[], buf);
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut glyphs = Vec::new();
    let mut pen = 0.0;
    let infos = out.glyph_infos();
    let pos = out.glyph_positions();
    for (k, (info, p)) in infos.iter().zip(pos).enumerate() {
        let next_cluster = infos.get(k + 1).map(|n| n.cluster);
        let new_cluster_next = next_cluster != Some(info.cluster);
        if info.glyph_id == 0 {
            let start = info.cluster as usize;
            let end = next_cluster.map(|c| c as usize).filter(|&c| c > start).unwrap_or(text.len());
            for &(_, ch) in chars.iter().filter(|(i, _)| *i >= start && *i < end) {
                if let Some(gid) = fallback.glyph_index(ch) {
                    glyphs.push(Glyph { face: 1, id: gid.0, x: pen });
                    pen += fallback.glyph_hor_advance(gid).unwrap_or(0) as f64 * fscale + spacing;
                } else if !ch.is_whitespace() {
                    pen += BASE * 0.5 + spacing;
                }
            }
            continue;
        }
        glyphs.push(Glyph { face: 0, id: info.glyph_id as u16, x: pen + p.x_offset as f64 * scale });
        pen += p.x_advance as f64 * scale;
        if new_cluster_next {
            pen += spacing;
        }
    }
    Line { width: pen, glyphs }
}

/// break a paragraph into lines no wider than `max` (BASE units); a single long word stays whole
fn wrap_paragraph(text: &str, max: f64, measure: &dyn Fn(&str) -> f64) -> Vec<String> {
    let mut lines = Vec::new();
    let mut cur = String::new();
    for word in text.split(' ') {
        let candidate = if cur.is_empty() { word.to_string() } else { format!("{cur} {word}") };
        if !cur.is_empty() && measure(&candidate) > max {
            lines.push(std::mem::take(&mut cur));
            cur = word.to_string();
        } else {
            cur = candidate;
        }
    }
    lines.push(cur);
    lines
}

/// the font size of a text layer, mm per em
pub fn em_mm(l: &Layer) -> f64 {
    l.size.unwrap_or(10.0) * cb_core::MM_PER_PT
}

/// Lay out `text` in the font of layer `l`. With wrapping on, lines break at the box width.
pub fn layout(l: &Layer, text: &str) -> TextBlock {
    let family = l.font.as_deref().unwrap_or("Inter");
    let pick = pick(family, l.weight.unwrap_or(400.0), l.italic.unwrap_or(false));
    let fallback = Face::from_slice(INTER_VAR, 0).expect("bundled font");
    let scale = BASE / pick.face.units_per_em() as f64;
    let fscale = BASE / fallback.units_per_em() as f64;
    let spacing = l.letter_spacing.unwrap_or(0.0) * BASE;
    let wrap_at = if l.wrap.unwrap_or(false) { Some(l.w / em_mm(l) * BASE) } else { None };
    let mut raw: Vec<String> = Vec::new();
    for para in text.split('\n') {
        match wrap_at {
            Some(max) => raw.extend(wrap_paragraph(para, max, &|s| shape_line(s, &pick, &fallback, scale, fscale, spacing).width)),
            None => raw.push(para.to_string()),
        }
    }
    let lines: Vec<Line> = raw
        .iter()
        .map(|s| {
            let mut line = shape_line(if s.is_empty() { " " } else { s }, &pick, &fallback, scale, fscale, spacing);
            if s.is_empty() {
                line.glyphs.clear();
            }
            line.width = line.width.max(0.0);
            line
        })
        .collect();
    let line_h = BASE * l.line_height.unwrap_or(1.2);
    let w = lines.iter().map(|l| l.width).fold(1.0, f64::max);
    let h = (line_h * lines.len() as f64).max(1.0);
    let asc = pick.face.ascender() as f64 * scale;
    let desc = -(pick.face.descender() as f64) * scale;
    TextBlock { w, h, lines, line_h, mid_to_base: (asc - desc) / 2.0, pick, fallback, scale, fallback_scale: fscale }
}

/// how text sits in its layer box
pub struct Fitted {
    pub block: TextBlock,
    /// mm per BASE unit (font size, shrunk if the text had to fit the box)
    pub k: f64,
    /// box height, mm
    pub h: f64,
}

/// lay out a text layer's text and fit it to the box width
pub fn fit(l: &Layer, text: &str) -> Fitted {
    let block = layout(l, text);
    let mut k = em_mm(l) / BASE;
    if !l.wrap.unwrap_or(false) && block.w * k > l.w && block.w > 0.0 {
        k = l.w / block.w;
    }
    let h = block.h * k;
    Fitted { block, k, h }
}

/// natural width of a text layer's text at its font size, mm (no wrapping or shrinking)
pub fn natural_width(l: &Layer, text: &str) -> f64 {
    let mut probe = l.clone();
    probe.wrap = Some(false);
    layout(&probe, text).w * em_mm(l) / BASE
}

struct Outline<'a> {
    pb: &'a mut PathBuilder,
    x: f64,
    y: f64,
    s: f64,
    skew: f64,
}
impl Outline<'_> {
    fn p(&self, x: f32, y: f32) -> (f32, f32) {
        let (x, y) = (x as f64, y as f64);
        ((self.x + (x + self.skew * y) * self.s) as f32, (self.y - y * self.s) as f32)
    }
}
impl ttf_parser::OutlineBuilder for Outline<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        let (a, b) = self.p(x, y);
        self.pb.move_to(a, b);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        let (a, b) = self.p(x, y);
        self.pb.line_to(a, b);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (a, b) = self.p(x1, y1);
        let (c, d) = self.p(x, y);
        self.pb.quad_to(a, b, c, d);
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let (a, b) = self.p(x1, y1);
        let (c, d) = self.p(x2, y2);
        let (e, f) = self.p(x, y);
        self.pb.cubic_to(a, b, c, d, e, f);
    }
    fn close(&mut self) {
        self.pb.close();
    }
}

impl TextBlock {
    pub fn fake_bold(&self) -> bool {
        self.pick.fake_bold
    }
    /// Outline of the block in BASE units, origin at the centre of a box `box_w` wide (BASE
    /// units), lines aligned inside it and the block centred vertically.
    pub fn path(&self, align: Align, box_w: f64) -> Option<Path> {
        let mut pb = PathBuilder::new();
        let skew = if self.pick.fake_italic { 0.2 } else { 0.0 };
        for (i, line) in self.lines.iter().enumerate() {
            let y_mid = -self.h / 2.0 + self.line_h * (i as f64 + 0.5);
            let baseline = y_mid + self.mid_to_base;
            let x0 = match align {
                Align::Left => -box_w / 2.0,
                Align::Right => box_w / 2.0 - line.width,
                Align::Center => -line.width / 2.0,
            };
            for g in &line.glyphs {
                let (face, s) = if g.face == 0 { (&self.pick.face, self.scale) } else { (&self.fallback, self.fallback_scale) };
                let mut o = Outline { pb: &mut pb, x: x0 + g.x, y: baseline, s, skew };
                face.outline_glyph(ttf_parser::GlyphId(g.id), &mut o);
            }
        }
        pb.finish()
    }
}

/// a one-line label (sheet captions): outline at `size` with the baseline at y, anchored left /
/// centre / right of x
pub fn label_path(text: &str, family: &str, weight: f64, size: f64, x: f64, y: f64, align: Align) -> Option<Path> {
    let pick = pick(family, weight, false);
    let fallback = Face::from_slice(INTER_VAR, 0).expect("bundled font");
    let scale = size / pick.face.units_per_em() as f64;
    let fscale = size / fallback.units_per_em() as f64;
    let line = shape_line(text, &pick, &fallback, scale, fscale, 0.0);
    let x0 = match align {
        Align::Left => x,
        Align::Center => x - line.width / 2.0,
        Align::Right => x - line.width,
    };
    let mut pb = PathBuilder::new();
    for g in &line.glyphs {
        let (face, s) = if g.face == 0 { (&pick.face, scale) } else { (&fallback, fscale) };
        let mut o = Outline { pb: &mut pb, x: x0 + g.x, y, s, skew: 0.0 };
        face.outline_glyph(ttf_parser::GlyphId(g.id), &mut o);
    }
    pb.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cb_core::doc::Side;

    #[test]
    fn every_font_loads_and_draws() {
        for (f, _) in FONTS {
            let l = Layer::text(Side::Front, 0.0, 0.0, 50.0, "Hello Ag", f, 700.0, 12.0, "#000000", Align::Center);
            let fit = fit(&l, "Hello Ag");
            assert!(fit.block.w > 100.0, "{f}");
            assert!(fit.block.path(Align::Center, fit.block.w).is_some(), "{f}");
            // 12 pt, one line at 1.2 line height ≈ 5.1 mm
            assert!((fit.h - 12.0 * cb_core::MM_PER_PT * 1.2).abs() < 0.01, "{f}: {}", fit.h);
        }
    }

    #[test]
    fn wraps_at_the_box_width_or_shrinks_to_fit() {
        let mut l = Layer::text(Side::Front, 0.0, 0.0, 30.0, "", "Inter", 400.0, 10.0, "#000000", Align::Left);
        let text = "The quick brown fox jumps over the lazy dog again and again";
        let natural = natural_width(&l, text);
        assert!(natural > 60.0);
        // no wrapping: one line, shrunk to the 30 mm box
        let f = fit(&l, text);
        assert_eq!(f.block.lines.len(), 1);
        assert!((f.block.w * f.k - 30.0).abs() < 1e-6);
        // wrapping: several lines, none wider than the box, at full size
        l.wrap = Some(true);
        let f = fit(&l, text);
        assert!(f.block.lines.len() >= 3);
        assert!((f.k - 10.0 * cb_core::MM_PER_PT / BASE).abs() < 1e-12);
        for line in &f.block.lines {
            assert!(line.width * f.k <= 30.0 + 1e-6);
        }
    }
}
