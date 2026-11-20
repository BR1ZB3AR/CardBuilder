//! Vector shapes: SVG path data (a template's artwork, an SVG's outlines) parsed into tiny-skia
//! paths, once per path, and fitted into their layer's box.

use cb_core::doc::Layer;
use resvg::tiny_skia::{FillRule, Mask, Path, PathBuilder, Stroke, Transform};
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::{Arc, Mutex, OnceLock};

/// SVG path data as a path in its own units (None when it draws nothing). A mistake part-way
/// through keeps what came before it, as browsers do.
pub fn parse(d: &str) -> Option<Path> {
    let mut pb = PathBuilder::new();
    for seg in svgtypes::SimplifyingPathParser::from(d) {
        let Ok(seg) = seg else { break };
        match seg {
            svgtypes::SimplePathSegment::MoveTo { x, y } => pb.move_to(x as f32, y as f32),
            svgtypes::SimplePathSegment::LineTo { x, y } => pb.line_to(x as f32, y as f32),
            svgtypes::SimplePathSegment::CurveTo { x1, y1, x2, y2, x, y } => pb.cubic_to(x1 as f32, y1 as f32, x2 as f32, y2 as f32, x as f32, y as f32),
            svgtypes::SimplePathSegment::Quadratic { x1, y1, x, y } => pb.quad_to(x1 as f32, y1 as f32, x as f32, y as f32),
            svgtypes::SimplePathSegment::ClosePath => pb.close(),
        }
    }
    pb.finish()
}

/// parsed paths by their data (a design's artwork is drawn over and over)
fn parsed(d: &str) -> Option<Arc<Path>> {
    static CACHE: OnceLock<Mutex<HashMap<u64, Option<Arc<Path>>>>> = OnceLock::new();
    let mut h = DefaultHasher::new();
    d.hash(&mut h);
    let key = h.finish();
    let cache = CACHE.get_or_init(Default::default);
    if let Some(p) = cache.lock().ok().and_then(|m| m.get(&key).cloned()) {
        return p;
    }
    let p = parse(d).map(Arc::new);
    if let Ok(mut m) = cache.lock() {
        if m.len() > 1024 {
            m.clear();
        }
        m.insert(key, p.clone());
    }
    p
}

/// A vector shape's outline in its layer's box: mm, centred on the layer's centre (so that
/// outline widths stay in mm however the shape is stretched).
pub fn layer_path(l: &Layer) -> Option<Path> {
    let spec = l.path.as_ref()?;
    let p = parsed(&spec.d)?;
    let sx = if spec.w > 1e-9 { l.w / spec.w } else { 1.0 };
    let sy = if spec.h > 1e-9 { l.h / spec.h } else { 1.0 };
    (*p).clone().transform(Transform::from_row(sx as f32, 0.0, 0.0, sy as f32, (-l.w / 2.0) as f32, (-l.h / 2.0) as f32))
}

pub fn fill_rule(l: &Layer) -> FillRule {
    if l.path.as_ref().is_some_and(|p| p.even_odd) {
        FillRule::EvenOdd
    } else {
        FillRule::Winding
    }
}

/// Whether the point (`x`, `y`) in the layer's own box (mm, centred, unrotated) is on the
/// painted part of `path` — inside its fill or on its outline — give or take `tol` mm.
pub fn paints(path: &Path, rule: FillRule, filled: bool, outline_w: Option<f64>, x: f64, y: f64, tol: f64) -> bool {
    let tol = tol.max(1e-3);
    // a 3 × 3 pixel mask round the point, one pixel per `tol`
    let Some(mut m) = Mask::new(3, 3) else { return false };
    let ts = Transform::from_scale((1.0 / tol) as f32, (1.0 / tol) as f32).post_translate((1.5 - x / tol) as f32, (1.5 - y / tol) as f32);
    if filled {
        m.fill_path(path, rule, true, ts);
    }
    if let Some(w) = outline_w.filter(|w| *w > 0.0) {
        let stroke = Stroke { width: w as f32, ..Default::default() };
        if let Some(o) = path.stroke(&stroke, 1.0) {
            m.fill_path(&o, FillRule::Winding, true, ts);
        }
    }
    m.data().iter().any(|&v| v > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cb_core::doc::{Side, VectorPath};

    #[test]
    fn path_data_fills_the_layer_box() {
        // a 10 × 5 unit triangle, stretched onto a 20 × 20 mm box
        let l = Layer::vector(Side::Front, 50.0, 50.0, 20.0, 20.0, VectorPath { d: "M0 0 L10 0 L0 5 Z".into(), w: 10.0, h: 5.0, even_odd: false }, Some("#000"));
        let p = layer_path(&l).unwrap();
        let b = p.bounds();
        assert_eq!((b.left(), b.top(), b.right(), b.bottom()), (-10.0, -10.0, 10.0, 10.0));
        // relative commands, arcs and curves parse too
        assert!(parse("m1 1 h4 v4 a2 2 0 0 1 -4 0 c0 -1 1 -2 2 -2 q1 0 1 1 z").is_some());
        // nonsense draws nothing; a mistake part-way keeps the part before it
        assert!(parse("hello").is_none());
        assert!(parse("M0 0 L5 5 L5 0 Z X 3").is_some());
    }

    #[test]
    fn hits_land_on_the_painted_part_only() {
        // a ring: a square with a square hole
        let ring = parse("M0 0H10V10H0Z M3 3H7V7H3Z").unwrap();
        assert!(paints(&ring, FillRule::EvenOdd, true, None, 1.0, 5.0, 0.2), "on the ring");
        assert!(!paints(&ring, FillRule::EvenOdd, true, None, 5.0, 5.0, 0.2), "in the hole");
        // an outline only: on the line, not inside it
        let sq = parse("M0 0H10V10H0Z").unwrap();
        assert!(paints(&sq, FillRule::Winding, false, Some(0.5), 10.1, 5.0, 0.2));
        assert!(!paints(&sq, FillRule::Winding, false, Some(0.5), 5.0, 5.0, 0.2));
    }
}
