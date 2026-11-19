//! The card as a 3D object: a thin slab of card stock with rounded corners — or, for a folded
//! card, two panels hinged at the fold and opened to an angle, standing on the table.
//!
//! World units are mm, y up, the camera's default view looks along −z. Each vertex says which
//! printed side it shows (`face`): 0 = front / outside print, 1 = back / inside print, 2 = paper
//! edge. Texture coordinates address that side's whole sheet (trim box, 0…1, v down).

use crate::format::{CardFormat, Fold};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub nrm: [f32; 3],
    pub uv: [f32; 2],
    pub face: f32,
}

#[derive(Clone, Debug, Default)]
pub struct CardMesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub min: [f32; 3],
    pub max: [f32; 3],
}

type V3 = [f64; 3];
fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn mul(a: V3, k: f64) -> V3 {
    [a[0] * k, a[1] * k, a[2] * k]
}
fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn cross(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn norm(a: V3) -> V3 {
    let l = dot(a, a).sqrt().max(1e-12);
    mul(a, 1.0 / l)
}
fn f3(a: V3) -> [f32; 3] {
    [a[0] as f32, a[1] as f32, a[2] as f32]
}

/// one flat piece of card: a region of the sheet placed in the world
struct Panel {
    /// sheet rectangle (x, y, w, h), mm
    rect: (f64, f64, f64, f64),
    /// corner radii: top-left, top-right, bottom-right, bottom-left
    radii: [f64; 4],
    /// world point of sheet point `p0`
    origin: V3,
    p0: (f64, f64),
    /// world direction of +x and +y on the sheet
    ex: V3,
    ey: V3,
    /// outward normal of the front / outside print
    n: V3,
    /// the slab spans origin + n·z for z in z0..z1 (the outside print at z1)
    z0: f64,
    z1: f64,
}

impl Panel {
    fn world(&self, x: f64, y: f64, z: f64) -> V3 {
        add(add(add(self.origin, mul(self.ex, x - self.p0.0)), mul(self.ey, y - self.p0.1)), mul(self.n, z))
    }
}

/// Build the card. `thickness` in mm; `open` = how far a folded card is opened, degrees
/// (0 closed … 180 flat). A flat card floats upright just above the table (y = 0).
pub fn card_mesh(fmt: &CardFormat, thickness: f64, open: f64) -> CardMesh {
    let (w, h) = (fmt.w, fmt.h);
    let (sw, sh) = fmt.sheet();
    let r = fmt.radius.min(w / 2.0).min(h / 2.0);
    let t = thickness.max(0.05);
    let half = (open.clamp(0.5, 180.0) / 2.0).to_radians();
    let (s, c) = half.sin_cos();
    let panels: Vec<Panel> = match fmt.fold {
        Fold::None => {
            let lift = (h * 0.16).max(6.0);
            vec![Panel { rect: (0.0, 0.0, w, h), radii: [r; 4], origin: [-w / 2.0, h + lift, 0.0], p0: (0.0, 0.0), ex: [1.0, 0.0, 0.0], ey: [0.0, -1.0, 0.0], n: [0.0, 0.0, 1.0], z0: -t / 2.0, z1: t / 2.0 }]
        }
        Fold::Side => {
            // spine on the y axis, the card opening towards +z like a book standing on the table
            let d_left = [-s, 0.0, c];
            let d_right = [s, 0.0, c];
            vec![
                // front cover: right half of the outside sheet, swings out to the left
                Panel { rect: (w, 0.0, w, h), radii: [0.0, r, r, 0.0], origin: [0.0, h, 0.0], p0: (w, 0.0), ex: d_left, ey: [0.0, -1.0, 0.0], n: [-c, 0.0, -s], z0: 0.0, z1: t },
                // back cover: left half, to the right
                Panel { rect: (0.0, 0.0, w, h), radii: [r, 0.0, 0.0, r], origin: [0.0, h, 0.0], p0: (w, 0.0), ex: mul(d_right, -1.0), ey: [0.0, -1.0, 0.0], n: [c, 0.0, -s], z0: 0.0, z1: t },
            ]
        }
        Fold::Top => {
            // a tent: the fold is the ridge along x, both panels reach down to the table
            let ridge = h * c;
            let d_front = [0.0, -c, s];
            let d_back = [0.0, -c, -s];
            vec![
                // front cover: bottom half of the outside sheet, facing the camera
                Panel { rect: (0.0, h, w, h), radii: [0.0, 0.0, r, r], origin: [-w / 2.0, ridge, 0.0], p0: (0.0, h), ex: [1.0, 0.0, 0.0], ey: d_front, n: [0.0, s, c], z0: 0.0, z1: t },
                // back cover: top half (printed upside down), facing away
                Panel { rect: (0.0, 0.0, w, h), radii: [r, r, 0.0, 0.0], origin: [-w / 2.0, ridge, 0.0], p0: (0.0, h), ex: [1.0, 0.0, 0.0], ey: mul(d_back, -1.0), n: [0.0, s, -c], z0: 0.0, z1: t },
            ]
        }
    };
    let mut m = CardMesh { min: [f32::MAX; 3], max: [f32::MIN; 3], ..Default::default() };
    for p in &panels {
        add_panel(&mut m, p, fmt, sw, sh);
    }
    for v in &m.vertices {
        for k in 0..3 {
            m.min[k] = m.min[k].min(v.pos[k]);
            m.max[k] = m.max[k].max(v.pos[k]);
        }
    }
    m
}

fn add_panel(m: &mut CardMesh, p: &Panel, fmt: &CardFormat, sw: f64, sh: f64) {
    let (x, y, w, h) = p.rect;
    let seg = 8;
    let outline = crate::geom::rounded_rect_corners(x, y, w, h, p.radii, seg);
    let (cx, cy) = (x + w / 2.0, y + h / 2.0);
    // the two printed faces: a fan round the centre (the outline is convex)
    for (z, face, out) in [(p.z1, 0.0f32, p.n), (p.z0, 1.0, mul(p.n, -1.0))] {
        let base = m.vertices.len() as u32;
        let uv = |px: f64, py: f64| -> [f32; 2] {
            if face == 0.0 {
                [(px / sw) as f32, (py / sh) as f32]
            } else {
                let (bx, by) = fmt.to_back(px, py);
                [(bx / sw) as f32, (by / sh) as f32]
            }
        };
        m.vertices.push(Vertex { pos: f3(p.world(cx, cy, z)), nrm: f3(out), uv: uv(cx, cy), face });
        for &(px, py) in &outline {
            m.vertices.push(Vertex { pos: f3(p.world(px, py, z)), nrm: f3(out), uv: uv(px, py), face });
        }
        let n = outline.len() as u32;
        for i in 0..n {
            let (a, b) = (base + 1 + i, base + 1 + (i + 1) % n);
            push_tri(m, [base, a, b], out);
        }
    }
    // the paper edge
    let n = outline.len();
    for i in 0..n {
        let (ax, ay) = outline[i];
        let (bx, by) = outline[(i + 1) % n];
        if (ax - bx).abs() < 1e-9 && (ay - by).abs() < 1e-9 {
            continue;
        }
        // outward in the sheet plane: away from the centre
        let (mx, my) = ((ax + bx) / 2.0, (ay + by) / 2.0);
        let (ex, ey) = (by - ay, -(bx - ax));
        let flip = if (mx - cx) * ex + (my - cy) * ey < 0.0 { -1.0 } else { 1.0 };
        let out2 = norm(add(mul(p.ex, ex * flip), mul(p.ey, ey * flip)));
        let base = m.vertices.len() as u32;
        for (px, py, z) in [(ax, ay, p.z0), (bx, by, p.z0), (bx, by, p.z1), (ax, ay, p.z1)] {
            m.vertices.push(Vertex { pos: f3(p.world(px, py, z)), nrm: f3(out2), uv: [0.0, 0.0], face: 2.0 });
        }
        push_tri(m, [base, base + 1, base + 2], out2);
        push_tri(m, [base, base + 2, base + 3], out2);
    }
}

/// add a triangle wound counter-clockwise as seen from the side `facing` points to
fn push_tri(m: &mut CardMesh, t: [u32; 3], facing: V3) {
    let p = |i: u32| {
        let v = m.vertices[i as usize].pos;
        [v[0] as f64, v[1] as f64, v[2] as f64]
    };
    let nrm = cross(sub(p(t[1]), p(t[0])), sub(p(t[2]), p(t[0])));
    if dot(nrm, facing) >= 0.0 {
        m.indices.extend_from_slice(&t);
    } else {
        m.indices.extend_from_slice(&[t[0], t[2], t[1]]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::preset;

    fn tri_normal(m: &CardMesh, i: usize) -> V3 {
        let p = |k: usize| {
            let v = m.vertices[m.indices[i * 3 + k] as usize].pos;
            [v[0] as f64, v[1] as f64, v[2] as f64]
        };
        norm(cross(sub(p(1), p(0)), sub(p(2), p(0))))
    }

    #[test]
    fn triangles_face_their_normals_and_sit_on_or_above_the_table() {
        for id in ["bc-eu", "id-cr80", "greet-a6", "greet-top", "tent-place"] {
            let fmt = CardFormat::from_preset(preset(id).unwrap());
            for open in [0.0, 90.0, 180.0] {
                let m = card_mesh(&fmt, 0.4, open);
                assert!(!m.indices.is_empty());
                for i in 0..m.indices.len() / 3 {
                    let v = m.vertices[m.indices[i * 3] as usize];
                    let n = [v.nrm[0] as f64, v.nrm[1] as f64, v.nrm[2] as f64];
                    let tn = tri_normal(&m, i);
                    if dot(tn, tn) > 0.5 {
                        assert!(dot(tn, n) > 0.99, "{id} {open}°: triangle {i} faces {tn:?}, normal {n:?}");
                    }
                }
                assert!(m.min[1] > -0.5, "{id} {open}°: below the table ({})", m.min[1]);
            }
        }
    }

    #[test]
    fn the_front_print_faces_the_camera() {
        // flat card and the tent's front cover: front print (face 0) with normals towards +z
        for id in ["bc-eu", "tent-place"] {
            let fmt = CardFormat::from_preset(preset(id).unwrap());
            let m = card_mesh(&fmt, 0.4, 70.0);
            // the front cover's print: face 0 with v (sheet y) in the front panel
            let front: Vec<_> = m.vertices.iter().filter(|v| v.face == 0.0 && v.uv[1] > 0.55 || (v.face == 0.0 && fmt.fold == crate::format::Fold::None)).collect();
            assert!(!front.is_empty());
            for v in front {
                assert!(v.nrm[2] > 0.3, "{id}: {:?}", v.nrm);
            }
        }
        // the open greeting card shows its inside (face 1) to the camera
        let fmt = CardFormat::from_preset(preset("greet-a6").unwrap());
        let m = card_mesh(&fmt, 0.4, 180.0);
        assert!(m.vertices.iter().filter(|v| v.face == 1.0).all(|v| v.nrm[2] > 0.99));
        assert!(m.vertices.iter().filter(|v| v.face == 0.0).all(|v| v.nrm[2] < -0.99));
    }

    #[test]
    fn texture_coordinates_read_the_right_way_round() {
        // flat card seen from the front: sheet x grows to the right (+x), y downwards (−y)
        let fmt = CardFormat::from_preset(preset("bc-eu").unwrap());
        let m = card_mesh(&fmt, 0.4, 0.0);
        let f: Vec<_> = m.vertices.iter().filter(|v| v.face == 0.0).collect();
        let left = f.iter().min_by(|a, b| a.pos[0].total_cmp(&b.pos[0])).unwrap();
        let right = f.iter().max_by(|a, b| a.pos[0].total_cmp(&b.pos[0])).unwrap();
        assert!(left.uv[0] < 0.05 && right.uv[0] > 0.95);
        let top = f.iter().max_by(|a, b| a.pos[1].total_cmp(&b.pos[1])).unwrap();
        assert!(top.uv[1] < 0.05);
        // the back seen from behind (−z): its left is world +x, so back u = 0 at the largest x
        let b: Vec<_> = m.vertices.iter().filter(|v| v.face == 1.0).collect();
        let max_x = b.iter().max_by(|a, b| a.pos[0].total_cmp(&b.pos[0])).unwrap();
        assert!(max_x.uv[0] < 0.05, "{:?}", max_x.uv);
    }
}
