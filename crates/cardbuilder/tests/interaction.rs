//! The editor with a real mouse: click to select, drag to move (following the pointer exactly),
//! corners to resize, the knob to rotate, tools to place, undo; the top bar at every window
//! width; batch rows.

use cardbuilder::CardBuilderApp;
use egui::{vec2, Event, Modifiers, PointerButton, Pos2};
use egui_kittest::kittest::{NodeT, Queryable as _};
use egui_kittest::Harness;

fn app() -> Harness<'static, CardBuilderApp> {
    let mut h = Harness::builder().with_size([1440.0, 900.0]).build_eframe(|cc| CardBuilderApp::new(cc));
    h.state_mut().set_layout("card");
    h.run_steps(8);
    h
}

/// press at `from`, move to `to` in small steps like a hand does, release
fn drag(h: &mut Harness<'_, CardBuilderApp>, from: Pos2, to: Pos2, modifiers: Modifiers) {
    h.event(Event::PointerMoved(from));
    h.step();
    h.event(Event::PointerButton { pos: from, button: PointerButton::Primary, pressed: true, modifiers });
    h.step();
    for i in 1..=12 {
        let p = from + (to - from) * (i as f32 / 12.0);
        h.event(Event::PointerMoved(p));
        h.step();
    }
    h.event(Event::PointerButton { pos: to, button: PointerButton::Primary, pressed: false, modifiers });
    h.run_steps(4);
}

fn click(h: &mut Harness<'_, CardBuilderApp>, at: Pos2) {
    drag(h, at, at, Modifiers::NONE);
}

/// the default business card's company name (its bold word), selected by clicking it on the card
fn select_name(h: &mut Harness<'_, CardBuilderApp>) -> cb_core::Layer {
    let l = h.state().doc_for_tests().layers.iter().find(|l| l.side == cb_core::Side::Front && l.text.as_deref() == Some("COMPANY")).unwrap().clone();
    let p = h.state().card_point_for_tests(l.x, l.y);
    click(h, p);
    let sel = h.state().selected_for_tests().cloned().expect("clicking the name selects it");
    assert_eq!(sel.id, l.id);
    sel
}

#[test]
fn clicking_selects_and_dragging_moves_with_the_pointer() {
    let mut h = app();
    let before = select_name(&mut h);
    let c = h.state().card_point_for_tests(before.x, before.y);
    // Alt: no snapping, so the move is exactly the pointer's
    drag(&mut h, c, c + vec2(40.0, 30.0), Modifiers::ALT);
    let after = h.state().selected_for_tests().unwrap().clone();
    let c2 = h.state().card_point_for_tests(after.x, after.y);
    assert!(((c2 - c) - vec2(40.0, 30.0)).length() < 1.0, "moved {:?} for a (40, 30) px drag", c2 - c);
    assert_eq!(after.size, before.size, "moving changed the size");
    // one undo step puts it back
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::Z);
    h.run_steps(2);
    let back = h.state().doc_for_tests().layer(&before.id).unwrap();
    assert!((back.x - before.x).abs() < 1e-9 && (back.y - before.y).abs() < 1e-9);
}

#[test]
fn moving_snaps_to_the_centre_line() {
    let mut h = app();
    let l = select_name(&mut h);
    let fmt = h.state().doc_for_tests().format.clone();
    let c = h.state().card_point_for_tests(l.x, l.y);
    // move the box's centre to within a few px of the card's middle
    let target = h.state().card_point_for_tests(fmt.w / 2.0, l.y) + vec2(3.0, 0.0);
    drag(&mut h, c, target, Modifiers::NONE);
    let after = h.state().selected_for_tests().unwrap();
    assert!((after.x - fmt.w / 2.0).abs() < 1e-6, "snapped to x = {} (centre {})", after.x, fmt.w / 2.0);
}

#[test]
fn corner_scales_text_and_knob_rotates() {
    let mut h = app();
    let l = select_name(&mut h);
    let fill = |s: &str| s.to_string();
    let (w, hh) = cb_print::draw::layer_size(&l, &fill);
    // bottom-right corner, dragged outwards
    let (cx, cy) = cb_core::geom::from_local(w / 2.0, hh / 2.0, l.x, l.y, l.rot);
    let corner = h.state().card_point_for_tests(cx, cy);
    let c = h.state().card_point_for_tests(l.x, l.y);
    drag(&mut h, corner, corner + (corner - c).normalized() * 40.0, Modifiers::NONE);
    let after = h.state().selected_for_tests().unwrap().clone();
    assert!(after.size.unwrap() > l.size.unwrap() * 1.1, "font size {} → {:?}", l.size.unwrap(), after.size);
    // the knob sits above the top edge; drag it round to the right
    let (tx, ty) = cb_core::geom::from_local(0.0, -cb_print::draw::layer_size(&after, &fill).1 / 2.0, after.x, after.y, after.rot);
    let top = h.state().card_point_for_tests(tx, ty);
    let knob = top + vec2(0.0, -24.0);
    let c = h.state().card_point_for_tests(after.x, after.y);
    drag(&mut h, knob, c + vec2((knob - c).length(), 0.0), Modifiers::NONE);
    let rot = h.state().selected_for_tests().unwrap().rot;
    assert!((rot - 90.0).abs() < 1.0, "rotated to {rot}°");
}

#[test]
fn tools_place_artwork_where_clicked() {
    let mut h = app();
    let n = h.state().doc_for_tests().layers.len();
    h.get_by_label("QR code (Q) — click the card").click();
    h.run_steps(2);
    let at = h.state().card_point_for_tests(20.0, 40.0);
    click(&mut h, at);
    let doc = h.state().doc_for_tests();
    assert_eq!(doc.layers.len(), n + 1);
    let q = doc.layers.last().unwrap();
    assert_eq!(q.kind, cb_core::LayerType::Qr);
    assert!((q.x - 20.0).abs() < 0.5 && (q.y - 40.0).abs() < 0.5, "placed at {},{}", q.x, q.y);
    // the sidebar's Add › Text adds text and puts the keyboard in its text box
    h.get_by_label("Text").click();
    h.run_steps(3);
    assert_eq!(h.state().doc_for_tests().layers.len(), n + 2);
    assert_eq!(h.state().selected_for_tests().unwrap().kind, cb_core::LayerType::Text);
}

#[test]
fn top_bar_buttons_never_overlap() {
    for w in [980.0, 1024.0, 1100.0, 1180.0, 1280.0, 1366.0, 1440.0, 1920.0] {
        let mut h = Harness::builder().with_size([w, 700.0]).build_eframe(|cc| CardBuilderApp::new(cc));
        h.run_steps(8);
        let mut buttons = Vec::new();
        for n in h.root().children_recursive() {
            let a = n.accesskit_node();
            if a.role() != egui::accesskit::Role::Button {
                continue;
            }
            let (Some(label), Some(b)) = (a.label(), a.bounding_box()) else { continue };
            if b.y1 > 52.0 {
                continue;
            }
            buttons.push((label.to_string(), egui::Rect::from_min_max(egui::pos2(b.x0 as f32, b.y0 as f32), egui::pos2(b.x1 as f32, b.y1 as f32))));
        }
        assert!(buttons.len() >= 14, "{w}: only {} buttons: {:?}", buttons.len(), buttons.iter().map(|b| &b.0).collect::<Vec<_>>());
        for (i, (a, ra)) in buttons.iter().enumerate() {
            assert!(ra.left() >= 0.0 && ra.right() <= w, "{w}: {a} leaves the window");
            for (b, rb) in &buttons[i + 1..] {
                let o = ra.intersect(*rb);
                assert!(o.width() <= 0.5 || o.height() <= 0.5, "{w} px ({}): {a} {ra:?} overlaps {b} {rb:?}", h.state().top_bar_density());
            }
        }
    }
}

#[test]
fn batch_rows_fill_the_card() {
    let mut h = app();
    h.get_by_label("Data").click();
    h.run_steps(2);
    h.get_by_label("Sample data").click();
    h.run_steps(2);
    h.get_by_label("{Name}").click();
    h.run_steps(3);
    let doc = h.state().doc_for_tests();
    assert_eq!(doc.data.rows.len(), 5);
    assert!(doc.layers.iter().any(|l| l.text.as_deref() == Some("{Name}")), "the field went onto the card");
    h.get_by_label("Next row ( ] )").click();
    h.run_steps(2);
    assert!(h.get_all_by_label_contains("2 / 5").next().is_some());
}

#[test]
fn browsing_formats_and_coming_back_keeps_the_layout() {
    let mut h = app();
    let before = h.state().doc_for_tests().layers.clone();
    h.get_by_label("Card format").click();
    h.run_steps(3);
    h.get_by_label("Playing & game cards").click();
    h.run_steps(2);
    h.get_by_label("Poker").click();
    h.run_steps(2);
    h.get_by_label("Business cards").click();
    h.run_steps(2);
    h.get_by_label("Europe").click();
    h.run_steps(3);
    let after = &h.state().doc_for_tests().layers;
    assert_eq!(after.len(), before.len());
    for (a, b) in before.iter().zip(after) {
        assert!((a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9 && (a.w - b.w).abs() < 1e-9 && a.size == b.size, "{} changed: {:?} → {:?}", a.name, (a.x, a.y, a.w, a.size), (b.x, b.y, b.w, b.size));
    }
}

#[test]
fn both_sides_are_on_the_canvas() {
    let mut h = app();
    // the back's job title can be clicked without switching sides first
    let q = h.state().doc_for_tests().layers.iter().find(|l| l.text.as_deref() == Some("Creative Designer")).unwrap().clone();
    assert_eq!(q.side, cb_core::Side::Back);
    h.state_mut().set_side_for_tests(true);
    let p = h.state().card_point_for_tests(q.x, q.y);
    h.state_mut().set_side_for_tests(false);
    click(&mut h, p);
    assert_eq!(h.state().selected_for_tests().map(|l| l.id.clone()), Some(q.id.clone()), "clicking the back's title selects it");
    // and the sidebar now edits the back
    assert!(h.get_all_by_label_contains("Layers · Back").next().is_some());
}

#[test]
fn local_templates_show_in_their_group_and_start_a_card() {
    // a template folder of our own, with one template in the Ingress Biocard group
    let dir = std::env::temp_dir().join(format!("cb-templates-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut doc = cb_core::doc::default_doc(cb_core::format::CardFormat::from_preset(cb_core::format::preset("game-tcg").unwrap()));
    doc.layers[0].text = Some("FROM THE TEMPLATE".into());
    let info = cb_core::doc::TemplateInfo { id: "test-origin".into(), name: "Test Origin".into(), category: "game".into(), group: Some("Ingress Biocard".into()), note: "63 × 88 mm".into() };
    std::fs::write(dir.join("t.cb"), cb_core::doc::save_cb_template(&doc, "test", &info)).unwrap();
    // SAFETY: only this test reads the variable while it runs (others never open the picker's templates)
    unsafe { std::env::set_var("CB_TEMPLATE_DIR", &dir) };

    let mut h = app();
    h.get_by_label("Card format").click();
    h.run_steps(3);
    h.get_by_label("Playing & game cards").click();
    h.run_steps(2);
    // the group is closed until opened
    assert!(h.query_by_label("Test Origin").is_none());
    h.get_by_label("Ingress Biocard").click();
    h.run_steps(6); // the pop-up grows: egui settles its new size over a few frames
    assert!(h.query_by_label("Enlightened").is_some() && h.query_by_label("Resistance").is_some());
    h.get_by_label("Test Origin").click();
    h.run_steps(3);
    let d = h.state().doc_for_tests();
    assert_eq!(d.template.as_deref(), Some("test-origin"));
    assert_eq!(d.layers[0].text.as_deref(), Some("FROM THE TEMPLATE"));
    assert!(!h.state().has_unsaved_changes(), "a fresh card from a template has nothing to save yet");
    assert!(!h.state().top_bar_density().is_empty());
    // New starts again from the template
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::N);
    h.run_steps(3);
    assert_eq!(h.state().doc_for_tests().template.as_deref(), Some("test-origin"));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn faction_buttons_switch_logos_and_colour() {
    // a biocard-like design with a Faction choice: one logo layer per option, the name recoloured
    let mut doc = cb_core::doc::default_doc(cb_core::format::CardFormat::from_preset(cb_core::format::preset("game-poker").unwrap()));
    doc.layers.clear();
    for (id, on) in [("res", true), ("enl", false), ("xfac", false)] {
        let mut l = cb_core::Layer::new(id, cb_core::doc::LayerType::Shape, cb_core::Side::Back, 30.0, 40.0, 20.0, 20.0);
        l.id = id.into();
        l.visible = on;
        doc.layers.push(l);
    }
    let mut name = cb_core::Layer::new("Agent name", cb_core::doc::LayerType::Text, cb_core::Side::Back, 30.0, 10.0, 40.0, 8.0);
    name.id = "name".into();
    name.color = Some("#0097ca".into());
    doc.layers.push(name);
    let opt = |n: &str, c: &str, id: &str| cb_core::doc::VariantOption { name: n.into(), color: c.into(), show: vec![id.into()], recolor: vec![("name".into(), c.into())] };
    doc.variants = Some(cb_core::doc::Variants {
        label: "Faction".into(),
        hint: None,
        options: vec![opt("RES", "#0097ca", "res"), opt("ENL", "#23c316", "enl"), opt("XFAC", "#e0262b", "xfac")],
        active: Some(0),
    });
    let mut h = app();
    h.state_mut().open_json("bio.cb", &cb_core::doc::save_cb(&doc, "test"));
    h.run_steps(4);
    let shown = |h: &Harness<'_, CardBuilderApp>| {
        h.state().doc_for_tests().layers.iter().filter(|l| l.visible && l.id != "name").map(|l| l.id.clone()).collect::<Vec<_>>()
    };
    let colour = |h: &Harness<'_, CardBuilderApp>| h.state().doc_for_tests().layers.iter().find(|l| l.id == "name").unwrap().color.clone().unwrap();
    assert_eq!(shown(&h), ["res"]);
    h.get_by_label("ENL").click();
    h.run_steps(3);
    assert_eq!(shown(&h), ["enl"]);
    assert_eq!(colour(&h), "#23c316");
    h.get_by_label("XFAC").click();
    h.run_steps(3);
    assert_eq!(shown(&h), ["xfac"]);
    assert_eq!(colour(&h), "#e0262b");
    // one undo step per press
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
    h.run_steps(3);
    assert_eq!(shown(&h), ["enl"]);
}

#[test]
fn leaving_a_card_with_a_look_of_its_own_starts_a_fresh_card() {
    // the built-in Enlightened biocard has its own design; picking a business card starts afresh
    let bio = cb_core::doc::default_doc(cb_core::format::CardFormat::from_preset(cb_core::format::preset("game-biocard-enl").unwrap()));
    let mut h = app();
    h.state_mut().open_json("bio.cb", &cb_core::doc::save_cb(&bio, "test"));
    h.run_steps(4);
    h.get_by_label("Card format").click();
    h.run_steps(3);
    h.get_by_label("Business cards").click();
    h.run_steps(2);
    h.get_by_label("Europe").click();
    h.run_steps(4);
    let d = h.state().doc_for_tests();
    assert_eq!(d.format.category(), cb_core::format::Category::Business);
    let texts: Vec<&str> = d.layers.iter().filter_map(|l| l.text.as_deref()).collect();
    assert!(texts.contains(&"COMPANY") && texts.contains(&"Creative Designer"), "the business card's own design: {texts:?}");
    assert!(d.layers.len() < bio.layers.len() + 20 && !d.layers.iter().any(|l| bio.layers.iter().any(|b| b.id == l.id)), "nothing of the biocard is left");
    assert!(d.variants.is_none() && d.template.is_none());
    assert!(!h.state().has_unsaved_changes(), "a fresh card has nothing to save yet");
    // Ctrl+Z brings the biocard back
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::Z);
    h.run_steps(3);
    assert_eq!(h.state().doc_for_tests().format.preset, "game-biocard-enl");
}

#[test]
fn leaving_an_edited_card_asks_to_save_first() {
    let bio = cb_core::doc::default_doc(cb_core::format::CardFormat::from_preset(cb_core::format::preset("game-biocard-enl").unwrap()));
    let mut h = app();
    h.state_mut().open_json("bio.cb", &cb_core::doc::save_cb(&bio, "test"));
    h.run_steps(4);
    // an edit: move the first layer with the keyboard
    let l = select_name_like(&mut h);
    h.key_press(egui::Key::ArrowRight);
    h.run_steps(2);
    assert!(h.state().has_unsaved_changes(), "edited {}", l.name);
    h.get_by_label("Card format").click();
    h.run_steps(3);
    h.get_by_label("Business cards").click();
    h.run_steps(2);
    h.get_by_label("Europe").click();
    h.run_steps(4);
    assert!(h.query_by_label("Don't save").is_some(), "Save changes? is asked");
    assert_eq!(h.state().doc_for_tests().format.preset, "game-biocard-enl", "nothing changes before the answer");
    h.get_by_label("Don't save").click();
    h.run_steps(4);
    assert_eq!(h.state().doc_for_tests().format.category(), cb_core::format::Category::Business);
}

/// any text layer on the front, selected by clicking it
fn select_name_like(h: &mut Harness<'_, CardBuilderApp>) -> cb_core::Layer {
    let l = h.state().doc_for_tests().layers.iter().find(|l| l.side == cb_core::Side::Front && l.kind == cb_core::LayerType::Text).unwrap().clone();
    let p = h.state().card_point_for_tests(l.x, l.y);
    click(h, p);
    h.state().selected_for_tests().cloned().expect("clicking a text selects it")
}

#[test]
fn an_outlined_number_can_be_edited_and_deleted() {
    // a biocard-like number: outline only, no fill
    let mut doc = cb_core::doc::default_doc(cb_core::format::CardFormat::from_preset(cb_core::format::preset("game-poker").unwrap()));
    doc.layers.clear();
    let mut num = cb_core::Layer::text(cb_core::Side::Front, 31.75, 60.0, 30.0, "1", "Exo", 900.0, 120.0, "#ffffff00", cb_core::doc::Align::Center);
    num.stroke = Some("#ffffff".into());
    num.stroke_w = Some(0.25);
    let id = num.id.clone();
    doc.layers.push(num.clone());
    let mut h = app();
    h.state_mut().open_json("bio.cb", &cb_core::doc::save_cb(&doc, "test"));
    h.run_steps(4);
    let p = h.state().card_point_for_tests(num.x, num.y);
    click(&mut h, p);
    assert_eq!(h.state().selected_for_tests().map(|l| l.id.clone()), Some(id.clone()), "clicking the number selects it");
    // its outline shows in the sidebar, and turning the fill on keeps the outline
    assert!(h.query_by_label("Outline width").is_some() || h.query_by_label_contains("Outline width").is_some());
    h.get_by_label("Fill").click();
    h.run_steps(3);
    let l = h.state().doc_for_tests().layer(&id).unwrap().clone();
    assert_eq!(l.color.as_deref(), Some("#ffffff"), "filled");
    assert_eq!(l.stroke_w, Some(0.25));
    // Delete removes it
    click(&mut h, p);
    h.key_press(egui::Key::Delete);
    h.run_steps(3);
    assert!(h.state().doc_for_tests().layer(&id).is_none(), "deleted");
}

#[test]
fn vector_artwork_is_picked_where_it_is_painted_and_restyled() {
    use cb_core::doc::{Dash, LayerType, VectorPath};
    // a red "photo" under a black frame with a window, and a picture clipped to the frame
    let mut doc = cb_core::doc::default_doc(cb_core::format::CardFormat::from_preset(cb_core::format::preset("game-poker").unwrap()));
    doc.layers.clear();
    let mut photo = cb_core::Layer::rect(cb_core::Side::Front, 31.75, 44.45, 63.5, 88.9, "#ff0000");
    photo.name = "Photo".into();
    let frame = cb_core::Layer::vector(cb_core::Side::Front, 31.75, 44.45, 63.5, 88.9, VectorPath { d: "M0 0H100V100H0Z M20 20H80V80H20Z".into(), w: 100.0, h: 100.0, even_odd: true }, Some("#000000"));
    let mut glow = cb_core::Layer::new("Glow", LayerType::Image, cb_core::Side::Front, 31.75, 44.45, 63.5, 88.9);
    glow.clip = true;
    glow.visible = false;
    let (photo_id, frame_id, glow_id) = (photo.id.clone(), frame.id.clone(), glow.id.clone());
    doc.layers = vec![photo, frame, glow];
    let mut h = app();
    h.state_mut().open_json("frame.cb", &cb_core::doc::save_cb(&doc, "test"));
    h.run_steps(4);
    // on the frame: the frame
    let on_frame = h.state().card_point_for_tests(4.0, 44.45);
    click(&mut h, on_frame);
    assert_eq!(h.state().selected_for_tests().map(|l| l.id.clone()), Some(frame_id.clone()), "a click on the frame picks it");
    assert!(h.query_all_by_label("Vector shape").count() >= 2, "the sidebar calls it a vector shape (its section and its row)");
    // give it a dotted outline
    h.get_by_label("Outline").click();
    h.run_steps(3);
    h.get_by_label("Dotted").click();
    h.run_steps(3);
    let f = h.state().doc_for_tests().layer(&frame_id).unwrap().clone();
    assert!(f.stroke.is_some() && f.stroke_w.unwrap_or(0.0) > 0.0, "outlined");
    assert_eq!(f.dash, Some(Dash::Dotted));
    assert_eq!(f.fill.as_deref(), Some("#000000"), "still filled");
    // in the window: the click goes through to the photo below
    let in_window = h.state().card_point_for_tests(31.75, 44.45);
    click(&mut h, in_window);
    assert_eq!(h.state().selected_for_tests().map(|l| l.id.clone()), Some(photo_id), "a click in the window picks what shows there");
    // the clipped picture keeps its setting through a save
    let back = cb_core::doc::load_design(&cb_core::doc::save_cb(h.state().doc_for_tests(), "test")).unwrap();
    assert!(back.layer(&glow_id).unwrap().clip);
}

#[test]
fn a_click_never_nudges_a_layer_onto_a_guide() {
    // a picture whose top edge is a hair inside the bleed edge, its centre near the middle line
    let mut doc = cb_core::doc::default_doc(cb_core::format::CardFormat::from_preset(cb_core::format::preset("game-poker").unwrap()));
    doc.layers.clear();
    let mut pic = cb_core::Layer::rect(cb_core::Side::Front, 31.9, 40.0, 40.0, 85.998, "#3366cc");
    pic.y = -2.999 + pic.h / 2.0;
    let before = pic.clone();
    doc.layers.push(pic);
    let mut h = app();
    h.state_mut().open_json("pic.cb", &cb_core::doc::save_cb(&doc, "test"));
    h.run_steps(4);
    let p = h.state().card_point_for_tests(before.x, 30.0);
    click(&mut h, p);
    assert_eq!(h.state().selected_for_tests().map(|l| l.id.clone()), Some(before.id.clone()));
    assert_eq!(h.state().doc_for_tests().layer(&before.id).unwrap(), &before, "selected, not moved");
    // a real drag still moves it, snapping as before
    drag(&mut h, p, p + vec2(40.0, 0.0), Modifiers::NONE);
    assert!(h.state().doc_for_tests().layer(&before.id).unwrap().x > before.x + 1.0);
}
