//! Small 2D helpers (mm, y down).

/// a rounded rectangle as a polygon, clockwise on screen (y down), starting at the top edge.
/// `segments` = points per quarter circle.
pub fn rounded_rect(x: f64, y: f64, w: f64, h: f64, r: f64, segments: usize) -> Vec<(f64, f64)> {
    rounded_rect_corners(x, y, w, h, [r; 4], segments)
}

/// like [`rounded_rect`] with its own radius per corner: top-left, top-right, bottom-right, bottom-left
pub fn rounded_rect_corners(x: f64, y: f64, w: f64, h: f64, radii: [f64; 4], segments: usize) -> Vec<(f64, f64)> {
    let lim = (w / 2.0).min(h / 2.0).max(0.0);
    let r = radii.map(|r| r.clamp(0.0, lim));
    // corner centres and their start angles (y down: angle 0 = +x, 90° = +y)
    let corners = [
        (x + w - r[1], y + r[1], r[1], -90.0f64),
        (x + w - r[2], y + h - r[2], r[2], 0.0),
        (x + r[3], y + h - r[3], r[3], 90.0),
        (x + r[0], y + r[0], r[0], 180.0),
    ];
    let mut pts = Vec::new();
    for (cx, cy, rr, a0) in corners {
        if rr <= 1e-9 {
            // square corner: the centre is the corner point itself
            pts.push((cx, cy));
            continue;
        }
        let n = segments.max(1);
        for i in 0..=n {
            let a = (a0 + 90.0 * i as f64 / n as f64).to_radians();
            pts.push((cx + rr * a.cos(), cy + rr * a.sin()));
        }
    }
    pts
}

/// a point in a box of size w × h centred at (cx, cy), rotated by `rot` degrees
pub fn point_in_box(px: f64, py: f64, cx: f64, cy: f64, w: f64, h: f64, rot: f64) -> bool {
    let (lx, ly) = to_local(px, py, cx, cy, rot);
    lx.abs() <= w / 2.0 && ly.abs() <= h / 2.0
}

/// a point in the frame of a box centred at (cx, cy) and rotated by `rot` degrees
pub fn to_local(px: f64, py: f64, cx: f64, cy: f64, rot: f64) -> (f64, f64) {
    let (s, c) = (-rot.to_radians()).sin_cos();
    let (dx, dy) = (px - cx, py - cy);
    (dx * c - dy * s, dx * s + dy * c)
}

/// a point of a box's local frame back in the design frame
pub fn from_local(lx: f64, ly: f64, cx: f64, cy: f64, rot: f64) -> (f64, f64) {
    let (s, c) = rot.to_radians().sin_cos();
    (cx + lx * c - ly * s, cy + lx * s + ly * c)
}

/// the four corners of a rotated box (top-left, top-right, bottom-right, bottom-left)
pub fn box_corners(cx: f64, cy: f64, w: f64, h: f64, rot: f64) -> [(f64, f64); 4] {
    [(-w / 2.0, -h / 2.0), (w / 2.0, -h / 2.0), (w / 2.0, h / 2.0), (-w / 2.0, h / 2.0)].map(|(x, y)| from_local(x, y, cx, cy, rot))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounded_rect_stays_in_its_box() {
        for r in [0.0, 3.0, 100.0] {
            let pts = rounded_rect(0.0, 0.0, 85.0, 55.0, r, 6);
            assert!(pts.len() >= 4);
            for (x, y) in pts {
                assert!((-1e-9..=85.0 + 1e-9).contains(&x) && (-1e-9..=55.0 + 1e-9).contains(&y), "{x},{y} r={r}");
            }
        }
        assert_eq!(rounded_rect(0.0, 0.0, 10.0, 5.0, 0.0, 6), vec![(10.0, 0.0), (10.0, 5.0), (0.0, 5.0), (0.0, 0.0)]);
    }

    #[test]
    fn local_frames_round_trip() {
        let (lx, ly) = to_local(12.0, 7.0, 10.0, 5.0, 30.0);
        let (x, y) = from_local(lx, ly, 10.0, 5.0, 30.0);
        assert!((x - 12.0).abs() < 1e-9 && (y - 7.0).abs() < 1e-9);
        assert!(point_in_box(10.0, 5.0, 10.0, 5.0, 2.0, 2.0, 45.0));
        // the corner of a 2 × 2 box turned 45° reaches √2 along the x axis
        assert!(point_in_box(11.35, 5.0, 10.0, 5.0, 2.0, 2.0, 45.0));
        assert!(!point_in_box(11.35, 5.0, 10.0, 5.0, 2.0, 2.0, 0.0));
    }
}
