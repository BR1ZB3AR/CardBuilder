//! The two work panes: the card editor (side switch, data row, toolbox, zoom bar, guide legend)
//! and the 3D preview's controls.

use crate::app::{CardBuilderApp, Tool, Unit, View3d};
use crate::style::{icon, icons, semibold, P};
use crate::widgets::seg;
use cb_core::doc::ShapeKind;
use cb_core::Fold;
use egui::{pos2, vec2, Align, Align2, Color32, CornerRadius, FontId, Layout, Rect, RichText, Sense, Stroke, StrokeKind, Ui, UiBuilder};
use lucide_icons::Icon;

pub const HEAD: f32 = 40.0;

pub(crate) fn pane_head(ui: &mut Ui, rect: Rect, title: &str, sub: Option<&str>, right: impl FnOnce(&mut Ui)) {
    let head = Rect::from_min_size(rect.min, vec2(rect.width(), HEAD));
    ui.painter().rect_filled(head, 0.0, P.panel);
    ui.painter().hline(head.x_range(), head.bottom() - 0.5, Stroke::new(1.0, P.border));
    ui.scope_builder(UiBuilder::new().max_rect(head.shrink2(vec2(12.0, 0.0))).layout(Layout::left_to_right(Align::Center)), |ui| {
        ui.label(RichText::new(title).family(semibold()).size(13.0).color(P.accent));
        if let Some(s) = sub {
            ui.add(egui::Label::new(RichText::new(s).size(12.0).color(P.muted)).truncate());
        }
        ui.with_layout(Layout::right_to_left(Align::Center), right);
    });
}

/// a floating rounded panel (toolbox, zoom bar, chips)
pub(crate) fn float_frame() -> egui::Frame {
    egui::Frame::NONE
        .fill(P.panel)
        .stroke(Stroke::new(1.0, P.border))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(egui::Margin::same(4))
        .shadow(egui::Shadow { offset: [0, 6], blur: 18, spread: 0, color: Color32::from_black_alpha(90) })
}

pub(crate) fn tool_button(ui: &mut Ui, ic: Icon, on: bool, tip: &str) -> bool {
    let (r, resp) = ui.allocate_exact_size(vec2(32.0, 30.0), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, on, tip));
    if on {
        ui.painter().rect_filled(r, CornerRadius::same(7), P.accent_strong);
    } else if resp.hovered() {
        ui.painter().rect_filled(r, CornerRadius::same(7), P.panel2);
    }
    ui.painter().text(r.center(), Align2::CENTER_CENTER, icon(ic), FontId::new(16.0, icons()), if on { Color32::WHITE } else { P.text });
    resp.on_hover_text(tip).on_hover_cursor(egui::CursorIcon::PointingHand).clicked()
}

pub(crate) fn text_button(ui: &mut Ui, label: &str, on: bool, w: f32) -> bool {
    let (r, resp) = ui.allocate_exact_size(vec2(w, 26.0), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, on, label));
    if on {
        ui.painter().rect_filled(r, CornerRadius::same(6), P.accent_strong);
    } else if resp.hovered() {
        ui.painter().rect_filled(r, CornerRadius::same(6), P.panel2);
    }
    ui.painter().text(r.center(), Align2::CENTER_CENTER, label, FontId::proportional(12.0), if on { Color32::WHITE } else { P.muted });
    resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked()
}

fn small_icon(ui: &mut Ui, ic: Icon, tip: &str, on: bool) -> bool {
    let (r, resp) = ui.allocate_exact_size(vec2(28.0, 26.0), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, on, tip));
    if on {
        ui.painter().rect_filled(r, CornerRadius::same(6), P.accent_strong);
    } else if resp.hovered() {
        ui.painter().rect_filled(r, CornerRadius::same(6), P.panel2);
    }
    ui.painter().text(r.center(), Align2::CENTER_CENTER, icon(ic), FontId::new(14.0, icons()), if on { Color32::WHITE } else { P.muted });
    resp.on_hover_text(tip).on_hover_cursor(egui::CursorIcon::PointingHand).clicked()
}

impl CardBuilderApp {
    /// ◀ 3 / 120 ▶ — which data row the card shows
    pub(crate) fn record_nav(&mut self, ui: &mut Ui) {
        let n = self.doc.data.rows.len();
        if n == 0 {
            return;
        }
        let cur = self.record.min(n - 1);
        if small_icon(ui, Icon::ChevronRight, "Next row ( ] )", false) {
            self.record = (cur + 1) % n;
        }
        ui.label(RichText::new(format!("{} / {n}", cur + 1)).size(12.0).color(P.text)).on_hover_text("The data row shown on the card");
        if small_icon(ui, Icon::ChevronLeft, "Previous row ( [ )", false) {
            self.record = (cur + n - 1) % n;
        }
        ui.label(RichText::new(icon(Icon::Table)).family(icons()).size(13.0).color(P.accent)).on_hover_text("Batch cards: one card per data row");
    }

    pub(crate) fn card_pane(&mut self, ui: &mut Ui, rect: Rect, now: f64) {
        let fmt = self.doc.format.clone();
        let sub = format!("{} · {}", self.format_title(), fmt.size_label());
        pane_head(ui, rect, "Card", Some(&sub), |ui| {
            self.record_nav(ui);
        });

        let body = Rect::from_min_max(pos2(rect.left(), rect.top() + HEAD), rect.max);
        let record = self.current_record();
        let inp = crate::canvas::Input { doc: &self.doc, side: self.side, record, tool: self.tool, selected: self.selected.as_deref(), guides: self.prefs.guides, snap: self.prefs.snap };
        let ev = self.canvas.show(ui, body, &inp, &mut self.render);
        if let Some(s) = ev.activate {
            self.side = s;
        }
        if let Some(sel) = ev.select {
            if let Some(l) = sel.as_deref().and_then(|id| self.doc.layer(id)) {
                self.side = l.side;
            }
            self.selected = sel;
        }
        if let Some((mut l, key)) = ev.set_layer {
            if l.kind == cb_core::doc::LayerType::Text && l.valign.is_none() {
                l.h = cb_print::draw::layer_size(&l, &self.doc.filler(record)).1;
            }
            self.edit(now, Some(&key), |d| {
                if let Some(x) = d.layer_mut(&l.id) {
                    *x = l;
                }
            });
        }
        if ev.edit_text {
            self.focus_text = true;
            self.prefs.tab = crate::app::Tab::Design;
        }
        if let Some((side, x, y)) = ev.place {
            self.side = side;
            let at = Some((x, y));
            match self.tool {
                Tool::Text => self.add_text(now, at),
                Tool::Shape => self.add_shape(now, at, ShapeKind::Rect),
                Tool::Qr => self.add_qr(now, at),
                Tool::Image => self.pick_image(now, at),
                _ => {}
            }
        }

        // ---- floating toolbox
        let tb = Rect::from_min_size(body.min + vec2(10.0, 12.0), vec2(44.0, 330.0));
        ui.scope_builder(UiBuilder::new().max_rect(tb).layout(Layout::top_down(Align::Center)), |ui| {
            float_frame().show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                for (t, ic, tip) in [
                    (Tool::Select, Icon::MousePointer2, "Select & move (V)"),
                    (Tool::Hand, Icon::Hand, "Pan (H)"),
                    (Tool::Text, Icon::Type, "Text (T) — click the card"),
                    (Tool::Image, Icon::Image, "Image (I) — click the card"),
                    (Tool::Shape, Icon::Square, "Shape (R) — click the card"),
                    (Tool::Qr, Icon::QrCode, "QR code (Q) — click the card"),
                ] {
                    if tool_button(ui, ic, self.tool == t, tip) {
                        self.tool = t;
                    }
                }
                ui.add(egui::Separator::default().spacing(6.0));
                for u in [Unit::Mm, Unit::In] {
                    if text_button(ui, u.label(), self.prefs.unit == u, 32.0) {
                        self.prefs.unit = u;
                    }
                }
            });
        });

        // ---- zoom bar
        let zb = Rect::from_center_size(pos2(body.center().x, body.bottom() - 30.0), vec2(250.0, 40.0));
        ui.scope_builder(UiBuilder::new().max_rect(zb).layout(Layout::left_to_right(Align::Center)), |ui| {
            float_frame().show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    if text_button(ui, "−", false, 28.0) {
                        self.canvas.zoom_by(1.0 / 1.2);
                    }
                    ui.add_sized([50.0, 26.0], egui::Label::new(RichText::new(format!("{:.0}%", self.canvas.zoom_percent())).size(12.5)));
                    if text_button(ui, "+", false, 28.0) {
                        self.canvas.zoom_by(1.2);
                    }
                    if text_button(ui, "Fit", false, 34.0) {
                        self.canvas.request_fit();
                    }
                    ui.add_space(4.0);
                    if small_icon(ui, Icon::SquareDashed, "Bleed, trim and safe-area guides", self.prefs.guides) {
                        self.prefs.guides = !self.prefs.guides;
                    }
                    if small_icon(ui, Icon::Magnet, "Snap to centre lines and the safe area (hold Alt to move freely)", self.prefs.snap) {
                        self.prefs.snap = !self.prefs.snap;
                    }
                });
            });
        });

        // ---- guide legend (top right of the canvas)
        if self.prefs.guides && body.width() > 460.0 {
            let painter = ui.painter_at(body);
            let items: Vec<(Color32, &str, &str)> = {
                let mut v = vec![
                    (Color32::from_rgb(0xe5, 0x48, 0x4d), "Bleed", "Artwork must reach this line: it is cut off, so no white edges show"),
                    (if P.dark { Color32::from_white_alpha(170) } else { Color32::from_black_alpha(170) }, "Trim", "Where the card is cut"),
                    (Color32::from_rgb(0x3f, 0xb8, 0xaf), "Safe area", "Keep text inside this line"),
                ];
                if fmt.fold != Fold::None {
                    v.push((P.accent, "Fold", "Where the card is folded"));
                }
                v
            };
            let widths: f32 = items.iter().map(|(_, n, _)| painter.layout_no_wrap(n.to_string(), FontId::proportional(11.0), P.muted).size().x + 36.0).sum();
            let mut x = body.right() - widths - 4.0;
            let y = body.top() + 22.0;
            for (col, name, tip) in items {
                let g = painter.layout_no_wrap(name.into(), FontId::proportional(11.0), P.muted);
                let r = Rect::from_min_size(pos2(x, y - 9.0), vec2(g.size().x + 26.0, 18.0));
                painter.extend(egui::Shape::dashed_line(&[pos2(x, y), pos2(x + 14.0, y)], Stroke::new(1.5, col), 4.0, 2.5));
                painter.galley(pos2(x + 20.0, y - g.size().y / 2.0), g, P.muted);
                ui.interact(r, ui.scope_id().with(("legend", name)), Sense::hover()).on_hover_text(tip);
                x = r.right() + 10.0;
            }
        }
    }

    pub(crate) fn viewer_pane(&mut self, ui: &mut Ui, rect: Rect, now: f64) {
        let folded = self.doc.format.fold != Fold::None;
        let mut v = self.view3d;
        let mut clicked = false;
        pane_head(ui, rect, "3D Preview", None, |ui| {
            let opts: Vec<(View3d, &str)> = if folded {
                vec![(View3d::Back, "Back"), (View3d::Inside, "Inside"), (View3d::Angle, "¾"), (View3d::Front, "Front")]
            } else {
                vec![(View3d::Back, "Back"), (View3d::Angle, "¾"), (View3d::Front, "Front")]
            };
            clicked = seg(ui, &mut v, &opts, false);
        });
        if clicked || v != self.view3d {
            self.view3d = v;
            self.viewer.set_view(v, &self.doc, now);
        }
        let body = Rect::from_min_max(pos2(rect.left(), rect.top() + HEAD), rect.max);
        if let Some(v) = self.viewer.show(ui, body, &self.doc, now) {
            self.view3d = v;
        }
        let painter = ui.painter_at(body);

        // stock chip (top left) and warnings
        let stock = self.doc.paper.stock.info();
        let label = format!("{} · {}", stock.name, stock.note);
        let g = painter.layout_no_wrap(label, FontId::proportional(11.5), P.text);
        let chip = Rect::from_min_size(pos2(body.left() + 12.0, body.top() + 12.0), vec2(g.size().x + 34.0, 26.0));
        let resp = ui.interact(chip, ui.scope_id().with("stock-chip"), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text("Paper and scene settings");
        painter.rect_filled(chip, CornerRadius::same(13), if resp.hovered() { P.panel2 } else { P.chip });
        painter.circle_filled(chip.left_center() + vec2(13.0, 0.0), 5.0, crate::widgets::hex_color(&self.doc.paper.color));
        painter.circle_stroke(chip.left_center() + vec2(13.0, 0.0), 5.0, Stroke::new(1.0, P.border));
        painter.galley(pos2(chip.left() + 23.0, chip.center().y - g.size().y / 2.0), g, P.text);
        if resp.clicked() {
            self.prefs.tab = crate::app::Tab::Scene;
        }
        let mut tag_top = chip.bottom() + 6.0;
        if self.doc.has_finishes() {
            let g2 = painter.layout_no_wrap("Foil / spot UV shown in 3D".into(), FontId::proportional(11.0), P.muted);
            let r = Rect::from_min_size(pos2(body.left() + 12.0, tag_top), g2.size() + vec2(20.0, 10.0));
            painter.rect_filled(r, CornerRadius::same(12), P.chip);
            painter.galley(r.min + vec2(10.0, 5.0), g2, P.muted);
            tag_top = r.bottom() + 6.0;
        }
        if self.gpu.on_cpu() || !self.viewer.gpu_ok {
            let amber = Color32::from_rgb(0xe0, 0xa8, 0x4a);
            let text = if !self.viewer.gpu_ok { "3D needs a graphics card driver" } else if self.gpu.problem.is_some() { "Graphics card unavailable — restart the computer" } else { "Drawing on the CPU, not the graphics card" };
            let g3 = painter.layout_no_wrap(text.into(), FontId::proportional(11.0), amber);
            let r = Rect::from_min_size(pos2(body.left() + 12.0, tag_top), g3.size() + vec2(38.0, 10.0));
            let resp = ui.interact(r, ui.scope_id().with("gpu-warning"), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
            painter.rect(r, CornerRadius::same(12), if resp.hovered() { P.panel2 } else { P.chip }, Stroke::new(1.0, amber.gamma_multiply(0.6)), StrokeKind::Inside);
            painter.text(pos2(r.left() + 18.0, r.center().y), Align2::CENTER_CENTER, icon(Icon::TriangleAlert), FontId::new(12.0, icons()), amber);
            painter.galley(r.min + vec2(28.0, 5.0), g3, amber);
            let tip = self.gpu.problem.clone().unwrap_or_else(|| "The 3D is slow or missing because no graphics card driver could be used. Click for Scene › Graphics card.".into());
            if resp.on_hover_text(tip).clicked() {
                self.prefs.tab = crate::app::Tab::Scene;
            }
        }

        // view bar (bottom centre): auto-rotate · flip · reset · how far a folded card is open
        let w = if folded { 360.0 } else { 254.0 };
        let vb = Rect::from_center_size(pos2(body.center().x, body.bottom() - 30.0), vec2(w, 38.0));
        let mut scene = self.doc.scene.clone();
        let mut reset = false;
        let mut flip = false;
        ui.scope_builder(UiBuilder::new().max_rect(vb).layout(Layout::left_to_right(Align::Center)), |ui| {
            float_frame().fill(P.chip).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    if tool_button(ui, Icon::RotateCcw, scene.auto_rotate, "Turn slowly") {
                        scene.auto_rotate = !scene.auto_rotate;
                    }
                    flip = tool_button(ui, Icon::FlipHorizontal2, false, "Turn the card over");
                    if !folded && tool_button(ui, Icon::Columns2, scene.pair, if scene.pair { "Front and back side by side — click for one card" } else { "One card — click to show the back beside it" }) {
                        scene.pair = !scene.pair;
                    }
                    reset = text_button(ui, "Reset view", false, 78.0);
                    if folded {
                        ui.add_space(6.0);
                        ui.label(RichText::new("Open").size(12.0).color(P.muted));
                        ui.spacing_mut().slider_width = 110.0;
                        ui.add(egui::Slider::new(&mut scene.open, 0.0..=180.0).show_value(false).step_by(1.0)).on_hover_text("How far the card is opened");
                        ui.label(RichText::new(format!("{:.0}°", scene.open)).size(12.0).color(P.muted));
                    }
                });
            });
        });
        if reset {
            self.view3d = View3d::Angle;
            self.viewer.reset_view();
        }
        if flip {
            self.viewer.flip(now);
        }
        if scene != self.doc.scene {
            self.edit(now, Some("scene-bar"), |d| d.scene = scene);
        }
    }
}
