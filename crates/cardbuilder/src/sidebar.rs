//! The sidebar: Design (add artwork, layers, the selected layer, background, card format),
//! Data (batch cards from a spreadsheet) and Scene (paper, 3D lighting, graphics card).

use crate::app::{kind_label, CardBuilderApp, Tab};
use crate::style::{icon, icons, semibold, P};
use crate::widgets::{big_button, button, hex_color, hint, section, seg, slider_row, swatch, to_hex, toggle_row, BtnKind};
use cb_core::doc::{Align, Backdrop, BgMode, Dash, Foil, Layer, LayerType, QrEcc, QrKind, ShapeKind, Stock, VAlign};
use cb_core::Fold;
use egui::{vec2, Align as EAlign, Align2, Color32, CornerRadius, FontFamily, FontId, Layout, RichText, Sense, Stroke, StrokeKind, Ui};
use lucide_icons::Icon;

/// colours offered next to every colour picker
const SWATCHES: [&str; 10] = ["#1f2347", "#000000", "#ffffff", "#ff7a59", "#9b7bff", "#3fb8af", "#e8c46a", "#d0567a", "#2f7fd1", "#6b3fa0"];

/// a colour button + swatches; true when changed
fn color_row(ui: &mut Ui, label: &str, hex: &mut String) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        let lbl = ui.label(label);
        ui.with_layout(Layout::right_to_left(EAlign::Center), |ui| {
            let mut c = hex_color(hex);
            if ui.color_edit_button_srgba(&mut c).labelled_by(lbl.id).changed() {
                *hex = to_hex(c);
                changed = true;
            }
            ui.label(RichText::new(hex.to_uppercase()).monospace().size(11.0).color(P.muted));
        });
    });
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(2.0, 2.0);
        for s in SWATCHES {
            if swatch(ui, hex_color(s), hex.eq_ignore_ascii_case(s), s).clicked() {
                *hex = s.into();
                changed = true;
            }
        }
    });
    changed
}

/// "label ……… [drag value]" with the value in the chosen unit
fn length_row(ui: &mut Ui, label: &str, mm: &mut f64, unit: crate::app::Unit, range: std::ops::RangeInclusive<f64>) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        let lbl = ui.label(label);
        ui.with_layout(Layout::right_to_left(EAlign::Center), |ui| {
            let mut v = *mm / unit.mm();
            let (lo, hi) = (*range.start() / unit.mm(), *range.end() / unit.mm());
            let speed = if unit == crate::app::Unit::Mm { 0.1 } else { 0.005 };
            let dec = if unit == crate::app::Unit::Mm { 1 } else { 3 };
            if ui.add(egui::DragValue::new(&mut v).range(lo..=hi).speed(speed).max_decimals(dec).suffix(format!(" {}", unit.label()))).labelled_by(lbl.id).changed() {
                *mm = v * unit.mm();
                changed = true;
            }
        });
    });
    changed
}

/// a small round icon toggle (layer rows)
fn row_toggle(ui: &mut Ui, on_icon: Icon, off_icon: Icon, on: bool, tip: &str) -> bool {
    let (r, resp) = ui.allocate_exact_size(vec2(24.0, 24.0), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::CheckBox, true, on, tip));
    if resp.hovered() {
        ui.painter().rect_filled(r, CornerRadius::same(6), P.panel3);
    }
    ui.painter().text(r.center(), Align2::CENTER_CENTER, icon(if on { on_icon } else { off_icon }), FontId::new(13.0, icons()), if resp.hovered() { P.text } else { P.muted });
    resp.on_hover_text(tip).clicked()
}

fn layer_icon(l: &Layer) -> Icon {
    match l.kind {
        LayerType::Text => Icon::Type,
        LayerType::Image => Icon::Image,
        LayerType::Qr => Icon::QrCode,
        LayerType::Shape => match l.shape.unwrap_or_default() {
            ShapeKind::Rect => Icon::Square,
            ShapeKind::Ellipse => Icon::Circle,
            ShapeKind::Line => Icon::Minus,
            ShapeKind::Path => Icon::PenTool,
        },
    }
}

impl CardBuilderApp {
    pub(crate) fn sidebar(&mut self, ui: &mut Ui, now: f64) {
        let full = ui.max_rect();
        let h = 40.0;
        let tabs = [(Tab::Design, "Design"), (Tab::Data, "Data"), (Tab::Scene, "Scene")];
        let tw = full.width() / tabs.len() as f32;
        for (i, (tab, label)) in tabs.into_iter().enumerate() {
            let r = egui::Rect::from_min_size(full.min + vec2(tw * i as f32, 0.0), vec2(tw, h));
            let resp = ui.interact(r, ui.scope_id().with(("tab", label)), Sense::click());
            let on = self.prefs.tab == tab;
            resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, on, label));
            let mut text = label.to_string();
            if tab == Tab::Data && !self.doc.data.is_empty() {
                text = format!("{label} · {}", self.doc.data.rows.len());
            }
            ui.painter().text(r.center(), Align2::CENTER_CENTER, text, FontId::new(13.0, semibold()), if on { P.accent } else if resp.hovered() { P.text } else { P.muted });
            if on {
                ui.painter().hline(r.x_range(), r.bottom() - 1.0, Stroke::new(2.0, P.accent));
            }
            if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                self.prefs.tab = tab;
            }
        }
        ui.painter().hline(full.x_range(), full.top() + h, Stroke::new(1.0, P.border));
        ui.add_space(h + 1.0);
        egui::ScrollArea::vertical().auto_shrink([false, false]).id_salt(("sidebar", self.prefs.tab as u8)).show(ui, |ui| match self.prefs.tab {
            Tab::Design => self.design_tab(ui, now),
            Tab::Data => self.data_tab(ui, now),
            Tab::Scene => self.scene_tab(ui, now),
        });
    }

    // ================================================================== Design

    /// a template's choice (e.g. a biocard's faction) as a row of coloured buttons
    fn variants_section(&mut self, ui: &mut Ui, now: f64) {
        let Some(v) = self.doc.variants.clone() else { return };
        let mut pick = None;
        section(ui, &v.label, v.hint.as_deref(), None, |ui| {
            let n = v.options.len().max(1) as f32;
            let gap = 8.0;
            let w = (ui.available_width() - gap * (n - 1.0)) / n;
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                for (i, o) in v.options.iter().enumerate() {
                    let on = v.active == Some(i);
                    let (r, resp) = ui.allocate_exact_size(egui::vec2(w, 34.0), egui::Sense::click());
                    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, on, &o.name));
                    let c = crate::widgets::hex_color(&o.color);
                    let p = ui.painter();
                    if on {
                        p.rect_filled(r, egui::CornerRadius::same(8), c);
                    } else {
                        p.rect_filled(r, egui::CornerRadius::same(8), if resp.hovered() { c.gamma_multiply(0.22) } else { c.gamma_multiply(0.08) });
                        p.rect_stroke(r, egui::CornerRadius::same(8), egui::Stroke::new(1.5, c), egui::StrokeKind::Inside);
                    }
                    // white on a dark button, near-black on a light one
                    let lum = 0.3 * c.r() as f32 + 0.59 * c.g() as f32 + 0.11 * c.b() as f32;
                    let text = if on { if lum > 165.0 { Color32::from_rgb(20, 20, 24) } else { Color32::WHITE } } else { P.text };
                    p.text(r.center(), egui::Align2::CENTER_CENTER, &o.name, egui::FontId::new(13.0, semibold()), text);
                    if resp.clicked() && !on {
                        pick = Some(i);
                    }
                    if resp.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                }
            });
        });
        if let Some(i) = pick {
            self.edit(now, None, |d| d.apply_variant(i));
            let name = self.doc.variants.as_ref().and_then(|v| v.options.get(i)).map(|o| o.name.clone()).unwrap_or_default();
            self.say(format!("{}: {name}", v.label), now);
        }
    }

    fn design_tab(&mut self, ui: &mut Ui, now: f64) {
        self.variants_section(ui, now);
        let side_name = self.doc.format.side_name(self.side).to_lowercase();
        let sub = format!("Add to the {side_name} — or pick a tool and click the card");
        section(ui, "Add", Some(&sub), None, |ui| {
            let w = (ui.available_width() - 8.0) / 2.0;
            ui.horizontal(|ui| {
                if big_button(ui, Icon::Type, "Text", w, true).clicked() {
                    self.add_text(now, None);
                }
                if big_button(ui, Icon::Image, "Image", w, true).on_hover_text("PNG, JPEG, WebP, GIF or SVG — or drop a picture onto the window").clicked() {
                    self.pick_image(now, None);
                }
            });
            ui.horizontal(|ui| {
                if big_button(ui, Icon::Shapes, "Shape", w, true).clicked() {
                    self.add_shape(now, None, ShapeKind::Rect);
                }
                if big_button(ui, Icon::QrCode, "QR code", w, true).on_hover_text("Website, contact card, Wi-Fi, phone or email").clicked() {
                    self.add_qr(now, None);
                }
            });
        });
        self.layers_section(ui, now);
        if let Some(l) = self.selected_layer().cloned() {
            self.layer_sections(ui, now, l);
        }
        self.background_section(ui, now);
        self.card_section(ui, now);
    }

    fn layers_section(&mut self, ui: &mut Ui, now: f64) {
        let side = self.side;
        let title = format!("Layers · {}", self.doc.format.side_name(side));
        let ids: Vec<(Layer, String)> = {
            let fill = self.doc.filler(self.current_record());
            self.doc
                .layers_on(side)
                .map(|l| {
                    let name = match l.kind {
                        // its words; a layer named for its job (a template's "Number") says so too
                        LayerType::Text => {
                            let t = l.text_for(&fill).lines().next().unwrap_or("").to_string();
                            let own = l.text.as_deref().and_then(|s| s.lines().next()).unwrap_or("");
                            let named = !l.name.is_empty() && l.name != "Text" && !own.to_lowercase().contains(&l.name.to_lowercase());
                            if named && !t.trim().is_empty() { format!("{} · {t}", l.name) } else { t }
                        }
                        LayerType::Qr => l.qr.as_ref().map(|q| q.summary()).unwrap_or_else(|| l.name.clone()),
                        _ => l.name.clone(),
                    };
                    (l.clone(), if name.trim().is_empty() { kind_label(l).to_string() } else { name })
                })
                .collect()
        };
        section(ui, &title, if ids.is_empty() { Some("Nothing on this side yet") } else { None }, None, |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            for (l, name) in ids.iter().rev() {
                let on = self.selected.as_deref() == Some(l.id.as_str());
                let w = ui.available_width();
                let (r, resp) = ui.allocate_exact_size(vec2(w, 30.0), Sense::click());
                resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, on, name.as_str()));
                if on || resp.hovered() {
                    ui.painter().rect_filled(r, CornerRadius::same(7), if on { P.accent_weak } else { P.panel2 });
                }
                let fg = if !l.visible { P.muted.gamma_multiply(0.6) } else { P.text };
                // a layer that shows through the one below it sits indented under it
                let indent = if l.clip { 14.0 } else { 0.0 };
                if l.clip {
                    ui.painter().text(r.left_center() + vec2(9.0, 0.0), Align2::LEFT_CENTER, icon(Icon::CornerDownRight), FontId::new(11.0, icons()), P.muted);
                }
                ui.painter().text(r.left_center() + vec2(10.0 + indent, 0.0), Align2::LEFT_CENTER, icon(layer_icon(l)), FontId::new(14.0, icons()), if on { P.accent } else { P.muted });
                let g = ui.painter().layout_no_wrap(name.clone(), FontId::proportional(12.5), fg);
                let clip = egui::Rect::from_min_max(r.min, egui::pos2(r.right() - 74.0, r.bottom()));
                ui.painter().with_clip_rect(clip).galley(egui::pos2(r.left() + 32.0 + indent, r.center().y - g.size().y / 2.0), g, fg);
                if l.foil.is_some() || l.spot_uv {
                    ui.painter().text(r.right_center() - vec2(62.0, 0.0), Align2::CENTER_CENTER, icon(Icon::Sparkles), FontId::new(11.0, icons()), P.accent);
                }
                // eye and lock at the right
                let eye = egui::Rect::from_center_size(r.right_center() - vec2(16.0, 0.0), vec2(24.0, 24.0));
                let lock = egui::Rect::from_center_size(r.right_center() - vec2(40.0, 0.0), vec2(24.0, 24.0));
                let mut toggled = None;
                ui.scope_builder(egui::UiBuilder::new().max_rect(eye), |ui| {
                    if row_toggle(ui, Icon::Eye, Icon::EyeOff, l.visible, if l.visible { "Hide" } else { "Show" }) {
                        toggled = Some(0);
                    }
                });
                ui.scope_builder(egui::UiBuilder::new().max_rect(lock), |ui| {
                    if row_toggle(ui, Icon::Lock, Icon::LockOpen, l.locked, if l.locked { "Unlock (can be moved on the card again)" } else { "Lock in place" }) {
                        toggled = Some(1);
                    }
                });
                if let Some(t) = toggled {
                    let id = l.id.clone();
                    self.edit(now, None, |d| {
                        if let Some(x) = d.layer_mut(&id) {
                            if t == 0 {
                                x.visible = !x.visible;
                            } else {
                                x.locked = !x.locked;
                            }
                        }
                    });
                } else if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                    self.selected = if on { None } else { Some(l.id.clone()) };
                }
            }
        });
    }

    /// the selected layer's own settings, then arrange and finish
    fn layer_sections(&mut self, ui: &mut Ui, now: f64, l: Layer) {
        let mut n = l.clone();
        let key = format!("prop-{}", l.id);
        let unit = self.prefs.unit;
        let columns = self.doc.data.columns.clone();
        match l.kind {
            LayerType::Text => {
                section(ui, "Text", Some("{Column} puts a data field here (Data tab)"), None, |ui| {
                    let mut t = n.text.clone().unwrap_or_default();
                    let resp = ui.add(egui::TextEdit::multiline(&mut t).desired_rows(3).desired_width(f32::INFINITY).hint_text("Type the words"));
                    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, "Text"));
                    if self.focus_text {
                        resp.request_focus();
                        self.focus_text = false;
                    }
                    if resp.changed() {
                        // a layer named after its words follows them; one with a name of its own keeps it
                        let was: String = n.text.as_deref().unwrap_or("").lines().next().unwrap_or("Text").chars().take(40).collect();
                        if n.name == was || n.name == "Text" || n.name.is_empty() {
                            n.name = t.lines().next().unwrap_or("Text").chars().take(40).collect();
                        }
                        n.text = Some(t);
                    }
                    if !columns.is_empty() {
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
                            ui.label(RichText::new("Insert").size(11.5).color(P.muted));
                            for c in &columns {
                                if ui.small_button(format!("{{{c}}}")).on_hover_text(format!("Add the {c} field")).clicked() {
                                    let mut t = n.text.clone().unwrap_or_default();
                                    if !t.is_empty() && !t.ends_with([' ', '\n']) {
                                        t.push(' ');
                                    }
                                    t += &format!("{{{c}}}");
                                    n.text = Some(t);
                                }
                            }
                        });
                    }
                    ui.add_space(4.0);
                    // font, in its own typeface (a design font egui hasn't got yet shows in Inter)
                    let cur = n.font.clone().unwrap_or_else(|| "Inter".into());
                    let in_face = |f: &str, size: f32| {
                        let t = RichText::new(f).size(size);
                        if self.previewable_font(f) {
                            t.family(FontFamily::Name(f.to_string().into()))
                        } else {
                            t
                        }
                    };
                    let design_fonts: Vec<String> = self.doc.fonts.iter().map(|f| f.family.clone()).collect();
                    let mut add_font = false;
                    ui.horizontal(|ui| {
                        let lbl = ui.label("Font");
                        ui.with_layout(Layout::right_to_left(EAlign::Center), |ui| {
                            egui::ComboBox::new("font", "")
                                .selected_text(in_face(&cur, 15.0))
                                .width(180.0)
                                .height(440.0)
                                .show_ui(ui, |ui| {
                                    if !design_fonts.is_empty() {
                                        ui.label(RichText::new("IN THIS DESIGN").size(10.0).color(P.muted));
                                        for f in &design_fonts {
                                            if ui.selectable_label(&cur == f, in_face(f, 16.0)).clicked() {
                                                n.font = Some(f.clone());
                                            }
                                        }
                                    }
                                    let mut last_kind = None;
                                    for (f, kind) in cb_print::text::FONTS {
                                        if last_kind != Some(kind) {
                                            ui.label(RichText::new(kind.name().to_uppercase()).size(10.0).color(P.muted));
                                            last_kind = Some(kind);
                                        }
                                        if ui.selectable_label(cur == f, in_face(f, 16.0)).clicked() {
                                            n.font = Some(f.into());
                                        }
                                    }
                                    ui.separator();
                                    if ui.button("Add a font file…").on_hover_text("A .ttf or .otf file — it's saved inside the design").clicked() {
                                        add_font = true;
                                    }
                                })
                                .response
                                .labelled_by(lbl.id);
                        });
                    });
                    if add_font {
                        if let Some(fam) = self.add_font_file(now, &l.id) {
                            n.font = Some(fam);
                        }
                    }
                    // size in points
                    ui.horizontal(|ui| {
                        let lbl = ui.label("Size");
                        ui.with_layout(Layout::right_to_left(EAlign::Center), |ui| {
                            let mut s = n.size.unwrap_or(10.0);
                            if ui.add(egui::DragValue::new(&mut s).range(2.0..=400.0).speed(0.1).max_decimals(1).suffix(" pt")).labelled_by(lbl.id).changed() {
                                n.size = Some(s);
                            }
                            for (v, tip) in [(24.0, "Headline"), (14.0, "Name"), (9.0, "Details"), (7.0, "Small print")] {
                                if ui.small_button(format!("{v}")).on_hover_text(format!("{tip} · {v} pt")).clicked() {
                                    n.size = Some(v);
                                }
                            }
                        });
                    });
                    let mut wght = n.weight.unwrap_or(400.0);
                    let wname = |w: f64| match w as i32 {
                        0..=150 => "Thin",
                        151..=250 => "Extra light",
                        251..=350 => "Light",
                        351..=450 => "Regular",
                        451..=550 => "Medium",
                        551..=650 => "Semibold",
                        651..=750 => "Bold",
                        751..=850 => "Extra bold",
                        _ => "Black",
                    };
                    if slider_row(ui, "Weight", &mut wght, 100.0..=900.0, 100.0, |v| wname(v).to_string(), true) {
                        n.weight = Some(wght);
                    }
                    let mut al = n.align.unwrap_or(Align::Center);
                    ui.horizontal(|ui| {
                        ui.label("Align");
                        ui.with_layout(Layout::right_to_left(EAlign::Center), |ui| {
                            if seg(ui, &mut al, &[(Align::Right, "Right"), (Align::Center, "Centre"), (Align::Left, "Left")], true) {
                                n.align = Some(al);
                            }
                        });
                    });
                    let mut italic = n.italic.unwrap_or(false);
                    if toggle_row(ui, "Italic", &mut italic) {
                        n.italic = Some(italic);
                    }
                    let mut caps = n.uppercase.unwrap_or(false);
                    if toggle_row(ui, "All capitals", &mut caps) {
                        n.uppercase = Some(caps);
                    }
                    let mut wrap = n.wrap.unwrap_or(false);
                    if toggle_row(ui, "Wrap lines at the box width", &mut wrap) {
                        n.wrap = Some(wrap);
                    }
                    hint(ui, if wrap { "Long lines break between words. Drag the side handles to change the width." } else { "Lines longer than the box shrink to fit it — handy for names from a spreadsheet." });
                    let mut va = n.valign;
                    ui.horizontal(|ui| {
                        ui.label("Text box");
                        ui.with_layout(Layout::right_to_left(EAlign::Center), |ui| {
                            seg(ui, &mut va, &[(Some(VAlign::Bottom), "Bottom"), (Some(VAlign::Middle), "Middle"), (Some(VAlign::Top), "Top"), (None, "Fits")], true);
                        });
                    });
                    if va != n.valign {
                        if n.valign.is_none() {
                            // becoming a frame: it keeps the height it has now
                            n.h = cb_print::draw::layer_size(&n, &self.doc.filler(self.current_record())).1;
                        }
                        n.valign = va;
                    }
                    if n.valign.is_some() {
                        hint(ui, "A frame keeps its height (drag its top and bottom handles); the text sits at its top, middle or bottom however long it gets.");
                    }
                    let mut lh = n.line_height.unwrap_or(1.2);
                    if slider_row(ui, "Line height", &mut lh, 0.7..=2.5, 0.05, |v| format!("{v:.2}"), true) {
                        n.line_height = Some(lh);
                    }
                    let mut ls = n.letter_spacing.unwrap_or(0.0);
                    if slider_row(ui, "Letter spacing", &mut ls, -0.1..=0.6, 0.01, |v| format!("{v:.2} em"), true) {
                        n.letter_spacing = Some(ls);
                    }
                    // filled letters, an outline round them, or both (never neither)
                    let col = n.color.clone().unwrap_or_else(|| "#000000".into());
                    let opaque = |c: &str| c.trim_start_matches('#').get(..6).map(|h| format!("#{h}")).unwrap_or_else(|| "#000000".into());
                    let mut filled = cb_print::color::hex_alpha(&col) > 0.0;
                    let mut outlined = n.stroke.is_some() && n.stroke_w.unwrap_or(0.0) > 0.0;
                    if toggle_row(ui, "Fill", &mut filled) {
                        n.color = Some(if filled { opaque(&col) } else { format!("{}00", opaque(&col)) });
                        if !filled && !outlined {
                            n.stroke = Some(opaque(&col));
                            n.stroke_w = Some(0.25);
                            outlined = true;
                        }
                    }
                    if filled {
                        let mut c = opaque(&col);
                        if color_row(ui, "Colour", &mut c) {
                            n.color = Some(c);
                        }
                    }
                    if toggle_row(ui, "Outline", &mut outlined) {
                        if outlined {
                            n.stroke = Some(n.stroke.clone().unwrap_or_else(|| "#1f2347".into()));
                            n.stroke_w = Some(n.stroke_w.filter(|w| *w > 0.0).unwrap_or(0.25));
                        } else {
                            n.stroke_w = Some(0.0);
                            if !filled {
                                n.color = Some(opaque(&col));
                            }
                        }
                    }
                    if outlined {
                        let mut sw = n.stroke_w.unwrap_or(0.25);
                        if slider_row(ui, "Outline width", &mut sw, 0.05..=2.0, 0.05, |v| format!("{v:.2} mm"), true) {
                            n.stroke_w = Some(sw);
                        }
                        let mut sc = n.stroke.clone().unwrap_or_else(|| "#1f2347".into());
                        if color_row(ui, "Outline colour", &mut sc) {
                            n.stroke = Some(sc);
                        }
                    }
                });
            }
            LayerType::Image => {
                section(ui, "Image", None, None, |ui| {
                    ui.horizontal(|ui| {
                        if button(ui, Some(Icon::Image), "Replace…", BtnKind::Normal, true).clicked() {
                            self.replace_image(now, &l.id);
                        }
                        if button(ui, Some(Icon::Scaling), "Fill the panel", BtnKind::Normal, true).on_hover_text("Cover the whole panel, bleed included").clicked() {
                            let p = self.panel_at(l.x, l.y);
                            let b = self.doc.format.bleed;
                            let (bw, bh) = (p.w + 2.0 * b, p.h + 2.0 * b);
                            let k = (bw / l.w).max(bh / l.h);
                            n.w = l.w * k;
                            n.h = l.h * k;
                            n.x = p.x + p.w / 2.0;
                            n.y = p.y + p.h / 2.0;
                            n.rot = if p.upside_down { 180.0 } else { 0.0 };
                        }
                    });
                    if let Some(src) = &l.src {
                        if let Ok(d) = cb_print::images::decode(src) {
                            let (iw, ih) = d.size();
                            let dpi = iw / (l.w / 25.4);
                            let warn = dpi < 200.0 && !src.starts_with("data:image/svg");
                            let text = if src.starts_with("data:image/svg") { "Vector picture: sharp at any size".to_string() } else { format!("{:.0} × {:.0} px · prints at {:.0} dpi", iw, ih, dpi) };
                            ui.label(RichText::new(text).size(11.5).color(if warn { Color32::from_rgb(0xe0, 0xa8, 0x4a) } else { P.muted }));
                            if warn {
                                hint(ui, "Below 200 dpi a photo can look soft in print — use a bigger picture or show it smaller.");
                            }
                            if ui.small_button("Original proportions").clicked() {
                                n.h = n.w * ih / iw.max(1.0);
                            }
                        }
                    }
                });
            }
            LayerType::Shape => {
                let vector = n.shape == Some(ShapeKind::Path);
                let sub = if vector { Some("Artwork drawn as an outline: sharp at any size, in any colour") } else { None };
                section(ui, if vector { "Vector shape" } else { "Shape" }, sub, None, |ui| {
                    let mut k = n.shape.unwrap_or_default();
                    if !vector && seg(ui, &mut k, &[(ShapeKind::Rect, "Rectangle"), (ShapeKind::Ellipse, "Ellipse"), (ShapeKind::Line, "Line")], true) {
                        n.shape = Some(k);
                        if k == ShapeKind::Line {
                            n.stroke = n.stroke.clone().or(n.fill.clone()).or(Some("#1f2347".into()));
                            n.stroke_w = Some(n.stroke_w.unwrap_or(0.5).max(0.1));
                            n.fill = None;
                        } else if n.fill.is_none() && n.stroke_w.unwrap_or(0.0) <= 0.0 {
                            n.fill = Some("#ff7a59".into());
                        }
                    }
                    if k != ShapeKind::Line {
                        let mut has_fill = n.fill.is_some();
                        if toggle_row(ui, "Fill", &mut has_fill) {
                            n.fill = if has_fill { Some("#ff7a59".into()) } else { None };
                        }
                        if let Some(mut f) = n.fill.clone() {
                            if color_row(ui, "Fill colour", &mut f) {
                                n.fill = Some(f);
                            }
                        }
                    }
                    let mut has_stroke = n.stroke.is_some() && n.stroke_w.unwrap_or(0.0) > 0.0;
                    if k != ShapeKind::Line && toggle_row(ui, "Outline", &mut has_stroke) {
                        if has_stroke {
                            n.stroke = Some(n.stroke.clone().unwrap_or_else(|| "#1f2347".into()));
                            n.stroke_w = Some(n.stroke_w.filter(|w| *w > 0.0).unwrap_or(0.5));
                        } else {
                            n.stroke_w = Some(0.0);
                        }
                    }
                    if has_stroke || k == ShapeKind::Line {
                        let mut sw = n.stroke_w.unwrap_or(0.5);
                        if slider_row(ui, if k == ShapeKind::Line { "Thickness" } else { "Outline width" }, &mut sw, 0.05..=5.0, 0.05, |v| format!("{v:.2} mm"), true) {
                            n.stroke_w = Some(sw);
                        }
                        let mut sc = n.stroke.clone().unwrap_or_else(|| "#1f2347".into());
                        if color_row(ui, if k == ShapeKind::Line { "Colour" } else { "Outline colour" }, &mut sc) {
                            n.stroke = Some(sc);
                        }
                        let mut dash = n.dash;
                        ui.horizontal(|ui| {
                            ui.label(if k == ShapeKind::Line { "Style" } else { "Outline style" });
                            ui.with_layout(Layout::right_to_left(EAlign::Center), |ui| {
                                if seg(ui, &mut dash, &[(Some(Dash::Dotted), "Dotted"), (Some(Dash::Dashed), "Dashed"), (None, "Solid")], true) {
                                    n.dash = dash;
                                }
                            });
                        });
                    }
                    if let Some(pw) = n.path.as_ref().filter(|p| p.w > 0.0 && p.h > 0.0).map(|p| p.h / p.w) {
                        if ui.small_button("Original proportions").on_hover_text("Undo any stretching: the height follows the width").clicked() {
                            n.h = n.w * pw;
                        }
                    }
                    if k == ShapeKind::Rect {
                        let mut r = n.radius.unwrap_or(0.0);
                        if slider_row(ui, "Corners", &mut r, 0.0..=(n.w.min(n.h) / 2.0).max(0.1), 0.1, |v| format!("{v:.1} mm"), true) {
                            n.radius = Some(r);
                        }
                    }
                });
            }
            LayerType::Qr => {
                section(ui, "QR code", Some("Scans with any phone camera"), None, |ui| self.qr_editor(ui, &mut n, &columns));
            }
        }
        // ---- arrange
        section(ui, "Arrange", None, None, |ui| {
            let (sw, sh) = self.doc.format.sheet();
            let b = self.doc.format.bleed + 50.0;
            length_row(ui, "Across (x)", &mut n.x, unit, -b..=sw + b);
            length_row(ui, "Down (y)", &mut n.y, unit, -b..=sh + b);
            if l.kind != LayerType::Text {
                let qr = l.kind == LayerType::Qr;
                if length_row(ui, if qr { "Size" } else { "Width" }, &mut n.w, unit, 1.0..=1000.0) && qr {
                    n.h = n.w;
                }
                if !qr && l.shape != Some(ShapeKind::Line) {
                    length_row(ui, "Height", &mut n.h, unit, 0.5..=1000.0);
                }
            } else {
                length_row(ui, "Box width", &mut n.w, unit, 2.0..=1000.0);
            }
            ui.horizontal(|ui| {
                let lbl = ui.label("Rotation");
                ui.with_layout(Layout::right_to_left(EAlign::Center), |ui| {
                    if ui.small_button("0°").clicked() {
                        n.rot = 0.0;
                    }
                    ui.add(egui::DragValue::new(&mut n.rot).range(-180.0..=180.0).speed(0.5).suffix("°")).labelled_by(lbl.id);
                });
            });
            let mut op = n.opacity;
            if slider_row(ui, "Opacity", &mut op, 0.0..=1.0, 0.01, |v| format!("{:.0}%", v * 100.0), true) {
                n.opacity = op;
            }
            let mut clip = n.clip;
            if toggle_row(ui, "Clip to the layer below", &mut clip) {
                n.clip = clip;
            }
            if n.clip {
                hint(ui, "Shows only inside the layer under it — a photo in a window shape. Replace the picture and it stays in the window.");
            }
            ui.add_space(4.0);
            let p = self.panel_at(l.x, l.y);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
                if crate::widgets::icon_button(ui, Icon::AlignHorizontalJustifyCenter, "Centre across the panel", true).clicked() {
                    n.x = p.x + p.w / 2.0;
                }
                if crate::widgets::icon_button(ui, Icon::AlignVerticalJustifyCenter, "Centre down the panel", true).clicked() {
                    n.y = p.y + p.h / 2.0;
                }
                if crate::widgets::icon_button(ui, Icon::ArrowUp, "Bring forward", true).clicked() {
                    self.move_layer(now, &l.id, 1);
                }
                if crate::widgets::icon_button(ui, Icon::ArrowDown, "Send backward", true).clicked() {
                    self.move_layer(now, &l.id, -1);
                }
                if crate::widgets::icon_button(ui, Icon::Copy, "Duplicate (Ctrl+D)", true).clicked() {
                    self.duplicate_layer(now, &l.id);
                }
                let other = self.doc.format.side_name(self.side.other()).to_lowercase();
                if crate::widgets::icon_button(ui, Icon::ArrowLeftRight, &format!("Move to the {other}"), true).clicked() {
                    self.move_to_other_side(now, &l.id);
                }
                if crate::widgets::icon_button(ui, Icon::Trash2, "Delete (Del)", true).clicked() {
                    self.selected = Some(l.id.clone());
                    self.remove_selected(now);
                }
            });
        });
        // ---- finish
        section(ui, "Finish", Some("Foil and spot UV are extra print steps — check your print shop offers them"), None, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
                let none = n.foil.is_none();
                let (r, resp) = ui.allocate_exact_size(vec2(56.0, 28.0), Sense::click());
                resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, none, "No foil"));
                ui.painter().rect(r, CornerRadius::same(14), if resp.hovered() { P.panel2 } else { Color32::TRANSPARENT }, Stroke::new(if none { 2.0 } else { 1.0 }, if none { P.accent } else { P.border }), StrokeKind::Inside);
                ui.painter().text(r.center(), Align2::CENTER_CENTER, "None", FontId::proportional(12.0), P.text);
                if resp.on_hover_text("Printed ink only").clicked() {
                    n.foil = None;
                }
                for f in Foil::ALL {
                    let on = n.foil == Some(f);
                    let (dark, light) = f.colors();
                    let (r, resp) = ui.allocate_exact_size(vec2(28.0, 28.0), Sense::click());
                    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, on, f.name()));
                    let c = r.center();
                    if on {
                        ui.painter().circle_stroke(c, 13.5, Stroke::new(2.0, P.accent));
                    }
                    ui.painter().circle_filled(c, 11.0, hex_color(dark));
                    ui.painter().circle_filled(c + vec2(-2.5, -2.5), 6.5, hex_color(light));
                    if resp.on_hover_text(format!("{} foil", f.name())).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                        n.foil = Some(f);
                    }
                }
            });
            let mut uv = n.spot_uv;
            if toggle_row(ui, "Spot UV (glossy varnish)", &mut uv) {
                n.spot_uv = uv;
            }
        });
        if n != l {
            if n.kind == LayerType::Text && n.valign.is_none() {
                n.h = cb_print::draw::layer_size(&n, &self.doc.filler(self.current_record())).1;
            }
            let id = l.id.clone();
            self.edit(now, Some(&key), |d| {
                if let Some(x) = d.layer_mut(&id) {
                    *x = n;
                }
            });
        }
    }

    fn qr_editor(&mut self, ui: &mut Ui, n: &mut Layer, columns: &[String]) {
        let Some(mut q) = n.qr.clone() else { return };
        egui::ComboBox::new("qr-kind", "")
            .selected_text(q.kind.name())
            .width(ui.available_width() - 8.0)
            .show_ui(ui, |ui| {
                for k in QrKind::ALL {
                    ui.selectable_value(&mut q.kind, k, k.name());
                }
            });
        let field = |ui: &mut Ui, label: &str, v: &mut String, hint_text: &str| {
            ui.label(RichText::new(label).size(12.0).color(P.muted));
            ui.add(egui::TextEdit::singleline(v).desired_width(f32::INFINITY).hint_text(hint_text)).widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, label));
        };
        match q.kind {
            QrKind::Url => field(ui, "Website", &mut q.text, "https://example.com"),
            QrKind::Text => {
                ui.add(egui::TextEdit::multiline(&mut q.text).desired_rows(2).desired_width(f32::INFINITY).hint_text("Any text"));
            }
            QrKind::Phone => field(ui, "Phone number", &mut q.text, "+60 12-345 6789"),
            QrKind::Email => field(ui, "Email address", &mut q.text, "name@example.com"),
            QrKind::Vcard => {
                field(ui, "Name", &mut q.name, "Alex Morgan");
                field(ui, "Company", &mut q.org, "");
                field(ui, "Job title", &mut q.title, "");
                field(ui, "Phone", &mut q.phone, "");
                field(ui, "Email", &mut q.email, "");
                field(ui, "Website", &mut q.url, "");
                field(ui, "Address", &mut q.address, "");
            }
            QrKind::Wifi => {
                field(ui, "Network name (SSID)", &mut q.ssid, "Cafe Wi-Fi");
                ui.label(RichText::new("Security").size(12.0).color(P.muted));
                let mut s: &str = match q.security.as_str() {
                    "WEP" => "WEP",
                    "nopass" => "nopass",
                    _ => "WPA",
                };
                if seg(ui, &mut s, &[("WPA", "WPA/WPA2"), ("WEP", "WEP"), ("nopass", "Open")], true) {
                    q.security = s.to_string();
                }
                if q.security != "nopass" {
                    field(ui, "Password", &mut q.password, "");
                }
                ui.checkbox(&mut q.hidden, "Hidden network");
            }
        }
        if !columns.is_empty() {
            hint(ui, &format!("Fields work here too, e.g. {{{}}}", columns[0]));
        }
        ui.add_space(4.0);
        let mut code = n.color.clone().unwrap_or_else(|| "#000000".into());
        if color_row(ui, "Code colour", &mut code) {
            n.color = Some(code.clone());
        }
        let mut tile = n.fill.is_some();
        if toggle_row(ui, "Background tile", &mut tile) {
            n.fill = if tile { Some("#ffffff".into()) } else { None };
        }
        if let Some(mut f) = n.fill.clone() {
            if color_row(ui, "Tile colour", &mut f) {
                n.fill = Some(f);
            }
        }
        ui.horizontal(|ui| {
            ui.label("Error correction");
            ui.with_layout(Layout::right_to_left(EAlign::Center), |ui| {
                seg(ui, &mut q.ecc, &[(QrEcc::H, "H"), (QrEcc::Q, "Q"), (QrEcc::M, "M"), (QrEcc::L, "L")], true);
            });
        });
        hint(ui, "Higher correction scans when part of the code is covered (e.g. by a logo), but makes the code denser.");
        // will it scan?
        let fill = self.doc.filler(self.current_record());
        let data = q.payload(&fill);
        let quiet = if n.fill.is_some() { 2 } else { 0 };
        match cb_print::qr::module_mm(&data, q.ecc, n.w.min(n.h), quiet) {
            Some(m) => {
                let ok = m >= cb_print::qr::MIN_MODULE_MM;
                let text = if ok { format!("Dots of {m:.2} mm — scans easily") } else { format!("Dots of {m:.2} mm — too small to scan reliably: make the code bigger or hold less data") };
                ui.label(RichText::new(text).size(11.5).color(if ok { P.muted } else { Color32::from_rgb(0xe0, 0xa8, 0x4a) }));
                let lum = |h: &str| cb_print::color::luminance(h);
                let bg = n.fill.clone().unwrap_or_else(|| match self.doc.background(self.side).mode {
                    BgMode::Solid | BgMode::Gradient => self.doc.background(self.side).color.clone(),
                    _ => self.doc.paper.color.clone(),
                });
                if lum(&code) > lum(&bg) - 0.35 {
                    ui.label(RichText::new("The code needs to be clearly darker than what's behind it to scan.").size(11.5).color(Color32::from_rgb(0xe0, 0xa8, 0x4a)));
                }
            }
            None => {
                ui.label(RichText::new("Too much data for a QR code").size(11.5).color(Color32::from_rgb(0xe5, 0x48, 0x4d)));
            }
        }
        if q != *n.qr.as_ref().unwrap() {
            n.qr = Some(q);
        }
    }

    fn background_section(&mut self, ui: &mut Ui, now: f64) {
        let side = self.side;
        let title = format!("Background · {}", self.doc.format.side_name(side));
        let mut bg = self.doc.background(side).clone();
        let mut pick = false;
        section(ui, &title, None, None, |ui| {
            seg(ui, &mut bg.mode, &[(BgMode::Paper, "Paper"), (BgMode::Solid, "Colour"), (BgMode::Gradient, "Gradient"), (BgMode::Image, "Picture")], true);
            match bg.mode {
                BgMode::Paper => hint(ui, "Nothing printed: the paper shows (Scene › Paper)."),
                BgMode::Solid => {
                    color_row(ui, "Colour", &mut bg.color);
                }
                BgMode::Gradient => {
                    color_row(ui, "From", &mut bg.color);
                    color_row(ui, "To", &mut bg.color2);
                    slider_row(ui, "Direction", &mut bg.angle, 0.0..=360.0, 5.0, |v| format!("{v:.0}°"), true);
                }
                BgMode::Image => {
                    ui.horizontal(|ui| {
                        if button(ui, Some(Icon::Image), if bg.src.is_some() { "Replace…" } else { "Choose picture…" }, BtnKind::Normal, true).clicked() {
                            pick = true;
                        }
                        if bg.src.is_some() && button(ui, Some(Icon::X), "Remove", BtnKind::Normal, true).clicked() {
                            bg.src = None;
                        }
                    });
                    hint(ui, "The picture covers the whole side, bleed included.");
                }
            }
        });
        if pick {
            if let Some(src) = self.choose_picture(now, side) {
                bg.src = Some(src);
            }
        }
        if bg != *self.doc.background(side) {
            self.edit(now, Some(&format!("bg-{side:?}")), |d| *d.background_mut(side) = bg);
        }
    }

    fn card_section(&mut self, ui: &mut Ui, now: f64) {
        let mut f = self.doc.format.clone();
        let mut anchor = egui::Rect::NOTHING;
        let title = self.format_title();
        section(ui, "Card", None, None, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(icon(crate::formats::category_icon(f.category()))).family(icons()).size(18.0).color(P.accent));
                // the button first, so a long name is cut short instead of running under it
                ui.with_layout(Layout::right_to_left(EAlign::Center), |ui| {
                    let r = button(ui, None, "Change…", BtnKind::Normal, true);
                    anchor = r.rect;
                    if r.clicked() {
                        self.popups.format_top = !self.popups.format_top;
                        self.popups.format_cat = None;
                        if self.popups.format_top {
                            self.rescan_templates();
                        }
                    }
                    ui.with_layout(Layout::top_down(EAlign::Min), |ui| {
                        ui.add(egui::Label::new(RichText::new(&title).family(semibold())).truncate());
                        ui.add(egui::Label::new(RichText::new(format!("{} · {}", f.size_label(), f.fold.label())).size(11.5).color(P.muted)).truncate());
                    });
                });
            });
            if (f.w - f.h).abs() > 0.01 {
                let mut land = f.landscape();
                ui.horizontal(|ui| {
                    ui.label("Orientation");
                    ui.with_layout(Layout::right_to_left(EAlign::Center), |ui| {
                        if seg(ui, &mut land, &[(true, "Landscape"), (false, "Portrait")], true) {
                            f = f.rotated();
                        }
                    });
                });
            }
            let max_r = (f.w.min(f.h) / 2.0).min(30.0);
            slider_row(ui, "Corners", &mut f.radius, 0.0..=max_r, 0.25, |v| if v == 0.0 { "Square".into() } else { format!("{v:.1} mm") }, true);
            slider_row(ui, "Bleed", &mut f.bleed, 0.0..=6.0, 0.25, |v| format!("{v:.1} mm"), true);
            slider_row(ui, "Safe area", &mut f.safe, 0.0..=(f.w.min(f.h) / 4.0), 0.25, |v| format!("{v:.1} mm"), true);
            hint(ui, "Most print shops want 3 mm bleed (⅛ in in the US) and text at least 3–5 mm inside the trim.");
            if f.fold != Fold::None {
                hint(ui, &format!("Printed on a {} × {} mm sheet, folded in half.", cb_core::format::trim_num(f.sheet().0), cb_core::format::trim_num(f.sheet().1)));
            }
        });
        if f != self.doc.format {
            let rot = (f.w - self.doc.format.w).abs() > 1e-9;
            if rot {
                self.set_format(now, f);
            } else {
                self.edit(now, Some("format-sliders"), |d| d.format = f);
            }
        }
        let _ = anchor;
    }

    // ================================================================== Data

    fn data_tab(&mut self, ui: &mut Ui, now: f64) {
        section(ui, "Batch cards", Some("One card per row of a spreadsheet: names, titles, numbers, QR links…"), None, |ui| {
            let w = (ui.available_width() - 8.0) / 2.0;
            ui.horizontal(|ui| {
                if big_button(ui, Icon::FileSpreadsheet, "Import CSV…", w, true).on_hover_text("A spreadsheet saved as CSV (Excel, LibreOffice, Google Sheets) — or drop one onto the window").clicked() {
                    self.import_data(now);
                }
                if big_button(ui, Icon::Sparkles, "Sample data", w, true).on_hover_text("Five example people to try it with").clicked() {
                    let mut d = cb_core::data::sample();
                    d.name = "Sample team".into();
                    self.edit(now, None, |doc| doc.data = d);
                    self.record = 0;
                    self.say("Sample data added — put {Name} or {Title} in a text box", now);
                }
            });
            if self.doc.data.is_empty() {
                hint(ui, "The first row of the file names the columns. Then type {Name} (or any column) into a text box or QR code, and every card gets its own row.");
            } else {
                let d = &self.doc.data;
                ui.label(RichText::new(format!("{} · {} rows · {} columns", if d.name.is_empty() { "Data" } else { &d.name }, d.rows.len(), d.columns.len())).color(P.text));
                ui.horizontal(|ui| {
                    if button(ui, Some(Icon::Trash2), "Clear data", BtnKind::Normal, true).clicked() {
                        self.edit(now, None, |doc| doc.data = Default::default());
                        self.say("Data cleared — Ctrl+Z brings it back", now);
                    }
                });
            }
        });
        if self.doc.data.columns.is_empty() {
            return;
        }
        // ---- fields
        let cols = self.doc.data.columns.clone();
        section(ui, "Fields", Some("Click to add a field to the selected text (or as new text)"), None, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
                for c in &cols {
                    if ui.button(RichText::new(format!("{{{c}}}")).monospace()).clicked() {
                        self.insert_field(now, c);
                    }
                }
            });
            // which placeholders are used, and do they all exist?
            let mut used: Vec<String> = Vec::new();
            for l in &self.doc.layers {
                let mut texts = vec![l.text.clone().unwrap_or_default()];
                if let Some(q) = &l.qr {
                    texts.extend([q.text.clone(), q.name.clone(), q.org.clone(), q.title.clone(), q.phone.clone(), q.email.clone(), q.url.clone(), q.address.clone(), q.ssid.clone(), q.password.clone()]);
                }
                for t in texts {
                    for p in cb_core::data::placeholders(&t) {
                        if !used.contains(&p) {
                            used.push(p);
                        }
                    }
                }
            }
            let missing: Vec<&String> = used.iter().filter(|p| !cols.iter().any(|c| c.eq_ignore_ascii_case(p.trim()))).collect();
            if used.is_empty() {
                hint(ui, "No fields on the card yet.");
            } else if !missing.is_empty() {
                ui.label(RichText::new(format!("No column for {}", missing.iter().map(|m| format!("{{{m}}}")).collect::<Vec<_>>().join(", "))).size(11.5).color(Color32::from_rgb(0xe0, 0xa8, 0x4a)));
            } else {
                hint(ui, &format!("On the card: {}", used.iter().map(|m| format!("{{{m}}}")).collect::<Vec<_>>().join(" ")));
            }
        });
        // ---- the rows
        let n = self.doc.data.rows.len();
        let sub = format!("Row {} of {n} is on the card — click a row number to show it", self.record.min(n.saturating_sub(1)) + 1);
        let mut data = self.doc.data.clone();
        let mut show_row: Option<usize> = None;
        let mut remove_col: Option<usize> = None;
        section(ui, "Rows", Some(&sub), None, |ui| {
            let cw = 120.0;
            egui::ScrollArea::horizontal().id_salt("data-h").show(ui, |ui| {
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.add_sized([30.0, 22.0], egui::Label::new(RichText::new("#").color(P.muted)));
                        for (ci, c) in data.columns.iter_mut().enumerate() {
                            ui.allocate_ui(vec2(cw, 22.0), |ui| {
                                ui.horizontal(|ui| {
                                    ui.add(egui::TextEdit::singleline(c).desired_width(cw - 26.0).font(FontId::new(12.0, semibold())));
                                    if ui.small_button("×").on_hover_text("Remove this column").clicked() {
                                        remove_col = Some(ci);
                                    }
                                });
                            });
                        }
                    });
                    let row_h = 24.0;
                    egui::ScrollArea::vertical().id_salt("data-v").max_height(360.0).show_rows(ui, row_h, data.rows.len(), |ui, range| {
                        for ri in range {
                            ui.horizontal(|ui| {
                                let cur = ri == self.record;
                                let num = ui.add_sized([30.0, 22.0], egui::Button::new(RichText::new(format!("{}", ri + 1)).size(11.5).color(if cur { P.accent } else { P.muted })).frame(cur));
                                if num.on_hover_text("Show this row on the card").clicked() {
                                    show_row = Some(ri);
                                }
                                for cell in data.rows[ri].iter_mut() {
                                    ui.add(egui::TextEdit::singleline(cell).desired_width(cw));
                                }
                            });
                        }
                    });
                });
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if button(ui, Some(Icon::Plus), "Row", BtnKind::Normal, true).clicked() {
                    data.rows.push(vec![String::new(); data.columns.len()]);
                    show_row = Some(data.rows.len() - 1);
                }
                if button(ui, Some(Icon::Plus), "Column", BtnKind::Normal, true).clicked() {
                    data.columns.push(format!("Column {}", data.columns.len() + 1));
                    for r in &mut data.rows {
                        r.push(String::new());
                    }
                }
                if !data.rows.is_empty() && button(ui, Some(Icon::Trash2), "Row", BtnKind::Normal, true).on_hover_text("Delete the row shown on the card").clicked() {
                    let r = self.record.min(data.rows.len() - 1);
                    data.rows.remove(r);
                }
            });
        });
        if let Some(c) = remove_col {
            data.columns.remove(c);
            for r in &mut data.rows {
                if c < r.len() {
                    r.remove(c);
                }
            }
        }
        if data != self.doc.data {
            self.edit(now, Some("data-table"), |d| d.data = data);
        }
        if let Some(r) = show_row {
            self.record = r;
        }
        self.fix_selection();
    }

    /// Add a font file (.ttf / .otf) to the design: it travels inside the .cb file. Returns the
    /// family name to use (the web version sets it on text layer `layer` itself, once the
    /// browser has the file).
    pub(crate) fn add_font_file(&mut self, now: f64, layer: &str) -> Option<String> {
        let (name, bytes) = self.pick_file(crate::pick::PickFor::Font(layer.to_string()), "Add a font", "Font", &["ttf", "otf"], now)?;
        self.add_font_bytes(&name, &bytes, now)
    }

    /// add the font in `bytes` (from the file `name`) to the design; its family name
    pub(crate) fn add_font_bytes(&mut self, name: &str, bytes: &[u8], now: f64) -> Option<String> {
        use base64::Engine as _;
        let Some(mut family) = cb_print::text::family_name(bytes) else {
            self.say("That isn't a font CardBuilder can use", now);
            return None;
        };
        if cb_print::text::is_bundled(&family) {
            family = format!("{family} (file)");
        }
        let mime = if name.to_ascii_lowercase().ends_with(".otf") { "font/otf" } else { "font/ttf" };
        let src = format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes));
        let fam = family.clone();
        self.edit(now, None, |d| {
            d.fonts.retain(|f| f.family != fam);
            d.fonts.push(cb_core::doc::FontFile { family: fam.clone(), src });
        });
        cb_print::text::register_fonts(&self.doc.fonts);
        self.say(format!("Added the font {family} — it's saved inside this design"), now);
        Some(family)
    }

    /// put `{column}` into the selected text layer, or add a text layer with it
    pub(crate) fn insert_field(&mut self, now: f64, column: &str) {
        let field = format!("{{{column}}}");
        match self.selected_layer().filter(|l| l.kind == LayerType::Text).map(|l| l.id.clone()) {
            Some(id) => {
                self.edit(now, None, |d| {
                    if let Some(l) = d.layer_mut(&id) {
                        let mut t = l.text.clone().unwrap_or_default();
                        if t == "Your text" {
                            t.clear();
                        }
                        if !t.is_empty() && !t.ends_with([' ', '\n']) {
                            t.push(' ');
                        }
                        t += &field;
                        l.text = Some(t);
                    }
                });
            }
            None => {
                self.add_text(now, None);
                let id = self.selected.clone().unwrap_or_default();
                self.edit(now, None, |d| {
                    if let Some(l) = d.layer_mut(&id) {
                        l.text = Some(field.clone());
                        l.name = field.clone();
                    }
                });
                self.focus_text = false;
            }
        }
        self.prefs.tab = Tab::Design;
    }

    // ================================================================== Scene

    fn scene_tab(&mut self, ui: &mut Ui, now: f64) {
        let mut paper = self.doc.paper.clone();
        section(ui, "Paper", Some("The card stock — how it looks in 3D, and its colour where nothing is printed"), None, |ui| {
            let cols = 3;
            let gap = 6.0;
            let w = (ui.available_width() - gap * (cols as f32 - 1.0)) / cols as f32;
            for row in Stock::ALL.chunks(cols) {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = gap;
                    for s in row {
                        let info = s.info();
                        let on = paper.stock == *s;
                        let (r, resp) = ui.allocate_exact_size(vec2(w, 50.0), Sense::click());
                        resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, on, info.name));
                        ui.painter().rect(r, CornerRadius::same(8), if on { P.accent_weak } else if resp.hovered() { P.panel2 } else { Color32::TRANSPARENT }, Stroke::new(if on { 1.5 } else { 1.0 }, if on { P.accent } else { P.border }), StrokeKind::Inside);
                        let sw = egui::Rect::from_min_size(r.min + vec2(8.0, 8.0), vec2(w - 16.0, 14.0));
                        ui.painter().rect(sw, CornerRadius::same(3), hex_color(info.color), Stroke::new(1.0, P.border), StrokeKind::Inside);
                        if info.gloss > 0.2 {
                            ui.painter().line_segment([sw.left_top() + vec2(sw.width() * 0.55, 1.0), sw.left_bottom() + vec2(sw.width() * 0.45, -1.0)], Stroke::new(3.0, Color32::from_white_alpha(140)));
                        }
                        ui.painter().text(egui::pos2(r.center().x, r.bottom() - 9.0), Align2::CENTER_BOTTOM, info.name, FontId::proportional(11.5), if on { P.text } else { P.muted });
                        if resp.on_hover_text(info.note).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                            paper.stock = *s;
                            paper.color = info.color.into();
                        }
                    }
                });
                ui.add_space(gap);
            }
            hint(ui, paper.stock.info().note);
            color_row(ui, "Paper colour", &mut paper.color);
        });
        if paper != self.doc.paper {
            self.edit(now, Some("paper"), |d| d.paper = paper);
        }
        let mut sc = self.doc.scene.clone();
        let folded = self.doc.format.fold != Fold::None;
        section(ui, "3D scene", None, None, |ui| {
            ui.horizontal(|ui| {
                ui.label("Backdrop");
                ui.with_layout(Layout::right_to_left(EAlign::Center), |ui| {
                    seg(ui, &mut sc.backdrop, &[(Backdrop::Solid, "Colour"), (Backdrop::Desk, "Desk"), (Backdrop::Dark, "Dark"), (Backdrop::Studio, "Studio")], true);
                });
            });
            if sc.backdrop == Backdrop::Solid {
                color_row(ui, "Backdrop colour", &mut sc.bg_color);
            }
            slider_row(ui, "Light", &mut sc.light, 0.3..=1.8, 0.05, |v| format!("{:.0}%", v * 100.0), true);
            slider_row(ui, "Light from", &mut sc.light_angle, -90.0..=90.0, 1.0, |v| format!("{v:.0}°"), true);
            toggle_row(ui, "Soft shadows", &mut sc.shadows);
            toggle_row(ui, "Turn slowly", &mut sc.auto_rotate);
            if !folded {
                toggle_row(ui, "Front and back side by side", &mut sc.pair);
            }
            if folded {
                slider_row(ui, "Opened", &mut sc.open, 0.0..=180.0, 1.0, |v| format!("{v:.0}°"), true);
            }
        });
        if sc != self.doc.scene {
            self.edit(now, Some("scene"), |d| d.scene = sc);
        }
        self.gpu_section(ui, now);
    }

    /// Scene › Graphics card: what draws the 3D, every card found, and the choice for next start
    fn gpu_section(&mut self, ui: &mut Ui, now: f64) {
        #[cfg_attr(target_arch = "wasm32", allow(unused_mut))]
        let mut restart = false;
        section(ui, "Graphics card", None, None, |ui| {
            let gpu = &mut self.gpu;
            match &gpu.active {
                Some(a) => {
                    let (ic, tint) = if a.software() { (Icon::TriangleAlert, Color32::from_rgb(0xe0, 0xa8, 0x4a)) } else { (Icon::Cpu, P.accent) };
                    egui::Frame::NONE.fill(P.panel2).corner_radius(CornerRadius::same(10)).inner_margin(egui::Margin::same(10)).stroke(Stroke::new(1.0, P.border)).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(icon(ic)).family(icons()).size(18.0).color(tint));
                            ui.vertical(|ui| {
                                ui.label(RichText::new(&a.name).family(semibold()));
                                ui.label(RichText::new(format!("{} · {}", a.backend, a.kind_label())).size(11.0).color(P.muted));
                                if !a.driver.is_empty() {
                                    ui.label(RichText::new(format!("Driver {}", a.driver)).size(11.0).color(P.muted));
                                }
                            });
                        });
                    });
                    if let (true, Some(p)) = (a.software(), &gpu.problem) {
                        ui.label(RichText::new(p).size(11.5).color(Color32::from_rgb(0xe0, 0xa8, 0x4a)));
                    } else if a.software() {
                        hint(
                            ui,
                            if gpu.better_than_active().is_some() {
                                "The 3D is being drawn by the CPU although a graphics card was found. Choose it below and restart."
                            } else if cfg!(target_os = "windows") {
                                "No graphics card driver could be used, so the 3D is drawn by the CPU (slow). Install or update your graphics driver from NVIDIA, AMD or Intel and restart."
                            } else {
                                "No graphics card driver could be used, so the 3D is drawn by the CPU (slow). Install your card's Vulkan driver (NVIDIA: nvidia-utils · AMD: vulkan-radeon · Intel: vulkan-intel) and restart."
                            },
                        );
                    } else {
                        hint(ui, "The 3D preview and the whole window are drawn by this graphics card.");
                    }
                }
                None => hint(ui, "This build draws without wgpu, so the graphics card can't be chosen."),
            }
            #[cfg(not(target_arch = "wasm32"))]
            {
                let label = |c: &crate::gpu::GpuChoice| match c {
                    crate::gpu::GpuChoice::Auto => "Auto-detect (best card)".to_string(),
                    crate::gpu::GpuChoice::Adapter { name, backend } => format!("{name} · {backend}"),
                };
                let mut choice = gpu.choice.clone();
                ui.horizontal(|ui| {
                    let lbl = ui.label("Use");
                    ui.with_layout(Layout::right_to_left(EAlign::Center), |ui| {
                        egui::ComboBox::new("gpu-choice", "")
                            .selected_text(label(&choice))
                            .width(210.0)
                            .truncate()
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut choice, crate::gpu::GpuChoice::Auto, label(&crate::gpu::GpuChoice::Auto));
                                for g in &gpu.all {
                                    let text = format!("{} · {}{}", g.name, g.backend, if g.software() { " (CPU)" } else { "" });
                                    ui.selectable_value(&mut choice, g.key(), text);
                                }
                            })
                            .response
                            .labelled_by(lbl.id);
                    });
                });
                if choice != gpu.choice {
                    match crate::gpu::save_choice(&choice) {
                        Ok(()) => gpu.choice = choice,
                        Err(e) => self.status = Some((format!("Couldn't save the graphics card choice: {e}"), now)),
                    }
                }
                let gpu = &self.gpu;
                if gpu.choice != gpu.started_with {
                    hint(ui, "The new choice takes effect when CardBuilder restarts.");
                    if ui.add_sized([ui.available_width(), 30.0], egui::Button::new("Restart CardBuilder now")).clicked() {
                        restart = true;
                    }
                }
            }
            // in the browser, the browser chooses the graphics card
            #[cfg(target_arch = "wasm32")]
            hint(ui, "In the browser, the browser decides which graphics card draws CardBuilder.");
        });
        #[cfg(not(target_arch = "wasm32"))]
        if restart {
            match crate::gpu::relaunch() {
                Ok(()) => ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close),
                Err(e) => self.say(format!("Couldn't restart: {e}"), now),
            }
        }
        #[cfg(target_arch = "wasm32")]
        let _ = (restart, now);
    }
}
