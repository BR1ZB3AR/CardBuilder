//! App state, editing actions and the overall layout:
//! top bar · card editor | 3D preview · sidebar (Design / Data / Scene).

use crate::style::{self, P};
use cb_core::doc::{load_doc, save_doc, Doc, Layer, LayerType, QrSpec, ShapeKind, Side};
use cb_core::format::CardFormat;
use cb_core::history::History;
use egui::{Key, KeyboardShortcut, Modifiers};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

const DOC_KEY: &str = "cardbuilder.doc.v1";
const PREFS_KEY: &str = "cardbuilder.ui.v1";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// the design and preferences saved by the last session (or fresh ones)
pub fn restore_saved(doc_json: Option<&str>, prefs_json: Option<&str>) -> (Doc, Prefs) {
    let doc = doc_json.and_then(|j| load_doc(j).ok()).unwrap_or_default();
    let prefs: Prefs = prefs_json.and_then(|j| serde_json::from_str(j).ok()).unwrap_or_default();
    (doc, prefs)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    Card,
    Split,
    ThreeD,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tab {
    Design,
    Data,
    Scene,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Select,
    Hand,
    Text,
    Image,
    Shape,
    Qr,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View3d {
    Front,
    Angle,
    Back,
    Inside,
}

/// how lengths are shown (designs are always stored in mm)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Unit {
    Mm,
    In,
}
impl Unit {
    pub fn label(self) -> &'static str {
        match self {
            Unit::Mm => "mm",
            Unit::In => "in",
        }
    }
    /// mm per unit
    pub fn mm(self) -> f64 {
        match self {
            Unit::Mm => 1.0,
            Unit::In => 25.4,
        }
    }
    pub fn fmt(self, mm: f64) -> String {
        match self {
            Unit::Mm => format!("{} mm", cb_core::format::trim_num(mm)),
            Unit::In => format!("{:.3} in", mm / 25.4).replace(".000", ""),
        }
    }
}

/// per-device preferences (not part of the design)
#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Prefs {
    pub mode: Mode,
    pub tab: Tab,
    pub unit: Unit,
    /// bleed, trim and safe-area lines on the card
    pub guides: bool,
    /// snap artwork to centre lines and the safe area
    pub snap: bool,
    pub seen_version: String,
    pub theme: String,
    pub theme_system: bool,
    pub theme_light: String,
    pub theme_dark: String,
    pub accent: Option<String>,
    /// the .cb file being edited when the app last closed
    pub last_file: Option<String>,
}
impl Default for Prefs {
    fn default() -> Self {
        Prefs {
            mode: Mode::Split,
            tab: Tab::Design,
            unit: Unit::Mm,
            guides: true,
            snap: true,
            seen_version: String::new(),
            theme: "card-studio".into(),
            theme_system: false,
            theme_light: "daylight".into(),
            theme_dark: "card-studio".into(),
            accent: None,
            last_file: None,
        }
    }
}

#[derive(Default)]
pub struct Popups {
    pub format_top: bool,
    pub export: bool,
    pub theme: bool,
    pub changelog: bool,
    pub help: bool,
    pub print: bool,
    /// theme gallery filters: Some(true) = dark only, Some(false) = light only
    pub theme_mode: Option<bool>,
    /// 0 standard, 1 liquid glass, 2 cyberpunk, 3 high contrast
    pub theme_kind: Option<u8>,
    /// card category shown in the format picker
    pub format_cat: Option<cb_core::format::Category>,
    /// groups opened or closed in the format picker (by name)
    pub groups: std::collections::HashMap<String, bool>,
    /// a group just opened in the format picker: scrolled into view once its cards are shown
    pub scroll_group: Option<String>,
    /// the format picker has been on screen since it last opened (it scrolls to the current
    /// card when it opens)
    pub format_seen: bool,
}

pub struct CardBuilderApp {
    pub(crate) doc: Doc,
    pub(crate) history: History<Doc>,
    /// the printed side being edited
    pub(crate) side: Side,
    /// data row shown (batch cards)
    pub(crate) record: usize,
    pub(crate) prefs: Prefs,
    pub(crate) tool: Tool,
    pub(crate) selected: Option<String>,
    pub(crate) view3d: View3d,
    pub(crate) popups: Popups,
    pub(crate) status: Option<(String, f64)>,
    pub(crate) gpu: crate::gpu::GpuState,
    pub(crate) canvas: crate::canvas::CanvasView,
    /// background renderer for the editor and the 3D textures
    pub(crate) render: crate::worker::Renderer,
    pub(crate) viewer: crate::view3d::Viewer3d,
    pub(crate) jobs: crate::exports::Jobs,
    pending_files: Arc<Mutex<Vec<(String, Result<Vec<u8>, String>)>>>,
    applied_theme: Option<(String, Option<String>)>,
    pub(crate) top_density: crate::topbar::Density,
    pub(crate) file: crate::file::DocFile,
    /// copied layers (Ctrl+C / Ctrl+V)
    pub(crate) clipboard: Vec<Layer>,
    /// put the keyboard in the text box of the selected text layer (double-click on the card)
    pub(crate) focus_text: bool,
    /// the custom size being typed in the format picker
    pub(crate) custom: crate::formats::CustomDraft,
    /// Browsing formats: the format and artwork before the first switch, and the artwork the
    /// last switch produced. While nothing is edited in between, every switch starts from the
    /// original, so trying sizes and coming back gives the same layout (no compounding).
    pub(crate) format_origin: Option<(CardFormat, Vec<Layer>, Vec<Layer>)>,
    /// local templates (the card picker lists them)
    pub(crate) templates: Vec<crate::templates::LocalTemplate>,
    /// design fonts already given to egui (for the font menu's previews)
    pub(crate) egui_fonts: std::collections::HashSet<String>,
    /// card picker thumbnails
    pub(crate) thumbs: crate::thumbs::Thumbs,
}

impl CardBuilderApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        style::install_fonts(&cc.egui_ctx);
        egui_extras::install_image_loaders(&cc.egui_ctx);
        if let Ok(d) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
            cb_core::doc::seed_uids(d.as_millis() as u64);
        }
        let saved_doc = cc.storage.and_then(|s| s.get_string(DOC_KEY));
        let (doc, mut prefs) = restore_saved(saved_doc.as_deref(), cc.storage.and_then(|s| s.get_string(PREFS_KEY)).as_deref());
        let mut file = crate::file::DocFile::restore(prefs.last_file.as_deref(), &doc);
        if saved_doc.is_none() && file.path.is_none() {
            // a first start: the starter design is not an unsaved change
            file.baseline = doc.clone();
        }
        if file.path.is_none() {
            prefs.last_file = None;
        }
        let custom = crate::formats::CustomDraft::from(&doc.format);
        CardBuilderApp {
            doc,
            history: History::default(),
            side: Side::Front,
            record: 0,
            prefs,
            tool: Tool::Select,
            selected: None,
            view3d: View3d::Angle,
            popups: Popups::default(),
            status: None,
            gpu: crate::gpu::GpuState::new(cc.wgpu_render_state.as_ref()),
            canvas: crate::canvas::CanvasView::default(),
            render: crate::worker::Renderer::new(&cc.egui_ctx),
            viewer: crate::view3d::Viewer3d::new(cc.wgpu_render_state.clone()),
            jobs: Default::default(),
            pending_files: Default::default(),
            applied_theme: None,
            top_density: crate::topbar::Density::Full,
            file,
            clipboard: Vec::new(),
            focus_text: false,
            custom,
            format_origin: None,
            templates: crate::templates::scan(),
            egui_fonts: Default::default(),
            thumbs: Default::default(),
        }
    }

    // ------------------------------------------------------------------ editing

    /// apply an edit as one undo step (or merged with the last one for the same `key`)
    pub(crate) fn edit(&mut self, now: f64, key: Option<&str>, f: impl FnOnce(&mut Doc)) {
        let before = self.doc.clone();
        f(&mut self.doc);
        if self.doc != before {
            self.history.record(&before, key, now);
        }
    }

    pub(crate) fn say(&mut self, msg: impl Into<String>, now: f64) {
        self.status = Some((msg.into(), now));
    }

    pub(crate) fn undo(&mut self) {
        if let Some(d) = self.history.undo(&self.doc) {
            self.doc = d;
            self.fix_selection();
        }
    }
    pub(crate) fn redo(&mut self) {
        if let Some(d) = self.history.redo(&self.doc) {
            self.doc = d;
            self.fix_selection();
        }
    }
    pub(crate) fn fix_selection(&mut self) {
        if let Some(id) = &self.selected {
            if !self.doc.layers.iter().any(|l| &l.id == id) {
                self.selected = None;
            }
        }
        self.record = self.record.min(self.doc.data.rows.len().saturating_sub(1));
    }

    pub(crate) fn selected_layer(&self) -> Option<&Layer> {
        self.selected.as_ref().and_then(|id| self.doc.layer(id))
    }

    /// the data row shown, if there is data
    pub(crate) fn current_record(&self) -> Option<usize> {
        if self.doc.data.is_empty() {
            None
        } else {
            Some(self.record.min(self.doc.data.rows.len() - 1))
        }
    }

    /// where new artwork goes on the side being edited: the middle of the panel you see first
    /// (the front cover outside, the right / bottom panel inside)
    pub(crate) fn default_spot(&self) -> (f64, f64, cb_core::format::Panel) {
        let ps = self.doc.format.panels(self.side);
        let p = *ps.last().unwrap();
        (p.x + p.w / 2.0, p.y + p.h / 2.0, p)
    }

    /// the panel holding a design point on the current side
    pub(crate) fn panel_at(&self, x: f64, y: f64) -> cb_core::format::Panel {
        let ps = self.doc.format.panels(self.side);
        *ps.iter().find(|p| x >= p.x && x <= p.x + p.w && y >= p.y && y <= p.y + p.h).unwrap_or(ps.last().unwrap())
    }

    fn place(&mut self, now: f64, mut l: Layer) {
        let panel = self.panel_at(l.x, l.y);
        if panel.upside_down {
            l.rot = 180.0;
        }
        let id = l.id.clone();
        self.edit(now, None, |d| d.layers.push(l));
        self.selected = Some(id);
        self.tool = Tool::Select;
    }

    /// size scale for new artwork: 1 on a business card, more on bigger cards
    fn art_scale(p: &cb_core::format::Panel) -> f64 {
        (p.w.min(p.h) / 55.0).clamp(0.6, 4.0)
    }

    /// a dark or light ink that shows on the side's background
    pub(crate) fn contrast_ink(&self) -> &'static str {
        let bg = self.doc.background(self.side);
        let hex = match bg.mode {
            cb_core::doc::BgMode::Paper | cb_core::doc::BgMode::Image => self.doc.paper.color.as_str(),
            _ => bg.color.as_str(),
        };
        if cb_print::color::luminance(hex) < 0.5 {
            "#ffffff"
        } else {
            "#1f2347"
        }
    }

    pub(crate) fn add_text(&mut self, now: f64, at: Option<(f64, f64)>) {
        let (dx, dy, dp) = self.default_spot();
        let (x, y) = at.unwrap_or((dx, dy));
        let p = if at.is_some() { self.panel_at(x, y) } else { dp };
        let k = Self::art_scale(&p);
        let l = Layer::text(self.side, x, y, p.w * 0.7, "Your text", "Montserrat", 600.0, (12.0 * k).round(), self.contrast_ink(), cb_core::doc::Align::Center);
        self.place(now, l);
        self.focus_text = true;
    }

    pub(crate) fn add_shape(&mut self, now: f64, at: Option<(f64, f64)>, kind: ShapeKind) {
        let (dx, dy, dp) = self.default_spot();
        let (x, y) = at.unwrap_or((dx, dy));
        let p = if at.is_some() { self.panel_at(x, y) } else { dp };
        let k = Self::art_scale(&p);
        let mut l = Layer::rect(self.side, x, y, 20.0 * k, if kind == ShapeKind::Line { 2.0 } else { 12.0 * k }, "#ff7a59");
        l.shape = Some(kind);
        l.name = match kind {
            ShapeKind::Rect => "Rectangle",
            ShapeKind::Ellipse => "Ellipse",
            ShapeKind::Line => "Line",
        }
        .into();
        if kind == ShapeKind::Ellipse {
            l.h = l.w;
        }
        if kind == ShapeKind::Line {
            l.fill = None;
            l.stroke = Some(self.contrast_ink().into());
            l.stroke_w = Some(0.5);
        }
        self.place(now, l);
    }

    pub(crate) fn add_qr(&mut self, now: f64, at: Option<(f64, f64)>) {
        let (dx, dy, dp) = self.default_spot();
        let (x, y) = at.unwrap_or((dx, dy));
        let p = if at.is_some() { self.panel_at(x, y) } else { dp };
        let size = (p.w.min(p.h) * 0.4).clamp(14.0, 60.0);
        let ink = self.contrast_ink();
        let mut l = Layer::qr(self.side, x, y, size, QrSpec::url("https://example.com"), ink);
        if ink == "#ffffff" {
            // light codes on dark cards scan poorly: dark code on a white tile instead
            l.color = Some("#111111".into());
            l.fill = Some("#ffffff".into());
            l.radius = Some(1.0);
        }
        self.place(now, l);
    }

    pub(crate) fn remove_selected(&mut self, now: f64) {
        if let Some(id) = self.selected.take() {
            self.edit(now, None, |d| d.layers.retain(|l| l.id != id));
        }
    }

    /// copy a layer, nudged 2 mm, just above the original (Ctrl+D)
    pub(crate) fn duplicate_layer(&mut self, now: f64, id: &str) {
        let Some(i) = self.doc.layers.iter().position(|l| l.id == id) else { return };
        let mut copy = self.doc.layers[i].clone();
        copy.id = cb_core::doc::uid();
        copy.x += 2.0;
        copy.y += 2.0;
        let new_id = copy.id.clone();
        self.edit(now, None, |d| d.layers.insert(i + 1, copy));
        self.selected = Some(new_id);
    }

    /// move a layer up (+1) or down (−1) among the layers of its side
    pub(crate) fn move_layer(&mut self, now: f64, id: &str, dir: isize) {
        let id = id.to_string();
        self.edit(now, None, |d| {
            let Some(i) = d.layers.iter().position(|l| l.id == id) else { return };
            let side = d.layers[i].side;
            let mut j = i as isize;
            loop {
                j += dir;
                if j < 0 || j as usize >= d.layers.len() {
                    return;
                }
                if d.layers[j as usize].side == side {
                    break;
                }
            }
            let l = d.layers.remove(i);
            d.layers.insert(j as usize, l);
        });
    }

    /// move a layer to the other printed side, at the same place on the card
    pub(crate) fn move_to_other_side(&mut self, now: f64, id: &str) {
        let fmt = self.doc.format.clone();
        let id = id.to_string();
        let mut to = Side::Front;
        self.edit(now, None, |d| {
            if let Some(l) = d.layer_mut(&id) {
                // a point keeps its panel position when the side's sheet is turned over the other way
                to = l.side.other();
                l.side = to;
                let _ = &fmt;
            }
        });
        self.side = to;
        self.say(format!("Moved to the {}", self.doc.format.side_name(to).to_lowercase()), now);
    }

    /// switch to another card format, carrying the artwork over
    pub(crate) fn set_format(&mut self, now: f64, to: CardFormat) {
        if to == self.doc.format {
            return;
        }
        // start from the layout before browsing began, unless it has been edited since
        let (from, layers) = match &self.format_origin {
            Some((f0, l0, last)) if *last == self.doc.layers => (f0.clone(), l0.clone()),
            _ => (self.doc.format.clone(), self.doc.layers.clone()),
        };
        let to2 = to.clone();
        let remapped = if to2 == from { layers.clone() } else { cb_core::doc::remap_layers(&layers, &from, &to2) };
        self.format_origin = Some((from.clone(), layers, remapped.clone()));
        let from = self.doc.format.clone();
        self.edit(now, None, |d| {
            d.layers = remapped;
            if from.fold != to2.fold {
                d.scene.open = cb_core::doc::default_open(to2.fold);
            }
            d.format = to2;
        });
        self.custom = crate::formats::CustomDraft::from(&self.doc.format);
        self.canvas.request_fit();
        self.viewer.reset_view();
        let name = cb_core::format::preset(&to.preset).map(|p| p.name).unwrap_or("Custom size");
        self.say(format!("{name} · {} · {}", to.size_label(), to.fold.label()), now);
    }

    // ------------------------------------------------------------------ for tests and scripting

    /// open a design from its JSON text (as if the file had been opened)
    pub fn open_json(&mut self, name: &str, json: &str) {
        self.request_open_text(name.to_string(), json.to_string(), 0.0);
    }
    pub fn top_bar_density(&self) -> String {
        format!("{:?}", self.top_density)
    }
    pub fn doc_for_tests(&self) -> &Doc {
        &self.doc
    }
    pub fn selected_for_tests(&self) -> Option<&Layer> {
        self.selected_layer()
    }
    /// screen position of a design point (mm) in the card editor
    pub fn card_point_for_tests(&self, x: f64, y: f64) -> egui::Pos2 {
        self.canvas.to_screen(self.side, x, y)
    }
    /// switch the layout: "card", "split" or "3d"
    pub fn set_layout(&mut self, layout: &str) {
        self.prefs.mode = match layout {
            "card" => Mode::Card,
            "3d" => Mode::ThreeD,
            _ => Mode::Split,
        };
    }
    pub fn set_side_for_tests(&mut self, back: bool) {
        self.side = if back { Side::Back } else { Side::Front };
    }
    /// the last message shown at the bottom of the window
    pub fn status_for_tests(&self) -> Option<String> {
        self.status.as_ref().map(|s| s.0.clone())
    }
    pub fn editor_ready_for_tests(&self) -> bool {
        self.canvas.has_texture(self.side)
    }

    // ------------------------------------------------------------------ dropped files

    /// files dropped onto the window: a design opens, a spreadsheet becomes data, a picture artwork
    fn dropped_files(&mut self, ctx: &egui::Context, now: f64) {
        let files = ctx.input(|i| i.raw.dropped_files.clone());
        for f in files {
            let name = f.path().file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "file".into());
            if is_design_file(&name) && f.path().is_file() {
                self.request_open_path(f.path().to_path_buf(), now);
                continue;
            }
            let bytes = match f.bytes() {
                Ok(b) => Ok(b.to_vec()),
                Err(_) => std::fs::read(f.path()).map_err(|e| e.to_string()),
            };
            match bytes {
                Ok(b) => self.open_dropped(&name, b, now),
                Err(e) => self.say(format!("Couldn't read {name}: {e}"), now),
            }
        }
        let arrived: Vec<_> = self.pending_files.lock().unwrap().drain(..).collect();
        for (name, r) in arrived {
            match r {
                Ok(bytes) => self.open_dropped(&name, bytes, now),
                Err(e) => self.say(format!("Couldn't read {name}: {e}"), now),
            }
        }
    }

    fn open_dropped(&mut self, name: &str, bytes: Vec<u8>, now: f64) {
        let lower = name.to_lowercase();
        if is_design_file(name) {
            self.request_open_text(name.to_string(), String::from_utf8_lossy(&bytes).into_owned(), now);
        } else if lower.ends_with(".csv") || lower.ends_with(".tsv") || lower.ends_with(".txt") {
            self.import_data_text(name, &String::from_utf8_lossy(&bytes), now);
        } else {
            self.add_image_bytes(now, name, &bytes, None);
        }
    }

    // ------------------------------------------------------------------ keyboard & clipboard

    fn shortcuts(&mut self, ctx: &egui::Context, now: f64) {
        let sc = |m: Modifiers, k: Key| KeyboardShortcut::new(m, k);
        let (redo, redo2, undo, open, save_as, save, new, print) = ctx.input_mut(|i| {
            (
                i.consume_shortcut(&sc(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z)),
                i.consume_shortcut(&sc(Modifiers::COMMAND, Key::Y)),
                i.consume_shortcut(&sc(Modifiers::COMMAND, Key::Z)),
                i.consume_shortcut(&sc(Modifiers::COMMAND, Key::O)),
                i.consume_shortcut(&sc(Modifiers::COMMAND | Modifiers::SHIFT, Key::S)),
                i.consume_shortcut(&sc(Modifiers::COMMAND, Key::S)),
                i.consume_shortcut(&sc(Modifiers::COMMAND, Key::N)),
                i.consume_shortcut(&sc(Modifiers::COMMAND, Key::P)),
            )
        });
        let typing = ctx.egui_wants_keyboard_input();
        if !typing {
            if redo || redo2 {
                self.redo();
            } else if undo {
                self.undo();
            }
        }
        if open {
            self.request_open(now);
        }
        if save_as {
            self.save_design_as(now);
        } else if save {
            self.save_design(now);
        }
        if new {
            self.request_new(now);
        }
        if print {
            self.popups.print = true;
        }
        if typing {
            return;
        }
        let dup = ctx.input_mut(|i| i.consume_shortcut(&sc(Modifiers::COMMAND, Key::D)));
        if dup {
            if let Some(id) = self.selected.clone() {
                self.duplicate_layer(now, &id);
            }
        }
        // copy / paste: layers between sides and designs; plain text pastes as a text layer
        let events = ctx.input(|i| i.events.clone());
        for e in events {
            match e {
                egui::Event::Copy | egui::Event::Cut => {
                    if let Some(l) = self.selected_layer().cloned() {
                        ctx.copy_text(serde_json::to_string(&l).unwrap_or_default());
                        self.clipboard = vec![l];
                        if matches!(e, egui::Event::Cut) {
                            self.remove_selected(now);
                        }
                    }
                }
                egui::Event::Paste(text) => self.paste(&text, now),
                _ => {}
            }
        }
        let pressed = |k: Key| ctx.input(|i| i.key_pressed(k) && !i.modifiers.command);
        if pressed(Key::Delete) || pressed(Key::Backspace) {
            self.remove_selected(now);
        }
        if pressed(Key::Escape) {
            self.selected = None;
            self.tool = Tool::Select;
            self.popups = Popups { format_cat: self.popups.format_cat, groups: std::mem::take(&mut self.popups.groups), ..Popups::default() };
        }
        for (k, t) in [(Key::V, Tool::Select), (Key::H, Tool::Hand), (Key::T, Tool::Text), (Key::R, Tool::Shape), (Key::I, Tool::Image), (Key::Q, Tool::Qr)] {
            if pressed(k) {
                self.tool = t;
            }
        }
        if pressed(Key::F) {
            self.canvas.request_fit();
        }
        if pressed(Key::Tab) {
            self.side = self.side.other();
            self.selected = None;
        }
        for (k, m) in [(Key::Num1, Mode::Card), (Key::Num2, Mode::Split), (Key::Num3, Mode::ThreeD)] {
            if pressed(k) {
                self.prefs.mode = m;
            }
        }
        if !self.doc.data.is_empty() {
            let n = self.doc.data.rows.len();
            if pressed(Key::OpenBracket) {
                self.record = (self.record + n - 1) % n;
            }
            if pressed(Key::CloseBracket) {
                self.record = (self.record + 1) % n;
            }
        }
        if ctx.input(|i| i.key_pressed(Key::Questionmark)) {
            self.popups.help = true;
        }
        // arrow keys nudge the selected layer (0.5 mm, Shift = 5 mm)
        if let Some(id) = self.selected.clone() {
            let step = if ctx.input(|i| i.modifiers.shift) { 5.0 } else { 0.5 };
            let (dx, dy) = ctx.input(|i| {
                let mut d = (0.0, 0.0);
                if i.key_pressed(Key::ArrowLeft) {
                    d.0 -= step;
                }
                if i.key_pressed(Key::ArrowRight) {
                    d.0 += step;
                }
                if i.key_pressed(Key::ArrowUp) {
                    d.1 -= step;
                }
                if i.key_pressed(Key::ArrowDown) {
                    d.1 += step;
                }
                d
            });
            if dx != 0.0 || dy != 0.0 {
                let key = format!("nudge-{id}");
                self.edit(now, Some(&key), |d| {
                    if let Some(l) = d.layer_mut(&id) {
                        l.x += dx;
                        l.y += dy;
                    }
                });
            }
        }
    }

    fn paste(&mut self, text: &str, now: f64) {
        let layers: Vec<Layer> = match serde_json::from_str::<Layer>(text) {
            Ok(l) => vec![l],
            Err(_) if !self.clipboard.is_empty() && text.trim().is_empty() => self.clipboard.clone(),
            Err(_) => {
                let t = text.trim();
                if t.is_empty() {
                    return;
                }
                // plain text from elsewhere becomes a text layer
                self.add_text(now, None);
                let id = self.selected.clone().unwrap_or_default();
                let t = t.to_string();
                self.edit(now, None, |d| {
                    if let Some(l) = d.layer_mut(&id) {
                        l.name = t.lines().next().unwrap_or("Text").chars().take(40).collect();
                        l.text = Some(t);
                    }
                });
                self.focus_text = false;
                return;
            }
        };
        let side = self.side;
        let mut last = None;
        for mut l in layers {
            let same_side = l.side == side;
            l.id = cb_core::doc::uid();
            l.side = side;
            if same_side {
                l.x += 2.0;
                l.y += 2.0;
            }
            last = Some(l.id.clone());
            self.edit(now, None, |d| d.layers.push(l));
        }
        self.selected = last;
    }
}

impl CardBuilderApp {
    /// the theme in force: the chosen one, or the light / dark slot when following the system
    pub(crate) fn active_theme_id(&self, ctx: &egui::Context) -> String {
        if self.prefs.theme_system {
            let dark = ctx.system_theme().map(|t| t == egui::Theme::Dark).unwrap_or(true);
            if dark {
                self.prefs.theme_dark.clone()
            } else {
                self.prefs.theme_light.clone()
            }
        } else {
            self.prefs.theme.clone()
        }
    }

    fn apply_theme(&mut self, ctx: &egui::Context) {
        let id = self.active_theme_id(ctx);
        let th = crate::themes::theme(&id);
        let accent = self.prefs.accent.clone().filter(|_| !th.hc && th.fx == style::Fx::None);
        let key = (id, accent);
        if self.applied_theme.as_ref() != Some(&key) {
            style::apply(ctx, &crate::themes::palette(th, key.1.as_deref()));
            self.applied_theme = Some(key);
        }
    }

    /// Fonts the design carries: available for drawing and measuring text, and in egui for the
    /// font menu's previews.
    fn sync_design_fonts(&mut self, ctx: &egui::Context) {
        cb_print::text::register_fonts(&self.doc.fonts);
        for f in &self.doc.fonts {
            if self.egui_fonts.contains(&f.family) || cb_print::text::is_bundled(&f.family) {
                continue;
            }
            let Some(bytes) = cb_print::text::custom_font(&f.family) else { continue };
            ctx.add_font(egui::epaint::text::FontInsert::new(
                &format!("design-{}", f.family),
                egui::FontData::from_static(bytes),
                vec![egui::epaint::text::InsertFontFamily { family: egui::FontFamily::Name(f.family.clone().into()), priority: egui::epaint::text::FontPriority::Highest }],
            ));
            self.egui_fonts.insert(f.family.clone());
        }
    }

    /// a font egui can show its name in (bundled, or a design font already given to egui)
    pub(crate) fn previewable_font(&self, family: &str) -> bool {
        cb_print::text::is_bundled(family) || self.egui_fonts.contains(family)
    }

    /// a message at the bottom of the window for a few seconds (and export progress)
    fn toast(&self, ctx: &egui::Context, now: f64) {
        let progress = self.jobs.progress();
        let text = match (&progress, &self.status) {
            (Some((what, p)), _) => format!("{what} {:.0}%", p * 100.0),
            (None, Some((m, t))) if now - t < 4.0 => m.clone(),
            _ => return,
        };
        let screen = ctx.content_rect();
        egui::Area::new(egui::Id::unique("toast")).order(egui::Order::Tooltip).interactable(false).fixed_pos(egui::pos2(screen.center().x, screen.bottom() - 70.0)).pivot(egui::Align2::CENTER_BOTTOM).show(ctx, |ui| {
            egui::Frame::NONE
                .fill(P.menu)
                .stroke(egui::Stroke::new(1.0, P.border))
                .corner_radius(egui::CornerRadius::same(18))
                .inner_margin(egui::Margin::symmetric(16, 9))
                .shadow(egui::Shadow { offset: [0, 6], blur: 18, spread: 0, color: egui::Color32::from_black_alpha(90) })
                .show(ui, |ui| {
                    ui.set_max_width(screen.width() * 0.7);
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(style::icon(if progress.is_some() { lucide_icons::Icon::LoaderCircle } else { lucide_icons::Icon::Info })).family(style::icons()).color(P.accent));
                        ui.add(egui::Label::new(egui::RichText::new(text).color(P.text)).wrap());
                    });
                    if let Some((_, p)) = progress {
                        ui.add(egui::ProgressBar::new(p).desired_height(4.0));
                    }
                });
        });
        ctx.request_repaint_after(std::time::Duration::from_millis(200));
    }
}

impl eframe::App for CardBuilderApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let now = ctx.input(|i| i.time);
        self.shortcuts(&ctx, now);
        self.dropped_files(&ctx, now);
        self.fix_selection();
        self.poll_exports(now);
        self.apply_theme(&ctx);
        self.sync_design_fonts(&ctx);
        // background rendering: the editor's card image and the 3D textures
        self.render.sync(&self.doc, self.current_record());
        if let Some(tex) = self.render.take_textures() {
            self.viewer.set_textures(tex);
        }
        let pal = *P;
        let th = crate::themes::theme(&self.applied_theme.as_ref().map(|a| a.0.clone()).unwrap_or_default());
        if pal.fx != style::Fx::None {
            crate::themes::paint_app_background(&ctx.layer_painter(egui::LayerId::background()), ctx.content_rect(), th, &pal);
        }

        egui::Panel::top("topbar")
            .exact_size(52.0)
            .frame(egui::Frame::NONE.fill(pal.panel).inner_margin(egui::Margin::symmetric(12, 0)).stroke(egui::Stroke::new(1.0, pal.border)))
            .show(ui, |ui| self.top_bar(ui, now));
        egui::Panel::right("sidebar")
            .exact_size(320.0)
            .resizable(false)
            .frame(egui::Frame::NONE.fill(pal.panel).stroke(egui::Stroke::new(1.0, pal.border)))
            .show(ui, |ui| self.sidebar(ui, now));
        egui::CentralPanel::default().frame(egui::Frame::NONE.fill(if pal.fx == style::Fx::None { pal.bg } else { egui::Color32::TRANSPARENT })).show(ui, |ui| {
            let full = ui.max_rect();
            match self.prefs.mode {
                Mode::Card => self.card_pane(ui, full, now),
                Mode::ThreeD => self.viewer_pane(ui, full, now),
                Mode::Split => {
                    let mid = (full.left() + full.width() * 0.56).round();
                    let left = egui::Rect::from_min_max(full.min, egui::pos2(mid, full.max.y));
                    let right = egui::Rect::from_min_max(egui::pos2(mid, full.min.y), full.max);
                    self.card_pane(ui, left, now);
                    self.viewer_pane(ui, right, now);
                    ui.painter().vline(mid, full.y_range(), egui::Stroke::new(1.0, pal.border));
                }
            }
        });
        self.windows(&ctx, now);
        self.print_dialog(&ctx, now);
        self.save_changes_dialog(&ctx, now);
        self.update_window_title(&ctx);
        self.toast(&ctx, now);
        if crate::titlebar::ENABLED {
            crate::titlebar::window_outline(&ctx);
            crate::titlebar::resize_zones(ui);
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string(DOC_KEY, save_doc(&self.doc));
        if let Ok(j) = serde_json::to_string(&self.prefs) {
            storage.set_string(PREFS_KEY, j);
        }
    }
}

/// a design file CardBuilder opens
pub(crate) fn is_design_file(name: &str) -> bool {
    let n = name.to_lowercase();
    n.ends_with(&format!(".{}", cb_core::doc::CB_EXT)) || n.ends_with(".json")
}

/// what kind of layer, for lists and headings
pub(crate) fn kind_label(l: &Layer) -> &'static str {
    match l.kind {
        LayerType::Text => "Text",
        LayerType::Image => "Image",
        LayerType::Shape => match l.shape.unwrap_or_default() {
            ShapeKind::Rect => "Rectangle",
            ShapeKind::Ellipse => "Ellipse",
            ShapeKind::Line => "Line",
        },
        LayerType::Qr => "QR code",
    }
}
