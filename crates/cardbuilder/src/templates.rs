//! Local templates: `.cb` designs in CardBuilder's data folder (on Linux
//! `~/.local/share/cardbuilder/templates`) that show in the card picker next to the built-in
//! sizes — in the category and group their file names, e.g. Playing & game cards › Ingress
//! Biocard. Picking one starts a new design from it. Templates stay on this computer: artwork or
//! fonts that may not be shared never go into the app itself.

use crate::app::CardBuilderApp;
use cb_core::doc::{load_design, template_info, TemplateInfo, CB_EXT};
use cb_core::format::{CardFormat, Category};
use std::path::{Path, PathBuf};

/// the group a template without one is listed under
pub const MY_TEMPLATES: &str = "My templates";

#[derive(Clone, Debug)]
pub struct LocalTemplate {
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

/// every template in the folder, sorted by group and name
pub fn scan() -> Vec<LocalTemplate> {
    let Some(dir) = templates_dir() else { return vec![] };
    let Ok(rd) = std::fs::read_dir(&dir) else { return vec![] };
    let mut out: Vec<LocalTemplate> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case(CB_EXT)))
        .filter_map(|path| {
            let text = std::fs::read_to_string(&path).ok()?;
            let info = template_info(&text)?;
            let format = load_design(&text).ok()?.format;
            Some(LocalTemplate { path, info, format })
        })
        .collect();
    out.sort_by(|a, b| (a.group(), &a.info.name).cmp(&(b.group(), &b.info.name)));
    out
}

/// a file-name-safe id from a name
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
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
        let text = match std::fs::read_to_string(path) {
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

    /// Save as template…: the design goes into the templates folder and shows in the card picker
    /// (in the same group as the template or preset it came from, else under My templates).
    pub(crate) fn save_as_template(&mut self, now: f64) {
        #[cfg(target_arch = "wasm32")]
        return self.say("Templates are kept in a folder on the computer: save them from the desktop app", now);
        #[cfg(not(target_arch = "wasm32"))]
        self.save_template_file(now);
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn save_template_file(&mut self, now: f64) {
        let Some(dir) = templates_dir() else { return self.say("No folder for templates on this computer", now) };
        if let Err(e) = std::fs::create_dir_all(&dir) {
            return self.say(format!("Couldn't make the templates folder: {e}"), now);
        }
        let suggested = format!("{}.{CB_EXT}", self.format_title().replace(['/', '\\', ':'], "-"));
        let Some(path) = rfd::FileDialog::new().add_filter("CardBuilder template", &[CB_EXT]).set_directory(&dir).set_file_name(&suggested).set_title("Save as template").save_file() else {
            return;
        };
        let path = if path.extension().is_some_and(|e| e.eq_ignore_ascii_case(CB_EXT)) { path } else { path.with_extension(CB_EXT) };
        let name = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "Template".into());
        let (category, group) = match self.current_template() {
            Some(t) => (t.info.category.clone(), t.info.group.clone()),
            None => {
                let g = cb_core::format::preset(&self.doc.format.preset).and_then(|p| p.group).map(String::from);
                (self.doc.format.category().id().to_string(), g.or(Some(MY_TEMPLATES.into())))
            }
        };
        let info = TemplateInfo { id: format!("user-{}", slug(&name)), name: name.clone(), category, group, note: self.doc.format.size_label() };
        let text = cb_core::doc::save_cb_template(&self.doc, &format!("CardBuilder {}", crate::app::VERSION), &info);
        match std::fs::write(&path, text) {
            Ok(()) => {
                self.rescan_templates();
                self.say(format!("Saved the {name} template — it's in the card picker"), now);
            }
            Err(e) => self.say(format!("Couldn't save the template: {e}"), now),
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn slugs() {
        assert_eq!(super::slug("Ingress Biocard · 2023 Origin!"), "ingress-biocard-2023-origin");
    }
}
