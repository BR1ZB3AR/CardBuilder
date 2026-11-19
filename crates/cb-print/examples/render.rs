//! Render starter designs to files for a visual check:
//!   cargo run -p cb-print --example render -- <out-dir> [preset-id …]

use cb_core::doc::{default_doc, Side};
use cb_core::format::{preset, CardFormat};
use cb_print::ImageCache;

fn main() {
    let mut args = std::env::args().skip(1);
    let out = std::path::PathBuf::from(args.next().unwrap_or_else(|| "cb-render".into()));
    std::fs::create_dir_all(&out).unwrap();
    let mut ids: Vec<String> = args.collect();
    if ids.is_empty() {
        ids = ["bc-eu", "greet-a6", "game-poker", "tent-place", "id-cr80"].iter().map(|s| s.to_string()).collect();
    }
    let mut imgs = ImageCache::default();
    for id in ids {
        // a preset id, or a .cb design file
        let mut doc = if id.ends_with(".cb") {
            cb_core::doc::load_design(&std::fs::read_to_string(&id).expect("read design")).expect("a CardBuilder design")
        } else {
            default_doc(CardFormat::from_preset(preset(&id).expect("preset id")))
        };
        let id = std::path::Path::new(&id).file_stem().unwrap().to_string_lossy().replace(' ', "-");
        imgs.prepare(cb_print::image_srcs(&doc));
        for side in [Side::Front, Side::Back] {
            let png = cb_print::sheets::side_png(&doc, side, None, 300.0, true, &imgs).unwrap();
            std::fs::write(out.join(format!("{id}-{side:?}.png").to_lowercase()), png).unwrap();
        }
        let pdf = cb_print::sheets::cards_pdf(&doc, &[None], &imgs, &|_| {}).unwrap();
        std::fs::write(out.join(format!("{id}-cards.pdf")), pdf).unwrap();
        doc.data = cb_core::data::sample();
        let (pdf, s) = cb_print::sheets::print_sheets_pdf(&doc, &imgs, &|_| {}).unwrap();
        std::fs::write(out.join(format!("{id}-sheets.pdf")), pdf).unwrap();
        println!("{id}: {s:?}");
    }
}
