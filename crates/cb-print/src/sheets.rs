//! Print files: a PNG of one side, a card PDF (one page per side with bleed and trim boxes, the
//! way print shops take them) and print sheets — many cards per page, crop marks, the backs on
//! the reverse for double-sided printing, and separate pages for foil and spot UV.

use crate::color::Rgba;
use crate::draw::{draw_side, render_side, DrawOptions, Mode};
use crate::images::ImageCache;
use crate::pdf::{PdfCanvas, PdfWriter};
use cb_core::doc::{Align, Doc, Side};
use cb_core::format::sheet_size;
use cb_core::impose::{impose, ImposeOptions, Layout};
use resvg::tiny_skia::{FillRule, Pixmap, Transform};
use std::collections::HashMap;

/// a premultiplied pixmap as PNG bytes
pub fn png_bytes(pm: &Pixmap) -> Result<Vec<u8>, String> {
    let mut data = pm.data().to_vec();
    crate::unpremultiply(&mut data);
    let img = image::RgbaImage::from_raw(pm.width(), pm.height(), data).ok_or("bad image size")?;
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).map_err(|e| e.to_string())?;
    Ok(out.into_inner())
}

/// one side as a PNG at `dpi`, with or without bleed
pub fn side_png(doc: &Doc, side: Side, record: Option<usize>, dpi: f64, bleed: bool, imgs: &ImageCache) -> Result<Vec<u8>, String> {
    let ppm = dpi / 25.4;
    let pm = render_side(doc, side, &DrawOptions::preview(record), ppm, bleed, imgs).ok_or("the image would be too big")?;
    png_bytes(&pm)
}

/// the sides that have something on them for a finishing mode
fn has(doc: &Doc, side: Side, mode: Mode) -> bool {
    doc.layers_on(side).any(|l| {
        l.visible
            && match mode {
                Mode::Foil => l.foil.is_some(),
                Mode::Uv => l.spot_uv,
                _ => true,
            }
    })
}

fn mode_name(mode: Mode) -> &'static str {
    match mode {
        Mode::Foil | Mode::Holo => "Foil",
        Mode::Uv => "Spot UV",
        Mode::Color => "Print",
    }
}

/// the cards to print, one entry per card: every data row, or `copies` of the design
pub fn print_jobs(doc: &Doc, per_sheet: usize) -> Vec<Option<usize>> {
    if !doc.data.is_empty() {
        (0..doc.data.rows.len()).map(Some).collect()
    } else {
        let n = if doc.print.copies == 0 { per_sheet.max(1) } else { doc.print.copies as usize };
        vec![None; n]
    }
}

/// the sheet layout the print settings give
pub fn sheet_layout(doc: &Doc) -> Layout {
    let s = sheet_size(&doc.print.sheet);
    let (sw, sh) = doc.format.sheet();
    let o = ImposeOptions {
        sheet_w: s.w,
        sheet_h: s.h,
        margin: doc.print.margin,
        gap: doc.print.gap,
        bleed: if doc.print.bleed { doc.format.bleed } else { 0.0 },
        allow_rotate: doc.print.allow_rotate,
        short_edge: doc.print.short_edge,
    };
    impose(sw, sh, &o)
}

/// Draw a card side as a reusable drawing in the PDF; its box is the bleed box (or the trim
/// box with `bleed` off).
fn card_form(pdf: &mut PdfWriter, doc: &Doc, side: Side, o: &DrawOptions, bleed: bool, imgs: &ImageCache) -> String {
    let (sw, sh) = doc.format.sheet();
    let b = if bleed { doc.format.bleed } else { 0.0 };
    let mut c = PdfCanvas::new(pdf);
    draw_side(&mut c, doc, side, o, imgs, Transform::identity());
    let content = std::mem::take(&mut c.out);
    pdf.add_form([-b, -b, sw + b, sh + b], &content)
}

/// One page per side of every card (front, back, then foil / spot UV pages when used), each
/// with bleed: the usual file for a print shop.
pub fn cards_pdf(doc: &Doc, records: &[Option<usize>], imgs: &ImageCache, progress: &dyn Fn(f32)) -> Result<Vec<u8>, String> {
    let mut pdf = PdfWriter::new("CardBuilder cards", doc.print.dpi.max(150) as f64);
    let (sw, sh) = doc.format.sheet();
    let b = doc.format.bleed;
    let mut modes = vec![Mode::Color];
    for m in [Mode::Foil, Mode::Uv] {
        if has(doc, Side::Front, m) || has(doc, Side::Back, m) {
            modes.push(m);
        }
    }
    let total = (records.len() * 2 * modes.len()).max(1) as f32;
    let mut done = 0.0;
    for &rec in records {
        for &mode in &modes {
            for side in [Side::Front, Side::Back] {
                done += 1.0;
                progress(done / total);
                if mode != Mode::Color && !has(doc, side, mode) {
                    continue;
                }
                let o = DrawOptions { record: rec, paper: false, mode, mask: Rgba::BLACK };
                let mut c = PdfCanvas::new(&mut pdf);
                draw_side(&mut c, doc, side, &o, imgs, Transform::from_translate(b as f32, b as f32));
                let content = std::mem::take(&mut c.out);
                pdf.add_page(sw + 2.0 * b, sh + 2.0 * b, &content, Some(b));
            }
        }
    }
    if pdf.page_count() == 0 {
        return Err("nothing to print".into());
    }
    Ok(pdf.finish())
}

#[derive(Clone, Debug, PartialEq)]
pub struct SheetSummary {
    pub sheets: usize,
    pub per_sheet: usize,
    pub cards: usize,
    pub pages: usize,
}

/// a small grey caption at the bottom of a sheet
fn caption(c: &mut PdfCanvas, text: &str, sheet_h: f64, margin: f64, flip_x: Option<f64>) {
    if margin < 5.0 {
        return;
    }
    let size = 2.3;
    let y = sheet_h - (margin / 2.0 - size / 3.0).max(1.5);
    let (x, align) = match flip_x {
        Some(w) => (w - margin, Align::Right),
        None => (margin, Align::Left),
    };
    if let Some(p) = crate::text::label_path(text, "Inter", 400.0, size, x, y, align) {
        use crate::canvas::{Canvas, Paint};
        c.fill(&p, &Paint::Solid(Rgba { r: 120, g: 120, b: 120, a: 1.0 }), Transform::identity(), FillRule::Winding);
    }
}

/// Print sheets: as many cards per sheet as fit (see `cb_core::impose`), crop marks round the
/// grid, and — when printing double-sided — each sheet's backs on the next page, placed to land
/// behind their fronts. Foil and spot UV come last as their own pages, for the print shop.
pub fn print_sheets_pdf(doc: &Doc, imgs: &ImageCache, progress: &dyn Fn(f32)) -> Result<(Vec<u8>, SheetSummary), String> {
    let layout = sheet_layout(doc);
    let per = layout.per_sheet();
    if per == 0 {
        let s = sheet_size(&doc.print.sheet);
        return Err(format!("the card doesn't fit on {} with {} mm margins — choose a bigger sheet or smaller margins", s.name, cb_core::format::trim_num(doc.print.margin)));
    }
    let jobs = print_jobs(doc, per);
    let sheets = jobs.len().div_ceil(per);
    let bleed = doc.print.bleed;
    let fmt = &doc.format;
    let mut pdf = PdfWriter::new("CardBuilder print sheets", doc.print.dpi.max(150) as f64);
    let mut modes = vec![Mode::Color];
    for m in [Mode::Foil, Mode::Uv] {
        if has(doc, Side::Front, m) || has(doc, Side::Back, m) {
            modes.push(m);
        }
    }
    let sides: Vec<Side> = if doc.print.duplex { vec![Side::Front, Side::Back] } else { vec![Side::Front] };
    // each distinct card side is drawn once and placed wherever it's needed
    let mut forms: HashMap<(Option<usize>, Side, u8), String> = HashMap::new();
    let distinct: Vec<Option<usize>> = {
        let mut d: Vec<Option<usize>> = Vec::new();
        for j in &jobs {
            if !d.contains(j) {
                d.push(*j);
            }
        }
        d
    };
    let total = (distinct.len() * sides.len() * modes.len()).max(1) as f32;
    let mut done = 0.0;
    for &mode in &modes {
        for &side in &sides {
            if mode != Mode::Color && !has(doc, side, mode) {
                continue;
            }
            for &rec in &distinct {
                let o = DrawOptions { record: rec, paper: false, mode, mask: Rgba::BLACK };
                let name = card_form(&mut pdf, doc, side, &o, bleed, imgs);
                forms.insert((rec, side, mode as u8), name);
                done += 1.0;
                progress(done / total);
            }
        }
    }
    let marks = if doc.print.crop_marks { layout.crop_marks() } else { vec![] };
    let flip = layout.flip();
    let sheet_name = sheet_size(&doc.print.sheet).name;
    for &mode in &modes {
        for s in 0..sheets {
            let cards = &jobs[s * per..((s + 1) * per).min(jobs.len())];
            for &side in &sides {
                if mode != Mode::Color && !has(doc, side, mode) {
                    continue;
                }
                let mut c = PdfCanvas::new(&mut pdf);
                for (i, rec) in cards.iter().enumerate() {
                    let Some(name) = forms.get(&(*rec, side, mode as u8)) else { continue };
                    let m = layout.place(i, side, fmt);
                    c.place(name, [m.a, m.b, m.c, m.d, m.e, m.f]);
                }
                let back = side == Side::Back;
                for (a, b) in &marks {
                    let (a, b) = if back { (flip.apply(a.0, a.1), flip.apply(b.0, b.1)) } else { (*a, *b) };
                    c.line(a, b, 0.09);
                }
                let what = if mode == Mode::Color { fmt.side_name(side).to_string() } else { format!("{} · {}", mode_name(mode), fmt.side_name(side)) };
                let text = format!("CardBuilder · {what} · sheet {} of {sheets} · {sheet_name} · {} per sheet{}", s + 1, per, if back { " · print on the reverse" } else { "" });
                caption(&mut c, &text, layout.sheet_h, doc.print.margin, None);
                let content = std::mem::take(&mut c.out);
                pdf.add_page(layout.sheet_w, layout.sheet_h, &content, None);
            }
        }
    }
    let pages = pdf.page_count();
    Ok((pdf.finish(), SheetSummary { sheets, per_sheet: per, cards: jobs.len(), pages }))
}
