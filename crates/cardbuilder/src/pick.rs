//! Files the user picks to bring in (a design, a picture, a spreadsheet, a font). The desktop's
//! system dialog hands the file over at once; the browser's file chooser answers a moment later,
//! so there each pick says what it is for and is handled once its bytes are in (`take_picked`).

use crate::app::CardBuilderApp;
use cb_core::doc::Side;
use std::sync::{Arc, Mutex};

/// what a picked file is for (the desktop handles it on the spot, so only the web version
/// reads this)
#[derive(Clone, Debug)]
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub(crate) enum PickFor {
    /// a design to open (web only: the desktop opens it from its path, to save back into it)
    Design,
    /// a picture to add as artwork (where on the card, or in the middle)
    Image(Option<(f64, f64)>),
    /// the background picture of a side
    Background(Side),
    /// a new picture for the image layer with this id
    Replace(String),
    /// a spreadsheet for the data merge
    Data,
    /// a font file, for the text layer with this id
    Font(String),
    /// a template to keep in the browser (web only; several can be picked at once)
    Template,
}

/// a picked file: what it's for, its name, its bytes
type Picked = (PickFor, String, Vec<u8>);

/// picked files waiting to be handled (web)
pub(crate) struct Inbox {
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    items: Arc<Mutex<Vec<Picked>>>,
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    ctx: egui::Context,
}

impl Inbox {
    pub fn new(ctx: &egui::Context) -> Inbox {
        Inbox { items: Default::default(), ctx: ctx.clone() }
    }
}

impl CardBuilderApp {
    /// Pick one file. On the desktop: its name and bytes, right away (None when cancelled or
    /// unreadable). In the browser: None, and the file comes back later through the inbox.
    pub(crate) fn pick_file(&mut self, what: PickFor, title: &str, kind: &str, exts: &[&str], now: f64) -> Option<(String, Vec<u8>)> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = what;
            let path = rfd::FileDialog::new().add_filter(kind, exts).set_title(title).pick_file()?;
            let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            match std::fs::read(&path) {
                Ok(b) => Some((name, b)),
                Err(e) => {
                    self.say(format!("Couldn't read {name}: {e}"), now);
                    None
                }
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = now;
            let dialog = rfd::AsyncFileDialog::new().add_filter(kind, exts).set_title(title);
            let (items, ctx) = (self.picks.items.clone(), self.picks.ctx.clone());
            wasm_bindgen_futures::spawn_local(async move {
                if let Some(f) = dialog.pick_file().await {
                    let bytes = f.read().await;
                    items.lock().unwrap().push((what, f.file_name(), bytes));
                    ctx.request_repaint();
                }
            });
            None
        }
    }

    /// Import templates… (web): pick any number of `.cb` files; they come back through the inbox
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn pick_templates(&mut self) {
        let dialog = rfd::AsyncFileDialog::new().add_filter("CardBuilder template", &[cb_core::doc::CB_EXT]).set_title("Import templates");
        let (items, ctx) = (self.picks.items.clone(), self.picks.ctx.clone());
        wasm_bindgen_futures::spawn_local(async move {
            let Some(files) = dialog.pick_files().await else { return };
            let mut read = Vec::new();
            for f in files {
                read.push((PickFor::Template, f.file_name(), f.read().await));
            }
            // all together, so they are added as one batch
            items.lock().unwrap().extend(read);
            ctx.request_repaint();
        });
    }

    /// handle the files the browser has handed over since the last frame
    pub(crate) fn take_picked(&mut self, now: f64) {
        #[cfg(target_arch = "wasm32")]
        {
            let arrived: Vec<_> = self.picks.items.lock().unwrap().drain(..).collect();
            let mut templates = Vec::new();
            for (what, name, bytes) in arrived {
                match what {
                    PickFor::Template => templates.push((name, bytes)),
                    what => self.picked(what, &name, bytes, now),
                }
            }
            if !templates.is_empty() {
                self.import_templates(templates, now);
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        let _ = now;
    }

    /// do what the file was picked for
    #[cfg(target_arch = "wasm32")]
    fn picked(&mut self, what: PickFor, name: &str, bytes: Vec<u8>, now: f64) {
        match what {
            PickFor::Design => self.open_picked_design(name, &String::from_utf8_lossy(&bytes), now),
            PickFor::Image(at) => self.add_image_bytes(now, name, &bytes, at),
            PickFor::Background(side) => match crate::exports::image_data_url(name, &bytes) {
                Ok((src, _, _)) => self.edit(now, None, |d| d.background_mut(side).src = Some(src)),
                Err(e) => self.say(format!("Could not read that image: {e}"), now),
            },
            PickFor::Replace(id) => self.replace_image_bytes(now, &id, name, &bytes),
            PickFor::Data => self.import_data_text(name, &String::from_utf8_lossy(&bytes), now),
            PickFor::Font(layer) => {
                if let Some(fam) = self.add_font_bytes(name, &bytes, now) {
                    self.edit(now, None, |d| {
                        if let Some(l) = d.layer_mut(&layer) {
                            l.font = Some(fam);
                        }
                    });
                }
            }
            PickFor::Template => self.import_templates(vec![(name.to_string(), bytes)], now),
        }
    }
}
