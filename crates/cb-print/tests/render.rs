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
