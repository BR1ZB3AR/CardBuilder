//! Drawing one printed side of a card: background, then every layer in order — text, images,
//! shapes and QR codes — with data placeholders filled in. The same code draws the editor
//! preview, PNG exports, the 3D textures and the PDF print files.
//!
//! Besides the colour print it can draw the finishing masks: where foil is stamped and where
//! spot UV varnish goes (for the 3D preview and for the print shop).

use crate::canvas::{Canvas, Paint, SkiaCanvas};
use crate::color::Rgba;
use crate::images::ImageCache;
use cb_core::doc::{Align, BgMode, Doc, Foil, Layer, LayerType, ShapeKind, Side, VAlign};
use resvg::tiny_skia::{FillRule, Path, PathBuilder, Pixmap, Rect, Transform};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// the print as it looks
    Color,
    /// only where foil goes
    Foil,
    /// only where spot UV varnish goes
    Uv,
    /// only holographic foil
    Holo,
}

#[derive(Clone, Copy, Debug)]
pub struct DrawOptions {
    /// data row for `{Column}` placeholders (None = show the placeholders)
    pub record: Option<usize>,
    /// fill unprinted areas with the paper colour (previews; never in print files)
    pub paper: bool,
    pub mode: Mode,
    /// colour masks are drawn in
    pub mask: Rgba,
}

impl DrawOptions {
    pub fn preview(record: Option<usize>) -> DrawOptions {
        DrawOptions { record, paper: true, mode: Mode::Color, mask: Rgba::WHITE }
    }
    pub fn print(record: Option<usize>) -> DrawOptions {
        DrawOptions { record, paper: false, mode: Mode::Color, mask: Rgba::BLACK }
    }
}

/// a rectangle with rounded corners centred on the origin (cubic arcs)
pub fn rounded_rect_path(w: f32, h: f32, r: f32) -> Option<Path> {
    let r = r.clamp(0.0, w.min(h) / 2.0);
    let (x0, y0, x1, y1) = (-w / 2.0, -h / 2.0, w / 2.0, h / 2.0);
    if r <= 1e-4 {
        return Rect::from_ltrb(x0, y0, x1, y1).map(PathBuilder::from_rect);
    }
    let k = r * 0.552_284_8;
    let mut pb = PathBuilder::new();
    pb.move_to(x0 + r, y0);
    pb.line_to(x1 - r, y0);
    pb.cubic_to(x1 - r + k, y0, x1, y0 + r - k, x1, y0 + r);
    pb.line_to(x1, y1 - r);
    pb.cubic_to(x1, y1 - r + k, x1 - r + k, y1, x1 - r, y1);
    pb.line_to(x0 + r, y1);
    pb.cubic_to(x0 + r - k, y1, x0, y1 - r + k, x0, y1 - r);
    pb.line_to(x0, y0 + r);
    pb.cubic_to(x0, y0 + r - k, x0 + r - k, y0, x0 + r, y0);
    pb.close();
    pb.finish()
}

fn rect_path(x: f64, y: f64, w: f64, h: f64) -> Option<Path> {
    Rect::from_xywh(x as f32, y as f32, w.max(0.001) as f32, h.max(0.001) as f32).map(PathBuilder::from_rect)
}

/// a gradient across the rectangle (x, y, w, h) at `angle` degrees, covering it corner to corner
fn gradient_across(x: f64, y: f64, w: f64, h: f64, angle: f64, stops: Vec<(f32, Rgba)>) -> Paint {
    let (s, c) = angle.to_radians().sin_cos();
    let half = (w * c.abs() + h * s.abs()) / 2.0;
    let (cx, cy) = (x + w / 2.0, y + h / 2.0);
    Paint::Linear { from: ((cx - c * half) as f32, (cy - s * half) as f32), to: ((cx + c * half) as f32, (cy + s * half) as f32), stops }
}

/// the brushed-metal look of a foil over a box (w × h, centred on the origin)
pub fn foil_paint(foil: Foil, w: f64, h: f64, opacity: f64) -> Paint {
    let (dark, light) = foil.colors();
    let c = |hex: &str| Rgba::hex(hex, opacity);
    let stops = match foil {
        Foil::Holo => vec![(0.0, c("#8fd3ff")), (0.25, c("#c9a2ff")), (0.5, c("#ffb8ec")), (0.75, c("#fff1a8")), (1.0, c("#9ff5d0"))],
        _ => vec![(0.0, c(dark)), (0.3, c(light)), (0.5, c(&crate::color::mix(dark, light, 0.45))), (0.75, c(light)), (1.0, c(dark))],
    };
    gradient_across(-w / 2.0, -h / 2.0, w, h, 30.0, stops)
}

/// Draw side `side` of the card. `base` maps design mm (origin at the trim's top-left) onto the
/// canvas. Everything is clipped to the bleed box.
pub fn draw_side(c: &mut dyn Canvas, doc: &Doc, side: Side, o: &DrawOptions, imgs: &ImageCache, base: Transform) {
    crate::text::register_fonts(&doc.fonts);
    let f = &doc.format;
    let (sw, sh) = f.sheet();
    let b = f.bleed;
    let Some(bleed_box) = rect_path(-b, -b, sw + 2.0 * b, sh + 2.0 * b) else { return };
    c.push_clip(&bleed_box, base);
    if o.mode == Mode::Color {
        draw_background(c, doc, side, o, imgs, base, &bleed_box);
    }
    let fill = doc.filler(o.record);
    for l in doc.layers_on(side).filter(|l| l.visible && l.opacity > 0.0) {
        let wanted = match o.mode {
            Mode::Color => true,
            Mode::Foil => l.foil.is_some(),
            Mode::Uv => l.spot_uv,
            Mode::Holo => l.foil == Some(Foil::Holo),
        };
        if wanted {
            draw_layer(c, l, o, imgs, base, &fill);
        }
    }
    c.pop_clip();
}

fn draw_background(c: &mut dyn Canvas, doc: &Doc, side: Side, o: &DrawOptions, imgs: &ImageCache, base: Transform, bleed_box: &Path) {
    let f = &doc.format;
    let (sw, sh) = f.sheet();
    let b = f.bleed;
    let bg = doc.background(side);
    let paper = |c: &mut dyn Canvas| {
        if o.paper {
            c.fill(bleed_box, &Paint::Solid(Rgba::hex(&doc.paper.color, 1.0)), base, FillRule::Winding);
        }
    };
    match bg.mode {
        BgMode::Paper => paper(c),
        BgMode::Solid => c.fill(bleed_box, &Paint::Solid(Rgba::hex(&bg.color, 1.0)), base, FillRule::Winding),
        BgMode::Gradient => {
            let paint = gradient_across(-b, -b, sw + 2.0 * b, sh + 2.0 * b, bg.angle, vec![(0.0, Rgba::hex(&bg.color, 1.0)), (1.0, Rgba::hex(&bg.color2, 1.0))]);
            c.fill(bleed_box, &paint, base, FillRule::Winding);
        }
        BgMode::Image => {
            paper(c);
            if let Some(img) = bg.src.as_deref().and_then(|s| imgs.get(s).map(|d| (s, d))) {
                let (src, d) = img;
                // cover the bleed box, centred
                let (iw, ih) = d.size();
                let (bw, bh) = (sw + 2.0 * b, sh + 2.0 * b);
                let k = (bw / iw).max(bh / ih);
                let (w, h) = (iw * k, ih * k);
                let ts = Transform::from_row(w as f32, 0.0, 0.0, h as f32, (-b + (bw - w) / 2.0) as f32, (-b + (bh - h) / 2.0) as f32);
                c.image(d, crate::images::key(src), base.pre_concat(ts), 1.0);
            }
        }
    }
}

fn layer_ts(l: &Layer, base: Transform) -> Transform {
    base.pre_concat(Transform::from_translate(l.x as f32, l.y as f32)).pre_concat(Transform::from_rotate(l.rot as f32))
}

/// the paint of a layer's main colour (foil overrides it in the colour print)
fn ink(l: &Layer, o: &DrawOptions, hex: &str, w: f64, h: f64) -> Paint {
    match (o.mode, l.foil) {
        (Mode::Color, Some(foil)) => foil_paint(foil, w, h, l.opacity),
        (Mode::Color, None) => Paint::Solid(Rgba::hex(hex, l.opacity)),
        _ => Paint::Solid(o.mask),
    }
}

/// the box a layer takes on the card (text boxes follow their text), mm
pub fn layer_size(l: &Layer, fill: &dyn Fn(&str) -> String) -> (f64, f64) {
    if l.kind == LayerType::Text && l.valign.is_none() {
        let t = l.text_for(fill);
        let fit = crate::text::fit(l, &t);
        (l.w, fit.h.max(0.5))
    } else {
        (l.w, l.h)
    }
}

pub fn draw_layer(c: &mut dyn Canvas, l: &Layer, o: &DrawOptions, imgs: &ImageCache, base: Transform, fill: &dyn Fn(&str) -> String) {
    let ts = layer_ts(l, base);
    match l.kind {
        LayerType::Text => {
            let text = l.text_for(fill);
            if text.trim().is_empty() {
                return;
            }
            let fit = crate::text::fit(l, &text);
            let k = fit.k;
            if k <= 0.0 {
                return;
            }
            let box_w = l.w / k;
            let Some(path) = fit.block.path(l.align.unwrap_or(Align::Center), box_w) else { return };
            // a text frame puts the block at its top or bottom
            let shift = match l.valign {
                Some(VAlign::Top) => -l.h / 2.0 + fit.h / 2.0,
                Some(VAlign::Bottom) => l.h / 2.0 - fit.h / 2.0,
                _ => 0.0,
            };
            let tts = ts.pre_translate(0.0, shift as f32).pre_scale(k as f32, k as f32);
            let col = l.color.as_deref().unwrap_or("#000000");
            // see-through letters (an outline-only number) have no fill at all, in any finish
            if crate::color::hex_alpha(col) > 0.0 {
                let paint = ink(l, o, col, box_w, fit.block.h);
                c.fill(&path, &paint, tts, FillRule::Winding);
                if fit.block.fake_bold() {
                    c.stroke(&path, &paint, (crate::text::BASE * 0.035) as f32, tts);
                }
            }
            // an outline round the letters (width in mm)
            if let (Some(sc), Some(sw)) = (l.stroke.as_deref(), l.stroke_w) {
                if sw > 0.0 {
                    c.stroke(&path, &ink(l, o, sc, box_w, fit.block.h), (sw / k) as f32, tts);
                }
            }
        }
        LayerType::Image => {
            let Some(src) = l.src.as_deref() else { return };
            let Some(d) = imgs.get(src) else { return };
            if o.mode == Mode::Color {
                let its = ts.pre_concat(Transform::from_row(l.w as f32, 0.0, 0.0, l.h as f32, (-l.w / 2.0) as f32, (-l.h / 2.0) as f32));
                c.image(d, crate::images::key(src), its, l.opacity as f32);
            } else if let Some(p) = rect_path(-l.w / 2.0, -l.h / 2.0, l.w, l.h) {
                // a finish on a picture covers its box
                c.fill(&p, &Paint::Solid(o.mask), ts, FillRule::Winding);
            }
        }
        LayerType::Shape => {
            let shape = l.shape.unwrap_or(ShapeKind::Rect);
            let path = match shape {
                ShapeKind::Rect => rounded_rect_path(l.w as f32, l.h as f32, l.radius.unwrap_or(0.0) as f32),
                ShapeKind::Ellipse => Rect::from_xywh((-l.w / 2.0) as f32, (-l.h / 2.0) as f32, l.w as f32, l.h as f32).and_then(PathBuilder::from_oval),
                ShapeKind::Line => {
                    let mut pb = PathBuilder::new();
                    pb.move_to((-l.w / 2.0) as f32, 0.0);
                    pb.line_to((l.w / 2.0) as f32, 0.0);
                    pb.finish()
                }
            };
            let Some(path) = path else { return };
            if shape == ShapeKind::Line {
                let col = l.stroke.as_deref().or(l.fill.as_deref()).unwrap_or("#000000");
                c.stroke(&path, &ink(l, o, col, l.w, l.h), l.stroke_w.unwrap_or(0.5).max(0.05) as f32, ts);
                return;
            }
            if let Some(fc) = l.fill.as_deref() {
                c.fill(&path, &ink(l, o, fc, l.w, l.h), ts, FillRule::Winding);
            }
            if let (Some(sc), Some(sw)) = (l.stroke.as_deref(), l.stroke_w) {
                if sw > 0.0 {
                    c.stroke(&path, &ink(l, o, sc, l.w, l.h), sw as f32, ts);
                }
            }
        }
        LayerType::Qr => {
            let Some(spec) = &l.qr else { return };
            let data = spec.payload(fill);
            let Some((n, dark)) = crate::qr::matrix(&data, spec.ecc) else { return };
            let size = l.w.min(l.h);
            // a background colour gets the quiet zone scanners need round the code
            let quiet = if l.fill.is_some() { 2 } else { 0 };
            if let Some(bg) = l.fill.as_deref() {
                if o.mode == Mode::Color {
                    if let Some(p) = rounded_rect_path(size as f32, size as f32, l.radius.unwrap_or(0.0) as f32) {
                        c.fill(&p, &Paint::Solid(Rgba::hex(bg, l.opacity)), ts, FillRule::Winding);
                    }
                }
            }
            if let Some(p) = crate::qr::path(n, &dark, size as f32, quiet) {
                c.fill(&p, &ink(l, o, l.color.as_deref().unwrap_or("#000000"), size, size), ts, FillRule::Winding);
            }
        }
    }
}

/// Render side `side` to pixels at `ppm` pixels per mm: the trim box, or with `bleed` the
/// bleed box. Premultiplied RGBA (tiny-skia).
pub fn render_side(doc: &Doc, side: Side, o: &DrawOptions, ppm: f64, bleed: bool, imgs: &ImageCache) -> Option<Pixmap> {
    let (sw, sh) = doc.format.sheet();
    let b = if bleed { doc.format.bleed } else { 0.0 };
    let (w, h) = (((sw + 2.0 * b) * ppm).round().max(1.0) as u32, ((sh + 2.0 * b) * ppm).round().max(1.0) as u32);
    let pm = Pixmap::new(w, h)?;
    // whole pixels: stretch a hair so the box fills the image exactly (no half-covered edge)
    let (kx, ky) = (w as f64 / (sw + 2.0 * b), h as f64 / (sh + 2.0 * b));
    let base = Transform::from_scale(kx as f32, ky as f32).pre_translate(b as f32, b as f32);
    let mut c = SkiaCanvas::new(pm, Transform::identity());
    draw_side(&mut c, doc, side, o, imgs, base);
    Some(c.pixmap)
}

/// The finishing mask of a side for the 3D preview: red = foil, green = spot UV, blue =
/// holographic foil (straight RGBA, trim box).
pub fn render_finish_mask(doc: &Doc, side: Side, record: Option<usize>, ppm: f64, imgs: &ImageCache) -> Option<(u32, u32, Vec<u8>)> {
    let mut chans = Vec::new();
    for mode in [Mode::Foil, Mode::Uv, Mode::Holo] {
        let o = DrawOptions { record, paper: false, mode, mask: Rgba::WHITE };
        chans.push(render_side(doc, side, &o, ppm, false, imgs)?);
    }
    let (w, h) = (chans[0].width(), chans[0].height());
    let mut out = vec![0u8; (w * h * 4) as usize];
    for (i, px) in out.chunks_exact_mut(4).enumerate() {
        for (k, ch) in chans.iter().enumerate() {
            px[k] = ch.data()[i * 4 + 3];
        }
        px[3] = 255;
    }
    Some((w, h, out))
}
