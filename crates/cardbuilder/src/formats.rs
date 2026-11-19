//! The card format picker: every kind of card by category (each size drawn to scale), portrait
//! or landscape, and a custom size with any fold and corner radius.

use crate::app::CardBuilderApp;
use crate::style::{icon, icons, semibold, P};
use crate::widgets::{button, seg, BtnKind};
use cb_core::format::{preset, CardFormat, Category, Fold, Preset, PRESETS};
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

/// one card in the picker: a to-scale picture, the name and a note
fn item_row(ui: &mut Ui, name: &str, note: &str, shape: (f64, f64, f64, Fold), on: bool, indent: bool, picture: Option<egui::TextureId>) -> bool {
    let x0 = if indent { 14.0 } else { 0.0 };
    let (r, resp) = ui.allocate_exact_size(vec2(326.0, 46.0), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, on, name));
    let r = Rect::from_min_max(r.min + vec2(x0, 0.0), r.max);
    if on || resp.hovered() {
        ui.painter().rect_filled(r, CornerRadius::same(8), if on { P.accent_weak } else { P.panel2 });
    }
    let t = Rect::from_min_size(r.min + vec2(8.0, 6.0), vec2(48.0, 34.0));
    match picture {
        // the card's front, to scale
        Some(tex) => {
            let (w, h) = match shape.3 {
                Fold::None => (shape.0, shape.1),
                Fold::Side => (2.0 * shape.0, shape.1),
                Fold::Top => (shape.0, 2.0 * shape.1),
            };
            let k = (t.width() as f64 / w).min(t.height() as f64 / h);
            let pr = Rect::from_center_size(t.center(), vec2((w * k) as f32, (h * k) as f32));
            ui.painter().image(tex, pr, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), egui::Color32::WHITE);
            ui.painter().rect_stroke(pr, CornerRadius::same(2), egui::Stroke::new(1.0, if on { P.accent } else { P.border }), egui::StrokeKind::Outside);
        }
        None => thumb(ui, t, shape.0, shape.1, shape.2, shape.3, on),
    }
    ui.painter().text(pos2(r.left() + 66.0, r.top() + 8.0), Align2::LEFT_TOP, name, FontId::new(13.0, semibold()), if on { P.accent } else { P.text });
    ui.painter().with_clip_rect(r.shrink(2.0)).text(pos2(r.left() + 66.0, r.bottom() - 8.0), Align2::LEFT_BOTTOM, note, FontId::proportional(11.0), P.muted);
    if on {
        ui.painter().text(r.right_center() - vec2(12.0, 0.0), Align2::RIGHT_CENTER, icon(Icon::Check), FontId::new(14.0, icons()), P.accent);
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked()
}

/// a group's heading in the picker: click to open or close it
fn group_header(ui: &mut Ui, name: &str, count: usize, open: bool, holds_current: bool) -> bool {
    ui.add_space(2.0);
    let (r, resp) = ui.allocate_exact_size(vec2(326.0, 34.0), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, open, name));
    ui.painter().rect(r, CornerRadius::same(8), if resp.hovered() { P.panel3 } else { P.panel2 }, egui::Stroke::new(1.0, P.border), egui::StrokeKind::Inside);
    ui.painter().text(r.left_center() + vec2(12.0, 0.0), Align2::LEFT_CENTER, icon(if open { Icon::ChevronDown } else { Icon::ChevronRight }), FontId::new(14.0, icons()), P.muted);
    ui.painter().text(r.left_center() + vec2(32.0, 0.0), Align2::LEFT_CENTER, icon(Icon::Layers), FontId::new(14.0, icons()), P.accent);
    ui.painter().text(r.left_center() + vec2(54.0, 0.0), Align2::LEFT_CENTER, name, FontId::new(13.0, semibold()), P.text);
    let tail = format!("{count} card{}", if count == 1 { "" } else { "s" });
    ui.painter().text(r.right_center() - vec2(12.0, 0.0), Align2::RIGHT_CENTER, tail, FontId::proportional(11.0), P.muted);
    if holds_current {
        ui.painter().circle_filled(r.right_center() - vec2(64.0, 0.0), 3.0, P.accent);
    }
    resp.on_hover_text(if open { "Close the group" } else { "Open the group" }).on_hover_cursor(egui::CursorIcon::PointingHand).clicked()
}

impl CardBuilderApp {
    /// the picker's contents (top bar chip and the sidebar's Change… button)
    pub(crate) fn format_menu(&mut self, ui: &mut Ui, now: f64) {
        let cur = self.doc.format.clone();
        let cat = self.popups.format_cat.unwrap_or(cur.category());
        let mut next: Option<CardFormat> = None;
        // a card picked from the list (not a turn or a custom size)
        let mut picked: Option<CardFormat> = None;
        let mut pick_template: Option<std::path::PathBuf> = None;
        let mut gap_x = 0.0;
        let row = ui.horizontal_top(|ui| {
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
            // a divider drawn at the columns' height afterwards (a Separator would stretch the
            // pop-up to the bottom of the window)
            ui.add_space(6.0);
            gap_x = ui.cursor().left();
            ui.add_space(6.0);
            // ---- sizes of that kind (or the custom size)
            ui.vertical(|ui| {
                ui.set_width(330.0);
                ui.label(RichText::new(cat.name().to_uppercase()).size(10.5).color(P.muted));
                if cat == Category::Custom {
                    next = self.custom_size(ui);
                    return;
                }
                let opening = !self.popups.format_seen;
                self.popups.format_seen = true;
                egui::ScrollArea::vertical().max_height(520.0).id_salt(("format-presets", cat.id())).show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    // the current card's row, to scroll to when the picker opens
                    let mut current_row: Option<Rect> = None;
                    // sizes without a group first, then each group (built-in sizes and local
                    // templates together) under a heading that opens and closes
                    for p in PRESETS.iter().filter(|p| p.category == cat && p.group.is_none()) {
                        if self.preset_row(ui, p, &cur) {
                            let mut f = CardFormat::from_preset(p);
                            f.bleed = cur.bleed; // keep the bleed the user set
                            picked = Some(f);
                        }
                    }
                    let mut groups: Vec<String> = Vec::new();
                    for g in PRESETS.iter().filter(|p| p.category == cat).filter_map(|p| p.group).map(String::from).chain(self.templates.iter().filter(|t| t.category() == cat).map(|t| t.group().to_string())) {
                        if !groups.contains(&g) {
                            groups.push(g);
                        }
                    }
                    for g in groups {
                        let templates: Vec<crate::templates::LocalTemplate> = self.templates.iter().filter(|t| t.category() == cat && t.group() == g).cloned().collect();
                        // a local template named like a built-in card of the group takes its place
                        let presets: Vec<&'static Preset> = PRESETS.iter().filter(|p| p.category == cat && p.group == Some(g.as_str()) && !templates.iter().any(|t| t.info.name == p.name)).collect();
                        let holds_current = presets.iter().any(|p| p.id == cur.preset && self.doc.template.is_none()) || templates.iter().any(|t| self.doc.template.as_deref() == Some(t.info.id.as_str()));
                        let open = *self.popups.groups.get(&g).unwrap_or(&holds_current);
                        let group_top = ui.cursor().top();
                        if group_header(ui, &g, presets.len() + templates.len(), open, holds_current) {
                            self.popups.groups.insert(g.clone(), !open);
                            if !open {
                                self.popups.scroll_group = Some(g.clone());
                            }
                            // let the pop-up settle on its new size straight away
                            ui.ctx().request_repaint();
                        }
                        if !open {
                            continue;
                        }
                        for p in presets {
                            let top = ui.cursor().top();
                            if self.preset_row(ui, p, &cur) {
                                let mut f = CardFormat::from_preset(p);
                                f.bleed = cur.bleed;
                                picked = Some(f);
                            }
                            if p.id == cur.preset && self.doc.template.is_none() {
                                current_row = Some(Rect::from_x_y_ranges(ui.max_rect().x_range(), top..=ui.cursor().top()));
                            }
                        }
                        for t in templates {
                            let on = self.doc.template.as_deref() == Some(t.info.id.as_str());
                            let key = crate::thumbs::file_key(&t.path);
                            let path = t.path.clone();
                            let picture = self.thumbs.get(ui.ctx(), &key, || crate::thumbs::Source::File(path));
                            let top = ui.cursor().top();
                            if item_row(ui, &t.info.name, &t.info.note, (t.format.w, t.format.h, t.format.radius, t.format.fold), on, true, picture) && !on {
                                pick_template = Some(t.path.clone());
                            }
                            if on {
                                current_row = Some(Rect::from_x_y_ranges(ui.max_rect().x_range(), top..=ui.cursor().top()));
                            }
                        }
                        // a group just opened: its heading at the top, as many of its cards as fit
                        if self.popups.scroll_group.as_deref() == Some(g.as_str()) {
                            ui.scroll_to_rect(Rect::from_x_y_ranges(ui.max_rect().x_range(), group_top..=ui.cursor().top()), Some(egui::Align::TOP));
                            self.popups.scroll_group = None;
                        }
                    }
                    if opening {
                        if let Some(r) = current_row {
                            ui.scroll_to_rect(r, Some(egui::Align::Center));
                        }
                    }
                    if cat != Category::Custom && self.templates.iter().all(|t| t.category() != cat) {
                        ui.add_space(4.0);
                        crate::widgets::hint(ui, "Export › Save as template… keeps your own designs here.");
                    }
                });
            });
        });
        ui.painter().vline(gap_x, row.response.rect.y_range(), egui::Stroke::new(1.0, P.border));
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
        if let Some(f) = picked {
            // Cards with a look of their own (a template's group such as Ingress Biocard, or a
            // built-in card in a group) don't carry over to another kind of card: picking one,
            // or leaving one, starts a fresh card. Other sizes keep the layout.
            let grouped = |id: &str| cb_core::format::preset(id).is_some_and(|p| p.group.is_some());
            let own_look = match (&self.doc.template, self.current_template()) {
                (None, _) => grouped(&self.doc.format.preset),
                (Some(_), Some(t)) => t.group() != crate::templates::MY_TEMPLATES || t.category() != f.category(),
                (Some(_), None) => true,
            };
            if own_look || grouped(&f.preset) {
                self.popups.format_top = false;
                self.request_fresh(f, now);
            } else {
                next = Some(f);
            }
        }
        if let Some(f) = next {
            // a built-in size: the design no longer follows a template
            if self.doc.template.is_some() {
                self.edit(now, None, |d| d.template = None);
            }
            self.set_format(now, f);
        }
        if let Some(path) = pick_template {
            self.popups.format_top = false;
            self.request_template(path, now);
        }
    }

    /// a built-in size in the picker; true when clicked (and not already the current one)
    fn preset_row(&mut self, ui: &mut Ui, p: &'static Preset, cur: &CardFormat) -> bool {
        let on = cur.preset == p.id && self.doc.template.is_none();
        // the current card may be turned the other way: show it as it is
        let (w, h) = if on { (cur.w, cur.h) } else { (p.w, p.h) };
        // cards in a group are designs of their own: show their front
        let picture = if p.group.is_some() && (w - p.w).abs() < 1e-9 { self.thumbs.get(ui.ctx(), p.id, || crate::thumbs::Source::Preset(p)) } else { None };
        item_row(ui, p.name, p.note, (w, h, p.radius, p.fold), on, p.group.is_some(), picture) && !on
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

    /// "Business card · Europe", "Ingress Biocard · 2023 Origin" — what the card is called
    pub(crate) fn format_title(&self) -> String {
        if let Some(t) = self.current_template() {
            return format!("{} · {}", t.group(), t.info.name);
        }
        match preset(&self.doc.format.preset) {
            Some(p) => {
                if let Some(g) = p.group {
                    return format!("{g} · {}", p.name);
                }
                let noun = p.category.noun();
                let cap = noun[..1].to_uppercase() + &noun[1..];
                if p.category == Category::Business || p.category == Category::Game {
                    format!("{cap} · {}", p.name)
                } else {
                    p.name.to_string()
                }
            }
            None => "Custom card".into(),
        }
    }
}
