//! Background rendering, so editing never waits for pixels: one thread draws the side shown in
//! the editor (at the zoom it is shown at), another the textures of the 3D card (both sides and
//! the foil / spot UV masks). Each keeps only the newest job.
//!
//! The web version has no threads: a job runs in the frame that asks for its result, and the 3D
//! textures wait for a pause in the editing, so typing stays quick.

use cb_core::doc::{Doc, Side};
use cb_print::{render_finish_mask, render_side, DrawOptions, ImageCache};
use std::sync::Arc;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::{Condvar, Mutex};

/// the editor's images of both sides (premultiplied RGBA of each bleed box)
pub struct Preview {
    /// front, back: (width, height, pixels)
    pub sides: [(u32, u32, Vec<u8>); 2],
}

/// one side of the 3D card: straight RGBA of the trim box (with its mip levels), and its finish mask
pub struct SideTex {
    pub w: u32,
    pub h: u32,
    /// level 0 first, each level half the size of the one before
    pub color: Vec<Vec<u8>>,
    pub mask_w: u32,
    pub mask_h: u32,
    pub mask: Vec<u8>,
}

pub struct Textures {
    pub sides: [SideTex; 2],
}

#[cfg(not(target_arch = "wasm32"))]
struct Slot<J, D> {
    job: Mutex<Option<J>>,
    wake: Condvar,
    done: Mutex<Option<D>>,
}

/// one worker that keeps only the newest job: a thread, or in the browser the frame itself
#[cfg(not(target_arch = "wasm32"))]
struct Lane<J, D> {
    slot: Arc<Slot<J, D>>,
}

#[cfg(not(target_arch = "wasm32"))]
impl<J: Send + 'static, D: Send + 'static> Lane<J, D> {
    /// `settle` only matters in the browser (see there)
    fn new(name: &str, ctx: egui::Context, _settle: f64, work: impl Fn(J, &mut ImageCache) -> Option<D> + Send + 'static) -> Lane<J, D> {
        let slot = Arc::new(Slot { job: Mutex::new(None), wake: Condvar::new(), done: Mutex::new(None) });
        let s = slot.clone();
        std::thread::Builder::new()
            .name(name.into())
            .spawn(move || {
                let mut imgs = ImageCache::default();
                loop {
                    let job = {
                        let mut j = s.job.lock().unwrap();
                        while j.is_none() {
                            j = s.wake.wait(j).unwrap();
                        }
                        j.take().unwrap()
                    };
                    if let Some(d) = work(job, &mut imgs) {
                        *s.done.lock().unwrap() = Some(d);
                        ctx.request_repaint();
                    }
                }
            })
            .expect("render thread");
        Lane { slot }
    }

    fn submit(&mut self, job: J) {
        *self.slot.job.lock().unwrap() = Some(job);
        self.slot.wake.notify_one();
    }

    fn take(&mut self) -> Option<D> {
        self.slot.done.lock().unwrap().take()
    }
}

#[cfg(target_arch = "wasm32")]
struct Lane<J, D> {
    job: Option<J>,
    work: Box<dyn Fn(J, &mut ImageCache) -> Option<D>>,
    imgs: ImageCache,
    ctx: egui::Context,
    /// seconds a job waits for a newer one before it runs (a pause in the editing)
    settle: f64,
    since: f64,
}

#[cfg(target_arch = "wasm32")]
impl<J, D> Lane<J, D> {
    fn new(_name: &str, ctx: egui::Context, settle: f64, work: impl Fn(J, &mut ImageCache) -> Option<D> + 'static) -> Lane<J, D> {
        Lane { job: None, work: Box::new(work), imgs: ImageCache::default(), ctx, settle, since: 0.0 }
    }

    fn submit(&mut self, job: J) {
        self.job = Some(job);
        self.since = self.ctx.input(|i| i.time);
    }

    /// run the waiting job once it has settled
    fn take(&mut self) -> Option<D> {
        self.job.as_ref()?;
        let waited = self.ctx.input(|i| i.time) - self.since;
        if waited < self.settle {
            self.ctx.request_repaint_after(std::time::Duration::from_secs_f64(self.settle - waited));
            return None;
        }
        let job = self.job.take()?;
        (self.work)(job, &mut self.imgs)
    }
}

type PreviewJob = (Arc<Doc>, Option<usize>, f64);
type TexJob = (Arc<Doc>, Option<usize>);

pub struct Renderer {
    preview: Lane<PreviewJob, Preview>,
    textures: Lane<TexJob, Textures>,
    /// the design the jobs were made from (shared, so a frame without changes costs one compare)
    doc: Option<Arc<Doc>>,
    preview_key: Option<(Option<usize>, u64)>,
    tex_key: Option<Option<usize>>,
    tex_doc: Option<Arc<Doc>>,
}

fn prepare(imgs: &mut ImageCache, doc: &Doc) {
    imgs.prepare(cb_print::image_srcs(doc));
    imgs.retain(cb_print::image_srcs(doc));
}

/// 3D texture resolution: 24 px per mm (about 600 dpi, sharp when zoomed in close), at most
/// 8192 px across (egui's device always allows that) and 16 million pixels a side. The web
/// version draws them in the frame, so it settles for 12 px per mm and 4 million pixels.
pub fn tex_ppm(doc: &Doc) -> f64 {
    let (sw, sh) = doc.format.sheet();
    let (ppm, budget) = if cfg!(target_arch = "wasm32") { (12.0, 4.0e6) } else { (24.0, 16.0e6) };
    f64::min(ppm, 8192.0 / sw.max(sh)).min((budget / (sw * sh)).sqrt())
}

impl Renderer {
    pub fn new(ctx: &egui::Context) -> Renderer {
        let preview = Lane::new("card-preview", ctx.clone(), 0.0, |(doc, record, ppm): PreviewJob, imgs| {
            prepare(imgs, &doc);
            let side = |s: Side| -> Option<(u32, u32, Vec<u8>)> {
                let pm = render_side(&doc, s, &DrawOptions::preview(record), ppm, true, imgs)?;
                Some((pm.width(), pm.height(), pm.take()))
            };
            Some(Preview { sides: [side(Side::Front)?, side(Side::Back)?] })
        });
        let textures = Lane::new("card-3d-textures", ctx.clone(), 0.35, |(doc, record): TexJob, imgs| {
            prepare(imgs, &doc);
            let ppm = tex_ppm(&doc);
            let finishes = doc.has_finishes();
            let side = |s: Side| -> Option<SideTex> {
                let pm = render_side(&doc, s, &DrawOptions::preview(record), ppm, false, imgs)?;
                let (w, h) = (pm.width(), pm.height());
                let mut color = pm.take();
                cb_print::unpremultiply(&mut color);
                let color = mips(w, h, color);
                let (mask_w, mask_h, mask) = if finishes { render_finish_mask(&doc, s, record, ppm / 2.0, imgs)? } else { (1, 1, vec![0, 0, 0, 255]) };
                Some(SideTex { w, h, color, mask_w, mask_h, mask })
            };
            Some(Textures { sides: [side(Side::Front)?, side(Side::Back)?] })
        });
        Renderer { preview, textures, doc: None, preview_key: None, tex_key: None, tex_doc: None }
    }

    /// keep the shared copy of the design current (call once a frame)
    pub fn sync(&mut self, doc: &Doc, record: Option<usize>) {
        if self.doc.as_deref() != Some(doc) {
            self.doc = Some(Arc::new(doc.clone()));
        }
        let d = self.doc.clone().unwrap();
        let same_doc = self.tex_doc.as_ref().is_some_and(|t| Arc::ptr_eq(t, &d));
        if !same_doc || self.tex_key != Some(record) {
            self.tex_doc = Some(d.clone());
            self.tex_key = Some(record);
            self.textures.submit((d, record));
        }
    }

    /// ask for the editor images of both sides at `ppm` pixels per mm (only when something changed)
    pub fn want_preview(&mut self, record: Option<usize>, ppm: f64) {
        let Some(d) = self.doc.clone() else { return };
        let key = (record, Arc::as_ptr(&d) as u64 ^ (ppm.to_bits().rotate_left(17)));
        if self.preview_key != Some(key) {
            self.preview_key = Some(key);
            self.preview.submit((d, record, ppm));
        }
    }

    pub fn take_preview(&mut self) -> Option<Preview> {
        self.preview.take()
    }

    pub fn take_textures(&mut self) -> Option<Textures> {
        self.textures.take()
    }
}

/// a mip chain by 2×2 box filtering (sizes rounded down, at least 1 px)
pub fn mips(w: u32, h: u32, base: Vec<u8>) -> Vec<Vec<u8>> {
    let mut levels = vec![base];
    let (mut w, mut h) = (w as usize, h as usize);
    while w > 1 || h > 1 {
        let (nw, nh) = ((w / 2).max(1), (h / 2).max(1));
        let prev = levels.last().unwrap();
        let mut next = vec![0u8; nw * nh * 4];
        for y in 0..nh {
            for x in 0..nw {
                for c in 0..4 {
                    let mut s = 0u32;
                    for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                        let sx = (2 * x + dx).min(w - 1);
                        let sy = (2 * y + dy).min(h - 1);
                        s += prev[(sy * w + sx) * 4 + c] as u32;
                    }
                    next[(y * nw + x) * 4 + c] = ((s + 2) / 4) as u8;
                }
            }
        }
        levels.push(next);
        w = nw;
        h = nh;
    }
    levels
}
