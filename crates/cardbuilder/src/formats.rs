//! The card format picker: every kind of card by category (each size drawn to scale), portrait
//! or landscape, and a custom size with any fold and corner radius.

use crate::app::CardBuilderApp;
use crate::style::{icon, icons, semibold, P};
use crate::widgets::{button, seg, BtnKind};
use cb_core::format::{preset, CardFormat, Category, Fold, PRESETS};
use egui::{pos2, vec2, Align2, CornerRadius, FontId, Rect, RichText, Sense, Stroke, StrokeKind, Ui};
use lucide_icons::Icon;

/// the custom size being typed in
#[derive(Clone, Debug, PartialEq)]
pub struct CustomDraft {
    pub w: f64,
    pub h: f64,
    pub fold: Fold,
    pub radius: f64,
}

impl From<&CardFormat> for CustomDraft {
    fn from(f: &CardFormat) -> Self {
        CustomDraft { w: f.w, h: f.h, fold: f.fold, radius: f.radius }
    }
}

pub fn category_icon(c: Category) -> Icon {
    match c {
        Category::Business => Icon::IdCard,
        Category::Id => Icon::CreditCard,
        Category::Game => Icon::Spade,
        Category::Postcard => Icon::Mail,
        Category::Greeting => Icon::Gift,
        Category::Tent => Icon::Tent,
        Category::Tag => Icon::Bookmark,
        Category::Custom => Icon::Ruler,
    }
}

/// a small to-scale picture of a format (folded cards show their fold)
fn thumb(ui: &Ui, r: Rect, w: f64, h: f64, radius: f64, fold: Fold, on: bool) {
    let (fw, fh) = match fold {
        Fold::None => (w, h),
        Fold::Side => (2.0 * w, h),
        Fold::Top => (w, 2.0 * h),
    };
    let k = (r.width() as f64 / fw).min(r.height() as f64 / fh);
    let size = vec2((fw * k) as f32, (fh * k) as f32);
    let c = Rect::from_center_size(r.center(), size);
    let fill = if on { P.accent_weak } else { P.panel2 };
    ui.painter().rect(c, CornerRadius::same((radius * k).min(8.0) as u8), fill, Stroke::new(1.0, if on { P.accent } else { P.muted.gamma_multiply(0.7) }), StrokeKind::Inside);
    match fold {
        Fold::Side => {
            ui.painter().vline(c.center().x, c.y_range(), Stroke::new(1.0, P.muted.gamma_multiply(0.6)));
        }
        Fold::Top => {
            ui.painter().hline(c.x_range(), c.center().y, Stroke::new(1.0, P.muted.gamma_multiply(0.6)));
        }
        Fold::None => {}
    }
}

impl CardBuilderApp {
    /// the picker's contents (top bar chip and the sidebar's Change… button)
    pub(crate) fn format_menu(&mut self, ui: &mut Ui, now: f64) {
        let cur = self.doc.format.clone();
        let cat = self.popups.format_cat.unwrap_or(cur.category());
        let mut next: Option<CardFormat> = None;
        ui.horizontal_top(|ui| {
            // ---- categories
            ui.vertical(|ui| {
                ui.set_width(196.0);
                ui.label(RichText::new("KIND OF CARD").size(10.5).color(P.muted));
                for c in Category::ALL {
                    let on = c == cat;
                    let (r, resp) = ui.allocate_exact_size(vec2(196.0, 30.0), Sense::click());
                    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, on, c.name()));
                    if on || resp.hovered() {
                        ui.painter().rect_filled(r, CornerRadius::same(7), if on { P.accent_weak } else { P.panel2 });
                    }
                    let fg = if on { P.accent } else { P.text };
                    ui.painter().text(r.left_center() + vec2(10.0, 0.0), Align2::LEFT_CENTER, icon(category_icon(c)), FontId::new(15.0, icons()), fg);
                    ui.painter().text(r.left_center() + vec2(34.0, 0.0), Align2::LEFT_CENTER, c.name(), FontId::proportional(13.0), fg);
                    if c == cur.category() {
                        ui.painter().circle_filled(r.right_center() - vec2(12.0, 0.0), 3.0, P.accent);
                    }
                    if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                        self.popups.format_cat = Some(c);
                    }
                }
            });
            ui.separator();
            // ---- sizes of that kind (or the custom size)
            ui.vertical(|ui| {
                ui.set_width(330.0);
                ui.label(RichText::new(cat.name().to_uppercase()).size(10.5).color(P.muted));
                if cat == Category::Custom {
                    next = self.custom_size(ui);
                    return;
                }
                egui::ScrollArea::vertical().max_height(360.0).id_salt("format-presets").show(ui, |ui| {
                    for p in PRESETS.iter().filter(|p| p.category == cat) {
                        let on = cur.preset == p.id;
                        let (r, resp) = ui.allocate_exact_size(vec2(326.0, 46.0), Sense::click());
                        resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, on, p.name));
                        if on || resp.hovered() {
                            ui.painter().rect_filled(r, CornerRadius::same(8), if on { P.accent_weak } else { P.panel2 });
                        }
                        let t = Rect::from_min_size(r.min + vec2(8.0, 6.0), vec2(48.0, 34.0));
                        // the current card may be turned the other way: show it as it is
                        let (w, h) = if on { (cur.w, cur.h) } else { (p.w, p.h) };
                        thumb(ui, t, w, h, p.radius, p.fold, on);
                        ui.painter().text(pos2(r.left() + 66.0, r.top() + 8.0), Align2::LEFT_TOP, p.name, FontId::new(13.0, semibold()), if on { P.accent } else { P.text });
                        ui.painter().text(pos2(r.left() + 66.0, r.bottom() - 8.0), Align2::LEFT_BOTTOM, p.note, FontId::proportional(11.0), P.muted);
                        if on {
                            ui.painter().text(r.right_center() - vec2(12.0, 0.0), Align2::RIGHT_CENTER, icon(Icon::Check), FontId::new(14.0, icons()), P.accent);
                        }
                        if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() && !on {
                            let mut f = CardFormat::from_preset(p);
                            // keep the bleed and safe area the user set
                            f.bleed = cur.bleed;
                            next = Some(f);
                        }
                    }
                });
            });
        });
        ui.add_space(4.0);
        ui.separator();
        // ---- turn the current card
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("{} · {}", cur.size_label(), cur.fold.label())).color(P.muted));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if (cur.w - cur.h).abs() > 0.01 {
                    let mut land = cur.landscape();
                    if seg(ui, &mut land, &[(false, "Portrait"), (true, "Landscape")], true) {
                        next = Some(cur.rotated());
                    }
                }
            });
        });
        if let Some(f) = next {
            self.set_format(now, f);
        }
    }

    /// width, height, fold and corners typed in; returns the format when applied
    fn custom_size(&mut self, ui: &mut Ui) -> Option<CardFormat> {
        let mut out = None;
        let d = &mut self.custom;
        ui.add_space(4.0);
        egui::Grid::new("custom-size").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
            ui.label("Width");
            ui.add(egui::DragValue::new(&mut d.w).range(10.0..=600.0).speed(0.5).suffix(" mm"));
            ui.end_row();
            ui.label("Height");
            ui.add(egui::DragValue::new(&mut d.h).range(10.0..=600.0).speed(0.5).suffix(" mm"));
            ui.end_row();
            ui.label("Fold");
            seg(ui, &mut d.fold, &[(Fold::None, "Flat"), (Fold::Side, "Side fold"), (Fold::Top, "Top fold")], true);
            ui.end_row();
            ui.label("Corners");
            let max = (d.w.min(d.h) / 2.0).max(0.0);
            ui.add(egui::Slider::new(&mut d.radius, 0.0..=max.min(30.0)).suffix(" mm").step_by(0.5));
            ui.end_row();
        });
        crate::widgets::hint(ui, if d.fold == Fold::None { "Width × height of the card." } else { "Width × height once folded; the printed sheet is twice as big." });
        let t = Rect::from_min_size(ui.cursor().min + vec2(0.0, 4.0), vec2(320.0, 90.0));
        ui.allocate_rect(t, Sense::hover());
        thumb(ui, t, d.w, d.h, d.radius, d.fold, true);
        ui.add_space(6.0);
        let d2 = d.clone();
        if button(ui, Some(Icon::Check), "Use this size", BtnKind::Primary, true).clicked() {
            let mut f = self.doc.format.clone();
            f.preset = "custom".into();
            f.w = d2.w;
            f.h = d2.h;
            f.fold = d2.fold;
            f.radius = d2.radius.min(d2.w.min(d2.h) / 2.0);
            out = Some(f);
        }
        out
    }

    /// "Business card" / "Custom size" — what the card is called
    pub(crate) fn format_title(&self) -> String {
        match preset(&self.doc.format.preset) {
            Some(p) => {
                let noun = p.category.noun();
                let mut s = noun[..1].to_uppercase() + &noun[1..];
                if p.category == Category::Business || p.category == Category::Game {
                    s = format!("{s} · {}", p.name);
                } else {
                    s = p.name.to_string();
                }
                s
            }
            None => "Custom card".into(),
        }
    }
}
