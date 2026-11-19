//! Design files and the "unsaved changes" state: opening a design and just looking at it never
//! counts as a change (the app asked "Save changes?" for nothing once), saving and reopening gives
//! back the same design, and a first start has nothing to save.

use cardbuilder::CardBuilderApp;
use egui_kittest::Harness;

fn harness() -> Harness<'static, CardBuilderApp> {
    let mut h = Harness::builder().with_size([1440.0, 900.0]).build_eframe(|cc| CardBuilderApp::new(cc));
    h.run_steps(4);
    h
}

#[test]
fn a_first_start_has_nothing_to_save() {
    let h = harness();
    assert!(!h.state().has_unsaved_changes());
}

#[test]
fn looking_at_a_design_changes_nothing() {
    let mut h = harness();
    for p in cb_core::format::PRESETS {
        let doc = cb_core::doc::default_doc(cb_core::format::CardFormat::from_preset(p));
        h.state_mut().open_json("test.cb", &cb_core::doc::save_cb(&doc, "test"));
        h.run_steps(6);
        let now = h.state().doc_for_tests().clone();
        if now != doc {
            let a = serde_json::to_value(&doc).unwrap();
            let b = serde_json::to_value(&now).unwrap();
            for k in a.as_object().unwrap().keys() {
                if a[k] != b[k] {
                    panic!("{}: opening changed `{k}`:\n{}\n→\n{}", p.id, a[k], b[k]);
                }
            }
        }
        assert!(!h.state().has_unsaved_changes(), "{}", p.id);
    }
}

#[test]
fn save_and_reopen() {
    let dir = std::env::temp_dir().join(format!("cardbuilder-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("cards.cb");
    let mut doc = cb_core::doc::default_doc(cb_core::format::CardFormat::from_preset(cb_core::format::preset("game-tcg").unwrap()));
    doc.data = cb_core::data::sample();
    std::fs::write(&path, cb_core::doc::save_cb(&doc, "test")).unwrap();
    let mut h = harness();
    assert!(h.state_mut().open_file(&path));
    h.run_steps(4);
    assert_eq!(h.state().doc_for_tests(), &doc);
    assert_eq!(h.state().file_name(), "cards.cb");
    assert!(!h.state().has_unsaved_changes());
    assert!(h.state_mut().save_design(0.0));
    let back = cb_core::doc::load_design(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(back, doc);
    std::fs::remove_dir_all(&dir).ok();
}
