//! The 3D preview: the card in its paper stock, standing in a little photo studio. Orbit with
//! the mouse (right-drag pans, the wheel zooms), animated views (front, ¾, back, inside), a flip,
//! auto-rotate, a view gizmo (`gizmo.rs`) and a 2000 px snapshot. Textures come from the background renderer
//! (`worker.rs`); drawing happens on the GPU (`gpu.rs`).

mod gizmo;
pub mod gpu;

use crate::app::View3d;
use crate::worker::Textures;
use cb_core::doc::{Backdrop, Doc};
use cb_core::Fold;
use eframe::egui_wgpu::{self, wgpu};
use egui::{pos2, Color32, Rect, Sense};
use glam::{Mat4, Vec3};
use gpu::{FrameParams, Globals, Gpu3d, Paint3d, SnapshotState};
use std::f64::consts::PI;

fn lin(c: f64) -> f32 {
    let c = c / 255.0;
    (if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }) as f32
}
fn lin_hex(hex: &str) -> [f32; 3] {
    let [r, g, b] = cb_print::color::hex_to_rgb(hex);
    [lin(r), lin(g), lin(b)]
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Cam {
    yaw: f64,
    pitch: f64,
    /// distance as a multiple of the fitted distance
    zoom: f64,
}

struct Anim {
    from: Cam,
    to: Cam,
    t0: f64,
    dur: f64,
    /// the pan and zoom it started from, when it also brings them back to normal (home view)
    home: Option<(Vec3, f64)>,
}

pub struct Viewer3d {
    rs: Option<egui_wgpu::RenderState>,
    pub gpu_ok: bool,
    cam: Cam,
    pan: Vec3,
    anim: Option<Anim>,
    /// what the card mesh was built for
    mesh_key: Option<String>,
    center: Vec3,
    radius: f32,
    last_time: f64,
    pub snapshot_pending: bool,
    /// the camera of the last frame (for snapshots)
    last_params: Option<FrameParams>,
    /// put the camera on the ¾ view of the card shown next (start, new format)
    fresh: bool,
}

/// the backdrop's colours (top, bottom)
fn backdrop_colors(doc: &Doc) -> (Color32, Color32) {
    let c = crate::widgets::hex_color;
    match doc.scene.backdrop {
        Backdrop::Studio => (c("#f1f1f6"), c("#c3c5d6")),
        Backdrop::Dark => (c("#3a3d52"), c("#101119")),
        Backdrop::Desk => (c("#e9dcc8"), c("#9c7a58")),
        Backdrop::Solid => {
            let base = &doc.scene.bg_color;
            (c(&cb_print::color::mix(base, "#ffffff", 0.18)), c(&cb_print::color::shade(base, 0.82)))
        }
    }
}

impl Viewer3d {
    pub fn new(rs: Option<egui_wgpu::RenderState>) -> Viewer3d {
        if let Some(rs) = &rs {
            gpu::install(rs);
        }
        Viewer3d {
            gpu_ok: rs.is_some(),
            rs,
            cam: Cam { yaw: 0.55, pitch: 0.22, zoom: 1.0 },
            pan: Vec3::ZERO,
            anim: None,
            mesh_key: None,
            center: Vec3::new(0.0, 30.0, 0.0),
            radius: 50.0,
            last_time: 0.0,
            snapshot_pending: false,
            last_params: None,
            fresh: true,
        }
    }

    fn with_gpu<R>(&self, f: impl FnOnce(&wgpu::Device, &wgpu::Queue, &mut Gpu3d) -> R) -> Option<R> {
        let rs = self.rs.as_ref()?;
        let mut renderer = rs.renderer.write();
        let gpu = renderer.callback_resources.get_mut::<Gpu3d>()?;
        Some(f(&rs.device, &rs.queue, gpu))
    }

    /// new pictures of the card's sides
    pub fn set_textures(&mut self, t: Textures) {
        self.with_gpu(|device, queue, gpu| {
            for (i, s) in t.sides.iter().enumerate() {
                gpu.write_texture(device, queue, i, s.w, s.h, &s.color);
                gpu.write_texture(device, queue, 2 + i, s.mask_w, s.mask_h, std::slice::from_ref(&s.mask));
            }
        });
    }

    /// rebuild the card when its shape, stock or opening changes
    fn sync_mesh(&mut self, doc: &Doc) {
        let thick = doc.paper.stock.info().thickness;
        let open = if doc.format.fold == Fold::None { 0.0 } else { doc.scene.open };
        let pair = doc.format.fold == Fold::None && doc.scene.pair;
        // the shape part of the key (before the first '|') decides when to re-frame the camera
        let key = format!("{:?} pair={pair}|{thick}|{open:.1}", doc.format);
        if self.mesh_key.as_deref() == Some(key.as_str()) {
            return;
        }
        let shape_changed = self.mesh_key.as_ref().is_none_or(|k| k.split('|').next() != key.split('|').next());
        self.mesh_key = Some(key);
        let build = |open: f64| if pair { cb_core::card3d::card_pair(&doc.format, thick) } else { cb_core::card3d::card_mesh(&doc.format, thick, open) };
        let m = build(open);
        let v: Vec<gpu::Vertex> = m.vertices.iter().map(|v| gpu::Vertex { pos: v.pos, nrm: v.nrm, uv: v.uv, face: v.face }).collect();
        let (lo, hi) = (Vec3::from(m.min), Vec3::from(m.max));
        // frame the card as it stands (the centre doesn't jump while opening it)
        if shape_changed {
            let full = build(if doc.format.fold == Fold::None { 0.0 } else { 110.0 });
            let (a, b) = (Vec3::from(full.min), Vec3::from(full.max));
            self.center = (a + b) / 2.0;
            self.radius = ((b - a).length() / 2.0).max(10.0);
        }
        let floor_r = (hi - lo).length().max(self.radius * 2.0) * 1.6;
        self.with_gpu(|device, queue, gpu| {
            gpu.set_card(device, queue, &v, &m.indices);
            gpu.set_floor(device, queue, floor_r);
        });
    }

    fn view_cam(&self, v: View3d, doc: &Doc) -> Cam {
        let half = (doc.scene.open.clamp(0.0, 180.0) / 2.0).to_radians();
        let zoom = 1.0;
        match (doc.format.fold, v) {
            (Fold::None, View3d::Front | View3d::Inside) => Cam { yaw: 0.0, pitch: 0.08, zoom },
            // the pair: both cards in view, a little from the side
            (Fold::None, View3d::Angle) if doc.scene.pair => Cam { yaw: 0.32, pitch: 0.2, zoom },
            (Fold::None, View3d::Angle) => Cam { yaw: 0.55, pitch: 0.22, zoom },
            (Fold::None, View3d::Back) => Cam { yaw: PI, pitch: 0.08, zoom },
            // book-style card: the front cover faces out to the left, the back cover to the right
            (Fold::Side, View3d::Front) => Cam { yaw: (-half.cos()).atan2(-half.sin()), pitch: 0.15, zoom },
            (Fold::Side, View3d::Back) => Cam { yaw: half.cos().atan2(-half.sin()), pitch: 0.15, zoom },
            (Fold::Side, View3d::Inside) => Cam { yaw: 0.0, pitch: 0.2, zoom },
            // the front cover at an angle, with a glimpse inside
            (Fold::Side, View3d::Angle) => Cam { yaw: (-half.cos()).atan2(-half.sin()) + 0.55, pitch: 0.25, zoom },
            (Fold::Top, View3d::Front) => Cam { yaw: 0.0, pitch: 0.18, zoom },
            (Fold::Top, View3d::Back) => Cam { yaw: PI, pitch: 0.18, zoom },
            (Fold::Top, View3d::Inside) => Cam { yaw: 0.0, pitch: -0.4, zoom },
            (Fold::Top, View3d::Angle) => Cam { yaw: 0.6, pitch: 0.3, zoom },
        }
    }

    fn animate_to(&mut self, mut to: Cam, now: f64, dur: f64) {
        // the short way round
        let d = (to.yaw - self.cam.yaw).rem_euclid(2.0 * PI);
        to.yaw = self.cam.yaw + if d > PI { d - 2.0 * PI } else { d };
        self.anim = Some(Anim { from: self.cam, to, t0: now, dur, home: None });
    }

    pub fn set_view(&mut self, v: View3d, doc: &Doc, now: f64) {
        let to = self.view_cam(v, doc);
        self.pan = Vec3::ZERO;
        self.animate_to(to, now, 0.6);
    }

    /// turn the card over (half a turn)
    pub fn flip(&mut self, now: f64) {
        let from = self.anim.as_ref().map(|a| a.to).unwrap_or(self.cam);
        self.anim = Some(Anim { from: self.cam, to: Cam { yaw: from.yaw + PI, ..from }, t0: now, dur: 0.8, home: None });
    }

    /// turn the camera round the card by a mouse drag of `d` points
    fn orbit(&mut self, d: egui::Vec2) {
        self.anim = None;
        self.cam.yaw -= d.x as f64 * 0.008;
        self.cam.pitch = (self.cam.pitch + d.y as f64 * 0.006).clamp(-gizmo::MAX_PITCH, gizmo::MAX_PITCH);
    }

    /// the camera's (yaw, pitch, zoom), for tests
    pub fn camera(&self) -> (f64, f64, f64) {
        (self.cam.yaw, self.cam.pitch, self.cam.zoom)
    }

    pub fn reset_view(&mut self) {
        self.cam = Cam { yaw: 0.55, pitch: 0.22, zoom: 1.0 };
        self.fresh = true;
        self.pan = Vec3::ZERO;
        self.anim = None;
        self.mesh_key = None;
    }

    fn eye(&self, aspect: f32) -> (Vec3, Vec3) {
        let fov = 30f32.to_radians();
        // fit the bounding sphere in the narrower field of view
        let fit_fov = if aspect < 1.0 { 2.0 * ((fov / 2.0).tan() * aspect).atan() } else { fov };
        let dist = self.radius / (fit_fov / 2.0).sin() * 1.08 * self.cam.zoom as f32;
        let (y, p) = (self.cam.yaw as f32, self.cam.pitch as f32);
        let dir = Vec3::new(y.sin() * p.cos(), p.sin(), y.cos() * p.cos());
        let target = self.center + self.pan;
        (target + dir * dist, target)
    }

    fn params(&self, doc: &Doc, size: [u32; 2], time: f64) -> FrameParams {
        let aspect = size[0] as f32 / size[1].max(1) as f32;
        let (eye, target) = self.eye(aspect);
        let dist = (eye - target).length();
        let view = Mat4::look_at_rh(eye, target, Vec3::Y);
        let proj = Mat4::perspective_rh(30f32.to_radians(), aspect, (dist * 0.05).max(1.0), dist + self.radius * 8.0);
        // key light: from the chosen side, high and in front
        // the studio light turns with the camera, so whichever side you look at is lit
        let az = (doc.scene.light_angle as f32).to_radians() + self.cam.yaw as f32;
        let el = 52f32.to_radians();
        let key = Vec3::new(az.sin() * el.cos(), el.sin(), az.cos() * el.cos()).normalize();
        let r = self.radius * 2.2;
        let lview = Mat4::look_at_rh(self.center + key * r * 2.0, self.center, if key.y.abs() > 0.99 { Vec3::Z } else { Vec3::Y });
        let lproj = Mat4::orthographic_rh(-r, r, -r, r, 0.1, r * 4.0);
        let stock = doc.paper.stock.info();
        let p = lin_hex(&doc.paper.color);
        let (top, bottom) = backdrop_colors(doc);
        let to_lin = |c: Color32| [lin(c.r() as f64), lin(c.g() as f64), lin(c.b() as f64)];
        let sky = to_lin(top);
        let ground = to_lin(bottom);
        let dark = doc.scene.backdrop == Backdrop::Dark;
        let fill = Vec3::new(-key.x, 0.35, key.z.abs() * 0.6 + 0.4).normalize();
        let g = Globals {
            view_proj: (proj * view).to_cols_array_2d(),
            light_vp: (lproj * lview).to_cols_array_2d(),
            cam_pos: [eye.x, eye.y, eye.z, 1.0],
            key_dir: [key.x, key.y, key.z, (2.2 * doc.scene.light) as f32],
            key_col: [1.0, 0.97, 0.92, 1.0],
            fill_dir: [fill.x, fill.y, fill.z, if dark { 0.18 } else { 0.32 }],
            sky: [sky[0] * 0.9, sky[1] * 0.9, sky[2] * 0.95, 1.0],
            ground: [ground[0] * 0.6, ground[1] * 0.6, ground[2] * 0.6, 1.0],
            paper: [p[0], p[1], p[2], stock.roughness as f32],
            stock: [stock.gloss as f32, stock.grain as f32, if doc.scene.shadows { 1.0 } else { 0.0 }, if dark { 0.5 } else { 0.3 }],
            misc: [1.0, (time % 1000.0) as f32, 1.0 / gpu::SHADOW as f32, self.radius * 2.6],
        };
        FrameParams { globals: g, size }
    }

    /// draw and handle the 3D view in `rect`; returns the view the pane's header should show
    /// when the view gizmo picks one
    pub fn show(&mut self, ui: &mut egui::Ui, rect: Rect, doc: &Doc, now: f64) -> Option<View3d> {
        let resp = ui.interact(rect, ui.scope_id().with("view3d"), Sense::click_and_drag());
        // backdrop
        let (top, bottom) = backdrop_colors(doc);
        let painter = ui.painter_at(rect);
        let mut mesh = egui::Mesh::default();
        for (p, c) in [(rect.left_top(), top), (rect.right_top(), top), (rect.right_bottom(), bottom), (rect.left_bottom(), bottom)] {
            mesh.colored_vertex(p, c);
        }
        mesh.add_triangle(0, 1, 2);
        mesh.add_triangle(0, 2, 3);
        painter.add(egui::Shape::mesh(mesh));
        if doc.scene.backdrop == Backdrop::Studio || doc.scene.backdrop == Backdrop::Solid {
            crate::themes::glow(&painter, pos2(rect.center().x, rect.top() + rect.height() * 0.42), rect.width() * 0.55, rect.height() * 0.5, Color32::from_white_alpha(70));
        }
        if !self.gpu_ok {
            painter.text(rect.center(), egui::Align2::CENTER_CENTER, "3D preview needs a graphics card driver (Vulkan, DirectX 12 or OpenGL)", egui::FontId::proportional(13.0), Color32::from_gray(90));
            return None;
        }
        self.sync_mesh(doc);
        if self.fresh {
            self.cam = self.view_cam(View3d::Angle, doc);
            self.fresh = false;
        }

        // input
        let dt = (now - self.last_time).clamp(0.0, 0.1);
        self.last_time = now;
        if resp.dragged_by(egui::PointerButton::Primary) {
            self.orbit(resp.drag_delta());
        }
        if resp.dragged_by(egui::PointerButton::Secondary) || resp.dragged_by(egui::PointerButton::Middle) {
            let d = resp.drag_delta();
            let aspect = rect.width() / rect.height().max(1.0);
            let (eye, target) = self.eye(aspect);
            let fwd = (target - eye).normalize();
            let right = fwd.cross(Vec3::Y).normalize();
            let up = right.cross(fwd);
            let k = (eye - target).length() * 0.0018;
            self.pan += (-right * d.x + up * d.y) * k;
        }
        if resp.hovered() {
            let (scroll, zoom) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
            if scroll != 0.0 {
                self.cam.zoom = (self.cam.zoom * (-scroll as f64 * 0.0015).exp()).clamp(0.25, 4.0);
            }
            if zoom != 1.0 {
                self.cam.zoom = (self.cam.zoom / zoom as f64).clamp(0.25, 4.0);
            }
        }
        if resp.double_clicked() {
            self.cam.zoom = 1.0;
            self.pan = Vec3::ZERO;
        }
        if let Some(a) = &self.anim {
            let t = ((now - a.t0) / a.dur).clamp(0.0, 1.0);
            let e = t * t * (3.0 - 2.0 * t);
            let lerp = |x: f64, y: f64| x + (y - x) * e;
            let mut zoom = self.cam.zoom;
            if let Some((pan, z)) = gizmo::home_step(a, e) {
                self.pan = pan;
                zoom = z;
            }
            self.cam = Cam { yaw: lerp(a.from.yaw, a.to.yaw), pitch: lerp(a.from.pitch, a.to.pitch), zoom };
            if t >= 1.0 {
                self.anim = None;
            }
            ui.ctx().request_repaint();
        }
        if doc.scene.auto_rotate && !resp.dragged() && !ui.ctx().is_being_dragged(gizmo::area_id(ui)) {
            self.cam.yaw += dt * 0.45;
            ui.ctx().request_repaint();
        }
        if resp.dragged() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        } else if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        }

        let ppp = ui.ctx().pixels_per_point();
        // exactly the pixels egui gives the paint callback (it rounds each edge), so the blit
        // copies pixel for pixel instead of resampling the picture
        let span = |a: f32, b: f32| ((b * ppp).round() - (a * ppp).round()).max(1.0) as u32;
        let size = [span(rect.min.x, rect.max.x), span(rect.min.y, rect.max.y)];
        let params = self.params(doc, size, now);
        self.last_params = Some(params.clone());
        painter.add(egui_wgpu::Callback::new_paint_callback(rect, Paint3d { params }));
        // the gizmo last: drawn over the card, and on top for the mouse
        self.gizmo(ui, rect, doc, now)
    }

    pub fn request_snapshot(&mut self, doc: &Doc) {
        let Some(p) = &self.last_params else {
            self.snapshot_pending = false;
            return;
        };
        let aspect = p.size[0] as f64 / p.size[1].max(1) as f64;
        let (w, h) = if aspect >= 1.0 { (2000, (2000.0 / aspect).round() as u32) } else { ((2000.0 * aspect).round() as u32, 2000) };
        let mut params = self.params(doc, [w, h], self.last_time);
        params.size = [w, h];
        let ok = self.with_gpu(|_, _, gpu| *gpu.snapshot.lock().unwrap() = Some(SnapshotState::Requested { w, h, params: Box::new(params) })).is_some();
        self.snapshot_pending = ok;
    }

    /// the snapshot as PNG bytes, once the GPU has it (composited over the backdrop)
    pub fn take_snapshot(&mut self, doc: &Doc) -> Option<Result<Vec<u8>, String>> {
        if !self.snapshot_pending {
            return None;
        }
        let done = self.with_gpu(|_, _, gpu| {
            let mut s = gpu.snapshot.lock().unwrap();
            match s.take() {
                Some(SnapshotState::Done(r)) => Some(r),
                other => {
                    *s = other;
                    None
                }
            }
        })??;
        self.snapshot_pending = false;
        Some(done.and_then(|(w, h, rgba)| composite_snapshot(doc, w, h, rgba)))
    }
}

/// the rendered card over the backdrop gradient, as a PNG
fn composite_snapshot(doc: &Doc, w: u32, h: u32, rgba: Vec<u8>) -> Result<Vec<u8>, String> {
    use cb_print::tiny_skia::{GradientStop, LinearGradient, Paint, Pixmap, PixmapPaint, Point, Rect, SpreadMode, Transform};
    let mut bg = Pixmap::new(w, h).ok_or("snapshot size")?;
    let (top, bottom) = backdrop_colors(doc);
    let c = |c: Color32| cb_print::tiny_skia::Color::from_rgba8(c.r(), c.g(), c.b(), 255);
    let shader = LinearGradient::new(Point::from_xy(0.0, 0.0), Point::from_xy(0.0, h as f32), vec![GradientStop::new(0.0, c(top)), GradientStop::new(1.0, c(bottom))], SpreadMode::Pad, Transform::identity()).ok_or("gradient")?;
    let paint = Paint { shader, ..Default::default() };
    bg.fill_rect(Rect::from_xywh(0.0, 0.0, w as f32, h as f32).ok_or("rect")?, &paint, Transform::identity(), None);
    // the card render is straight alpha: premultiply for tiny-skia
    let mut data = rgba;
    for px in data.chunks_exact_mut(4) {
        let a = px[3] as u32;
        for ch in &mut px[..3] {
            *ch = ((*ch as u32 * a + 127) / 255) as u8;
        }
    }
    let card = Pixmap::from_vec(data, cb_print::tiny_skia::IntSize::from_wh(w, h).ok_or("size")?).ok_or("pixels")?;
    bg.draw_pixmap(0, 0, card.as_ref(), &PixmapPaint::default(), Transform::identity(), None);
    cb_print::sheets::png_bytes(&bg)
}
