//! The renderer end to end: every preset's starter design draws, QR codes scan, data fills in,
//! and the PDFs come out well-formed.

use cb_core::doc::{default_doc, Layer, QrKind, QrSpec, Side};
use cb_core::format::{preset, CardFormat, PRESETS};
use cb_print::{render_side, DrawOptions, ImageCache};

fn decode_qr(pm: &cb_print::tiny_skia::Pixmap) -> Vec<String> {
    let (w, h) = (pm.width() as usize, pm.height() as usize);
    let data = pm.data();
    let mut img = rqrr::PreparedImage::prepare_from_greyscale(w, h, |x, y| {
        let p = &data[(y * w + x) * 4..];
        // over white (premultiplied)
        let a = p[3] as u32;
        let lum = (p[0] as u32 * 299 + p[1] as u32 * 587 + p[2] as u32 * 114) / 1000 + (255 - a);
        lum.min(255) as u8
    });
    img.detect_grids().into_iter().filter_map(|g| g.decode().ok().map(|(_, s)| s)).collect()
}

#[test]
fn every_preset_renders_both_sides() {
    let imgs = ImageCache::default();
    for p in PRESETS {
        let doc = default_doc(CardFormat::from_preset(p));
        for side in [Side::Front, Side::Back] {
            let pm = render_side(&doc, side, &DrawOptions::preview(None), 4.0, true, &imgs).expect(p.id);
            let (sw, sh) = doc.format.sheet_with_bleed();
            assert_eq!((pm.width(), pm.height()), ((sw * 4.0).round() as u32, (sh * 4.0).round() as u32), "{}", p.id);
            // fully painted (paper or background everywhere)
            assert!(pm.data().chunks_exact(4).all(|px| px[3] == 255), "{} {side:?} has holes", p.id);
        }
    }
}

#[test]
fn qr_codes_scan_with_data_filled_in() {
    let mut doc = default_doc(CardFormat::from_preset(preset("bc-eu").unwrap()));
    doc.layers.retain(|l| l.side == Side::Front);
    doc.data = cb_core::data::sample();
    let spec = QrSpec { kind: QrKind::Vcard, name: "{Name}".into(), phone: "{Phone}".into(), email: "{Email}".into(), ..Default::default() };
    let mut l = Layer::qr(Side::Back, 42.5, 27.5, 40.0, spec, "#1f2347");
    l.fill = Some("#ffffff".into());
    doc.layers.push(l);
    let imgs = ImageCache::default();
    for rec in [0, 3] {
        let pm = render_side(&doc, Side::Back, &DrawOptions::preview(Some(rec)), 12.0, false, &imgs).unwrap();
        let found = decode_qr(&pm);
        assert_eq!(found.len(), 1, "record {rec}: {found:?}");
        let name = &doc.data.rows[rec][0];
        assert!(found[0].contains(&format!("FN:{name}")), "{}", found[0]);
        assert!(found[0].contains(&doc.data.rows[rec][2]));
    }
    // a plain website code
    let mut d2 = default_doc(CardFormat::default());
    d2.layers = vec![Layer::qr(Side::Front, 42.5, 27.5, 30.0, QrSpec::url("https://example.com/card?id=7"), "#000000")];
    let pm = render_side(&d2, Side::Front, &DrawOptions::preview(None), 12.0, false, &imgs).unwrap();
    assert_eq!(decode_qr(&pm), vec!["https://example.com/card?id=7".to_string()]);
}

#[test]
fn card_pdf_and_print_sheets() {
    let imgs = ImageCache::default();
    let mut doc = default_doc(CardFormat::from_preset(preset("bc-eu").unwrap()));
    let pdf = cb_print::sheets::cards_pdf(&doc, &[None], &imgs, &|_| {}).unwrap();
    assert!(pdf.starts_with(b"%PDF-1.4"));
    let count = |b: &[u8], n: &[u8]| b.windows(n.len()).filter(|w| *w == n).count();
    assert_eq!(count(&pdf, b"/Type /Page "), 2, "front and back");

    // 9 cards per A4 with bleed; 5 data rows → one sheet, front + back
    doc.data = cb_core::data::sample();
    let (pdf, s) = cb_print::sheets::print_sheets_pdf(&doc, &imgs, &|_| {}).unwrap();
    assert_eq!((s.sheets, s.per_sheet, s.cards, s.pages), (1, 9, 5, 2));
    assert_eq!(count(&pdf, b"/Type /Page "), 2);
    // each record's card is drawn once per side and placed once
    assert_eq!(count(&pdf, b"/Subtype /Form"), 10);

    // foil adds its own pages
    doc.layers[0].foil = Some(cb_core::doc::Foil::Gold);
    doc.print.duplex = false;
    let (_, s) = cb_print::sheets::print_sheets_pdf(&doc, &imgs, &|_| {}).unwrap();
    assert_eq!(s.pages, 2, "print + foil (front only)");

    // too big for the sheet
    doc.format.w = 400.0;
    assert!(cb_print::sheets::print_sheets_pdf(&doc, &imgs, &|_| {}).is_err());
}

#[test]
fn text_placeholders_follow_the_record() {
    let imgs = ImageCache::default();
    let mut doc = default_doc(CardFormat::default());
    doc.data = cb_core::data::sample();
    doc.layers = vec![Layer::text(Side::Front, 42.5, 27.5, 70.0, "{Name}", "Inter", 700.0, 14.0, "#000000", cb_core::doc::Align::Center)];
    let a = render_side(&doc, Side::Front, &DrawOptions::preview(Some(0)), 6.0, false, &imgs).unwrap();
    let b = render_side(&doc, Side::Front, &DrawOptions::preview(Some(1)), 6.0, false, &imgs).unwrap();
    let raw = render_side(&doc, Side::Front, &DrawOptions::preview(None), 6.0, false, &imgs).unwrap();
    assert_ne!(a.data(), b.data());
    assert_ne!(a.data(), raw.data());
}

#[test]
fn business_card_starter_fits_every_size() {
    // every business card size: no text has to shrink to fit its box, every box is on the card,
    // and the two-word names keep one size
    for p in PRESETS.iter().filter(|p| p.category == cb_core::format::Category::Business) {
        let d = default_doc(CardFormat::from_preset(p));
        let (w, h) = d.format.sheet();
        for l in d.layers.iter().filter(|l| l.kind == cb_core::LayerType::Text) {
            let text = l.text.clone().unwrap_or_default();
            if !l.wrap.unwrap_or(false) {
                let natural = cb_print::text::natural_width(l, &text);
                assert!(natural <= l.w + 1e-6, "{}: {text:?} is {natural:.2} mm in a {:.2} mm box", p.id, l.w);
            }
            assert!(l.x - l.w / 2.0 >= -1e-6 && l.x + l.w / 2.0 <= w + 1e-6, "{}: {text:?} box runs off the card", p.id);
            assert!(l.y > 0.0 && l.y < h, "{}: {text:?} off the card", p.id);
        }
        for (a, b) in [("COMPANY", "NAME"), ("JOHN", "DOE")] {
            for side in [Side::Front, Side::Back] {
                let find = |t: &str| d.layers.iter().find(|l| l.side == side && l.text.as_deref() == Some(t)).map(|l| l.size);
                if let (Some(x), Some(y)) = (find(a), find(b)) {
                    assert_eq!(x, y, "{}: {a} {b}", p.id);
                }
            }
        }
        let imgs = {
            let mut c = ImageCache::default();
            c.prepare(cb_print::image_srcs(&d));
            c
        };
        for side in [Side::Front, Side::Back] {
            let pm = render_side(&d, side, &DrawOptions::preview(None), 6.0, false, &imgs).unwrap();
            if let Ok(dir) = std::env::var("CB_SCREENSHOT_DIR") {
                std::fs::create_dir_all(&dir).unwrap();
                pm.save_png(format!("{dir}/biz-{}-{side:?}.png", p.id)).unwrap();
            }
        }
    }
}

#[test]
fn outlined_text_leaves_the_letters_open() {
    let mut d = default_doc(CardFormat::from_preset(preset("bc-eu").unwrap()));
    d.layers.clear();
    d.front = cb_core::doc::Background { mode: cb_core::doc::BgMode::Solid, color: "#ffffff".into(), ..Default::default() };
    let mut l = Layer::text(Side::Front, 42.5, 27.5, 60.0, "I", "Exo", 900.0, 150.0, "#00000000", cb_core::doc::Align::Center);
    l.stroke = Some("#000000".into());
    l.stroke_w = Some(0.4);
    d.layers.push(l.clone());
    let imgs = ImageCache::default();
    let dark = |d: &cb_core::Doc| {
        let pm = render_side(d, Side::Front, &DrawOptions::preview(None), 8.0, false, &imgs).unwrap();
        let px = pm.pixels();
        let mid = px[(pm.height() as usize / 2) * pm.width() as usize + pm.width() as usize / 2];
        (px.iter().filter(|p| p.red() < 100).count(), mid.red())
    };
    let (n, mid) = dark(&d);
    assert!(n > 200, "the outline shows ({n} dark pixels)");
    assert!(mid > 200, "the inside of the I stays open");
    // filled, no outline: the middle is ink
    d.layers[0].color = Some("#000000".into());
    d.layers[0].stroke_w = Some(0.0);
    assert!(dark(&d).1 < 50, "a filled I");
}

/// A design file rendered to PNGs (both sides, bleed included):
/// `CB_RENDER_FILE=card.cb CB_SCREENSHOT_DIR=out cargo test -p cb-print --test render -- --ignored render_a_file`
#[test]
#[ignore = "a tool: renders the file named by CB_RENDER_FILE"]
fn render_a_file() {
    let Ok(file) = std::env::var("CB_RENDER_FILE") else { return };
    let dir = std::env::var("CB_SCREENSHOT_DIR").unwrap_or_else(|_| ".".into());
    let ppm: f64 = std::env::var("CB_RENDER_PPM").ok().and_then(|v| v.parse().ok()).unwrap_or(600.0 / 25.4);
    let d = cb_core::doc::load_design(&std::fs::read_to_string(&file).unwrap()).unwrap();
    let mut imgs = ImageCache::default();
    imgs.prepare(cb_print::image_srcs(&d));
    let stem = std::path::Path::new(&file).file_stem().unwrap().to_string_lossy().to_string();
    for side in [Side::Front, Side::Back] {
        let pm = render_side(&d, side, &DrawOptions::preview(None), ppm, false, &imgs).unwrap();
        pm.save_png(format!("{dir}/{stem}-{side:?}.png")).unwrap();
    }
}
