//! The card editor: both printed sides of the card on one canvas — front and back (or the
//! outside and inside of a folded card), side by side or one above the other, whichever shows
//! them bigger — each drawn at the zoom it is shown at (rendered in the background), with the
//! trim, bleed, safe-area and fold guides. Direct editing on either side: click to select, drag
//! to move (snapping to centre lines and the safe area), corners to resize, the side handles to
//! change a box's width, the knob to rotate. Empty space pans; the wheel zooms.
//!
//! What a press grabs is decided where the button goes down, so small handles work even when
//! the mouse moves a little before the drag registers (a lesson from ShirtBuilder).

use crate::app::Tool;
use crate::style::{semibold, P};
use crate::worker::Renderer;
use cb_core::doc::{Doc, Layer, LayerType, ShapeKind, Side};
use cb_core::geom::{box_corners, from_local, point_in_box, to_local};
use egui::{pos2, vec2, Color32, CornerRadius, FontId, Pos2, Rect, Sense, Shape, Stroke, Ui, Vec2};

const SIDES: [Side; 2] = [Side::Front, Side::Back];

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
    Grip { grip: Grip, start: Layer, p0: (f64, f64) },
}

/// a snap line shown while dragging: vertical (x = v) or horizontal (y = v), on a side
#[derive(Clone, Copy)]
struct Guide {
    side: Side,
    vertical: bool,
    v: f64,
}

pub struct CanvasView {
    /// screen points per mm
    scale: f32,
    /// screen position of the front's design origin, relative to the canvas corner
    origin: Vec2,
    rect: Rect,
    fit: bool,
    /// the sheet the view was last fitted to
    fitted_for: (f64, f64, f64),
    /// the back sits beside the front (true) or below it (false)
    row: bool,
    /// mm between the two sheets (bleed boxes)
    gap: f64,
    drag: Option<Drag>,
    /// front, back
    tex: [Option<egui::TextureHandle>; 2],
    guides: Vec<Guide>,
    /// the zoom the last frame showed, and when it last changed (s): the sides are rendered
    /// at exactly the screen's resolution once zooming stops
    shown_scale: f32,
    zoomed_at: f64,
}

impl Default for CanvasView {
    fn default() -> Self {
        CanvasView {
            scale: 6.0,
            origin: Vec2::ZERO,
            rect: Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0)),
            fit: true,
            fitted_for: (0.0, 0.0, 0.0),
            row: true,
            gap: 12.0,
            drag: None,
            tex: [None, None],
            guides: vec![],
            shown_scale: 0.0,
            zoomed_at: 0.0,
        }
    }
}

/// what the user did in the editor this frame
#[derive(Default)]
pub struct Events {
    pub select: Option<Option<String>>,
    /// a layer changed (with an undo key: edits with the same key merge)
    pub set_layer: Option<(Layer, String)>,
    /// a placing tool was clicked here (side, mm)
    pub place: Option<(Side, f64, f64)>,
    /// a text layer was double-clicked: type in its text box
    pub edit_text: bool,
    /// the user worked on this side (it becomes the one the sidebar edits)
    pub activate: Option<Side>,
}

pub struct Input<'a> {
    pub doc: &'a Doc,
    /// the side the sidebar is editing
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
    /// screen position of the front's design origin
    o: Pos2,
    /// where each side's design origin is, in mm from the front's
    off: [Vec2; 2],
}
impl Xf {
    fn s(&self, side: Side, x: f64, y: f64) -> Pos2 {
        self.o + (self.off[side.index()] + vec2(x as f32, y as f32)) * self.scale
    }
    fn d(&self, side: Side, p: Pos2) -> (f64, f64) {
        let v = (p - self.o) / self.scale - self.off[side.index()];
        (v.x as f64, v.y as f64)
    }
}

impl CanvasView {
    fn sheet_box(doc: &Doc) -> (f64, f64, f64) {
        let (sw, sh) = doc.format.sheet();
        let b = doc.format.bleed;
        (sw + 2.0 * b, sh + 2.0 * b, b)
    }
    fn xf(&self) -> Xf {
        let back = if self.row { vec2((self.fitted_for.0 + self.gap) as f32, 0.0) } else { vec2(0.0, (self.fitted_for.1 + self.gap) as f32) };
        Xf { scale: self.scale, o: self.rect.min + self.origin, off: [Vec2::ZERO, back] }
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
    /// screen position of a design point (mm) on `side`
    pub fn to_screen(&self, side: Side, x: f64, y: f64) -> Pos2 {
        self.xf().s(side, x, y)
    }
    /// the editor's picture of `side` (bleed box), if it has one
    pub fn texture_for(&self, side: Side) -> Option<egui::TextureId> {
        self.tex[side.index()].as_ref().map(|t| t.id())
    }
    pub fn has_texture(&self, side: Side) -> bool {
        self.tex[side.index()].is_some()
    }

    /// both sheets as large as the pane allows: side by side, or one above the other
    fn fit_to(&mut self, doc: &Doc) {
        let (bw, bh, b) = Self::sheet_box(doc);
        let gap = (bw.max(bh) * 0.1).max(8.0);
        // room: the toolbox on the left, the legend and side names above, the zoom bar below
        let avail = vec2((self.rect.width() - 110.0).max(80.0), (self.rect.height() - 140.0).max(80.0));
        let row_scale = (avail.x / (2.0 * bw + gap) as f32).min(avail.y / bh as f32);
        let col_scale = (avail.x / bw as f32).min((avail.y - 24.0) / (2.0 * bh + gap) as f32);
        self.row = row_scale >= col_scale;
        self.gap = gap;
        self.scale = row_scale.max(col_scale).clamp(0.5, 60.0);
        let total = if self.row { vec2((2.0 * bw + gap) as f32, bh as f32) } else { vec2(bw as f32, (2.0 * bh + gap) as f32) } * self.scale;
        let centre = vec2(self.rect.width() / 2.0 + 24.0, self.rect.height() / 2.0 + 4.0);
        // the front's trim origin sits one bleed in from the content's corner
        self.origin = centre - total / 2.0 + vec2(b as f32, b as f32) * self.scale;
        self.fitted_for = (bw, bh, b);
        self.fit = false;
    }

    /// the box a layer occupies (mm): text boxes follow their text
    fn layer_box(l: &Layer, fill: &dyn Fn(&str) -> String) -> (f64, f64) {
        cb_print::draw::layer_size(l, fill)
    }

    fn grips(xf: Xf, l: &Layer, w: f64, h: f64) -> Vec<(Grip, Pos2)> {
        let s = |x: f64, y: f64| xf.s(l.side, x, y);
        let mut out = Vec::new();
        for (i, (x, y)) in box_corners(l.x, l.y, w, h, l.rot).into_iter().enumerate() {
            out.push((Grip::Corner(i), s(x, y)));
        }
        let framed = l.kind == LayerType::Text && l.valign.is_some();
        let edges: &[(usize, f64, f64)] = match l.kind {
            LayerType::Qr => &[],
            LayerType::Text if !framed => &[(0, -0.5, 0.0), (1, 0.5, 0.0)],
            _ if l.shape == Some(ShapeKind::Line) => &[(0, -0.5, 0.0), (1, 0.5, 0.0)],
            _ => &[(0, -0.5, 0.0), (1, 0.5, 0.0), (2, 0.0, -0.5), (3, 0.0, 0.5)],
        };
        for &(i, fx, fy) in edges {
            let (x, y) = from_local(fx * w, fy * h, l.x, l.y, l.rot);
            out.push((Grip::Edge(i), s(x, y)));
        }
        // the rotate knob above the top edge
        let (tx, ty) = from_local(0.0, -h / 2.0, l.x, l.y, l.rot);
        let top = s(tx, ty);
        let a = (l.rot - 90.0).to_radians();
        out.push((Grip::Rotate, top + vec2(a.cos() as f32, a.sin() as f32) * 24.0));
        out
    }

    /// the sheet under a screen point (its bleed box, with a little margin)
    fn side_at(&self, xf: Xf, doc: &Doc, p: Pos2) -> Option<Side> {
        let (sw, sh) = doc.format.sheet();
        let b = doc.format.bleed;
        let m = 4.0 / xf.scale as f64 + 2.0;
        SIDES.into_iter().find(|&s| {
            let (x, y) = xf.d(s, p);
            x >= -b - m && x <= sw + b + m && y >= -b - m && y <= sh + b + m
        })
    }

    pub fn show(&mut self, ui: &mut Ui, rect: Rect, inp: &Input, render: &mut Renderer) -> Events {
        let mut ev = Events::default();
        let doc = inp.doc;
        let fmt = &doc.format;
        self.rect = rect;
        let (sw, sh) = fmt.sheet();
        let (bw, bh, b) = Self::sheet_box(doc);
        if self.fit || self.fitted_for != (bw, bh, b) {
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

        // ---- the rendered sides: at the screen's resolution (in steps, so zooming doesn't
        // re-render every frame), at most 4096 px across
        let ppp = ctx.pixels_per_point() as f64;
        let now = ui.input(|i| i.time);
        if self.scale != self.shown_scale {
            self.shown_scale = self.scale;
            self.zoomed_at = now;
        }
        let want = (self.scale as f64 * ppp).max(1.0);
        // while zooming: in steps; once it stops: exactly the screen's pixels, so the picture
        // is drawn one texel per pixel (no resampling blur)
        let settled = now - self.zoomed_at > 0.2;
        if !settled {
            ctx.request_repaint_after(std::time::Duration::from_millis(220));
        }
        let ppm = if settled { want } else { 2f64.powf((want.log2() * 4.0).ceil() / 4.0) };
        let cap = 4096.0 / bw.max(bh);
        render.want_preview(inp.record, ppm.min(cap).max(1.0));
        if let Some(p) = render.take_preview() {
            let opts = egui::TextureOptions { magnification: egui::TextureFilter::Linear, minification: egui::TextureFilter::Linear, ..Default::default() };
            for (i, (w, h, rgba)) in p.sides.iter().enumerate() {
                let img = egui::ColorImage::from_rgba_premultiplied([*w as usize, *h as usize], rgba);
                match &mut self.tex[i] {
                    Some(t) => t.set(img, opts),
                    None => self.tex[i] = Some(ctx.load_texture(format!("card-side-{i}"), img, opts)),
                }
            }
        }

        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, P.canvas);
        let xf = self.xf();
        for side in SIDES {
            self.paint_sheet(&painter, xf, doc, side, inp);
        }

        // ---- interaction
        let fill = doc.filler(inp.record);
        let ptr = ui.input(|i| i.pointer.interact_pos().or(i.pointer.hover_pos()));
        let (primary_pressed, middle_pressed, down, alt, shift) = ui.input(|i| {
            (i.pointer.primary_pressed(), i.pointer.button_pressed(egui::PointerButton::Middle), i.pointer.primary_down() || i.pointer.middle_down(), i.modifiers.alt, i.modifiers.shift)
        });
        let selected = inp.selected.and_then(|id| doc.layer(id));
        let hit = |p: Pos2, side: Side| -> Option<&Layer> {
            let (x, y) = xf.d(side, p);
            let tol = 3.0 / xf.scale as f64;
            doc.layers_on(side).filter(|l| l.visible && !l.locked).collect::<Vec<_>>().into_iter().rev().find(|l| {
                let (w, h) = Self::layer_box(l, &fill);
                point_in_box(x, y, l.x, l.y, w + 2.0 * tol, h + 2.0 * tol, l.rot)
            })
        };
        let placing = matches!(inp.tool, Tool::Text | Tool::Image | Tool::Shape | Tool::Qr);
        let side_under = ptr.and_then(|p| self.side_at(xf, doc, p));

        if resp.hovered() && (primary_pressed || middle_pressed) {
            if let Some(p) = ptr {
                if middle_pressed || inp.tool == Tool::Hand || ui.input(|i| i.key_down(egui::Key::Space)) {
                    self.drag = Some(Drag::Pan { last: p });
                } else if placing {
                    if let Some(side) = side_under {
                        let (x, y) = xf.d(side, p);
                        ev.place = Some((side, x.clamp(0.0, sw), y.clamp(0.0, sh)));
                    }
                } else {
                    // a handle of the selected layer first, then whatever is under the pointer
                    let grip = selected.filter(|l| !l.locked).and_then(|l| {
                        let (w, h) = Self::layer_box(l, &fill);
                        Self::grips(xf, l, w, h).into_iter().find(|(_, gp)| gp.distance(p) <= 9.0).map(|(g, _)| (g, l.clone()))
                    });
                    if let Some((grip, l)) = grip {
                        let p0 = xf.d(l.side, p);
                        self.drag = Some(Drag::Grip { grip, start: l, p0 });
                    } else if let Some(l) = side_under.and_then(|s| hit(p, s)) {
                        if inp.selected != Some(l.id.as_str()) {
                            ev.select = Some(Some(l.id.clone()));
                        }
                        if l.side != inp.side {
                            ev.activate = Some(l.side);
                        }
                        self.drag = Some(Drag::Move { start: l.clone(), p0: xf.d(l.side, p) });
                    } else {
                        if inp.selected.is_some() {
                            ev.select = Some(None);
                        }
                        if let Some(s) = side_under.filter(|s| *s != inp.side) {
                            ev.activate = Some(s);
                        }
                        self.drag = Some(Drag::Pan { last: p });
                    }
                }
            }
        }
        if resp.double_clicked() {
            match ptr.and_then(|p| side_under.and_then(|s| hit(p, s))) {
                Some(l) if l.kind == LayerType::Text => {
                    ev.select = Some(Some(l.id.clone()));
                    ev.activate = Some(l.side);
                    ev.edit_text = true;
                }
                Some(_) => {}
                None if side_under.is_none() => self.fit = true,
                None => {}
            }
        }
        self.guides.clear();
        if !down {
            self.drag = None;
        }
        if let (Some(drag), Some(p)) = (self.drag.clone(), ptr) {
            match drag {
                Drag::Pan { last } => {
                    self.origin += p - last;
                    self.drag = Some(Drag::Pan { last: p });
                }
                Drag::Move { start, p0 } => {
                    let (x, y) = xf.d(start.side, p);
                    let mut l = start.clone();
                    l.x = start.x + (x - p0.0);
                    l.y = start.y + (y - p0.1);
                    if inp.snap && !alt {
                        let (w, h) = Self::layer_box(&l, &fill);
                        self.snap(&mut l, w, h, doc);
                    }
                    if (l.x - start.x).abs() > 1e-9 || (l.y - start.y).abs() > 1e-9 {
                        let key = format!("move-{}", l.id);
                        ev.set_layer = Some((l, key));
                    }
                }
                Drag::Grip { grip, start, p0 } => {
                    let (x, y) = xf.d(start.side, p);
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
                    if l != start {
                        let key = format!("grip-{}", l.id);
                        ev.set_layer = Some((l, key));
                    }
                }
            }
        }

        // ---- hover outline, selection, handles, snap guides (after a pan, in the new place)
        let xf = self.xf();
        let shown = |id: &str| -> Option<Layer> {
            match &ev.set_layer {
                Some((l, _)) if l.id == id => Some(l.clone()),
                _ => doc.layer(id).cloned(),
            }
        };
        if self.drag.is_none() && resp.hovered() && !placing {
            if let Some(l) = ptr.and_then(|p| side_under.and_then(|s| hit(p, s))) {
                if inp.selected != Some(l.id.as_str()) {
                    let (w, h) = Self::layer_box(l, &fill);
                    let pts: Vec<Pos2> = box_corners(l.x, l.y, w, h, l.rot).into_iter().map(|(x, y)| xf.s(l.side, x, y)).collect();
                    painter.add(Shape::closed_line(pts, Stroke::new(1.0, P.accent.gamma_multiply(0.6))));
                }
            }
        }
        if let Some(l) = inp.selected.and_then(shown) {
            let (w, h) = Self::layer_box(&l, &fill);
            let s = |x: f64, y: f64| xf.s(l.side, x, y);
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
                            let r = l.rot.rem_euclid(180.0);
                            if (i % 2 == 0) == !(45.0..=135.0).contains(&r) {
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
                if let Some(Drag::Grip { grip, .. }) = self.drag {
                    let label = if grip == Grip::Rotate {
                        format!("{:.0}°", l.rot)
                    } else if l.kind == LayerType::Text {
                        format!("{:.1} pt · {:.1} mm wide", l.size.unwrap_or(10.0), l.w)
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
            let s = |x: f64, y: f64| xf.s(g.side, x, y);
            let (a, c) = if g.vertical { (s(g.v, -b - 3.0), s(g.v, sh + b + 3.0)) } else { (s(-b - 3.0, g.v), s(sw + b + 3.0, g.v)) };
            painter.line_segment([a, c], Stroke::new(1.0, SNAP_PINK));
        }

        // ---- cursor
        if self.drag.is_none() && resp.hovered() {
            let over_layer = ptr.and_then(|p| side_under.and_then(|s| hit(p, s))).is_some();
            let cursor = match inp.tool {
                Tool::Hand => egui::CursorIcon::Grab,
                Tool::Text if side_under.is_some() => egui::CursorIcon::Text,
                Tool::Image | Tool::Shape | Tool::Qr if side_under.is_some() => egui::CursorIcon::Crosshair,
                Tool::Select if over_layer => egui::CursorIcon::Move,
                _ => egui::CursorIcon::Default,
            };
            if ctx.output(|o| o.cursor_icon) == egui::CursorIcon::Default {
                ctx.set_cursor_icon(cursor);
            }
        } else if matches!(self.drag, Some(Drag::Pan { .. })) {
            ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
        }
        ev
    }

    /// one printed side: its picture, the veil outside the trim, guides, fold and panel names
    fn paint_sheet(&self, painter: &egui::Painter, xf: Xf, doc: &Doc, side: Side, inp: &Input) {
        let fmt = &doc.format;
        let (sw, sh) = fmt.sheet();
        let b = fmt.bleed;
        let s = |x: f64, y: f64| xf.s(side, x, y);
        let bleed_r = Rect::from_min_max(s(-b, -b), s(sw + b, sh + b));
        let trim_r = Rect::from_min_max(s(0.0, 0.0), s(sw, sh));
        let active = side == inp.side;
        // the side's name above it; the one the sidebar edits is highlighted
        let name = fmt.side_name(side).to_uppercase();
        let label_at = bleed_r.left_top() - vec2(0.0, 6.0);
        let g = painter.layout_no_wrap(name, FontId::new(11.5, semibold()), if active { P.accent } else { P.muted });
        if active {
            painter.circle_filled(label_at + vec2(4.0, -g.size().y / 2.0), 3.5, P.accent);
            painter.galley(label_at + vec2(12.0, -g.size().y), g, P.accent);
        } else {
            painter.galley(label_at - vec2(0.0, g.size().y), g, P.muted);
        }
        // soft shadow under the card
        painter.add(egui::epaint::RectShape::filled(trim_r.translate(vec2(0.0, 6.0)), CornerRadius::same((fmt.radius as f32 * xf.scale).min(60.0) as u8), Color32::from_black_alpha(70)).with_blur_width(22.0));
        match &self.tex[side.index()] {
            Some(t) => {
                // a picture made for this zoom goes on whole screen pixels, one texel each
                let ppp = painter.ctx().pixels_per_point();
                let [tw, th] = t.size();
                let exact = (tw as f32 - bleed_r.width() * ppp).abs() < 1.5 && (th as f32 - bleed_r.height() * ppp).abs() < 1.5;
                let r = if exact {
                    let min = (bleed_r.min.to_vec2() * ppp).round() / ppp;
                    Rect::from_min_size(min.to_pos2(), vec2(tw as f32, th as f32) / ppp)
                } else {
                    bleed_r
                };
                painter.image(t.id(), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
            }
            None => {
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
        painter.add(Shape::closed_line(outline, Stroke::new(if active { 1.4 } else { 1.0 }, if active { P.accent.gamma_multiply(0.85) } else { trim_ink })));

        // guides: bleed (red), safe area (teal)
        if inp.guides {
            let dash = |a: Pos2, c: Pos2, col: Color32| painter.extend(Shape::dashed_line(&[a, c], Stroke::new(1.0, col), 5.0, 4.0));
            let br = bleed_r;
            for (a, c) in [(br.left_top(), br.right_top()), (br.right_top(), br.right_bottom()), (br.right_bottom(), br.left_bottom()), (br.left_bottom(), br.left_top())] {
                dash(a, c, BLEED_RED.gamma_multiply(0.8));
            }
            for p in fmt.panels(side) {
                let m = fmt.safe;
                let r = Rect::from_min_max(s(p.x + m, p.y + m), s(p.x + p.w - m, p.y + p.h - m));
                for (a, c) in [(r.left_top(), r.right_top()), (r.right_top(), r.right_bottom()), (r.right_bottom(), r.left_bottom()), (r.left_bottom(), r.left_top())] {
                    dash(a, c, SAFE_TEAL.gamma_multiply(0.75));
                }
            }
        }
        // the fold, and the panel names inside each panel (so they never run into the other sheet)
        if let Some((a, c)) = fmt.fold_line() {
            let (a2, c2) = if fmt.fold == cb_core::Fold::Side { (s(a.0, a.1 - b), s(c.0, c.1 + b)) } else { (s(a.0 - b, a.1), s(c.0 + b, c.1)) };
            painter.extend(Shape::dashed_line(&[a2, c2], Stroke::new(1.2, P.accent), 7.0, 5.0));
            for p in fmt.panels(side) {
                let text = if p.upside_down { format!("{} · upside down", p.name) } else { p.name.to_string() };
                let g = painter.layout_no_wrap(text, FontId::proportional(10.0), Color32::WHITE);
                let r = Rect::from_min_size(s(p.x, p.y) + vec2(5.0, 5.0), g.size() + vec2(10.0, 4.0));
                painter.rect_filled(r, CornerRadius::same(5), Color32::from_black_alpha(120));
                painter.galley(r.min + vec2(5.0, 2.0), g, Color32::WHITE);
            }
        }
    }

    /// pull a moved layer onto nearby centre lines and safe-area edges (unrotated boxes)
    fn snap(&mut self, l: &mut Layer, w: f64, h: f64, doc: &Doc) {
        let th = 6.0 / self.scale as f64;
        let fmt = &doc.format;
        let (sw, sh) = fmt.sheet();
        let mut xs: Vec<(f64, f64, f64)> = vec![(sw / 2.0, 0.0, sw / 2.0)]; // (target, offset of the snapping point from the centre, guide)
        let mut ys: Vec<(f64, f64, f64)> = vec![(sh / 2.0, 0.0, sh / 2.0)];
        let straight = l.rot.rem_euclid(90.0).abs() < 1e-6;
        for p in fmt.panels(l.side) {
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
            self.guides.push(Guide { side: l.side, vertical: true, v: g });
        }
        if let Some((_, ny, g)) = best(l.y, &ys) {
            l.y = ny;
            self.guides.push(Guide { side: l.side, vertical: false, v: g });
        }
    }
}
