//! A small PDF writer for print files: vector paths (text outlines, shapes, QR codes), images
//! (with transparency), linear gradients and reusable card drawings (form XObjects: a card
//! repeated on a sheet is stored once). Content streams are deflate-compressed.
//!
//! Drawing coordinates are mm with y down from the top-left of the page (or of the card, inside
//! a card drawing), like everywhere else in CardBuilder.

use crate::canvas::{ts_scale, Canvas, Paint};
use crate::color::Rgba;
use crate::images::Decoded;
use resvg::tiny_skia::{FillRule, Path, PathSegment, Point, Transform};
use std::collections::HashMap;
use std::fmt::Write as _;

pub const PT_PER_MM: f64 = 72.0 / 25.4;

/// a number for a content stream: up to 4 decimals, no trailing zeros
pub fn num(v: f64) -> String {
    if !v.is_finite() {
        return "0".into();
    }
    let s = format!("{v:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" || s.is_empty() {
        "0".into()
    } else {
        s.to_string()
    }
}

fn pdf_string(s: &str) -> String {
    let mut o = String::from("(");
    for c in s.chars() {
        match c {
            '(' | ')' | '\\' => {
                o.push('\\');
                o.push(c);
            }
            c if c.is_ascii() && !c.is_control() => o.push(c),
            _ => o.push('?'),
        }
    }
    o.push(')');
    o
}

fn deflate(data: &[u8]) -> Vec<u8> {
    miniz_oxide::deflate::compress_to_vec_zlib(data, 6)
}

pub struct PdfWriter {
    /// object bodies by id − 1 (None = allocated, not written yet)
    objects: Vec<Option<Vec<u8>>>,
    pages: Vec<u32>,
    xobjects: Vec<(String, u32)>,
    gstates: HashMap<u32, String>,
    shadings: Vec<(String, u32)>,
    images: HashMap<u64, String>,
    title: String,
    /// resolution SVG artwork is rasterised at
    pub dpi: f64,
}

const CATALOG: u32 = 1;
const PAGES: u32 = 2;
const RESOURCES: u32 = 3;
const INFO: u32 = 4;

impl PdfWriter {
    pub fn new(title: &str, dpi: f64) -> PdfWriter {
        PdfWriter { objects: vec![None, None, None, None], pages: vec![], xobjects: vec![], gstates: HashMap::new(), shadings: vec![], images: HashMap::new(), title: title.into(), dpi }
    }

    fn alloc(&mut self) -> u32 {
        self.objects.push(None);
        self.objects.len() as u32
    }

    fn set(&mut self, id: u32, body: Vec<u8>) {
        self.objects[id as usize - 1] = Some(body);
    }

    fn add(&mut self, body: Vec<u8>) -> u32 {
        let id = self.alloc();
        self.set(id, body);
        id
    }

    fn stream(dict: &str, data: &[u8]) -> Vec<u8> {
        let z = deflate(data);
        let mut b = format!("<< {dict} /Filter /FlateDecode /Length {} >>\nstream\n", z.len()).into_bytes();
        b.extend_from_slice(&z);
        b.extend_from_slice(b"\nendstream");
        b
    }

    /// an ExtGState for a fill / stroke opacity
    fn alpha(&mut self, a: f32) -> Option<String> {
        if a >= 0.999 {
            return None;
        }
        let key = (a.clamp(0.0, 1.0) * 1000.0).round() as u32;
        if let Some(n) = self.gstates.get(&key) {
            return Some(n.clone());
        }
        let name = format!("GS{}", self.gstates.len() + 1);
        let id = self.add(format!("<< /Type /ExtGState /ca {0} /CA {0} >>", num(key as f64 / 1000.0)).into_bytes());
        self.gstates.insert(key, format!("{name} {id}"));
        Some(format!("{name} {id}"))
    }

    fn shading(&mut self, from: (f64, f64), to: (f64, f64), stops: &[(f32, Rgba)]) -> String {
        let c = |c: &Rgba| format!("[{} {} {}]", num(c.r as f64 / 255.0), num(c.g as f64 / 255.0), num(c.b as f64 / 255.0));
        let mut st: Vec<(f32, Rgba)> = stops.to_vec();
        st.sort_by(|a, b| a.0.total_cmp(&b.0));
        if st.is_empty() {
            st.push((0.0, Rgba::BLACK));
        }
        if st[0].0 > 0.0 {
            st.insert(0, (0.0, st[0].1));
        }
        if st.last().unwrap().0 < 1.0 {
            st.push((1.0, st.last().unwrap().1));
        }
        if st.len() == 1 {
            st.push((1.0, st[0].1));
        }
        let pieces: Vec<String> = st.windows(2).map(|w| format!("<< /FunctionType 2 /Domain [0 1] /C0 {} /C1 {} /N 1 >>", c(&w[0].1), c(&w[1].1))).collect();
        let function = if pieces.len() == 1 {
            pieces[0].clone()
        } else {
            let bounds: Vec<String> = st[1..st.len() - 1].iter().map(|s| num(s.0 as f64)).collect();
            let encode = vec!["0 1"; pieces.len()].join(" ");
            format!("<< /FunctionType 3 /Domain [0 1] /Functions [{}] /Bounds [{}] /Encode [{encode}] >>", pieces.join(" "), bounds.join(" "))
        };
        let id = self.add(
            format!("<< /ShadingType 2 /ColorSpace /DeviceRGB /Coords [{} {} {} {}] /Function {function} /Extend [true true] >>", num(from.0), num(from.1), num(to.0), num(to.1)).into_bytes(),
        );
        let name = format!("Sh{}", self.shadings.len() + 1);
        self.shadings.push((name.clone(), id));
        name
    }

    /// an image XObject (straight RGBA pixels); returns its resource name
    pub fn image_rgba(&mut self, key: u64, w: u32, h: u32, rgba: &[u8]) -> String {
        if let Some(n) = self.images.get(&key) {
            return n.clone();
        }
        let mut rgb = Vec::with_capacity((w * h * 3) as usize);
        let mut alpha = Vec::with_capacity((w * h) as usize);
        for px in rgba.chunks_exact(4) {
            rgb.extend_from_slice(&px[..3]);
            alpha.push(px[3]);
        }
        let smask = if alpha.iter().any(|a| *a < 255) {
            let id = self.add(Self::stream(&format!("/Type /XObject /Subtype /Image /Width {w} /Height {h} /ColorSpace /DeviceGray /BitsPerComponent 8"), &alpha));
            format!(" /SMask {id} 0 R")
        } else {
            String::new()
        };
        let id = self.add(Self::stream(&format!("/Type /XObject /Subtype /Image /Width {w} /Height {h} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Interpolate true{smask}"), &rgb));
        let name = format!("Im{}", self.images.len() + 1);
        self.xobjects.push((name.clone(), id));
        self.images.insert(key, name.clone());
        name
    }

    /// a reusable drawing (`content` in its own coordinates, clipped to `bbox`)
    pub fn add_form(&mut self, bbox: [f64; 4], content: &str) -> String {
        let id = self.add(Self::stream(&format!("/Type /XObject /Subtype /Form /BBox [{} {} {} {}] /Resources {RESOURCES} 0 R", num(bbox[0]), num(bbox[1]), num(bbox[2]), num(bbox[3])), content.as_bytes()));
        let name = format!("Fm{}", self.xobjects.len() + 1);
        self.xobjects.push((name.clone(), id));
        name
    }

    /// Add a page `w` × `h` mm. `content` is drawn in mm, y down from the top-left. `trim` (mm
    /// inset from each edge) marks the trim box of a page with bleed.
    pub fn add_page(&mut self, w: f64, h: f64, content: &str, trim: Option<f64>) {
        let (wp, hp) = (w * PT_PER_MM, h * PT_PER_MM);
        let mut c = format!("{} 0 0 {} 0 {} cm\n", num(PT_PER_MM), num(-PT_PER_MM), num(hp));
        c += content;
        let cid = self.add(Self::stream("", c.as_bytes()));
        let boxes = match trim {
            Some(b) => {
                let b = b * PT_PER_MM;
                format!(" /BleedBox [0 0 {0} {1}] /TrimBox [{2} {2} {3} {4}]", num(wp), num(hp), num(b), num(wp - b), num(hp - b))
            }
            None => String::new(),
        };
        let pid = self.add(format!("<< /Type /Page /Parent {PAGES} 0 R /MediaBox [0 0 {} {}]{boxes} /Resources {RESOURCES} 0 R /Contents {cid} 0 R >>", num(wp), num(hp)).into_bytes());
        self.pages.push(pid);
    }

    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    pub fn finish(mut self) -> Vec<u8> {
        let kids: Vec<String> = self.pages.iter().map(|p| format!("{p} 0 R")).collect();
        self.set(CATALOG, format!("<< /Type /Catalog /Pages {PAGES} 0 R >>").into_bytes());
        self.set(PAGES, format!("<< /Type /Pages /Kids [{}] /Count {} >>", kids.join(" "), self.pages.len()).into_bytes());
        let mut res = String::from("<< /ProcSet [/PDF /ImageB /ImageC]");
        if !self.xobjects.is_empty() {
            res += " /XObject <<";
            for (n, id) in &self.xobjects {
                let _ = write!(res, " /{n} {id} 0 R");
            }
            res += " >>";
        }
        if !self.gstates.is_empty() {
            res += " /ExtGState <<";
            for v in self.gstates.values() {
                let (n, id) = v.split_once(' ').unwrap();
                let _ = write!(res, " /{n} {id} 0 R");
            }
            res += " >>";
        }
        if !self.shadings.is_empty() {
            res += " /Shading <<";
            for (n, id) in &self.shadings {
                let _ = write!(res, " /{n} {id} 0 R");
            }
            res += " >>";
        }
        res += " >>";
        self.set(RESOURCES, res.into_bytes());
        self.set(INFO, format!("<< /Producer (CardBuilder {}) /Creator (CardBuilder) /Title {} >>", env!("CARGO_PKG_VERSION"), pdf_string(&self.title)).into_bytes());
        let mut out: Vec<u8> = b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n".to_vec();
        let mut offsets = Vec::with_capacity(self.objects.len());
        for (i, o) in self.objects.iter().enumerate() {
            offsets.push(out.len());
            out.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
            out.extend_from_slice(o.as_deref().unwrap_or(b"null"));
            out.extend_from_slice(b"\nendobj\n");
        }
        let xref = out.len();
        out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", self.objects.len() + 1).as_bytes());
        for off in offsets {
            out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
        }
        out.extend_from_slice(format!("trailer\n<< /Size {} /Root {CATALOG} 0 R /Info {INFO} 0 R >>\nstartxref\n{xref}\n%%EOF\n", self.objects.len() + 1).as_bytes());
        out
    }
}

/// path operators for `path` mapped through `ts`
fn path_ops(path: &Path, ts: Transform) -> String {
    let mut s = String::new();
    let map = |p: Point| {
        let mut q = p;
        ts.map_point(&mut q);
        q
    };
    let mut cur = Point::zero();
    for seg in path.segments() {
        match seg {
            PathSegment::MoveTo(p) => {
                let q = map(p);
                let _ = writeln!(s, "{} {} m", num(q.x as f64), num(q.y as f64));
                cur = p;
            }
            PathSegment::LineTo(p) => {
                let q = map(p);
                let _ = writeln!(s, "{} {} l", num(q.x as f64), num(q.y as f64));
                cur = p;
            }
            PathSegment::QuadTo(c, p) => {
                // quadratic → cubic
                let c1 = Point::from_xy(cur.x + 2.0 / 3.0 * (c.x - cur.x), cur.y + 2.0 / 3.0 * (c.y - cur.y));
                let c2 = Point::from_xy(p.x + 2.0 / 3.0 * (c.x - p.x), p.y + 2.0 / 3.0 * (c.y - p.y));
                let (a, b, e) = (map(c1), map(c2), map(p));
                let _ = writeln!(s, "{} {} {} {} {} {} c", num(a.x as f64), num(a.y as f64), num(b.x as f64), num(b.y as f64), num(e.x as f64), num(e.y as f64));
                cur = p;
            }
            PathSegment::CubicTo(c1, c2, p) => {
                let (a, b, e) = (map(c1), map(c2), map(p));
                let _ = writeln!(s, "{} {} {} {} {} {} c", num(a.x as f64), num(a.y as f64), num(b.x as f64), num(b.y as f64), num(e.x as f64), num(e.y as f64));
                cur = p;
            }
            PathSegment::Close => s.push_str("h\n"),
        }
    }
    s
}

fn rg(c: &Rgba) -> String {
    format!("{} {} {}", num(c.r as f64 / 255.0), num(c.g as f64 / 255.0), num(c.b as f64 / 255.0))
}

/// drawing into a PDF content stream (a page or a card drawing)
pub struct PdfCanvas<'a> {
    pub pdf: &'a mut PdfWriter,
    pub out: String,
}

impl<'a> PdfCanvas<'a> {
    pub fn new(pdf: &'a mut PdfWriter) -> PdfCanvas<'a> {
        PdfCanvas { pdf, out: String::new() }
    }

    fn gs(&mut self, a: f32) {
        if let Some(g) = self.pdf.alpha(a) {
            let name = g.split(' ').next().unwrap().to_string();
            let _ = writeln!(self.out, "/{name} gs");
        }
    }

    fn average_alpha(paint: &Paint) -> f32 {
        match paint {
            Paint::Solid(c) => c.a,
            Paint::Linear { stops, .. } => stops.iter().map(|s| s.1.a).sum::<f32>() / stops.len().max(1) as f32,
        }
    }

    /// place a form or image XObject with an affine matrix (pdf `cm` order)
    pub fn place(&mut self, name: &str, m: [f64; 6]) {
        let _ = writeln!(self.out, "q {} {} {} {} {} {} cm /{name} Do Q", num(m[0]), num(m[1]), num(m[2]), num(m[3]), num(m[4]), num(m[5]));
    }

    /// a hairline in registration black (crop marks)
    pub fn line(&mut self, a: (f64, f64), b: (f64, f64), width: f64) {
        let _ = writeln!(self.out, "q 0 G {} w 0 J {} {} m {} {} l S Q", num(width), num(a.0), num(a.1), num(b.0), num(b.1));
    }
}

impl Canvas for PdfCanvas<'_> {
    fn fill(&mut self, path: &Path, paint: &Paint, ts: Transform, rule: FillRule) {
        let a = Self::average_alpha(paint);
        if a <= 0.001 {
            return;
        }
        self.out.push_str("q\n");
        self.gs(a);
        let ops = path_ops(path, ts);
        let f = if rule == FillRule::EvenOdd { "f*" } else { "f" };
        match paint {
            Paint::Solid(c) => {
                let _ = write!(self.out, "{} rg\n{ops}{f}\n", rg(c));
            }
            Paint::Linear { from, to, stops } => {
                let mut p0 = Point::from_xy(from.0, from.1);
                let mut p1 = Point::from_xy(to.0, to.1);
                ts.map_point(&mut p0);
                ts.map_point(&mut p1);
                let sh = self.pdf.shading((p0.x as f64, p0.y as f64), (p1.x as f64, p1.y as f64), stops);
                let w = if rule == FillRule::EvenOdd { "W*" } else { "W" };
                let _ = write!(self.out, "{ops}{w} n\n/{sh} sh\n");
            }
        }
        self.out.push_str("Q\n");
    }

    fn stroke(&mut self, path: &Path, paint: &Paint, width: f32, ts: Transform) {
        let a = Self::average_alpha(paint);
        if a <= 0.001 {
            return;
        }
        let c = match paint {
            Paint::Solid(c) => *c,
            Paint::Linear { stops, .. } => stops.first().map(|s| s.1).unwrap_or(Rgba::BLACK),
        };
        let (sx, sy) = ts_scale(&ts);
        self.out.push_str("q\n");
        self.gs(a);
        let _ = write!(self.out, "{} RG {} w 1 J 1 j\n{}S\nQ\n", rg(&c), num(width as f64 * ((sx + sy) / 2.0) as f64), path_ops(path, ts));
    }

    fn stroke_dashed(&mut self, path: &Path, paint: &Paint, width: f32, dash: [f32; 2], ts: Transform) {
        // the dash pattern in page units, like the line width
        let (sx, sy) = ts_scale(&ts);
        let k = ((sx + sy) / 2.0) as f64;
        let _ = writeln!(self.out, "q [{} {}] 0 d", num(dash[0] as f64 * k), num(dash[1] as f64 * k));
        self.stroke(path, paint, width, ts);
        self.out.push_str("Q\n");
    }

    fn image(&mut self, img: &Decoded, key: u64, ts: Transform, opacity: f32) {
        if opacity <= 0.001 {
            return;
        }
        let name = match img {
            Decoded::Raster(pm) => {
                let mut data = pm.data().to_vec();
                crate::unpremultiply(&mut data);
                self.pdf.image_rgba(key, pm.width(), pm.height(), &data)
            }
            Decoded::Svg(tree) => {
                // vector artwork from SVG files is printed at the file's resolution
                let (sx, sy) = ts_scale(&ts);
                let px = |mm: f32| ((mm as f64 / 25.4 * self.pdf.dpi).round() as u32).clamp(1, 8000);
                let (w, h) = (px(sx), px(sy));
                let Some(pm) = crate::images::rasterize_svg(tree, w, h) else { return };
                let mut data = pm.data().to_vec();
                crate::unpremultiply(&mut data);
                self.pdf.image_rgba(key ^ ((w as u64) << 32) ^ h as u64, w, h, &data)
            }
        };
        self.out.push_str("q\n");
        self.gs(opacity);
        // the image space has its first row at the top of the unit square (y = 1); ours has y down
        let m = [ts.sx as f64, ts.ky as f64, ts.kx as f64, ts.sy as f64, ts.tx as f64, ts.ty as f64];
        let flipped = [m[0], m[1], -m[2], -m[3], m[2] + m[4], m[3] + m[5]];
        let _ = writeln!(self.out, "{} {} {} {} {} {} cm /{name} Do", num(flipped[0]), num(flipped[1]), num(flipped[2]), num(flipped[3]), num(flipped[4]), num(flipped[5]));
        self.out.push_str("Q\n");
    }

    fn push_clip(&mut self, path: &Path, ts: Transform, rule: FillRule) {
        let w = if rule == FillRule::EvenOdd { "W*" } else { "W" };
        let _ = write!(self.out, "q\n{}{w} n\n", path_ops(path, ts));
    }

    fn pop_clip(&mut self) {
        self.out.push_str("Q\n");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_are_compact() {
        assert_eq!(num(1.0), "1");
        assert_eq!(num(-0.00001), "0");
        assert_eq!(num(2.83464567), "2.8346");
        assert_eq!(num(f64::NAN), "0");
    }

    #[test]
    fn writes_a_well_formed_file() {
        let mut pdf = PdfWriter::new("Test (1)", 300.0);
        let mut c = PdfCanvas::new(&mut pdf);
        let path = resvg::tiny_skia::PathBuilder::from_rect(resvg::tiny_skia::Rect::from_xywh(1.0, 1.0, 5.0, 5.0).unwrap());
        c.fill(&path, &Paint::Solid(Rgba::hex("#ff000080", 1.0)), Transform::identity(), FillRule::Winding);
        c.fill(&path, &Paint::Linear { from: (0.0, 0.0), to: (5.0, 0.0), stops: vec![(0.0, Rgba::BLACK), (0.5, Rgba::WHITE), (1.0, Rgba::BLACK)] }, Transform::identity(), FillRule::Winding);
        let content = c.out;
        pdf.add_page(85.0, 55.0, &content, Some(3.0));
        let bytes = pdf.finish();
        let has = |needle: &[u8]| bytes.windows(needle.len()).any(|w| w == needle);
        assert!(bytes.starts_with(b"%PDF-1.4"));
        assert!(has(b"/Count 1") && has(b"/TrimBox") && has(b"/FunctionType 3"));
        assert!(bytes.ends_with(b"%%EOF\n"));
        // every xref offset points at its object (the streams are binary: work on bytes)
        let sx = bytes.windows(10).rposition(|w| w == b"startxref\n").unwrap() + 10;
        let xref_at: usize = std::str::from_utf8(&bytes[sx..]).unwrap().lines().next().unwrap().parse().unwrap();
        let table = std::str::from_utf8(&bytes[xref_at..]).unwrap();
        let mut n = 0;
        for (i, line) in table.lines().skip(3).take_while(|l| l.ends_with(" n ")).enumerate() {
            let off: usize = line[..10].parse().unwrap();
            assert!(bytes[off..].starts_with(format!("{} 0 obj", i + 1).as_bytes()), "object {}", i + 1);
            n += 1;
        }
        assert!(n >= 6);
    }
}
