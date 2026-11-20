//! Top bar (the window's title bar), pop-up menus and the changelog / help windows.

use crate::app::{CardBuilderApp, Mode, VERSION};
use crate::exports::Export;
use crate::style::{icon, icons, semibold, P};
use crate::widgets::{button, button_width, icon_button, icon_button_framed, seg, seg_size, BtnKind, ICON_BUTTON_FRAMED_W};
use cb_core::doc::Side;
use egui::{pos2, vec2, Align, Align2, Color32, CornerRadius, FontId, Layout, Rect, RichText, Sense, Stroke, StrokeKind, Ui, UiBuilder};
use lucide_icons::Icon;

const LOGO: &[u8] = include_bytes!("../assets/logo-mark-128.png");

/// a pop-up anchored under `anchor`; closes when clicking elsewhere
pub(crate) fn popup(ctx: &egui::Context, id: &str, anchor: Rect, right: bool, open: &mut bool, width: f32, add: impl FnOnce(&mut Ui)) {
    if !*open {
        return;
    }
    let (pos, pivot) = if right { (anchor.right_bottom() + vec2(0.0, 6.0), Align2::RIGHT_TOP) } else { (anchor.left_bottom() + vec2(0.0, 6.0), Align2::LEFT_TOP) };
    let area = egui::Area::new(egui::Id::unique(id)).order(egui::Order::Foreground).fixed_pos(pos).pivot(pivot).constrain(true).show(ctx, |ui| {
        egui::Frame::NONE
            .fill(P.menu)
            .stroke(Stroke::new(1.0, P.border))
            .corner_radius(CornerRadius::same(10))
            .inner_margin(egui::Margin::same(8))
            .shadow(egui::Shadow { offset: [0, 10], blur: 30, spread: 0, color: Color32::from_black_alpha(115) })
            .show(ui, |ui| {
                ui.set_width(width);
                add(ui);
            })
            .response
            .rect
    });
    // the pop-up as drawn this frame: the area's remembered size lags a frame behind when the
    // contents grow (opening a group), which made a click on the new bottom row close it
    let shown = area.response.rect.union(area.inner);
    let outside = ctx.input(|i| i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|p| !shown.contains(p) && !anchor.contains(p)));
    // a click in a combo box's list (its own layer) is not "outside"
    let in_other_popup = ctx.input(|i| i.pointer.interact_pos()).is_some_and(|p| ctx.layer_id_at(p).is_some_and(|l| l.order == egui::Order::Foreground && l.id != area.response.layer_id.id));
    if outside && !in_other_popup {
        *open = false;
    }
}

/// How compact the top bar is drawn — the least compact one that fits the window wins:
/// `Full` everything labelled · `IconButtons` Theme / Print become icons · `Short` + no
/// "CardBuilder" wordmark · `Tight` + the format chip becomes an icon and the divider before Help goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Density {
    Full,
    IconButtons,
    Short,
    Tight,
}

const GROUP_GAP: f32 = 12.0;

fn mode_options() -> [(Mode, &'static str); 3] {
    [(Mode::Card, "Card"), (Mode::Split, "Split"), (Mode::ThreeD, "3D")]
}

fn text_w(ui: &Ui, text: &str, font: FontId) -> f32 {
    ui.fonts_mut(|f| f.layout_no_wrap(text.to_string(), font, P.text).size().x)
}

/// widths of the left group, the right group and the layout switcher at density `d`
fn bar_widths(ui: &Ui, d: Density, chip: &(String, String)) -> (f32, f32, f32) {
    let sx = ui.spacing().item_spacing.x;
    let mut left = vec![28.0];
    if d < Density::Short {
        left.push(text_w(ui, "CardBuilder", FontId::new(17.0, semibold())));
    }
    left.push(text_w(ui, &format!("v{VERSION}"), FontId::proportional(11.0)) + 12.0);
    let c = if d < Density::Tight { text_w(ui, &chip.0, FontId::new(11.0, semibold())).max(text_w(ui, &chip.1, FontId::proportional(9.5))) + 12.0 } else { 32.0 };
    left.push(c);
    let left_w = left.iter().sum::<f32>() + sx * (left.len() as f32 - 1.0) + 10.0;
    let compact = d >= Density::IconButtons;
    let framed = |ic: bool, text: &str| if compact { ICON_BUTTON_FRAMED_W } else { button_width(ui, ic, text) };
    let mut right = vec![button_width(ui, true, "Export"), framed(true, "Print"), framed(true, "Theme"), 30.0, 30.0, 30.0, 30.0, 30.0];
    if d < Density::Tight {
        right.push(10.0);
    }
    right.push(30.0);
    let right_w = right.iter().sum::<f32>() + 6.0 * (right.len() as f32 - 1.0) + 4.0 + 4.0;
    (left_w, right_w, seg_size(ui, &mode_options(), false).x)
}

pub(crate) fn menu_label(ui: &mut Ui, text: &str) {
    ui.add_space(4.0);
    ui.label(RichText::new(text.to_uppercase()).size(10.5).color(P.muted));
}

/// a menu row: icon, title, small subtitle
pub(crate) fn menu_row(ui: &mut Ui, ic: Icon, title: &str, sub: &str, on: bool, enabled: bool) -> bool {
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(w, if sub.is_empty() { 30.0 } else { 40.0 }), if enabled { Sense::click() } else { Sense::hover() });
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, enabled, on, title));
    if on || (enabled && resp.hovered()) {
        ui.painter().rect_filled(rect, CornerRadius::same(7), P.panel2);
    }
    let fg = if enabled { P.text } else { P.muted };
    ui.painter().text(rect.left_center() + vec2(10.0, 0.0), Align2::LEFT_CENTER, icon(ic), FontId::new(15.0, icons()), fg);
    if sub.is_empty() {
        ui.painter().text(rect.left_center() + vec2(34.0, 0.0), Align2::LEFT_CENTER, title, FontId::proportional(13.0), fg);
    } else {
        ui.painter().text(pos2(rect.left() + 34.0, rect.top() + 7.0), Align2::LEFT_TOP, title, FontId::new(13.0, semibold()), fg);
        ui.painter().text(pos2(rect.left() + 34.0, rect.bottom() - 7.0), Align2::LEFT_BOTTOM, sub, FontId::proportional(11.0), P.muted);
    }
    enabled && resp.clicked()
}

impl CardBuilderApp {
    /// the format chip's two lines
    fn chip_lines(&self) -> (String, String) {
        (self.format_title().to_uppercase(), format!("{} · {}", self.doc.format.size_label(), self.doc.format.fold.label()).to_uppercase())
    }

    pub(crate) fn top_bar(&mut self, ui: &mut Ui, now: f64) {
        let mut full = ui.max_rect();
        let ctx = ui.ctx().clone();
        if crate::titlebar::ENABLED {
            let win = ctx.content_rect();
            let bar = Rect::from_min_max(win.left_top(), pos2(win.right(), win.top() + 51.0));
            crate::titlebar::drag_area(ui, bar);
            crate::titlebar::caption_buttons(ui, bar);
            full.max.x = bar.right() - crate::titlebar::WIDTH - 10.0;
        }
        let chip = self.chip_lines();
        let density = [Density::Full, Density::IconButtons, Density::Short, Density::Tight]
            .into_iter()
            .find(|&d| {
                let (l, r, s) = bar_widths(ui, d, &chip);
                l + r + s + 2.0 * GROUP_GAP <= full.width()
            })
            .unwrap_or(Density::Tight);
        self.top_density = density;

        // ---- left: logo · name · version · format
        let mut format_anchor = Rect::NOTHING;
        let left_end = ui
            .scope_builder(UiBuilder::new().max_rect(full).layout(Layout::left_to_right(Align::Center)), |ui| {
                ui.add(egui::Image::from_bytes("bytes://logo-mark.png", LOGO).fit_to_exact_size(vec2(28.0, 28.0)).corner_radius(7));
                if density < Density::Short {
                    ui.label(RichText::new("CardBuilder").family(semibold()).size(17.0));
                }
                let unseen = self.prefs.seen_version != VERSION;
                let galley = ui.painter().layout_no_wrap(format!("v{VERSION}"), FontId::proportional(11.0), P.muted);
                let (r, resp) = ui.allocate_exact_size(galley.size() + vec2(12.0, 4.0), Sense::click());
                resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, format!("Version {VERSION} — changelog")));
                let hovered = resp.hovered();
                ui.painter().rect(r, CornerRadius::same(5), if hovered { P.accent_weak } else { Color32::TRANSPARENT }, Stroke::new(1.0, if hovered { P.accent } else { P.border }), StrokeKind::Inside);
                ui.painter().galley(r.min + vec2(6.0, 2.0), galley, if hovered { P.text } else { P.muted });
                if unseen {
                    ui.painter().circle(r.right_top() + vec2(-1.0, 1.0), 3.5, P.accent, Stroke::new(2.0, P.panel));
                }
                if resp.on_hover_text(if unseen { "What's new — view changelog" } else { "View changelog" }).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                    self.popups.changelog = true;
                    self.prefs.seen_version = VERSION.to_string();
                }
                ui.add_space(10.0);
                let resp = if density < Density::Tight {
                    let title = ui.painter().layout_no_wrap(chip.0.clone(), FontId::new(11.0, semibold()), P.muted);
                    let sub = ui.painter().layout_no_wrap(chip.1.clone(), FontId::proportional(9.5), P.accent);
                    let w = title.size().x.max(sub.size().x) + 12.0;
                    let (r, resp) = ui.allocate_exact_size(vec2(w, 34.0), Sense::click());
                    if resp.hovered() || self.popups.format_top {
                        ui.painter().rect_filled(r, CornerRadius::same(8), P.panel2);
                    }
                    let (th, sh) = (title.size().y, sub.size().y);
                    let top = r.center().y - (th + sh) / 2.0;
                    ui.painter().galley(pos2(r.left() + 6.0, top), title, P.muted);
                    ui.painter().galley(pos2(r.left() + 6.0, top + th), sub, P.accent);
                    resp
                } else {
                    let (r, resp) = ui.allocate_exact_size(vec2(32.0, 34.0), Sense::click());
                    ui.painter().rect_filled(r, CornerRadius::same(8), if resp.hovered() { P.panel2 } else { Color32::TRANSPARENT });
                    ui.painter().text(r.center(), Align2::CENTER_CENTER, icon(crate::formats::category_icon(self.doc.format.category())), FontId::new(17.0, icons()), P.accent);
                    resp.on_hover_text(format!("{}\n{}", chip.0, chip.1))
                };
                format_anchor = resp.rect;
                resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, "Card format"));
                if resp.on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text("Choose the kind and size of card").clicked() {
                    self.popups.format_top = !self.popups.format_top;
                    self.popups.format_cat = None;
                    if self.popups.format_top {
                        self.rescan_templates();
                    }
                }
            })
            .response
            .rect
            .right();

        // ---- right: help · undo · redo · new · open · save | Theme · Print · Export ▾
        let mut export_anchor = Rect::NOTHING;
        let mut theme_anchor = Rect::NOTHING;
        let right_start = ui
            .scope_builder(UiBuilder::new().max_rect(full).layout(Layout::right_to_left(Align::Center)), |ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                let exp = button(ui, Some(Icon::Download), "Export", BtnKind::Primary, true).on_hover_text("Print files, pictures and the 3D snapshot");
                export_anchor = exp.rect;
                if exp.clicked() {
                    self.popups.export = !self.popups.export;
                }
                let compact = density >= Density::IconButtons;
                let labelled = |ui: &mut Ui, ic: Icon, name: &str| if compact { icon_button_framed(ui, ic, name, BtnKind::Normal, true) } else { button(ui, Some(ic), name, BtnKind::Normal, true) };
                if labelled(ui, Icon::Printer, "Print").on_hover_text("Print sheets: many cards per page with crop marks (Ctrl+P)").clicked() {
                    self.popups.print = true;
                }
                let th = labelled(ui, Icon::Palette, "Theme");
                theme_anchor = th.rect;
                if th.clicked() {
                    self.popups.theme = !self.popups.theme;
                }
                ui.add_space(4.0);
                let unsaved = self.has_unsaved_changes();
                let save = icon_button(ui, Icon::Save, "Save (Ctrl+S)", true);
                if unsaved {
                    ui.painter().circle(save.rect.right_top() + vec2(-6.0, 6.0), 3.5, P.accent, Stroke::new(2.0, P.panel));
                }
                let save_tip = match (unsaved, &self.file.path) {
                    (true, Some(_)) => format!("Save changes to {}", self.file.name()),
                    (false, Some(_)) => format!("All changes saved in {}", self.file.name()),
                    (_, None) => "Save this design as a .cb file".to_string(),
                };
                if save.on_hover_text(save_tip).clicked() {
                    self.save_design(now);
                }
                if icon_button(ui, Icon::FolderOpen, "Open design (Ctrl+O)", true).on_hover_text("Open a CardBuilder design (.cb)").clicked() {
                    self.request_open(now);
                }
                if icon_button(ui, Icon::FilePlus, "New design (Ctrl+N)", true).clicked() {
                    self.request_new(now);
                }
                ui.add_space(4.0);
                if icon_button(ui, Icon::Redo2, "Redo (Ctrl+Shift+Z)", self.history.can_redo()).clicked() {
                    self.redo();
                }
                if icon_button(ui, Icon::Undo2, "Undo (Ctrl+Z)", self.history.can_undo()).clicked() {
                    self.undo();
                }
                if density < Density::Tight {
                    ui.add(egui::Separator::default().vertical().spacing(10.0));
                }
                if icon_button(ui, Icon::CircleQuestionMark, "Shortcuts & help (?)", true).clicked() {
                    self.popups.help = true;
                }
            })
            .response
            .rect
            .left();

        // ---- centre: Card · Split · 3D
        let modes = mode_options();
        let size = seg_size(ui, &modes, false);
        let mut cx = full.center().x;
        if cx - size.x / 2.0 < left_end + GROUP_GAP || cx + size.x / 2.0 > right_start - GROUP_GAP {
            cx = (left_end + right_start) / 2.0;
        }
        let c = Rect::from_center_size(pos2(cx, full.center().y), size);
        ui.scope_builder(UiBuilder::new().max_rect(c).layout(Layout::left_to_right(Align::Center)), |ui| {
            seg(ui, &mut self.prefs.mode, &modes, false);
        });

        // ---- pop-ups
        let mut open = self.popups.format_top;
        popup(&ctx, "format-top", format_anchor, false, &mut open, 560.0, |ui| self.format_menu(ui, now));
        // the menu closes itself when a template is picked
        self.popups.format_top = open && self.popups.format_top;
        if !self.popups.format_top {
            self.popups.format_seen = false;
        }

        let mut open = self.popups.export;
        let mut chosen: Option<Export> = None;
        let mut save_as = false;
        let mut save_template = false;
        let mut print = false;
        let fmt = self.doc.format.clone();
        let records = self.doc.records();
        let has_data = !self.doc.data.is_empty();
        popup(&ctx, "export-menu", export_anchor, true, &mut open, 310.0, |ui| {
            menu_label(ui, "Print");
            if menu_row(ui, Icon::Printer, "Print sheets…", "PDF · many cards per page, crop marks, double-sided", false, true) {
                print = true;
            }
            let sub = if has_data { format!("PDF · one page per side · all {records} cards, with bleed") } else { "PDF · one page per side, with bleed — for print shops".into() };
            if menu_row(ui, Icon::FileText, "Card PDF", &sub, false, true) {
                chosen = Some(Export::CardPdf);
            }
            menu_label(ui, "Pictures");
            for side in [Side::Front, Side::Back] {
                let title = format!("{} · PNG", fmt.side_name(side));
                if menu_row(ui, Icon::FileImage, &title, "300 dpi, with bleed", false, true) {
                    chosen = Some(Export::Png { side });
                }
            }
            if menu_row(ui, Icon::Camera, "3D snapshot", "PNG, 2000 px, with the backdrop", false, self.viewer.gpu_ok) {
                chosen = Some(Export::Snapshot);
            }
            menu_label(ui, "Design file");
            if menu_row(ui, Icon::Save, "Save as…", ".cb — this app's design file (Ctrl+Shift+S)", false, true) {
                save_as = true;
            }
            // templates live in a folder on the computer, which the browser can't keep
            if cfg!(not(target_arch = "wasm32")) && menu_row(ui, Icon::LayoutTemplate, "Save as template…", "shows in the card picker, on this computer", false, true) {
                save_template = true;
            }
            if has_data && menu_row(ui, Icon::FileSpreadsheet, "Data as CSV", "the card data, to edit in a spreadsheet", false, true) {
                chosen = Some(Export::Csv);
            }
        });
        self.popups.export = open && chosen.is_none() && !save_as && !print && !save_template;
        if save_template {
            self.save_as_template(now);
        }
        if let Some(e) = chosen {
            self.export(&ctx, e, now);
        }
        if save_as {
            self.save_design_as(now);
        }
        if print {
            self.popups.print = true;
        }

        let mut open = self.popups.theme;
        popup(&ctx, "theme-menu", theme_anchor, true, &mut open, 540.0, |ui| self.theme_menu(ui));
        self.popups.theme = open;
    }

    /// changelog and help windows
    pub(crate) fn windows(&mut self, ctx: &egui::Context, _now: f64) {
        let mut open = self.popups.changelog;
        let releases = crate::changelog::CHANGELOG.len();
        let sub = format!("CardBuilder {VERSION} · {releases} release{}", if releases == 1 { "" } else { "s" });
        dialog(ctx, "changelog-dialog", "What's new", Some(&sub), 600.0, &mut open, |ui| {
            for (i, (version, date, title, items)) in crate::changelog::CHANGELOG.iter().enumerate() {
                if i > 0 {
                    ui.add_space(8.0);
                    ui.separator();
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("v{version}")).monospace().color(P.accent));
                    ui.label(RichText::new(*title).family(semibold()).size(14.0));
                    if i == 0 {
                        egui::Frame::NONE.fill(P.accent.gamma_multiply(0.18)).corner_radius(CornerRadius::same(6)).inner_margin(egui::Margin::symmetric(6, 1)).show(ui, |ui| {
                            ui.label(RichText::new("LATEST").size(9.5).family(semibold()).color(P.accent));
                        });
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| ui.label(RichText::new(*date).size(11.5).color(P.muted)));
                });
                ui.add_space(4.0);
                for (kind, text) in *items {
                    let (tag, color) = match *kind {
                        "new" => ("NEW", P.accent),
                        "improved" => ("IMPROVED", Color32::from_rgb(0x6f, 0xa8, 0xdc)),
                        "fixed" => ("FIXED", Color32::from_rgb(0xe0, 0xa8, 0x4a)),
                        other => (other, P.muted),
                    };
                    ui.horizontal_top(|ui| {
                        ui.allocate_ui_with_layout(vec2(64.0, 18.0), Layout::left_to_right(Align::Center), |ui| {
                            ui.set_min_width(64.0);
                            ui.label(RichText::new(tag.to_uppercase()).size(9.5).family(semibold()).color(color));
                        });
                        ui.add(egui::Label::new(*text).wrap());
                    });
                    ui.add_space(3.0);
                }
            }
        });
        self.popups.changelog = open;

        let mut open = self.popups.help;
        dialog(ctx, "help-dialog", "Shortcuts", None, 560.0, &mut open, |ui| {
            ui.label(
                RichText::new("Design both sides of the card in the editor; the 3D preview follows live. Designs are saved as .cb files — drop one onto the window (or double-click it in the file manager) to open it. Drop a picture to add it, or a spreadsheet (.csv) to make one card per row.")
                    .color(P.muted),
            );
            ui.add_space(6.0);
            egui::Grid::new("keys").num_columns(2).spacing([16.0, 6.0]).show(ui, |ui| {
                for (k, v) in [
                    ("V / H", "Select tool / Hand tool"),
                    ("T / I / R / Q", "Text / Image / Shape / QR code — then click the card"),
                    ("Drag", "Move the selected layer (snaps to centre lines and the safe area) · pan empty space"),
                    ("Corner / side / knob", "Resize (Shift: free) / change the width / rotate (Shift: 15° steps)"),
                    ("Alt + drag", "Move without snapping"),
                    ("Double-click text", "Edit its words"),
                    ("Tab", "Switch between the front and the back (outside / inside)"),
                    ("[ / ]", "Previous / next data row"),
                    ("Ctrl+C / Ctrl+V", "Copy / paste layers (also onto the other side); pasted text becomes a text layer"),
                    ("Ctrl+D", "Duplicate layer"),
                    ("Arrows", "Nudge 0.5 mm (Shift = 5 mm)"),
                    ("Del / Backspace", "Delete the selected layer"),
                    ("Wheel · middle-drag", "Zoom round the cursor · pan"),
                    ("F · double-click", "Fit the card to the view"),
                    ("Ctrl+Z / Ctrl+Shift+Z", "Undo / Redo"),
                    ("Ctrl+N / Ctrl+O", "New design / Open a design (.cb)"),
                    ("Ctrl+S / Ctrl+Shift+S", "Save / Save as… (.cb)"),
                    ("Ctrl+P", "Print sheets"),
                    ("1 / 2 / 3", "Card / Split / 3D layout"),
                    ("3D: drag / right-drag", "Turn / move the card · wheel zooms"),
                    ("Esc", "Deselect, close menus"),
                ] {
                    ui.label(RichText::new(k).monospace().color(P.text));
                    ui.add(egui::Label::new(v).wrap());
                    ui.end_row();
                }
            });
        });
        self.popups.help = open;
    }
}

/// A centred dialog over the dimmed app. It is never taller than the window — the body scrolls —
/// so its title and close button always stay on screen; Esc or a click outside also closes it.
pub(crate) fn dialog(ctx: &egui::Context, id: &str, title: &str, subtitle: Option<&str>, width: f32, open: &mut bool, body: impl FnOnce(&mut Ui)) {
    if !*open {
        return;
    }
    let screen = ctx.content_rect();
    let margin = 18.0;
    let w = width.min(screen.width() - 32.0 - 2.0 * margin).max(220.0);
    let max_h = (screen.height() - 80.0 - 2.0 * margin).clamp(140.0, 760.0);
    let frame = egui::Frame::NONE
        .shadow(egui::Shadow { offset: [0, 10], blur: 30, spread: 0, color: Color32::from_black_alpha(115) })
        .fill(P.menu)
        .stroke(Stroke::new(1.0, P.border))
        .corner_radius(CornerRadius::same(14))
        .inner_margin(egui::Margin::same(margin as i8));
    let resp = egui::Modal::new(egui::Id::unique(id)).frame(frame).backdrop_color(Color32::from_black_alpha(110)).show(ctx, |ui| {
        ui.set_width(w);
        let top = ui.cursor().top();
        let mut close = false;
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(RichText::new(title).family(semibold()).size(18.0).color(P.text));
                if let Some(s) = subtitle {
                    ui.label(RichText::new(s).size(11.5).color(P.muted));
                }
            });
            ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                close = icon_button(ui, Icon::X, "Close (Esc)", true).clicked();
            });
        });
        ui.add_space(6.0);
        ui.separator();
        let head = ui.cursor().top() - top;
        let list_h = (max_h - head).max(80.0);
        egui::ScrollArea::vertical().max_height(list_h).min_scrolled_height(list_h).auto_shrink([false, true]).show(ui, |ui| {
            ui.set_width(ui.available_width());
            body(ui);
            ui.add_space(4.0);
        });
        close
    });
    if resp.inner || resp.should_close() {
        *open = false;
    }
}
