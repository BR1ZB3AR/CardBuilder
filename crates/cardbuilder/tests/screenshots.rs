//! Renders the real app headlessly on the GPU (wgpu) and clicks through it like a user.
//! Needs a GPU, so it is opt-in:
//!   CB_SCREENSHOT_DIR=/some/dir cargo test -p cardbuilder --test screenshots -- --ignored --nocapture

use cardbuilder::CardBuilderApp;
use egui_kittest::kittest::Queryable as _;
use egui_kittest::Harness;

fn save(h: &mut Harness<'_, CardBuilderApp>, name: &str) {
    let img = h.render().expect("render");
    if let Ok(dir) = std::env::var("CB_SCREENSHOT_DIR") {
        std::fs::create_dir_all(&dir).unwrap();
        img.save(format!("{dir}/{name}.png")).unwrap();
    }
    assert!(img.width() > 0 && img.height() > 0);
}

/// let the background renderers finish and the view settle
fn settle(h: &mut Harness<'_, CardBuilderApp>) {
    for _ in 0..50 {
        h.step();
        std::thread::sleep(std::time::Duration::from_millis(30));
    }
}

fn harness(w: f32, h: f32) -> Harness<'static, CardBuilderApp> {
    Harness::builder().with_size([w, h]).wgpu().build_eframe(|cc| CardBuilderApp::new(cc))
}

fn open_preset(h: &mut Harness<'_, CardBuilderApp>, id: &str) {
    let doc = cb_core::doc::default_doc(cb_core::format::CardFormat::from_preset(cb_core::format::preset(id).unwrap()));
    h.state_mut().open_json("test.cb", &cb_core::doc::save_cb(&doc, "test"));
}

#[test]
#[ignore = "needs a GPU; run with --ignored"]
fn app_renders_and_responds() {
    let mut h = harness(1440.0, 900.0);
    settle(&mut h);
    assert!(h.state().editor_ready_for_tests(), "the card editor shows the rendered side");
    save(&mut h, "01-business-card");

    // the card pane's side switch (the 3D pane has a "Back" view button too)
    h.get_all_by_label("Back").next().unwrap().click();
    settle(&mut h);
    save(&mut h, "02-back");

    h.get_by_label("Data").click();
    h.run_steps(2);
    h.get_by_label("Sample data").click();
    settle(&mut h);
    save(&mut h, "03-data");

    h.get_by_label("Scene").click();
    h.run_steps(2);
    settle(&mut h);
    save(&mut h, "04-scene");
}

#[test]
#[ignore = "needs a GPU; run with --ignored"]
fn every_kind_of_card() {
    let mut h = harness(1440.0, 900.0);
    for (i, id) in ["greet-a6", "game-poker", "tent-place", "id-badge", "game-biocard-res"].into_iter().enumerate() {
        open_preset(&mut h, id);
        settle(&mut h);
        save(&mut h, &format!("1{i}-{id}"));
    }
    open_preset(&mut h, "greet-a6");
    h.state_mut().set_side_for_tests(true);
    h.state_mut().set_layout("3d");
    settle(&mut h);
    save(&mut h, "20-greeting-3d");
}

#[test]
#[ignore = "needs a GPU; run with --ignored"]
fn dialogs() {
    let mut h = harness(1440.0, 900.0);
    settle(&mut h);
    h.get_by_label("Print").click();
    settle(&mut h);
    save(&mut h, "30-print-dialog");
    h.key_press(egui::Key::Escape);
    h.run_steps(3);
    h.get_by_label("Card format").click();
    h.run_steps(5);
    save(&mut h, "31-format-picker");
    h.key_press(egui::Key::Escape);
    h.get_by_label("Theme").click();
    h.run_steps(5);
    save(&mut h, "32-themes");
}
