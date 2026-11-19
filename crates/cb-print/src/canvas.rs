//! What the card renderer draws on: filled and stroked paths, images and clips. Two backends
//! implement it — tiny-skia pixels (screen, PNG, 3D textures) and PDF vectors (print files),
//! so a card prints exactly as it looks.

use crate::color::Rgba;
use crate::images::Decoded;
use resvg::tiny_skia::{self, FillRule, Mask, Path, Pixmap, Transform};

/// how a path is painted; gradient points are in the path's own coordinates
#[derive(Clone, Debug, PartialEq)]
pub enum Paint {
    Solid(Rgba),
    Linear { from: (f32, f32), to: (f32, f32), stops: Vec<(f32, Rgba)> },
}

impl Paint {
    pub fn with_opacity(&self, k: f32) -> Paint {
        match self {
            Paint::Solid(c) => Paint::Solid(c.with_alpha(c.a * k)),
            Paint::Linear { from, to, stops } => Paint::Linear { from: *from, to: *to, stops: stops.iter().map(|(t, c)| (*t, c.with_alpha(c.a * k))).collect() },
        }
    }
}

pub trait Canvas {
    fn fill(&mut self, path: &Path, paint: &Paint, ts: Transform, rule: FillRule);
    fn stroke(&mut self, path: &Path, paint: &Paint, width: f32, ts: Transform);
    /// draw an image so that the unit square (0,0)–(1,1) — (0,0) its top-left corner — lands where `ts` puts it
    fn image(&mut self, img: &Decoded, key: u64, ts: Transform, opacity: f32);
    /// limit drawing to the inside of `path` until the matching `pop_clip`
    fn push_clip(&mut self, path: &Path, ts: Transform);
    fn pop_clip(&mut self);
}

/// drawing into pixels; design units (mm) map to pixels through `base`
pub struct SkiaCanvas {
    pub pixmap: Pixmap,
    pub base: Transform,
    clips: Vec<Mask>,
}

impl SkiaCanvas {
    pub fn new(pixmap: Pixmap, base: Transform) -> SkiaCanvas {
        SkiaCanvas { pixmap, base, clips: Vec::new() }
    }

    fn skia_paint(&self, paint: &Paint) -> tiny_skia::Paint<'static> {
        let mut p = tiny_skia::Paint { anti_alias: true, ..Default::default() };
        match paint {
            Paint::Solid(c) => p.set_color(c.skia()),
            Paint::Linear { from, to, stops } => {
                let st: Vec<tiny_skia::GradientStop> = stops.iter().map(|(t, c)| tiny_skia::GradientStop::new(*t, c.skia())).collect();
                match tiny_skia::LinearGradient::new(tiny_skia::Point::from_xy(from.0, from.1), tiny_skia::Point::from_xy(to.0, to.1), st, tiny_skia::SpreadMode::Pad, Transform::identity()) {
                    Some(s) => p.shader = s,
                    None => p.set_color(stops.first().map(|s| s.1.skia()).unwrap_or(tiny_skia::Color::BLACK)),
                }
            }
        }
        p
    }
}

/// scale factor of a transform (for picking image resolutions)
pub fn ts_scale(ts: &Transform) -> (f32, f32) {
    ((ts.sx * ts.sx + ts.ky * ts.ky).sqrt(), (ts.kx * ts.kx + ts.sy * ts.sy).sqrt())
}

impl Canvas for SkiaCanvas {
    fn fill(&mut self, path: &Path, paint: &Paint, ts: Transform, rule: FillRule) {
        let p = self.skia_paint(paint);
        let t = self.base.pre_concat(ts);
        self.pixmap.fill_path(path, &p, rule, t, self.clips.last());
    }

    fn stroke(&mut self, path: &Path, paint: &Paint, width: f32, ts: Transform) {
        let p = self.skia_paint(paint);
        let t = self.base.pre_concat(ts);
        let stroke = tiny_skia::Stroke { width, line_join: tiny_skia::LineJoin::Round, line_cap: tiny_skia::LineCap::Round, ..Default::default() };
        self.pixmap.stroke_path(path, &p, &stroke, t, self.clips.last());
    }

    fn image(&mut self, img: &Decoded, _key: u64, ts: Transform, opacity: f32) {
        let t = self.base.pre_concat(ts);
        let raster;
        let pm = match img {
            Decoded::Raster(p) => p,
            Decoded::Svg(tree) => {
                // draw the SVG sharp at the size it lands on screen
                let (sx, sy) = ts_scale(&t);
                let (w, h) = (sx.round().clamp(1.0, 6000.0) as u32, sy.round().clamp(1.0, 6000.0) as u32);
                match crate::images::rasterize_svg(tree, w, h) {
                    Some(p) => {
                        raster = p;
                        &raster
                    }
                    None => return,
                }
            }
        };
        let (iw, ih) = (pm.width() as f32, pm.height() as f32);
        // pixels of the image → unit square → the page
        let pattern = tiny_skia::Pattern::new(pm.as_ref(), tiny_skia::SpreadMode::Pad, tiny_skia::FilterQuality::Bicubic, opacity.clamp(0.0, 1.0), Transform::from_scale(1.0 / iw, 1.0 / ih));
        let paint = tiny_skia::Paint { shader: pattern, anti_alias: true, ..Default::default() };
        if let Some(r) = tiny_skia::Rect::from_xywh(0.0, 0.0, 1.0, 1.0) {
            self.pixmap.fill_rect(r, &paint, t, self.clips.last());
        }
    }

    fn push_clip(&mut self, path: &Path, ts: Transform) {
        let t = self.base.pre_concat(ts);
        let mask = match self.clips.last() {
            Some(m) => {
                let mut m = m.clone();
                m.intersect_path(path, FillRule::Winding, true, t);
                m
            }
            None => {
                let mut m = Mask::new(self.pixmap.width(), self.pixmap.height()).expect("pixmap size");
                m.fill_path(path, FillRule::Winding, true, t);
                m
            }
        };
        self.clips.push(mask);
    }

    fn pop_clip(&mut self) {
        self.clips.pop();
    }
}
