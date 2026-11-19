//! The card editor: the printed side drawn at the zoom it is shown at (rendered in the
//! background), the trim, bleed, safe-area and fold guides, and direct editing — click to
//! select, drag to move (snapping to centre lines and the safe area), corners to resize, the
//! side handles to change a box's width, the knob to rotate. Empty space pans; the wheel zooms.
//!
//! What a press grabs is decided where the button goes down, so small handles work even when
//! the mouse moves a little before the drag registers (a lesson from ShirtBuilder).

use crate::app::Tool;
use crate::style::P;
use crate::worker::Renderer;
use cb_core::doc::{Doc, Layer, LayerType, ShapeKind, Side};
use cb_core::geom::{box_corners, from_local, point_in_box, to_local};
use egui::{pos2, vec2, Align2, Color32, CornerRadius, FontId, Pos2, Rect, Sense, Shape, Stroke, Ui, Vec2};

#[derive(Clone, Copy, Debug, PartialEq)]
enum Grip {
    Corner(usize),
    /// 0 left, 1 right, 2 top, 3 bottom
    Edge(usize),
    Rotate,
}

#[derive(Clone, Debug)]
enum Drag {
    Pan { last: Pos2 },
    Move { start: Layer, p0: (f64, f64) },
    Grip { grip: Grip, start: Layer, p0: (f64, f64), h0: f64 },
}

struct Tex {
    handle: egui::TextureHandle,
    side: Side,
    ppm: f64,
}

/// a snap line shown while dragging: vertical (x = v) or horizontal (y = v)
#[derive(Clone, Copy)]
struct Guide {
    vertical: bool,
    v: f64,
}

pub struct CanvasView {
    /// screen points per mm
    scale: f32,
    /// screen position of the design origin, relative to the canvas corner
    origin: Vec2,
    rect: Rect,
    fit: bool,
    /// the sheet size the view was last fitted to
    fitted_for: (f64, f64),
    drag: Option<Drag>,
    tex: Option<Tex>,
    guides: Vec<Guide>,
}

impl Default for CanvasView {
    fn default() -> Self {
        CanvasView { scale: 6.0, origin: Vec2::ZERO, rect: Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0)), fit: true, fitted_for: (0.0, 0.0), drag: None, tex: None, guides: vec![] }
    }
}

/// what the user did in the editor this frame
#[derive(Default)]
pub struct Events {
    pub select: Option<Option<String>>,
    /// a layer changed (with an undo key: edits with the same key merge)
    pub set_layer: Option<(Layer, String)>,
    /// a placing tool was clicked here (mm)
    pub place: Option<(f64, f64)>,
    /// a text layer was double-clicked: type in its text box
    pub edit_text: bool,
}

pub struct Input<'a> {
    pub doc: &'a Doc,
    pub side: Side,
    pub record: Option<usize>,
    pub tool: Tool,
    pub selected: Option<&'a str>,
    pub guides: bool,
    pub snap: bool,
}

const BLEED_RED: Color32 = Color32::from_rgb(0xe5, 0x48, 0x4d);
const SAFE_TEAL: Color32 = Color32::from_rgb(0x3f, 0xb8, 0xaf);
const SNAP_PINK: Color32 = Color32::from_rgb(0xff, 0x4f, 0xb4);

/// the view transform of one frame (a copy, so drawing code can keep it while the view moves)
#[derive(Clone, Copy)]
struct Xf {
    scale: f32,
    o: Pos2,
}
impl Xf {
    fn s(&self, x: f64, y: f64) -> Pos2 {
        self.o + vec2(x as f32 * self.scale, y as f32 * self.scale)
    }
    fn d(&self, p: Pos2) -> (f64, f64) {
        let v = (p - self.o) / self.scale;
        (v.x as f64, v.y as f64)
    }
}

impl CanvasView {
    fn xf(&self) -> Xf {
        Xf { scale: self.scale, o: self.rect.min + self.origin }
    }
    pub fn request_fit(&mut self) {
        self.fit = true;
    }
    pub fn zoom_by(&mut self, k: f32) {
        let c = self.rect.center() - self.rect.min;
        self.zoom_at(c, k);
    }
    fn zoom_at(&mut self, at: Vec2, k: f32) {
        let new = (self.scale * k).clamp(0.5, 120.0);
        let k = new / self.scale;
        self.origin = at + (self.origin - at) * k;
        self.scale = new;
    }
    /// 100 % = the card at its real size on a 96 dpi screen
    pub fn zoom_percent(&self) -> f32 {
        self.scale / (96.0 / 25.4) * 100.0
    }
    pub fn to_screen(&self, x: f64, y: f64) -> Pos2 {
        self.rect.min + self.origin + vec2(x as f32 * self.scale, y as f32 * self.scale)
    }
    /// the editor's picture of `side` (bleed box), if it has one
    pub fn texture_for(&self, side: Side) -> Option<egui::TextureId> {
        self.tex.as_ref().filter(|t| t.side == side).map(|t| t.handle.id())
    }
    pub fn has_texture(&self, side: Side) -> bool {
        self.tex.as_ref().is_some_and(|t| t.side == side)
    }
    fn fit_to(&mut self, doc: &Doc) {
        let (sw, sh) = doc.format.sheet();
        let b = doc.format.bleed;
        let (bw, bh) = (sw + 2.0 * b, sh + 2.0 * b);
        let folded = doc.format.fold != cb_core::Fold::None;
        // room round the card: the floating toolbox on the left, the zoom bar below, and the
        // panel names (below a side-fold card, right of a top-fold one)
        let labels_right = if doc.format.fold == cb_core::Fold::Top { 120.0 } else { 0.0 };
        let avail = vec2((self.rect.width() - 150.0 - labels_right).max(80.0), (self.rect.height() - if folded { 150.0 } else { 120.0 }).max(80.0));
        self.scale = (avail.x / bw as f32).min(avail.y / bh as f32).clamp(0.5, 60.0);
        let size = vec2(sw as f32, sh as f32) * self.scale;
        let centre = vec2((self.rect.width() - labels_right) / 2.0 + 20.0, self.rect.height() / 2.0 - 12.0);
        self.origin = centre - size / 2.0;
        self.fitted_for = (sw, sh);
        self.fit = false;
    }

    /// the box a layer occupies (mm): text boxes follow their text
    fn layer_box(l: &Layer, fill: &dyn Fn(&str) -> String) -> (f64, f64) {
        cb_print::draw::layer_size(l, fill)
    }

    fn grips(xf: Xf, l: &Layer, w: f64, h: f64) -> Vec<(Grip, Pos2)> {
        let mut out = Vec::new();
        for (i, (x, y)) in box_corners(l.x, l.y, w, h, l.rot).into_iter().enumerate() {
            out.push((Grip::Corner(i), xf.s(x, y)));
        }
        let edges: &[(usize, f64, f64)] = match l.kind {
            LayerType::Qr => &[],
            LayerType::Text => &[(0, -0.5, 0.0), (1, 0.5, 0.0)],
            _ if l.shape == Some(ShapeKind::Line) => &[(0, -0.5, 0.0), (1, 0.5, 0.0)],
            _ => &[(0, -0.5, 0.0), (1, 0.5, 0.0), (2, 0.0, -0.5), (3, 0.0, 0.5)],
        };
        for &(i, fx, fy) in edges {
            let (x, y) = from_local(fx * w, fy * h, l.x, l.y, l.rot);
            out.push((Grip::Edge(i), xf.s(x, y)));
        }
        // the rotate knob above the top edge
        let (tx, ty) = from_local(0.0, -h / 2.0, l.x, l.y, l.rot);
        let top = xf.s(tx, ty);
        let up = {
            let a = (l.rot - 90.0).to_radians();
            vec2(a.cos() as f32, a.sin() as f32)
        };
        out.push((Grip::Rotate, top + up * 24.0));
        out
    }

    pub fn show(&mut self, ui: &mut Ui, rect: Rect, inp: &Input, render: &mut Renderer) -> Events {
        let mut ev = Events::default();
        let doc = inp.doc;
        let fmt = &doc.format;
        self.rect = rect;
        let (sw, sh) = fmt.sheet();
        if self.fit || self.fitted_for != (sw, sh) {
            self.fit_to(doc);
        }
        let ctx = ui.ctx().clone();
        let resp = ui.interact(rect, ui.scope_id().with("card-canvas"), Sense::click_and_drag());

        // ---- zoom: wheel / pinch round the cursor
        if resp.hovered() {
            let (scroll, zoom, ptr) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta(), i.pointer.hover_pos()));
            let at = ptr.map(|p| p - rect.min).unwrap_or(rect.center() - rect.min);
            if scroll != 0.0 && zoom == 1.0 {
                self.zoom_at(at, (scroll * 0.0018).exp());
            }
            if zoom != 1.0 {
                self.zoom_at(at, zoom);
            }
        }

        // ---- the rendered side: ask for it at the screen's resolution (in steps, so zooming
        // doesn't re-render every frame), at most 4096 px across
        let ppp = ctx.pixels_per_point() as f64;
        let b = fmt.bleed;
        let want = (self.scale as f64 * ppp).max(1.0);
        let step = 2f64.powf((want.log2() * 4.0).ceil() / 4.0);
        let cap = 4096.0 / (sw.max(sh) + 2.0 * b);
        let ppm = step.min(cap).max(1.0);
        render.want_preview(inp.side, inp.record, ppm);
        if let Some(p) = render.take_preview() {
            let img = egui::ColorImage::from_rgba_premultiplied([p.w as usize, p.h as usize], &p.rgba);
            let opts = egui::TextureOptions { magnification: egui::TextureFilter::Linear, minification: egui::TextureFilter::Linear, ..Default::default() };
            match &mut self.tex {
                Some(t) => {
                    t.handle.set(img, opts);
                    t.side = p.side;
                    t.ppm = p.ppm;
                }
                None => self.tex = Some(Tex { handle: ctx.load_texture("card-side", img, opts), side: p.side, ppm: p.ppm }),
            }
        }

        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, P.canvas);
        let xf = self.xf();
        let s = |x: f64, y: f64| xf.s(x, y);
        let bleed_r = Rect::from_min_max(s(-b, -b), s(sw + b, sh + b));
        let trim_r = Rect::from_min_max(s(0.0, 0.0), s(sw, sh));
        // soft shadow under the card
        painter.add(egui::epaint::RectShape::filled(trim_r.translate(vec2(0.0, 6.0)), CornerRadius::same((fmt.radius as f32 * self.scale).min(60.0) as u8), Color32::from_black_alpha(70)).with_blur_width(22.0));
        match &self.tex {
            Some(t) if t.side == inp.side => {
                painter.image(t.handle.id(), bleed_r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
            }
            _ => {
                painter.rect_filled(bleed_r, 0.0, crate::widgets::hex_color(&doc.paper.color));
            }
        }
        // outside the trim: veiled (it is cut off), with the rounded corners showing
        let veil = P.canvas.gamma_multiply(0.72);
        for r in [
            Rect::from_min_max(bleed_r.min, pos2(bleed_r.max.x, trim_r.min.y)),
            Rect::from_min_max(pos2(bleed_r.min.x, trim_r.max.y), bleed_r.max),
            Rect::from_min_max(pos2(bleed_r.min.x, trim_r.min.y), pos2(trim_r.min.x, trim_r.max.y)),
            Rect::from_min_max(pos2(trim_r.max.x, trim_r.min.y), pos2(bleed_r.max.x, trim_r.max.y)),
        ] {
            painter.rect_filled(r, 0.0, veil);
        }
        let outline: Vec<Pos2> = fmt.outline(10).into_iter().map(|(x, y)| s(x, y)).collect();
        if fmt.radius > 0.0 {
            let mut mesh = egui::Mesh::default();
            // each corner: a fan from the trim box corner to its arc
            let n = outline.len() / 4;
            let corners = [trim_r.right_top(), trim_r.right_bottom(), trim_r.left_bottom(), trim_r.left_top()];
            for (k, c) in corners.into_iter().enumerate() {
                let arc = &outline[k * n..(k + 1) * n];
                let base = mesh.vertices.len() as u32;
                mesh.colored_vertex(c, veil);
                for p in arc {
                    mesh.colored_vertex(*p, veil);
                }
                for i in 0..arc.len().saturating_sub(1) as u32 {
                    mesh.add_triangle(base, base + 1 + i, base + 2 + i);
                }
            }
            painter.add(Shape::mesh(mesh));
        }
        let trim_ink = if P.dark { Color32::from_white_alpha(150) } else { Color32::from_black_alpha(150) };
        painter.add(Shape::closed_line(outline.clone(), Stroke::new(1.0, trim_ink)));

        // ---- guides: bleed (red), safe area (teal), fold (accent), panel names
        if inp.guides {
            let dash = |a: Pos2, c: Pos2, col: Color32| painter.extend(Shape::dashed_line(&[a, c], Stroke::new(1.0, col), 5.0, 4.0));
            let br = bleed_r;
            for (a, c) in [(br.left_top(), br.right_top()), (br.right_top(), br.right_bottom()), (br.right_bottom(), br.left_bottom()), (br.left_bottom(), br.left_top())] {
                dash(a, c, BLEED_RED.gamma_multiply(0.8));
            }
            for p in fmt.panels(inp.side) {
                let m = fmt.safe;
                let r = Rect::from_min_max(s(p.x + m, p.y + m), s(p.x + p.w - m, p.y + p.h - m));
                for (a, c) in [(r.left_top(), r.right_top()), (r.right_top(), r.right_bottom()), (r.right_bottom(), r.left_bottom()), (r.left_bottom(), r.left_top())] {
                    dash(a, c, SAFE_TEAL.gamma_multiply(0.75));
                }
            }
        }
        if let Some((a, c)) = fmt.fold_line() {
            // the fold runs on through the bleed
            let (a2, c2) = if fmt.fold == cb_core::Fold::Side { (s(a.0, a.1 - b), s(c.0, c.1 + b)) } else { (s(a.0 - b, a.1), s(c.0 + b, c.1)) };
            painter.extend(Shape::dashed_line(&[a2, c2], Stroke::new(1.2, P.accent), 7.0, 5.0));
            let (lx, ly) = if fmt.fold == cb_core::Fold::Side { (a.0, -b) } else { (sw + b, a.1) };
            let at = s(lx, ly);
            let (pos, align) = if fmt.fold == cb_core::Fold::Side { (at - vec2(0.0, 4.0), Align2::CENTER_BOTTOM) } else { (at + vec2(6.0, 0.0), Align2::LEFT_CENTER) };
            painter.text(pos, align, "fold", FontId::proportional(10.5), P.accent);
        }
        if fmt.fold != cb_core::Fold::None {
            for p in fmt.panels(inp.side) {
                let (anchor, align) = if fmt.fold == cb_core::Fold::Side {
                    (s(p.x + p.w / 2.0, sh + b) + vec2(0.0, 6.0), Align2::CENTER_TOP)
                } else {
                    (s(sw + b, p.y + p.h / 2.0) + vec2(8.0, 0.0), Align2::LEFT_CENTER)
                };
                painter.text(anchor, align, p.name, FontId::proportional(11.0), P.muted);
                if p.upside_down {
                    painter.text(anchor + vec2(0.0, if fmt.fold == cb_core::Fold::Side { 16.0 } else { 14.0 }), align, "printed upside down", FontId::proportional(10.0), P.muted.gamma_multiply(0.8));
                }
            }
        }

        // ---- interaction
        let fill = doc.filler(inp.record);
        let ptr = ui.input(|i| i.pointer.interact_pos().or(i.pointer.hover_pos()));
        let (primary_pressed, middle_pressed, down, alt, shift) = ui.input(|i| {
            (i.pointer.primary_pressed(), i.pointer.button_pressed(egui::PointerButton::Middle), i.pointer.primary_down() || i.pointer.middle_down(), i.modifiers.alt, i.modifiers.shift)
        });
        let selected = inp.selected.and_then(|id| doc.layer(id)).filter(|l| l.side == inp.side);
        let hit = |p: Pos2| -> Option<&Layer> {
            let (x, y) = xf.d(p);
            let tol = 3.0 / xf.scale as f64;
            doc.layers_on(inp.side).filter(|l| l.visible && !l.locked).collect::<Vec<_>>().into_iter().rev().find(|l| {
                let (w, h) = Self::layer_box(l, &fill);
                point_in_box(x, y, l.x, l.y, w + 2.0 * tol, h + 2.0 * tol, l.rot)
            })
        };
        let placing = matches!(inp.tool, Tool::Text | Tool::Image | Tool::Shape | Tool::Qr);

        if resp.hovered() && (primary_pressed || middle_pressed) {
            if let Some(p) = ptr {
                if middle_pressed || inp.tool == Tool::Hand || ui.input(|i| i.key_down(egui::Key::Space)) {
                    self.drag = Some(Drag::Pan { last: p });
                } else if placing {
                    let (x, y) = xf.d(p);
                    ev.place = Some((x.clamp(0.0, sw), y.clamp(0.0, sh)));
                } else {
                    let d = xf.d(p);
                    // a handle of the selected layer first, then whatever is under the pointer
                    let grip = selected.filter(|l| !l.locked).and_then(|l| {
                        let (w, h) = Self::layer_box(l, &fill);
                        Self::grips(xf, l, w, h).into_iter().find(|(_, gp)| gp.distance(p) <= 9.0).map(|(g, _)| (g, l.clone(), h))
                    });
                    if let Some((grip, l, h0)) = grip {
                        self.drag = Some(Drag::Grip { grip, start: l, p0: d, h0 });
                    } else if let Some(l) = hit(p) {
                        if inp.selected != Some(l.id.as_str()) {
                            ev.select = Some(Some(l.id.clone()));
                        }
                        self.drag = Some(Drag::Move { start: l.clone(), p0: d });
                    } else {
                        if inp.selected.is_some() {
                            ev.select = Some(None);
                        }
                        self.drag = Some(Drag::Pan { last: p });
                    }
                }
            }
        }
        if resp.double_clicked() {
            match ptr.and_then(hit) {
                Some(l) if l.kind == LayerType::Text => {
                    ev.select = Some(Some(l.id.clone()));
                    ev.edit_text = true;
                }
                Some(_) => {}
                None => self.fit = true,
            }
        }
        self.guides.clear();
        if !down {
            self.drag = None;
        }
        if let (Some(drag), Some(p)) = (self.drag.clone(), ptr) {
            let (x, y) = xf.d(p);
            match drag {
                Drag::Pan { last } => {
                    self.origin += p - last;
                    self.drag = Some(Drag::Pan { last: p });
                }
                Drag::Move { start, p0 } => {
                    if start.locked {
                        return ev;
                    }
                    let mut l = start.clone();
                    l.x = start.x + (x - p0.0);
                    l.y = start.y + (y - p0.1);
                    if inp.snap && !alt {
                        let (w, h) = Self::layer_box(&l, &fill);
                        self.snap(&mut l, w, h, inp);
                    }
                    if (l.x - start.x).abs() > 1e-9 || (l.y - start.y).abs() > 1e-9 {
                        let key = format!("move-{}", l.id);
                        ev.set_layer = Some((l, key));
                    }
                }
                Drag::Grip { grip, start, p0, h0 } => {
                    let mut l = start.clone();
                    let (lx, ly) = to_local(x, y, start.x, start.y, start.rot);
                    match grip {
                        Grip::Corner(_) => {
                            let d0 = ((p0.0 - start.x).powi(2) + (p0.1 - start.y).powi(2)).sqrt().max(1e-6);
                            let d1 = ((x - start.x).powi(2) + (y - start.y).powi(2)).sqrt();
                            let k = (d1 / d0).max(0.02);
                            if shift && l.kind != LayerType::Qr && l.kind != LayerType::Text {
                                l.w = (2.0 * lx.abs()).max(1.0);
                                l.h = (2.0 * ly.abs()).max(0.5);
                            } else {
                                l.w = (start.w * k).max(1.0);
                                l.h = (start.h * k).max(0.5);
                                if let Some(sz) = start.size {
                                    l.size = Some((sz * k).clamp(2.0, 400.0));
                                }
                            }
                        }
                        Grip::Edge(i) => {
                            if i < 2 {
                                l.w = (2.0 * lx.abs()).max(1.0);
                            } else {
                                l.h = (2.0 * ly.abs()).max(0.5);
                            }
                        }
                        Grip::Rotate => {
                            let a = (y - start.y).atan2(x - start.x).to_degrees() + 90.0;
                            let mut a = (a + 360.0) % 360.0;
                            if shift {
                                a = (a / 15.0).round() * 15.0;
                            } else {
                                let q = (a / 90.0).round() * 90.0;
                                if (a - q).abs() < 4.0 {
                                    a = q;
                                }
                            }
                            l.rot = if a > 180.0 { a - 360.0 } else { a };
                        }
                    }
                    let _ = h0;
                    if l != start {
                        let key = format!("grip-{}", l.id);
                        ev.set_layer = Some((l, key));
                    }
                }
            }
        }

        // ---- hover outline, selection, handles, snap guides (after a pan, in the new place)
        let xf = self.xf();
        let s = |x: f64, y: f64| xf.s(x, y);
        let shown = |id: &str| -> Option<Layer> {
            match &ev.set_layer {
                Some((l, _)) if l.id == id => Some(l.clone()),
                _ => doc.layer(id).cloned(),
            }
        };
        if self.drag.is_none() {
            if let Some(l) = ptr.filter(|p| rect.contains(*p) && resp.hovered()).and_then(hit) {
                if inp.selected != Some(l.id.as_str()) && !placing {
                    let (w, h) = Self::layer_box(l, &fill);
                    let pts: Vec<Pos2> = box_corners(l.x, l.y, w, h, l.rot).into_iter().map(|(x, y)| s(x, y)).collect();
                    painter.add(Shape::closed_line(pts, Stroke::new(1.0, P.accent.gamma_multiply(0.6))));
                }
            }
        }
        if let Some(l) = inp.selected.and_then(shown).filter(|l| l.side == inp.side) {
            let (w, h) = Self::layer_box(&l, &fill);
            let pts: Vec<Pos2> = box_corners(l.x, l.y, w, h, l.rot).into_iter().map(|(x, y)| s(x, y)).collect();
            painter.add(Shape::closed_line(pts, Stroke::new(1.5, P.accent)));
            if !l.locked {
                let grips = Self::grips(xf, &l, w, h);
                let (tx, ty) = from_local(0.0, -h / 2.0, l.x, l.y, l.rot);
                let knob = grips.iter().find(|g| g.0 == Grip::Rotate).map(|g| g.1).unwrap_or_default();
                painter.line_segment([s(tx, ty), knob], Stroke::new(1.0, P.accent));
                let hot = ptr.and_then(|p| grips.iter().find(|(_, gp)| gp.distance(p) <= 9.0).map(|g| g.0));
                for (g, gp) in &grips {
                    let big = hot == Some(*g);
                    match g {
                        Grip::Rotate => {
                            painter.circle(*gp, if big { 6.5 } else { 5.0 }, Color32::WHITE, Stroke::new(1.5, P.accent));
                        }
                        Grip::Corner(_) => {
                            let r = Rect::from_center_size(*gp, Vec2::splat(if big { 11.0 } else { 9.0 }));
                            painter.rect(r, CornerRadius::same(2), Color32::WHITE, Stroke::new(1.5, P.accent), egui::StrokeKind::Middle);
                        }
                        Grip::Edge(_) => {
                            painter.circle(*gp, if big { 5.5 } else { 4.0 }, Color32::WHITE, Stroke::new(1.5, P.accent));
                        }
                    }
                }
                if let (Some(g), true) = (hot, self.drag.is_none()) {
                    let cursor = match g {
                        Grip::Rotate => egui::CursorIcon::Alias,
                        Grip::Corner(i) => {
                            if (i % 2 == 0) == ((l.rot.rem_euclid(180.0)) < 45.0 || l.rot.rem_euclid(180.0) > 135.0) {
                                egui::CursorIcon::ResizeNwSe
                            } else {
                                egui::CursorIcon::ResizeNeSw
                            }
                        }
                        Grip::Edge(i) if i < 2 => egui::CursorIcon::ResizeHorizontal,
                        Grip::Edge(_) => egui::CursorIcon::ResizeVertical,
                    };
                    ctx.set_cursor_icon(cursor);
                }
                if matches!(self.drag, Some(Drag::Grip { .. })) {
                    // the size while resizing
                    let label = if l.kind == LayerType::Text {
                        format!("{:.1} pt · {:.1} mm wide", l.size.unwrap_or(10.0), l.w)
                    } else if matches!(self.drag, Some(Drag::Grip { grip: Grip::Rotate, .. })) {
                        format!("{:.0}°", l.rot)
                    } else {
                        format!("{:.1} × {:.1} mm", l.w, l.h)
                    };
                    let at = s(l.x, l.y) + vec2(0.0, (h as f32 / 2.0) * self.scale + 26.0);
                    let g = painter.layout_no_wrap(label, FontId::proportional(11.0), Color32::WHITE);
                    let r = Rect::from_center_size(at, g.size() + vec2(14.0, 6.0));
                    painter.rect_filled(r, CornerRadius::same(6), P.accent_strong);
                    painter.galley(r.min + vec2(7.0, 3.0), g, Color32::WHITE);
                }
            }
        }
        for g in &self.guides {
            let (a, c) = if g.vertical { (s(g.v, -b - 4.0), s(g.v, sh + b + 4.0)) } else { (s(-b - 4.0, g.v), s(sw + b + 4.0, g.v)) };
            painter.line_segment([a, c], Stroke::new(1.0, SNAP_PINK));
        }

        // ---- cursor
        if self.drag.is_none() && resp.hovered() {
            let over_layer = ptr.and_then(hit).is_some();
            let cursor = match inp.tool {
                Tool::Hand => egui::CursorIcon::Grab,
                Tool::Text => egui::CursorIcon::Text,
                Tool::Image | Tool::Shape | Tool::Qr => egui::CursorIcon::Crosshair,
                Tool::Select if over_layer => egui::CursorIcon::Move,
                Tool::Select => egui::CursorIcon::Default,
            };
            if ctx.output(|o| o.cursor_icon) == egui::CursorIcon::Default {
                ctx.set_cursor_icon(cursor);
            }
        } else if matches!(self.drag, Some(Drag::Pan { .. })) {
            ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
        }
        ev
    }

    /// pull a moved layer onto nearby centre lines and safe-area edges (unrotated boxes)
    fn snap(&mut self, l: &mut Layer, w: f64, h: f64, inp: &Input) {
        let th = 6.0 / self.scale as f64;
        let fmt = &inp.doc.format;
        let (sw, sh) = fmt.sheet();
        let mut xs: Vec<(f64, f64, f64)> = Vec::new(); // (target, offset of the snapping point from the centre, guide)
        let mut ys: Vec<(f64, f64, f64)> = Vec::new();
        xs.push((sw / 2.0, 0.0, sw / 2.0));
        ys.push((sh / 2.0, 0.0, sh / 2.0));
        let straight = l.rot.rem_euclid(90.0).abs() < 1e-6;
        for p in fmt.panels(inp.side) {
            xs.push((p.x + p.w / 2.0, 0.0, p.x + p.w / 2.0));
            ys.push((p.y + p.h / 2.0, 0.0, p.y + p.h / 2.0));
            if straight {
                let (bw, bh) = if l.rot.rem_euclid(180.0) > 45.0 { (h, w) } else { (w, h) };
                let m = fmt.safe;
                xs.push((p.x + m, -bw / 2.0, p.x + m));
                xs.push((p.x + p.w - m, bw / 2.0, p.x + p.w - m));
                ys.push((p.y + m, -bh / 2.0, p.y + m));
                ys.push((p.y + p.h - m, bh / 2.0, p.y + p.h - m));
            }
        }
        let best = |v: f64, c: &[(f64, f64, f64)]| c.iter().map(|&(t, off, g)| ((v + off - t).abs(), t - off, g)).filter(|(d, _, _)| *d < th).min_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((_, nx, g)) = best(l.x, &xs) {
            l.x = nx;
            self.guides.push(Guide { vertical: true, v: g });
        }
        if let Some((_, ny, g)) = best(l.y, &ys) {
            l.y = ny;
            self.guides.push(Guide { vertical: false, v: g });
        }
    }
}
