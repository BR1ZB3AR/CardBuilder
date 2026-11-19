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

/// the default business card's name, selected by clicking it on the card
fn select_name(h: &mut Harness<'_, CardBuilderApp>) -> cb_core::Layer {
    let l = h.state().doc_for_tests().layers.iter().find(|l| l.text.as_deref() == Some("Alex Morgan")).unwrap().clone();
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
