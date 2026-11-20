//! Exports (print sheets, card PDF, PNG pictures, 3D snapshot, data as CSV), the Print dialog,
//! and bringing in pictures and spreadsheets. A save dialog first, then the file is made on a
//! background thread while a toast shows the progress. The web version makes the file at once
//! and hands it to the browser as a download.

use crate::app::{CardBuilderApp, Tool};
use crate::style::{semibold, P};
use crate::widgets::{button, hint, slider_row, toggle_row, BtnKind};
use base64::Engine as _;
use cb_core::doc::{Doc, Layer, LayerType, Side};
use cb_core::format::{sheet_size, SHEETS};
use cb_print::ImageCache;
use egui::{pos2, vec2, Color32, CornerRadius, Layout, Rect, RichText, Sense, Stroke, StrokeKind};
use crate::pick::PickFor;
use lucide_icons::Icon;
#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Export {
    PrintSheets,
    CardPdf,
    Png { side: Side },
    Snapshot,
    Csv,
}

/// background exports: finished messages and the running one's progress
#[derive(Default)]
pub struct Jobs {
    done: Arc<Mutex<Vec<String>>>,
    progress: Arc<Mutex<Option<(String, f32)>>>,
}

impl Jobs {
    pub fn progress(&self) -> Option<(String, f32)> {
        self.progress.lock().unwrap().clone()
    }
    pub fn busy(&self) -> bool {
        self.progress.lock().unwrap().is_some()
    }
}

/// a picture file → (data URL, width px, height px); big rasters are scaled down to 3600 px
pub fn image_data_url(name: &str, bytes: &[u8]) -> Result<(String, f64, f64), String> {
    let lower = name.to_lowercase();
    let svg = lower.ends_with(".svg") || bytes.iter().take(256).collect::<Vec<_>>().windows(4).any(|w| w == [&b'<', &b's', &b'v', &b'g']);
    let b64 = |b: &[u8]| base64::engine::general_purpose::STANDARD.encode(b);
    if svg {
        let d = cb_print::images::decode_bytes("image/svg+xml", bytes)?;
        let (w, h) = d.size();
        return Ok((format!("data:image/svg+xml;base64,{}", b64(bytes)), w, h));
    }
    let img = image::load_from_memory(bytes).map_err(|e| format!("not a picture CardBuilder can read ({e})"))?;
    let (w, h) = (img.width() as f64, img.height() as f64);
    const MAX: f64 = 3600.0;
    let jpeg = lower.ends_with(".jpg") || lower.ends_with(".jpeg");
    let mime = if lower.ends_with(".png") {
        "image/png"
    } else if lower.ends_with(".webp") {
        "image/webp"
    } else if lower.ends_with(".gif") {
        "image/gif"
    } else if jpeg {
        "image/jpeg"
    } else {
        "image/png"
    };
    if w.max(h) <= MAX {
        return Ok((format!("data:{mime};base64,{}", b64(bytes)), w, h));
    }
    let k = MAX / w.max(h);
    let small = img.resize((w * k).round() as u32, (h * k).round() as u32, image::imageops::FilterType::Lanczos3);
    let mut out = std::io::Cursor::new(Vec::new());
    let fmt = if jpeg { image::ImageFormat::Jpeg } else { image::ImageFormat::Png };
    let small = if jpeg { image::DynamicImage::ImageRgb8(small.to_rgb8()) } else { small };
    small.write_to(&mut out, fmt).map_err(|e| e.to_string())?;
    Ok((format!("data:{};base64,{}", if jpeg { "image/jpeg" } else { "image/png" }, b64(&out.into_inner())), small.width() as f64, small.height() as f64))
}

/// offer `bytes` to the user in a save dialog (the web version downloads them); Some(where)
/// when saved
pub fn save_bytes(name: &str, bytes: &[u8], mime: &str) -> Result<Option<String>, String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = mime;
        let ext = name.rsplit('.').next().unwrap_or("");
        let Some(path) = rfd::FileDialog::new().add_filter(ext.to_uppercase(), &[ext]).set_file_name(name).set_title("Save").save_file() else {
            return Ok(None);
        };
        std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
        Ok(Some(path.display().to_string()))
    }
    #[cfg(target_arch = "wasm32")]
    {
        web_download(name, bytes, mime)?;
        Ok(Some(name.to_string()))
    }
}

/// the browser saves `bytes` as a download named `name`
#[cfg(target_arch = "wasm32")]
fn web_download(name: &str, bytes: &[u8], mime: &str) -> Result<(), String> {
    use eframe::wasm_bindgen::JsCast as _;
    use eframe::web_sys;
    let parts = js_sys::Array::of1(&js_sys::Uint8Array::from(bytes));
    let opts = web_sys::BlobPropertyBag::new();
    opts.set_type(mime);
    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &opts).map_err(|e| format!("{e:?}"))?;
    let url = web_sys::Url::create_object_url_with_blob(&blob).map_err(|e| format!("{e:?}"))?;
    let doc = web_sys::window().and_then(|w| w.document()).ok_or("no document")?;
    let a = doc.create_element("a").map_err(|e| format!("{e:?}"))?.dyn_into::<web_sys::HtmlAnchorElement>().map_err(|_| "no link element")?;
    a.set_href(&url);
    a.set_download(name);
    a.click();
    let _ = web_sys::Url::revoke_object_url(&url);
    Ok(())
}

/// the media type of an exported file, from its name
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
fn mime_of(name: &str) -> &'static str {
    match name.rsplit('.').next().unwrap_or("").to_ascii_lowercase().as_str() {
        "pdf" => "application/pdf",
        "png" => "image/png",
        "csv" => "text/csv",
        _ => "application/octet-stream",
    }
}

const PICTURES: [&str; 7] = ["png", "jpg", "jpeg", "webp", "gif", "bmp", "svg"];

impl CardBuilderApp {
    /// the start of exported file names: the design file's name, else the kind of card
    fn export_stem(&self) -> String {
        match &self.file.path {
            Some(p) => p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "card".into()),
            None => format!("cardbuilder-{}", self.doc.format.category().noun().replace(' ', "-")),
        }
    }

    /// ask where to save (on this thread), then make the file in the background
    fn run_export(&mut self, ctx: &egui::Context, now: f64, name: String, what: &str, make: impl FnOnce(&Doc, &ImageCache, &dyn Fn(f32)) -> Result<(Vec<u8>, String), String> + Send + 'static) {
        if self.jobs.busy() {
            self.say("Another export is still running", now);
            return;
        }
        #[cfg(target_arch = "wasm32")]
        {
            // no threads in the browser: make it now, then download it
            let _ = (ctx, what);
            let mut imgs = ImageCache::default();
            imgs.prepare(cb_print::image_srcs(&self.doc));
            let msg = match make(&self.doc, &imgs, &|_| {}).and_then(|(bytes, note)| save_bytes(&name, &bytes, mime_of(&name)).map(|_| note)) {
                Ok(note) => format!("Downloaded {name}{note}"),
                Err(e) => format!("Export failed: {e}"),
            };
            self.say(msg, now);
        }
        #[cfg(not(target_arch = "wasm32"))]
        self.export_in_background(ctx, name, what, make);
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn export_in_background(&mut self, ctx: &egui::Context, name: String, what: &str, make: impl FnOnce(&Doc, &ImageCache, &dyn Fn(f32)) -> Result<(Vec<u8>, String), String> + Send + 'static) {
        let ext = name.rsplit('.').next().unwrap_or("").to_string();
        // automation: CB_EXPORT_DIR saves straight into a folder, no dialog
        let path = match std::env::var("CB_EXPORT_DIR") {
            Ok(dir) => Some(PathBuf::from(dir).join(&name)),
            Err(_) => rfd::FileDialog::new().add_filter(ext.to_uppercase(), &[ext.as_str()]).set_file_name(&name).set_title(what).save_file(),
        };
        let Some(path) = path else { return };
        let doc = self.doc.clone();
        let done = self.jobs.done.clone();
        let progress = self.jobs.progress.clone();
        let label = format!("{what}…");
        *progress.lock().unwrap() = Some((label.clone(), 0.0));
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let mut imgs = ImageCache::default();
            imgs.prepare(cb_print::image_srcs(&doc));
            let p2 = progress.clone();
            let c2 = ctx.clone();
            let report = move |f: f32| {
                *p2.lock().unwrap() = Some((label.clone(), f.clamp(0.0, 1.0)));
                c2.request_repaint();
            };
            let msg = match make(&doc, &imgs, &report).and_then(|(bytes, note)| std::fs::write(&path, bytes).map(|_| note).map_err(|e| e.to_string())) {
                Ok(note) => format!("Saved {}{note}", path.display()),
                Err(e) => format!("Export failed: {e}"),
            };
            *progress.lock().unwrap() = None;
            done.lock().unwrap().push(msg);
            ctx.request_repaint();
        });
    }

    pub(crate) fn export(&mut self, ctx: &egui::Context, e: Export, now: f64) {
        let stem = self.export_stem();
        let record = self.current_record();
        match e {
            Export::Snapshot => {
                self.viewer.request_snapshot(&self.doc);
                if self.viewer.snapshot_pending {
                    self.say("Rendering 3D snapshot…", now);
                } else {
                    self.say("The 3D snapshot needs the graphics card renderer", now);
                }
            }
            Export::Png { side } => {
                let name = format!("{stem}-{}.png", self.doc.format.side_name(side).to_lowercase());
                self.run_export(ctx, now, name, "Export picture", move |doc, imgs, _| Ok((cb_print::sheets::side_png(doc, side, record, 300.0, true, imgs)?, " · 300 dpi with bleed".into())));
            }
            Export::CardPdf => {
                let name = format!("{stem}-cards.pdf");
                self.run_export(ctx, now, name, "Export card PDF", |doc, imgs, progress| {
                    let records: Vec<Option<usize>> = if doc.data.is_empty() { vec![None] } else { (0..doc.data.rows.len()).map(Some).collect() };
                    let pdf = cb_print::sheets::cards_pdf(doc, &records, imgs, progress)?;
                    Ok((pdf, format!(" · {} card{}", records.len(), if records.len() == 1 { "" } else { "s" })))
                });
            }
            Export::PrintSheets => {
                let name = format!("{stem}-print-sheets.pdf");
                self.run_export(ctx, now, name, "Export print sheets", |doc, imgs, progress| {
                    let (pdf, s) = cb_print::sheets::print_sheets_pdf(doc, imgs, progress)?;
                    Ok((pdf, format!(" · {} card{} on {} sheet{} ({} per sheet, {} pages)", s.cards, if s.cards == 1 { "" } else { "s" }, s.sheets, if s.sheets == 1 { "" } else { "s" }, s.per_sheet, s.pages)))
                });
            }
            Export::Csv => {
                let name = format!("{stem}-data.csv");
                self.run_export(ctx, now, name, "Export data", |doc, _, _| Ok((cb_core::data::to_csv(&doc.data).into_bytes(), String::new())));
            }
        }
    }

    /// finished background exports and 3D snapshots
    pub(crate) fn poll_exports(&mut self, now: f64) {
        let msgs: Vec<String> = self.jobs.done.lock().unwrap().drain(..).collect();
        for m in msgs {
            self.say(m, now);
        }
        if let Some(r) = self.viewer.take_snapshot(&self.doc) {
            let name = format!("{}-3d.png", self.export_stem());
            if let Ok(dir) = std::env::var("CB_EXPORT_DIR") {
                let path = std::path::Path::new(&dir).join(&name);
                let msg = match r.and_then(|png| std::fs::write(&path, png).map_err(|e| e.to_string())) {
                    Ok(()) => format!("Saved 3D snapshot {}", path.display()),
                    Err(e) => format!("3D snapshot failed: {e}"),
                };
                self.say(msg, now);
                return;
            }
            match r.and_then(|png| save_bytes(&name, &png, "image/png")) {
                Ok(Some(p)) => self.say(format!("Saved 3D snapshot {p}"), now),
                Ok(None) => {}
                Err(e) => self.say(format!("3D snapshot failed: {e}"), now),
            }
        }
    }

    // ------------------------------------------------------------------ pictures

    /// add a picture as artwork (at `at`, else in the middle of the panel)
    pub(crate) fn add_image_bytes(&mut self, now: f64, name: &str, bytes: &[u8], at: Option<(f64, f64)>) {
        let (src, iw, ih) = match image_data_url(name, bytes) {
            Ok(v) => v,
            Err(e) => {
                self.say(format!("Could not read that image: {e}"), now);
                return;
            }
        };
        let (dx, dy, dp) = self.default_spot();
        let (x, y) = at.unwrap_or((dx, dy));
        let p = if at.is_some() { self.panel_at(x, y) } else { dp };
        let aspect = ih / iw.max(1.0);
        let mut w = p.w * 0.5;
        if w * aspect > p.h * 0.7 {
            w = p.h * 0.7 / aspect;
        }
        let stem = name.rsplit_once('.').map(|(s, _)| s).unwrap_or(name);
        let mut l = Layer::new(if stem.is_empty() { "Image" } else { stem }, LayerType::Image, self.side, x, y, w, w * aspect);
        l.src = Some(src);
        if p.upside_down {
            l.rot = 180.0;
        }
        let id = l.id.clone();
        self.edit(now, None, |d| d.layers.push(l));
        self.selected = Some(id);
        self.tool = Tool::Select;
    }

    /// pick a picture and add it
    pub(crate) fn pick_image(&mut self, now: f64, at: Option<(f64, f64)>) {
        if let Some((name, b)) = self.pick_file(PickFor::Image(at), "Add image", "Pictures", &PICTURES, now) {
            self.add_image_bytes(now, &name, &b, at);
        }
        self.tool = Tool::Select;
    }

    /// a picture for the background of `side`: its data URL (the web version puts it in place
    /// itself once the browser has the file)
    pub(crate) fn choose_picture(&mut self, now: f64, side: Side) -> Option<String> {
        let (name, b) = self.pick_file(PickFor::Background(side), "Background picture", "Pictures", &PICTURES, now)?;
        match image_data_url(&name, &b) {
            Ok((src, _, _)) => Some(src),
            Err(e) => {
                self.say(format!("Could not read that image: {e}"), now);
                None
            }
        }
    }

    /// replace the picture of image layer `id`, keeping its width
    pub(crate) fn replace_image(&mut self, now: f64, id: &str) {
        if let Some((name, b)) = self.pick_file(PickFor::Replace(id.to_string()), "Replace image", "Pictures", &PICTURES, now) {
            self.replace_image_bytes(now, id, &name, &b);
        }
    }

    pub(crate) fn replace_image_bytes(&mut self, now: f64, id: &str, name: &str, bytes: &[u8]) {
        match image_data_url(name, bytes) {
            Ok((src, iw, ih)) => {
                let id = id.to_string();
                self.edit(now, None, |d| {
                    if let Some(l) = d.layer_mut(&id) {
                        l.h = l.w * ih / iw.max(1.0);
                        l.src = Some(src);
                    }
                });
            }
            Err(e) => self.say(format!("Could not read that image: {e}"), now),
        }
    }

    // ------------------------------------------------------------------ data

    pub(crate) fn import_data(&mut self, now: f64) {
        if let Some((name, b)) = self.pick_file(PickFor::Data, "Import data", "Spreadsheet (CSV)", &["csv", "tsv", "txt"], now) {
            self.import_data_text(&name, &String::from_utf8_lossy(&b), now);
        }
    }

    pub(crate) fn import_data_text(&mut self, name: &str, text: &str, now: f64) {
        match cb_core::data::parse_csv(text) {
            Ok(mut d) => {
                d.name = name.to_string();
                let (rows, cols) = (d.rows.len(), d.columns.clone());
                self.edit(now, None, |doc| doc.data = d);
                self.record = 0;
                self.prefs.tab = crate::app::Tab::Data;
                let example = cols.first().map(|c| format!("{{{c}}}")).unwrap_or_default();
                self.say(format!("{rows} rows from {name} — put {example} in a text box to use them"), now);
            }
            Err(e) => self.say(format!("Couldn't read {name}: {e}"), now),
        }
    }

    // ------------------------------------------------------------------ the Print dialog

    pub(crate) fn print_dialog(&mut self, ctx: &egui::Context, now: f64) {
        if !self.popups.print {
            return;
        }
        let mut open = true;
        let mut ps = self.doc.print.clone();
        let mut save = false;
        let layout = cb_print::sheets::sheet_layout(&self.doc);
        let per = layout.per_sheet();
        let jobs = cb_print::sheets::print_jobs(&self.doc, per);
        let sheets = if per == 0 { 0 } else { jobs.len().div_ceil(per) };
        let fmt = self.doc.format.clone();
        let tex = self.canvas.texture_for(Side::Front);
        let has_data = !self.doc.data.is_empty();
        let sub = format!("{} · {} · {}", self.format_title(), fmt.size_label(), fmt.fold.label());
        crate::topbar::dialog(ctx, "print-dialog", "Print sheets", Some(&sub), 800.0, &mut open, |ui| {
            let mut gap_x = 0.0;
            let row = ui.horizontal_top(|ui| {
                // ---- settings
                ui.vertical(|ui| {
                    ui.set_width(320.0);
                    ui.horizontal(|ui| {
                        let lbl = ui.label("Paper");
                        ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                            let cur = sheet_size(&ps.sheet);
                            egui::ComboBox::new("sheet", "")
                                .selected_text(format!("{} · {} × {} mm", cur.name, cb_core::format::trim_num(cur.w), cb_core::format::trim_num(cur.h)))
                                .width(210.0)
                                .show_ui(ui, |ui| {
                                    for s in SHEETS {
                                        ui.selectable_value(&mut ps.sheet, s.id.to_string(), format!("{} · {} × {} mm", s.name, cb_core::format::trim_num(s.w), cb_core::format::trim_num(s.h)));
                                    }
                                })
                                .response
                                .labelled_by(lbl.id);
                        });
                    });
                    slider_row(ui, "Margins", &mut ps.margin, 0.0..=25.0, 0.5, |v| format!("{v:.1} mm"), true);
                    slider_row(ui, "Gap", &mut ps.gap, 0.0..=10.0, 0.5, |v| format!("{v:.1} mm"), true);
                    toggle_row(ui, "Bleed round each card", &mut ps.bleed);
                    toggle_row(ui, "Crop marks", &mut ps.crop_marks);
                    toggle_row(ui, "Turn cards to fit more", &mut ps.allow_rotate);
                    toggle_row(ui, "Double-sided (backs on the reverse)", &mut ps.duplex);
                    if ps.duplex {
                        ui.horizontal(|ui| {
                            ui.label("Printer turns the sheet on its");
                            crate::widgets::seg(ui, &mut ps.short_edge, &[(false, "Long edge"), (true, "Short edge")], true);
                        });
                        hint(ui, "Match your printer's duplex setting (most use the long edge). For a print shop, either works.");
                    }
                    ui.add_space(4.0);
                    if has_data {
                        ui.label(RichText::new(format!("All {} data rows, one card each", self.doc.data.rows.len())).color(P.text));
                    } else {
                        ui.horizontal(|ui| {
                            let lbl = ui.label("Copies");
                            ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                                if ps.copies == 0 {
                                    ui.label(RichText::new("a full sheet").size(11.5).color(P.muted));
                                }
                                ui.add(egui::DragValue::new(&mut ps.copies).range(0..=5000).speed(0.2)).labelled_by(lbl.id).on_hover_text("0 fills one sheet");
                            });
                        });
                    }
                    ui.horizontal(|ui| {
                        ui.label("Pictures at");
                        crate::widgets::seg(ui, &mut ps.dpi, &[(300, "300 dpi"), (600, "600 dpi")], true);
                    });
                    hint(ui, "Text, shapes and QR codes are sharp vectors at any resolution.");
                    ui.add_space(14.0);
                    let summary = if per == 0 {
                        format!("Doesn't fit on {} — choose a bigger sheet or smaller margins", sheet_size(&ps.sheet).name)
                    } else {
                        let pages = sheets * if ps.duplex { 2 } else { 1 };
                        format!("{per} per sheet · {} card{} · {sheets} sheet{} · {pages} page{}", jobs.len(), if jobs.len() == 1 { "" } else { "s" }, if sheets == 1 { "" } else { "s" }, if pages == 1 { "" } else { "s" })
                    };
                    ui.add(egui::Label::new(RichText::new(summary).family(semibold()).color(if per == 0 { Color32::from_rgb(0xe0, 0xa8, 0x4a) } else { P.text })).wrap());
                    ui.add_space(6.0);
                    if button(ui, Some(Icon::FileDown), "Save PDF…", BtnKind::Primary, per > 0).clicked() {
                        save = true;
                    }
                });
                ui.add_space(8.0);
                gap_x = ui.cursor().left();
                ui.add_space(8.0);
                // ---- the sheet, to scale
                ui.vertical(|ui| {
                    let avail = vec2(ui.available_width().max(260.0), 460.0);
                    let (area, _) = ui.allocate_exact_size(avail, Sense::hover());
                    let k = ((area.width() - 20.0) / layout.sheet_w as f32).min((area.height() - 20.0) / layout.sheet_h as f32);
                    let size = vec2(layout.sheet_w as f32, layout.sheet_h as f32) * k;
                    let sheet = Rect::from_center_size(area.center(), size);
                    let p = ui.painter_at(area);
                    p.add(egui::epaint::RectShape::filled(sheet.translate(vec2(0.0, 4.0)), CornerRadius::ZERO, Color32::from_black_alpha(70)).with_blur_width(14.0));
                    p.rect_filled(sheet, 0.0, Color32::WHITE);
                    let to = |x: f64, y: f64| pos2(sheet.left() + x as f32 * k, sheet.top() + y as f32 * k);
                    let (sw, sh) = fmt.sheet();
                    let b = if ps.bleed { fmt.bleed } else { 0.0 };
                    for i in 0..per.min(jobs.len().max(1)) {
                        let m = layout.place(i, Side::Front, &fmt);
                        let c = [(-b, -b), (sw + b, -b), (sw + b, sh + b), (-b, sh + b)].map(|(x, y)| {
                            let (a, bb) = m.apply(x, y);
                            to(a, bb)
                        });
                        match tex {
                            Some(id) => {
                                let mut mesh = egui::Mesh::with_texture(id);
                                for (pt, uv) in c.iter().zip([pos2(0.0, 0.0), pos2(1.0, 0.0), pos2(1.0, 1.0), pos2(0.0, 1.0)]) {
                                    mesh.vertices.push(egui::epaint::Vertex { pos: *pt, uv, color: Color32::WHITE });
                                }
                                mesh.add_triangle(0, 1, 2);
                                mesh.add_triangle(0, 2, 3);
                                p.add(egui::Shape::mesh(mesh));
                            }
                            None => {
                                p.add(egui::Shape::convex_polygon(c.to_vec(), P.accent.gamma_multiply(0.3), Stroke::new(1.0, P.accent)));
                            }
                        }
                    }
                    if ps.crop_marks {
                        for (a, c) in layout.crop_marks() {
                            p.line_segment([to(a.0, a.1), to(c.0, c.1)], Stroke::new(1.0, Color32::BLACK));
                        }
                    }
                    p.rect_stroke(sheet, 0.0, Stroke::new(1.0, P.border), StrokeKind::Outside);
                    if per == 0 {
                        p.text(sheet.center(), egui::Align2::CENTER_CENTER, "The card doesn't fit", egui::FontId::proportional(13.0), Color32::from_rgb(0xc4, 0x2b, 0x1c));
                    }
                });
            });
            ui.painter().vline(gap_x, row.response.rect.y_range(), Stroke::new(1.0, P.border));
        });
        if ps != self.doc.print {
            self.edit(now, Some("print-settings"), |d| d.print = ps);
        }
        if save {
            self.popups.print = false;
            self.export(ctx, Export::PrintSheets, now);
        } else {
            self.popups.print = open;
        }
    }
}
