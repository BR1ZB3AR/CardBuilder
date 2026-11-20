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

    // both sides are on the canvas; make the back the one the sidebar edits
    h.state_mut().set_side_for_tests(true);
    settle(&mut h);
    save(&mut h, "02-back-active");

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

/// the back of a flat card in 3D (the Back view and the flip)
#[test]
#[ignore = "needs a GPU; run with --ignored"]
fn back_in_3d() {
    let mut h = harness(1440.0, 900.0);
    open_preset(&mut h, "bc-round");
    h.state_mut().set_layout("3d");
    settle(&mut h);
    save(&mut h, "40-front-3d");
    h.get_by_label("Back").click();
    settle(&mut h);
    save(&mut h, "41-back-3d");
}

/// the README picture: a rounded business card on gloss stock, both sides in the editor
#[test]
#[ignore = "needs a GPU; run with --ignored"]
fn readme_picture() {
    let mut h = harness(1920.0, 1032.0);
    let mut doc = cb_core::doc::default_doc(cb_core::format::CardFormat::from_preset(cb_core::format::preset("bc-round").unwrap()));
    doc.paper = cb_core::doc::Paper { stock: cb_core::doc::Stock::Gloss, color: "#ffffff".into() };
    doc.scene.backdrop = cb_core::doc::Backdrop::Solid;
    doc.scene.bg_color = "#ffffff".into();
    doc.scene.light = 0.6;
    doc.scene.light_angle = -5.0;
    doc.scene.shadows = false;
    doc.scene.pair = false;
    h.state_mut().open_json("cards.cb", &cb_core::doc::save_cb(&doc, "test"));
    h.state_mut().set_layout("split");
    h.state_mut().set_side_for_tests(true);
    h.get_by_label("Scene").click();
    h.run_steps(2);
    // no mouse pointer in the picture
    h.event(egui::Event::PointerGone);
    settle(&mut h);
    save(&mut h, "readme");
}

/// the Ingress Biocard group in the picker, and a card from the local 2023 Origin template (when
/// it is installed on this computer)
#[test]
#[ignore = "needs a GPU; run with --ignored"]
fn ingress_biocard_group() {
    let mut h = harness(1440.0, 900.0);
    settle(&mut h);
    h.get_by_label("Card format").click();
    h.run_steps(3);
    h.get_by_label("Playing & game cards").click();
    h.run_steps(2);
    h.get_by_label("Ingress Biocard").click();
    h.run_steps(6);
    settle(&mut h); // the thumbnails render in the background
    save(&mut h, "50-biocard-group");
    // the classic Enlightened card (a local template when installed)
    h.get_by_label("Enlightened").click();
    h.run_steps(2);
    h.event(egui::Event::PointerGone);
    settle(&mut h);
    save(&mut h, "52-enlightened");
    h.get_by_label("Card format").click();
    h.run_steps(3);
    // the picker opens on the card's own group, still unfolded
    if h.query_by_label("2023 Origin").is_none() {
        h.get_by_label("Ingress Biocard").click();
        h.run_steps(6);
    }
    if h.query_by_label("2023 Origin").is_none() {
        return;
    }
    h.get_by_label("2023 Origin").click();
    h.run_steps(2);
    h.event(egui::Event::PointerGone);
    settle(&mut h);
    settle(&mut h);
    save(&mut h, "51-origin-template");
    // its photo sits in the window, clipped to it: a click in the window picks the photo
    if let Some(ph) = h.state().doc_for_tests().layers.iter().find(|l| l.clip && l.side == cb_core::Side::Front).cloned() {
        let p = h.state().card_point_for_tests(36.0, 40.0);
        h.event(egui::Event::PointerMoved(p));
        h.step();
        h.event(egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE });
        h.step();
        h.event(egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE });
        h.run_steps(3);
        h.event(egui::Event::PointerGone);
        settle(&mut h);
        save(&mut h, "51b-origin-photo-in-window");
        assert_eq!(h.state().selected_for_tests().map(|l| l.id.clone()), Some(ph.id.clone()), "the photo in the window is selected");
        h.key_press(egui::Key::Escape);
        h.run_steps(2);
    }
    // a 2024 card, when installed
    h.get_by_label("Card format").click();
    h.run_steps(3);
    if h.query_by_label("2024 Style 2").is_none() {
        h.get_by_label("Ingress Biocard").click();
        h.run_steps(6);
    }
    if h.query_by_label("2024 Style 2").is_none() {
        return;
    }
    h.get_by_label("2024 Style 2").click();
    h.run_steps(2);
    h.event(egui::Event::PointerGone);
    settle(&mut h);
    settle(&mut h);
    save(&mut h, "53-2024-style-2");
    // its number is a text layer: click it
    if let Some(n) = h.state().doc_for_tests().layers.iter().find(|l| l.name == "Number" && l.side == cb_core::Side::Front).cloned() {
        let p = h.state().card_point_for_tests(n.x, n.y + 6.0);
        h.event(egui::Event::PointerMoved(p));
        h.step();
        h.event(egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE });
        h.step();
        h.event(egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE });
        h.run_steps(3);
        h.event(egui::Event::PointerGone);
        settle(&mut h);
        save(&mut h, "53b-number-selected");
        assert_eq!(h.state().selected_for_tests().map(|l| l.id.clone()), Some(n.id.clone()), "the front number is selected");
        h.key_press(egui::Key::Escape);
        h.run_steps(2);
        // the back's number too (under the locked logo and artwork): click it, type a new one
        let b = h.state().doc_for_tests().layers.iter().find(|l| l.name == "Number" && l.side == cb_core::Side::Back).cloned().unwrap();
        h.state_mut().set_side_for_tests(true);
        let p = h.state().card_point_for_tests(b.x - 2.0, b.y + 6.0);
        h.state_mut().set_side_for_tests(false);
        for _ in 0..2 {
            // a double click: edit its text
            h.event(egui::Event::PointerMoved(p));
            h.step();
            h.event(egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE });
            h.step();
            h.event(egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE });
            h.step();
        }
        h.run_steps(3);
        assert_eq!(h.state().selected_for_tests().map(|l| l.id.clone()), Some(b.id.clone()), "the back number is selected");
        assert!(h.query_by_label_contains("Number · 2").is_some(), "the Layers list names it");
        // retype it in the sidebar's Text box (below the Layers list: scroll it into view)
        h.get_by_role_and_label(egui::accesskit::Role::TextInput, "Text").scroll_to_me();
        h.run_steps(3);
        h.get_by_role_and_label(egui::accesskit::Role::TextInput, "Text").click();
        h.run_steps(2);
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
        h.event(egui::Event::Text("7".into()));
        h.run_steps(3);
        assert_eq!(h.state().doc_for_tests().layer(&b.id).and_then(|l| l.text.clone()).as_deref(), Some("7"), "typed a new number");
        assert!(h.query_by_label_contains("Number · 7").is_some(), "still called Number");
        h.event(egui::Event::PointerGone);
        settle(&mut h);
        save(&mut h, "53c-back-number-7");
        // and the front number can go
        h.key_press(egui::Key::Escape);
        h.run_steps(2);
        h.state_mut().set_side_for_tests(false);
        let p = h.state().card_point_for_tests(n.x, n.y + 6.0);
        h.event(egui::Event::PointerMoved(p));
        h.step();
        h.event(egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE });
        h.step();
        h.event(egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE });
        h.run_steps(3);
        h.key_press(egui::Key::Delete);
        h.run_steps(3);
        assert!(
            h.state().doc_for_tests().layer(&n.id).is_none(),
            "the front number is deleted (selected {:?}, its text {:?})",
            h.state().selected_for_tests().map(|l| l.name.clone()),
            h.state().doc_for_tests().layer(&n.id).and_then(|l| l.text.clone())
        );
        h.event(egui::Event::PointerGone);
        settle(&mut h);
        save(&mut h, "53d-front-number-deleted");
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
        h.run_steps(3);
        assert!(h.state().doc_for_tests().layer(&n.id).is_some(), "Ctrl+Z brings it back");
    }
    // the artwork is vector shapes: click a triangle, give it another colour
    if let Some(t) = h.state().doc_for_tests().layers.iter().find(|l| l.name == "Triangle 2").cloned() {
        h.key_press(egui::Key::Escape);
        h.run_steps(2);
        h.state_mut().set_side_for_tests(false);
        let p = h.state().card_point_for_tests(t.x, t.y - t.h * 0.45);
        h.event(egui::Event::PointerMoved(p));
        h.step();
        h.event(egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE });
        h.step();
        h.event(egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE });
        h.run_steps(3);
        assert_eq!(h.state().selected_for_tests().map(|l| l.id.clone()), Some(t.id.clone()), "the triangle is selected");
        // the first swatch row is the shape's Fill colour (the Background section has one too)
        h.get_all_by_label("#ff7a59").next().unwrap().scroll_to_me();
        h.run_steps(3);
        h.get_all_by_label("#ff7a59").next().unwrap().click();
        h.run_steps(3);
        assert_eq!(h.state().doc_for_tests().layer(&t.id).and_then(|l| l.fill.clone()).as_deref(), Some("#ff7a59"), "recoloured");
        h.event(egui::Event::PointerGone);
        settle(&mut h);
        save(&mut h, "53e-triangle-recoloured");
        h.key_press(egui::Key::Escape);
        h.run_steps(2);
    }
    // its Faction buttons
    for (button, shot) in [("ENL", "54-2024-enl"), ("XFAC", "55-2024-xfac")] {
        h.get_by_label(button).scroll_to_me();
        h.run_steps(3);
        h.get_by_label(button).click();
        h.run_steps(2);
        // the back shows the faction's own dotted logo in its colour (XFAC: half of each, red)
        let d = h.state().doc_for_tests();
        let shown: Vec<(String, Option<String>)> = d.layers.iter().filter(|l| l.side == cb_core::Side::Back && l.name.contains("logo (dots)") && l.visible).map(|l| (l.name.clone(), l.fill.clone())).collect();
        let want = if button == "XFAC" { ("XFAC logo (dots)", "#ed1c24") } else { ("ENL logo (dots)", "#2bed1b") };
        assert_eq!(shown, vec![(want.0.to_string(), Some(want.1.to_string()))]);
        h.event(egui::Event::PointerGone);
        settle(&mut h);
        save(&mut h, shot);
    }
    // leaving the biocard for a business card starts a fresh business card
    h.get_by_label("Card format").click();
    h.run_steps(3);
    h.get_by_label("Business cards").click();
    h.run_steps(2);
    h.get_by_label("Europe").click();
    h.run_steps(2);
    // the faction was changed: Save changes? first
    h.get_by_label("Don't save").click();
    h.run_steps(2);
    h.event(egui::Event::PointerGone);
    settle(&mut h);
    save(&mut h, "56-back-to-business");
}

/// a detailed card in the 3D preview (the local 2024 biocard template when installed): fine
/// text and dots should stay sharp
#[test]
#[ignore = "needs a GPU; run with --ignored"]
fn detailed_card_in_3d() {
    let mut h = harness(1440.0, 900.0);
    let t = std::env::var("HOME").ok().map(|d| std::path::PathBuf::from(d).join(".local/share/cardbuilder/templates/Ingress Biocard 2024 Style 1.cb"));
    match t.and_then(|p| std::fs::read_to_string(p).ok()) {
        Some(json) => h.state_mut().open_json("biocard.cb", &json),
        None => open_preset(&mut h, "game-biocard-res"),
    }
    h.state_mut().set_layout("split");
    h.event(egui::Event::PointerGone);
    settle(&mut h);
    settle(&mut h);
    save(&mut h, "60-detailed-3d");
    // the 3D view on its own: the card bigger
    h.state_mut().set_layout("3d");
    settle(&mut h);
    save(&mut h, "61-detailed-3d-only");

    // the view gizmo (top right): an axis end looks along that axis, the house goes back home
    let near = |a: f64, b: f64| (a - b).rem_euclid(std::f64::consts::TAU).min((b - a).rem_euclid(std::f64::consts::TAU)) < 0.01;
    let start = h.state().camera_for_tests();
    h.get_by_label("Look from the right (X)").click();
    settle(&mut h);
    let (yaw, pitch, zoom) = h.state().camera_for_tests();
    assert!(near(yaw, std::f64::consts::FRAC_PI_2) && pitch.abs() < 0.01 && zoom == start.2, "right: {yaw} {pitch}");
    let g = h.get_by_label("Look from the right (X)").rect().center();
    h.event(egui::Event::PointerMoved(g));
    settle(&mut h);
    save(&mut h, "62-gizmo-right");
    h.get_by_label("Look from the top (Z)").click();
    h.event(egui::Event::PointerGone);
    settle(&mut h);
    let (_, pitch, _) = h.state().camera_for_tests();
    assert!(pitch > 1.5, "top: {pitch}");
    save(&mut h, "63-gizmo-top");
    h.get_by_label("Home view").click();
    h.event(egui::Event::PointerGone);
    settle(&mut h);
    let back_home = h.state().camera_for_tests();
    assert!(near(back_home.0, start.0) && (back_home.1 - start.1).abs() < 0.01 && back_home.2 == 1.0, "home: {back_home:?} vs {start:?}");
    save(&mut h, "64-gizmo-home");
}
