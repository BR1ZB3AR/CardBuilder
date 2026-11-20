//! Card picker thumbnails: the front of a template (or of a built-in card's starter design),
//! rendered small in the background and kept for the session.

use cb_core::doc::{default_doc, load_design, Side};
use cb_core::format::{CardFormat, Preset};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// what a thumbnail shows
pub enum Source {
    Preset(&'static Preset),
    File(PathBuf),
}

/// thumbnail height in pixels (sharp on high-density screens at the picker's size)
const HEIGHT: f64 = 200.0;

/// finished renders: key, width, height, RGBA
type Done = Arc<Mutex<Vec<(String, u32, u32, Vec<u8>)>>>;

#[derive(Default)]
pub struct Thumbs {
    tex: HashMap<String, egui::TextureHandle>,
    pending: HashSet<String>,
    done: Done,
    /// the web version draws them itself, one a frame: the frame it last drew one in
    #[cfg(target_arch = "wasm32")]
    drawn_in: Option<u64>,
}

/// the front of the card `src` shows, rendered small: width, height, premultiplied RGBA
fn render(src: Source) -> Option<(u32, u32, Vec<u8>)> {
    let doc = match src {
        Source::Preset(p) => Some(default_doc(CardFormat::from_preset(p))),
        Source::File(path) => std::fs::read_to_string(path).ok().and_then(|t| load_design(&t).ok()),
    }?;
    let mut imgs = cb_print::ImageCache::default();
    imgs.prepare(cb_print::image_srcs(&doc));
    let ppm = HEIGHT / doc.format.sheet().1.max(1.0);
    let pm = cb_print::render_side(&doc, Side::Front, &cb_print::DrawOptions::preview(None), ppm, false, &imgs)?;
    Some((pm.width(), pm.height(), pm.take()))
}

/// a key that changes when a template file is saved again
pub fn file_key(path: &std::path::Path) -> String {
    let stamp = std::fs::metadata(path).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_millis()).unwrap_or(0);
    format!("{}@{stamp}", path.display())
}

impl Thumbs {
    /// The thumbnail for `key`, once rendered; the first call starts rendering it.
    pub fn get(&mut self, ctx: &egui::Context, key: &str, source: impl FnOnce() -> Source) -> Option<egui::TextureId> {
        let finished: Vec<_> = self.done.lock().unwrap().drain(..).collect();
        for (k, w, h, rgba) in finished {
            let img = egui::ColorImage::from_rgba_premultiplied([w as usize, h as usize], &rgba);
            self.tex.insert(k.clone(), ctx.load_texture(format!("thumb-{k}"), img, egui::TextureOptions::LINEAR));
            self.pending.remove(&k);
        }
        if let Some(t) = self.tex.get(key) {
            return Some(t.id());
        }
        #[cfg(not(target_arch = "wasm32"))]
        if self.pending.insert(key.to_string()) {
            let (key, src, done, ctx) = (key.to_string(), source(), self.done.clone(), ctx.clone());
            std::thread::spawn(move || {
                if let Some((w, h, rgba)) = render(src) {
                    done.lock().unwrap().push((key, w, h, rgba));
                    ctx.request_repaint();
                }
            });
        }
        #[cfg(target_arch = "wasm32")]
        {
            let frame = ctx.cumulative_frame_nr();
            if self.drawn_in == Some(frame) {
                ctx.request_repaint();
            } else if self.pending.insert(key.to_string()) {
                self.drawn_in = Some(frame);
                if let Some((w, h, rgba)) = render(source()) {
                    self.done.lock().unwrap().push((key.to_string(), w, h, rgba));
                    ctx.request_repaint();
                }
            }
        }
        None
    }
}
