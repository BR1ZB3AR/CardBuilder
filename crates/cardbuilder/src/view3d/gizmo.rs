//! The view gizmo in the 3D view's top right corner: the axes as CAD apps show them, Z up.
//! Click an axis to look along it (click it again to look from the other end), drag the gizmo
//! to turn the card, and the house button below it goes back to the starting view.

use super::{Anim, Cam, Viewer3d};
use crate::app::View3d;
use crate::style::{icon, icons, semibold, P};
use cb_core::doc::Doc;
use cb_core::Fold;
use egui::{pos2, vec2, Align2, Color32, CursorIcon, FontId, PointerButton, Rect, Sense, Stroke, Ui, Vec2};
use glam::Vec3;
use lucide_icons::Icon;

/// how far up or down the camera can look: just short of straight, where "up" is undefined
pub const MAX_PITCH: f64 = 1.55;

struct Axis {
    name: &'static str,
    /// the axis in the scene's terms (the scene is Y up, the card's front facing +Z)
    dir: Vec3,
    color: Color32,
    /// where the camera is when it looks from the axis' positive / negative end
    plus: &'static str,
    minus: &'static str,
}

/// X to the right, Y going back (the scene's −Z), Z up (the scene's +Y)
const AXES: [Axis; 3] = [
    Axis { name: "X", dir: Vec3::X, color: Color32::from_rgb(0xf0, 0x3d, 0x62), plus: "right", minus: "left" },
    Axis { name: "Y", dir: Vec3::NEG_Z, color: Color32::from_rgb(0x8b, 0xcf, 0x2e), plus: "back", minus: "front" },
    Axis { name: "Z", dir: Vec3::Y, color: Color32::from_rgb(0x2f, 0x8c, 0xf2), plus: "top", minus: "bottom" },
];

/// length of an axis on screen, and the radii of the positive and negative ends
const LEN: f32 = 30.0;
const R_PLUS: f32 = 9.5;
const R_MINUS: f32 = 7.5;

/// (right, up, towards the camera) of a camera
fn basis(cam: &Cam) -> (Vec3, Vec3, Vec3) {
    let (y, p) = (cam.yaw as f32, cam.pitch as f32);
    let back = Vec3::new(y.sin() * p.cos(), p.sin(), y.cos() * p.cos());
    let right = (-back).cross(Vec3::Y).normalize();
    (right, right.cross(-back), back)
}

/// yaw and pitch of a camera looking from direction `d` (a unit vector)
fn looking_from(d: Vec3) -> (f64, f64) {
    let (x, y, z) = (d.x as f64, d.y as f64, d.z as f64);
    let yaw = if x.abs() + z.abs() < 1e-6 { 0.0 } else { x.atan2(z) };
    (yaw, y.clamp(-1.0, 1.0).asin().clamp(-MAX_PITCH, MAX_PITCH))
}

/// the gizmo's widget ids (orbit area, axis ends, home button)
pub(super) fn area_id(ui: &Ui) -> egui::Id {
    ui.scope_id().with("view3d-gizmo")
}

impl Viewer3d {
    /// the camera's starting view, reached smoothly (zoom and pan back to normal too)
    fn go_home(&mut self, doc: &Doc, now: f64) {
        let to = Cam { zoom: 1.0, ..self.view_cam(View3d::Angle, doc) };
        let from = (self.pan, self.cam.zoom);
        self.animate_to(to, now, 0.6);
        if let Some(a) = &mut self.anim {
            a.home = Some(from);
        }
    }

    /// draw the gizmo over the 3D view `view` and handle it; returns the view the 3D pane's
    /// header should show, when a click lands on one of them
    pub(super) fn gizmo(&mut self, ui: &mut Ui, view: Rect, doc: &Doc, now: f64) -> Option<View3d> {
        let painter = ui.painter_at(view);
        let id = area_id(ui);
        let c = pos2(view.right() - 18.0 - LEN - R_PLUS, view.top() + 18.0 + LEN + R_PLUS);
        let reach = LEN + R_PLUS + 5.0;
        let area = Rect::from_center_size(c, Vec2::splat(2.0 * reach));
        let mut picked = None;

        // the area behind the axes turns the card when dragged
        let bg = ui.interact(area, id, Sense::drag());
        if bg.dragged_by(PointerButton::Primary) {
            self.orbit(bg.drag_delta());
        }
        let near = ui.input(|i| i.pointer.hover_pos()).is_some_and(|p| p.distance(c) <= reach);
        if bg.dragged() || near {
            painter.circle_filled(c, reach, Color32::from_rgba_unmultiplied(120, 124, 140, 46));
        }
        if bg.dragged() {
            ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
        } else if bg.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::Grab);
        }

        // the six ends, far ones first (the nearer end is drawn, and clicked, on top)
        let (right, up, back) = basis(&self.cam);
        let mut ends: Vec<(usize, f32, Vec3, f32)> = (0..3).flat_map(|i| [1.0f32, -1.0].map(|s| (i, s, AXES[i].dir * s, (AXES[i].dir * s).dot(back)))).collect();
        ends.sort_by(|a, b| a.3.total_cmp(&b.3));
        for (i, s, v, _) in ends {
            let ax = &AXES[i];
            let p = c + vec2(v.dot(right), -v.dot(up)) * LEN;
            let plus = s > 0.0;
            let r = if plus { R_PLUS } else { R_MINUS };
            let side = if plus { ax.plus } else { ax.minus };
            let label = format!("Look from the {side} ({}{})", if plus { "" } else { "−" }, ax.name);
            let resp = ui.interact(Rect::from_center_size(p, Vec2::splat(2.0 * r + 2.0)), id.with((i, plus)), Sense::click());
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &label));
            let hot = resp.hovered();
            if plus {
                painter.line_segment([c, p], Stroke::new(2.0, ax.color));
                painter.circle_filled(p, r, if hot { ax.color.lerp_to_gamma(Color32::WHITE, 0.25) } else { ax.color });
                painter.text(p, Align2::CENTER_CENTER, ax.name, FontId::new(11.0, semibold()), Color32::WHITE);
            } else {
                let fill = ax.color.lerp_to_gamma(Color32::from_rgb(0x2a, 0x2c, 0x38), if hot { 0.2 } else { 0.6 });
                painter.circle_filled(p, r, fill.gamma_multiply(0.85));
                painter.circle_stroke(p, r - 0.75, Stroke::new(1.5, ax.color));
                if hot {
                    painter.text(p, Align2::CENTER_CENTER, format!("−{}", ax.name), FontId::new(9.0, semibold()), Color32::WHITE);
                }
            }
            if resp.on_hover_text(&label).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                // already looking along this axis: look from the other end
                let from = if v.dot(back) > 0.995 { -v } else { v };
                let (yaw, pitch) = looking_from(from);
                self.pan = Vec3::ZERO;
                self.animate_to(Cam { yaw, pitch, zoom: self.cam.zoom }, now, 0.6);
                // a flat card's front and back are the header's Front and Back views
                if doc.format.fold == Fold::None && i == 1 {
                    picked = Some(if from.z > 0.0 { View3d::Front } else { View3d::Back });
                }
            }
        }

        // home: back to the starting view
        let hc = pos2(view.right() - 18.0 - 13.0, area.bottom() + 22.0);
        let h = ui.interact(Rect::from_center_size(hc, Vec2::splat(28.0)), id.with("home"), Sense::click());
        h.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, "Home view"));
        painter.circle_filled(hc, 14.0, if h.hovered() { P.panel2 } else { P.chip });
        painter.circle_stroke(hc, 14.0, Stroke::new(1.0, P.border));
        painter.text(hc, Align2::CENTER_CENTER, icon(Icon::House), FontId::new(14.0, icons()), P.text);
        if h.on_hover_text("Back to the starting view").on_hover_cursor(CursorIcon::PointingHand).clicked() {
            self.go_home(doc, now);
            picked = Some(View3d::Angle);
        }
        if self.anim.is_some() {
            ui.ctx().request_repaint();
        }
        picked
    }
}

/// what an animation does to the pan and zoom on its way (`home` brings them back to normal)
pub(super) fn home_step(a: &Anim, e: f64) -> Option<(Vec3, f64)> {
    a.home.map(|(pan, zoom)| (pan * (1.0 - e) as f32, zoom + (1.0 - zoom) * e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_axis_end_puts_the_camera_on_that_side() {
        // the front (−Y) is the card's own front view, the back (+Y) behind it
        let (yaw, pitch) = looking_from(-AXES[1].dir);
        assert!(yaw.abs() < 1e-9 && pitch.abs() < 1e-9);
        let (yaw, _) = looking_from(AXES[1].dir);
        assert!((yaw.abs() - std::f64::consts::PI).abs() < 1e-6);
        // right (+X), and the top (+Z) as far up as the camera goes
        let (yaw, _) = looking_from(AXES[0].dir);
        assert!((yaw - std::f64::consts::FRAC_PI_2).abs() < 1e-6);
        assert_eq!(looking_from(AXES[2].dir), (0.0, MAX_PITCH));
        // seen from the ¾ view, X points right, Z up and Y away to the right
        let (r, u, b) = basis(&Cam { yaw: 0.55, pitch: 0.22, zoom: 1.0 });
        assert!(AXES[0].dir.dot(r) > 0.8 && AXES[2].dir.dot(u) > 0.9);
        assert!(AXES[1].dir.dot(r) > 0.3 && AXES[1].dir.dot(b) < -0.7);
    }
}
