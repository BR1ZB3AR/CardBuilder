//! Background rendering, so editing never waits for pixels: one thread draws the side shown in
//! the editor (at the zoom it is shown at), another the textures of the 3D card (both sides and
//! the foil / spot UV masks). Each keeps only the newest job.

use cb_core::doc::{Doc, Side};
use cb_print::{render_finish_mask, render_side, DrawOptions, ImageCache};
use std::sync::{Arc, Condvar, Mutex};

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

struct Slot<J, D> {
    job: Mutex<Option<J>>,
    wake: Condvar,
    done: Mutex<Option<D>>,
}

fn spawn<J: Send + 'static, D: Send + 'static>(name: &str, ctx: egui::Context, work: impl Fn(J, &mut ImageCache) -> Option<D> + Send + 'static) -> Arc<Slot<J, D>> {
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
    slot
}

fn submit<J, D>(slot: &Slot<J, D>, job: J) {
    *slot.job.lock().unwrap() = Some(job);
    slot.wake.notify_one();
}

type PreviewJob = (Arc<Doc>, Option<usize>, f64);
type TexJob = (Arc<Doc>, Option<usize>);

pub struct Renderer {
    preview: Arc<Slot<PreviewJob, Preview>>,
    textures: Arc<Slot<TexJob, Textures>>,
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

/// 3D texture resolution: 10 px per mm, at most 4096 px across
fn tex_ppm(doc: &Doc) -> f64 {
    let (sw, sh) = doc.format.sheet();
    (4096.0 / sw.max(sh)).min(10.0)
}

impl Renderer {
    pub fn new(ctx: &egui::Context) -> Renderer {
        let preview = spawn("card-preview", ctx.clone(), |(doc, record, ppm): PreviewJob, imgs| {
            prepare(imgs, &doc);
            let side = |s: Side| -> Option<(u32, u32, Vec<u8>)> {
                let pm = render_side(&doc, s, &DrawOptions::preview(record), ppm, true, imgs)?;
                Some((pm.width(), pm.height(), pm.take()))
            };
            Some(Preview { sides: [side(Side::Front)?, side(Side::Back)?] })
        });
        let textures = spawn("card-3d-textures", ctx.clone(), |(doc, record): TexJob, imgs| {
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
            submit(&self.textures, (d, record));
        }
    }

    /// ask for the editor images of both sides at `ppm` pixels per mm (only when something changed)
    pub fn want_preview(&mut self, record: Option<usize>, ppm: f64) {
        let Some(d) = self.doc.clone() else { return };
        let key = (record, Arc::as_ptr(&d) as u64 ^ (ppm.to_bits().rotate_left(17)));
        if self.preview_key != Some(key) {
            self.preview_key = Some(key);
            submit(&self.preview, (d, record, ppm));
        }
    }

    pub fn take_preview(&mut self) -> Option<Preview> {
        self.preview.done.lock().unwrap().take()
    }

    pub fn take_textures(&mut self) -> Option<Textures> {
        self.textures.done.lock().unwrap().take()
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
