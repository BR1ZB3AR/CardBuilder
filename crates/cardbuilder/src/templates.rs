//! Local templates: `.cb` designs that show in the card picker next to the built-in sizes — in
//! the category and group their header names, e.g. Playing & game cards › Ingress Biocard.
//! Picking one starts a new design from it. On the desktop they are files in CardBuilder's data
//! folder (on Linux `~/.local/share/cardbuilder/templates`); the web version keeps them in the
//! browser (`browser` below), where Import templates…, dropping template files on the page or
//! Save as template… puts them. Either way they stay with the user: artwork or fonts that may
//! not be shared never go into the app itself.

use crate::app::CardBuilderApp;
use cb_core::doc::{load_design, save_cb_template, template_info, Doc, TemplateInfo};
use cb_core::format::{CardFormat, Category};
use std::path::{Path, PathBuf};

/// the group a template without one is listed under
pub const MY_TEMPLATES: &str = "My templates";

#[derive(Clone, Debug)]
pub struct LocalTemplate {
    /// the file (desktop), or the browser's key for it: the template's id (web)
    pub path: PathBuf,
    pub info: TemplateInfo,
    pub format: CardFormat,
}

impl LocalTemplate {
    pub fn category(&self) -> Category {
        Category::from_id(&self.info.category)
    }
    pub fn group(&self) -> &str {
        self.info.group.as_deref().unwrap_or(MY_TEMPLATES)
    }
}

/// where local templates live (`CB_TEMPLATE_DIR` overrides it, for tests); none in the browser
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
pub fn templates_dir() -> Option<PathBuf> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        if let Ok(d) = std::env::var("CB_TEMPLATE_DIR") {
            return Some(PathBuf::from(d));
        }
        eframe::storage_dir("cardbuilder").map(|d| d.join("templates"))
    }
    #[cfg(target_arch = "wasm32")]
    None
}

/// every template there is (in the folder, or in the browser), sorted by group and name
pub fn scan() -> Vec<LocalTemplate> {
    let entry = |path: PathBuf, text: &str| {
        let info = template_info(text)?;
        let format = load_design(text).ok()?.format;
        Some(LocalTemplate { path, info, format })
    };
    #[cfg(not(target_arch = "wasm32"))]
    let mut out: Vec<LocalTemplate> = {
        let Some(dir) = templates_dir() else { return vec![] };
        let Ok(rd) = std::fs::read_dir(&dir) else { return vec![] };
        rd.flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case(cb_core::doc::CB_EXT)))
            .filter_map(|path| {
                let text = std::fs::read_to_string(&path).ok()?;
                entry(path, &text)
            })
            .collect()
    };
    #[cfg(target_arch = "wasm32")]
    let mut out: Vec<LocalTemplate> = browser::all().into_iter().filter_map(|(key, text)| entry(PathBuf::from(key), &text)).collect();
    out.sort_by(|a, b| (a.group(), &a.info.name).cmp(&(b.group(), &b.info.name)));
    out
}

/// a template's text: its file on the desktop, the browser's copy on the web
pub fn read_text(path: &Path) -> Result<String, String> {
    #[cfg(not(target_arch = "wasm32"))]
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string());
    #[cfg(target_arch = "wasm32")]
    let text = browser::text(&path.to_string_lossy()).ok_or_else(|| "it is no longer in this browser".to_string());
    text
}

/// something that changes whenever the template is saved again (for thumbnail keys)
pub fn stamp(path: &Path) -> u128 {
    #[cfg(not(target_arch = "wasm32"))]
    let stamp = std::fs::metadata(path).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_millis()).unwrap_or(0);
    #[cfg(target_arch = "wasm32")]
    let stamp = browser::rev(&path.to_string_lossy()) as u128;
    stamp
}

/// the header for a template made from `doc`: listed with the template it came from (`from`),
/// else with its built-in card's group, else under My templates
pub fn info_for(doc: &Doc, name: &str, from: Option<&LocalTemplate>) -> TemplateInfo {
    let (category, group) = match from {
        Some(t) => (t.info.category.clone(), t.info.group.clone()),
        None => {
            let g = cb_core::format::preset(&doc.format.preset).and_then(|p| p.group).map(String::from);
            (doc.format.category().id().to_string(), g.or(Some(MY_TEMPLATES.into())))
        }
    };
    TemplateInfo { id: format!("user-{}", slug(name)), name: name.to_string(), category, group, note: doc.format.size_label() }
}

/// A `.cb` file as a template, to import: a template file keeps its header; any other design
/// becomes a template named after the file, listed with the template it was made from (when it
/// is one of `known`). Gives the header and the template's text.
#[cfg_attr(not(any(target_arch = "wasm32", test)), allow(dead_code))]
pub fn as_template(file_name: &str, text: &str, known: &[LocalTemplate]) -> Result<(TemplateInfo, String), String> {
    let doc = load_design(text).map_err(|_| format!("{file_name} isn't a CardBuilder design"))?;
    if let Some(info) = template_info(text) {
        return Ok((info, text.to_string()));
    }
    let name = Path::new(file_name).file_stem().map(|s| s.to_string_lossy().into_owned()).filter(|s| !s.trim().is_empty()).unwrap_or_else(|| "Template".into());
    let from = doc.template.as_ref().and_then(|id| known.iter().find(|t| &t.info.id == id));
    let info = info_for(&doc, &name, from);
    let text = save_cb_template(&doc, &format!("CardBuilder {}", crate::app::VERSION), &info);
    Ok((info, text))
}

/// a file-name-safe id from a name
fn slug(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').to_string()
}

impl CardBuilderApp {
    pub(crate) fn rescan_templates(&mut self) {
        self.templates = scan();
    }

    /// the template the design came from, if it is still there
    pub(crate) fn current_template(&self) -> Option<&LocalTemplate> {
        let id = self.doc.template.as_ref()?;
        self.templates.iter().find(|t| &t.info.id == id)
    }

    /// Start a new design from a template file (no question asked — see `request_template`).
    /// The design on screen before stays one Ctrl+Z away.
    pub(crate) fn open_template(&mut self, path: &Path, now: f64) {
        let text = match read_text(path) {
            Ok(t) => t,
            Err(e) => return self.say(format!("Couldn't read the template: {e}"), now),
        };
        let (Some(info), Ok(mut doc)) = (template_info(&text), load_design(&text)) else {
            return self.say("That template can't be read", now);
        };
        doc.template = Some(info.id.clone());
        let before = std::mem::replace(&mut self.doc, doc.clone());
        if before != self.doc {
            self.history.record(&before, None, now);
        }
        self.selected = None;
        self.record = 0;
        self.side = cb_core::Side::Front;
        self.file.path = None;
        self.file.baseline = doc;
        self.prefs.last_file = None;
        self.custom = crate::formats::CustomDraft::from(&self.doc.format);
        self.canvas.request_fit();
        self.viewer.reset_view();
        self.say(format!("New card from the {} template — Save keeps it as your own .cb", info.name), now);
    }

    /// Save as template…: the design goes into the templates folder (desktop) or the browser
    /// (web, after asking for a name) and shows in the card picker, in the same group as the
    /// template or preset it came from, else under My templates.
    pub(crate) fn save_as_template(&mut self, now: f64) {
        #[cfg(target_arch = "wasm32")]
        {
            let _ = now;
            self.popups.template_name = Some(self.format_title());
        }
        #[cfg(not(target_arch = "wasm32"))]
        self.save_template_file(now);
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn save_template_file(&mut self, now: f64) {
        let Some(dir) = templates_dir() else { return self.say("No folder for templates on this computer", now) };
        if let Err(e) = std::fs::create_dir_all(&dir) {
            return self.say(format!("Couldn't make the templates folder: {e}"), now);
        }
        let suggested = format!("{}.{}", self.format_title().replace(['/', '\\', ':'], "-"), cb_core::doc::CB_EXT);
        let Some(path) = rfd::FileDialog::new().add_filter("CardBuilder template", &[cb_core::doc::CB_EXT]).set_directory(&dir).set_file_name(&suggested).set_title("Save as template").save_file() else {
            return;
        };
        let path = if path.extension().is_some_and(|e| e.eq_ignore_ascii_case(cb_core::doc::CB_EXT)) { path } else { path.with_extension(cb_core::doc::CB_EXT) };
        let name = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "Template".into());
        let info = info_for(&self.doc, &name, self.current_template());
        let text = save_cb_template(&self.doc, &format!("CardBuilder {}", crate::app::VERSION), &info);
        match std::fs::write(&path, text) {
            Ok(()) => {
                self.rescan_templates();
                self.say(format!("Saved the {name} template — it's in the card picker"), now);
            }
            Err(e) => self.say(format!("Couldn't save the template: {e}"), now),
        }
    }
}

#[cfg(target_arch = "wasm32")]
impl CardBuilderApp {
    /// Save as template… in the browser: the name to keep it under
    pub(crate) fn template_name_dialog(&mut self, ctx: &egui::Context, now: f64) {
        let Some(mut name) = self.popups.template_name.take() else { return };
        let (mut open, mut save, mut cancel) = (true, false, false);
        let focus_id = egui::Id::unique("template-name-focused");
        crate::topbar::dialog(ctx, "template-name", "Save as template", Some("Kept in this browser, in the card picker"), 420.0, &mut open, |ui| {
            use crate::widgets::{button, BtnKind};
            ui.add_space(8.0);
            ui.label(egui::RichText::new("Name").color(crate::style::P.muted));
            let r = ui.add(egui::TextEdit::singleline(&mut name).desired_width(f32::INFINITY));
            r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, "Template name"));
            // the name is ready to type over when the dialog opens
            if !ui.ctx().data(|d| d.get_temp::<bool>(focus_id).unwrap_or(false)) {
                r.request_focus();
                ui.ctx().data_mut(|d| d.insert_temp(focus_id, true));
            }
            let ok = !name.trim().is_empty();
            if ui.input(|i| i.key_pressed(egui::Key::Enter)) && ok {
                save = true;
            }
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if button(ui, None, "Save", BtnKind::Primary, ok).clicked() {
                    save = true;
                }
                if button(ui, None, "Cancel", BtnKind::Normal, true).clicked() {
                    cancel = true;
                }
            });
        });
        if cancel {
            open = false;
        }
        if save || !open {
            ctx.data_mut(|d| d.remove::<bool>(focus_id));
        }
        if save {
            let name = name.trim().to_string();
            let info = info_for(&self.doc, &name, self.current_template());
            let replaced = browser::text(&info.id).is_some();
            browser::put(&info.id, save_cb_template(&self.doc, &format!("CardBuilder {}", crate::app::VERSION), &info));
            self.rescan_templates();
            let what = if replaced { "Replaced" } else { "Saved" };
            self.say(format!("{what} the {name} template in this browser — it's in the card picker"), now);
        } else if open {
            self.popups.template_name = Some(name);
        }
    }

    /// Import templates… or template files dropped on the page: each goes into the browser.
    /// One template also starts a card from it, so it shows straight away.
    pub(crate) fn import_templates(&mut self, files: Vec<(String, Vec<u8>)>, now: f64) {
        let mut added: Vec<(PathBuf, TemplateInfo)> = Vec::new();
        let mut failed: Vec<String> = Vec::new();
        for (file, bytes) in files {
            match as_template(&file, &String::from_utf8_lossy(&bytes), &self.templates) {
                Ok((info, text)) => {
                    browser::put(&info.id, text);
                    added.push((PathBuf::from(&info.id), info));
                }
                Err(e) => failed.push(e),
            }
        }
        self.rescan_templates();
        if let Some(e) = failed.first() {
            let more = if failed.len() > 1 { format!(" (and {} more)", failed.len() - 1) } else { String::new() };
            self.say(format!("Couldn't add {e}{more}"), now);
        }
        match added.as_slice() {
            [] => {}
            [(path, _)] => self.request_template(path.clone(), now),
            many => {
                let group = many[0].1.group.clone().unwrap_or_else(|| MY_TEMPLATES.into());
                let same = many.iter().all(|(_, i)| i.group.as_deref().unwrap_or(MY_TEMPLATES) == group);
                let place = if same { format!(" under {group}") } else { String::new() };
                self.say(format!("Added {} templates to this browser — they're in the card picker{place}", many.len()), now);
            }
        }
    }

    /// take a template out of the browser
    pub(crate) fn remove_template(&mut self, path: &Path, now: f64) {
        let key = path.to_string_lossy().into_owned();
        let name = self.templates.iter().find(|t| t.path == path).map(|t| t.info.name.clone()).unwrap_or_else(|| key.clone());
        browser::remove(&key);
        self.rescan_templates();
        self.say(format!("Removed the {name} template from this browser"), now);
    }

    /// once a frame: the stored templates have arrived (or a browser storage error to show)
    pub(crate) fn browser_templates_frame(&mut self, now: f64) {
        if browser::take_changed() {
            self.rescan_templates();
        }
        if let Some(e) = browser::take_error() {
            self.say(e, now);
        }
    }
}

/// The web version's templates: kept in the browser's IndexedDB (database `cardbuilder`, store
/// `templates`: the template's id → its `.cb` text), with a copy in memory for the picker.
/// Writes go out one at a time, in order.
#[cfg(target_arch = "wasm32")]
pub mod browser {
    use std::cell::RefCell;
    use std::collections::{BTreeMap, VecDeque};
    use wasm_bindgen::{closure::Closure, JsCast, JsValue};
    use wasm_bindgen_futures::JsFuture;
    use web_sys::{IdbDatabase, IdbRequest, IdbTransaction, IdbTransactionMode};

    const DB: &str = "cardbuilder";
    const STORE: &str = "templates";

    #[derive(Default)]
    struct Mem {
        /// id → (text, revision)
        items: BTreeMap<String, (String, u64)>,
        rev: u64,
        changed: bool,
        error: Option<String>,
        /// writes waiting (id, text or None to remove), and whether they are being written
        queue: VecDeque<(String, Option<String>)>,
        writing: bool,
    }

    thread_local! {
        static MEM: RefCell<Mem> = RefCell::new(Mem::default());
    }

    pub fn all() -> Vec<(String, String)> {
        MEM.with(|m| m.borrow().items.iter().map(|(k, (t, _))| (k.clone(), t.clone())).collect())
    }
    pub fn text(key: &str) -> Option<String> {
        MEM.with(|m| m.borrow().items.get(key).map(|(t, _)| t.clone()))
    }
    pub fn rev(key: &str) -> u64 {
        MEM.with(|m| m.borrow().items.get(key).map(|(_, r)| *r).unwrap_or(0))
    }
    /// true once after the stored templates arrive
    pub fn take_changed() -> bool {
        MEM.with(|m| std::mem::take(&mut m.borrow_mut().changed))
    }
    pub fn take_error() -> Option<String> {
        MEM.with(|m| m.borrow_mut().error.take())
    }
    fn fail(e: String) {
        MEM.with(|m| m.borrow_mut().error = Some(e));
    }

    pub fn put(key: &str, text: String) {
        MEM.with(|m| {
            let mut m = m.borrow_mut();
            m.rev += 1;
            let r = m.rev;
            m.items.insert(key.to_string(), (text.clone(), r));
        });
        queue(key, Some(text));
    }

    pub fn remove(key: &str) {
        MEM.with(|m| m.borrow_mut().items.remove(key));
        queue(key, None);
    }

    fn queue(key: &str, text: Option<String>) {
        let start = MEM.with(|m| {
            let mut m = m.borrow_mut();
            m.queue.push_back((key.to_string(), text));
            !std::mem::replace(&mut m.writing, true)
        });
        if start {
            wasm_bindgen_futures::spawn_local(async {
                loop {
                    let next = MEM.with(|m| {
                        let mut m = m.borrow_mut();
                        let next = m.queue.pop_front();
                        m.writing = next.is_some();
                        next
                    });
                    let Some((key, text)) = next else { break };
                    if let Err(e) = write(&key, text.as_deref()).await {
                        fail(format!("Couldn't keep that in this browser: {e}"));
                    }
                }
            });
        }
    }

    /// read the stored templates (once, at start); the app lists them when they arrive
    pub fn load(ctx: &egui::Context) {
        let ctx = ctx.clone();
        wasm_bindgen_futures::spawn_local(async move {
            match read_all().await {
                Ok(items) => MEM.with(|m| {
                    let mut m = m.borrow_mut();
                    for (k, t) in items {
                        m.rev += 1;
                        let r = m.rev;
                        // a template saved while these were on their way is newer
                        m.items.entry(k).or_insert((t, r));
                    }
                    m.changed = true;
                }),
                // (a browser that keeps nothing says so when a template is added)
                Err(e) => log::warn!("couldn't read the templates kept in this browser: {e}"),
            }
            ctx.request_repaint();
        });
    }

    fn js_err(v: JsValue) -> String {
        if let Some(e) = v.dyn_ref::<web_sys::DomException>() {
            return e.message();
        }
        if let Some(e) = v.dyn_ref::<js_sys::Error>() {
            return String::from(e.message());
        }
        v.as_string().unwrap_or_else(|| "the browser said no".into())
    }

    /// the request's result, once the browser has it (the handlers are set at once, so no
    /// answer is missed while something else is awaited)
    fn done(req: &IdbRequest) -> JsFuture {
        let p = js_sys::Promise::new(&mut |resolve, reject| {
            let r = req.clone();
            let ok = Closure::once_into_js(move || {
                let _ = resolve.call1(&JsValue::NULL, &r.result().unwrap_or(JsValue::UNDEFINED));
            });
            let r = req.clone();
            let bad = Closure::once_into_js(move || {
                let e = r.error().ok().flatten().map(JsValue::from).unwrap_or(JsValue::NULL);
                let _ = reject.call1(&JsValue::NULL, &e);
            });
            req.set_onsuccess(Some(ok.unchecked_ref()));
            req.set_onerror(Some(bad.unchecked_ref()));
        });
        JsFuture::from(p)
    }

    /// the transaction, once it is committed (a full disk shows here)
    fn committed(tx: &IdbTransaction) -> JsFuture {
        let p = js_sys::Promise::new(&mut |resolve, reject| {
            let ok = Closure::once_into_js(move || {
                let _ = resolve.call0(&JsValue::NULL);
            });
            let t = tx.clone();
            let bad = Closure::once_into_js(move || {
                let e = t.error().map(JsValue::from).unwrap_or(JsValue::NULL);
                let _ = reject.call1(&JsValue::NULL, &e);
            });
            tx.set_oncomplete(Some(ok.unchecked_ref()));
            tx.set_onabort(Some(bad.unchecked_ref()));
        });
        JsFuture::from(p)
    }

    async fn open() -> Result<IdbDatabase, String> {
        let factory = web_sys::window().ok_or("no window")?.indexed_db().map_err(js_err)?.ok_or("this browser keeps nothing for web pages")?;
        let req = factory.open_with_u32(DB, 1).map_err(js_err)?;
        let r = req.clone();
        let upgrade = Closure::once_into_js(move || {
            if let Ok(db) = r.result().and_then(|v| v.dyn_into::<IdbDatabase>()) {
                let _ = db.create_object_store(STORE);
            }
        });
        req.set_onupgradeneeded(Some(upgrade.unchecked_ref()));
        done(&req).await.map_err(js_err)?.dyn_into::<IdbDatabase>().map_err(js_err)
    }

    async fn read_all() -> Result<Vec<(String, String)>, String> {
        let db = open().await?;
        let tx = db.transaction_with_str(STORE).map_err(js_err)?;
        db.close(); // once the transaction is done
        let store = tx.object_store(STORE).map_err(js_err)?;
        let keys = done(&store.get_all_keys().map_err(js_err)?);
        let values = done(&store.get_all().map_err(js_err)?);
        let keys = js_sys::Array::from(&keys.await.map_err(js_err)?);
        let values = js_sys::Array::from(&values.await.map_err(js_err)?);
        Ok(keys.iter().zip(values.iter()).filter_map(|(k, v)| Some((k.as_string()?, v.as_string()?))).collect())
    }

    async fn write(key: &str, text: Option<&str>) -> Result<(), String> {
        let db = open().await?;
        let tx = db.transaction_with_str_and_mode(STORE, IdbTransactionMode::Readwrite).map_err(js_err)?;
        db.close();
        let store = tx.object_store(STORE).map_err(js_err)?;
        let end = committed(&tx);
        let req = match text {
            Some(t) => store.put_with_key(&JsValue::from_str(t), &JsValue::from_str(key)),
            None => store.delete(&JsValue::from_str(key)),
        }
        .map_err(js_err)?;
        let req = done(&req);
        req.await.map_err(js_err)?;
        end.await.map_err(js_err)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use cb_core::doc::{default_doc, save_cb, template_info};
    use cb_core::format::{preset, CardFormat};

    #[test]
    fn slugs() {
        assert_eq!(super::slug("Ingress Biocard · 2023 Origin!"), "ingress-biocard-2023-origin");
    }

    /// importing: a template keeps its header, a plain design becomes one named after its file
    /// (with its card's group), anything else is refused
    #[test]
    fn files_become_templates() {
        let doc = default_doc(CardFormat::from_preset(preset("game-biocard-res").unwrap()));
        let (info, text) = super::as_template("My Agent Card.cb", &save_cb(&doc, "test"), &[]).unwrap();
        assert_eq!(info.name, "My Agent Card");
        assert_eq!(info.id, "user-my-agent-card");
        assert_eq!(info.group.as_deref(), Some("Ingress Biocard"));
        assert_eq!(info.category, doc.format.category().id());
        // the text is a template now, and imports again as the same one
        assert_eq!(template_info(&text).unwrap().id, info.id);
        let (again, text2) = super::as_template("renamed.cb", &text, &[]).unwrap();
        assert_eq!(again.id, info.id);
        assert_eq!(text2, text);
        // a design that isn't in a group goes under My templates
        let plain = default_doc(CardFormat::from_preset(preset("game-poker").unwrap()));
        assert_eq!(super::as_template("x.cb", &save_cb(&plain, "test"), &[]).unwrap().0.group.as_deref(), Some(super::MY_TEMPLATES));
        // a design made from a known template goes with it
        let mut from = plain.clone();
        from.template = Some("cards-of-mine".into());
        let known = super::LocalTemplate {
            path: "x".into(),
            info: cb_core::doc::TemplateInfo { id: "cards-of-mine".into(), name: "Mine".into(), category: "business".into(), group: Some("Cards of mine".into()), note: String::new() },
            format: plain.format.clone(),
        };
        let (info, _) = super::as_template("y.cb", &save_cb(&from, "test"), &[known]).unwrap();
        assert_eq!((info.category.as_str(), info.group.as_deref()), ("business", Some("Cards of mine")));
        assert!(super::as_template("notes.cb", "hello", &[]).is_err());
    }
}
