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

/// an MCP client over plain HTTP, like an agent: POST one JSON-RPC message, the answer back
fn mcp_post(port: u16, body: serde_json::Value, sid: Option<&str>) -> (Option<String>, serde_json::Value) {
    use std::io::{Read as _, Write as _};
    let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).expect("CardBuilder listens");
    let b = body.to_string();
    let extra = sid.map(|x| format!("Mcp-Session-Id: {x}\r\n")).unwrap_or_default();
    write!(s, "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nAccept: application/json, text/event-stream\r\n{extra}Content-Length: {}\r\nConnection: close\r\n\r\n{b}", b.len()).unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).unwrap();
    let sid = out.lines().find_map(|l| l.strip_prefix("Mcp-Session-Id: ").map(str::to_string));
    let body = out.split("\r\n\r\n").nth(1).unwrap_or("");
    (sid, serde_json::from_str(body).unwrap_or(serde_json::Value::Null))
}

/// An agent connects over MCP and designs with the user: it reads the design, adds a layer
/// (the agent's pointer shows it), looks at the card, and asks before saving (the approval
/// card; the user allows it). Then Preferences › Assistant and the Assistant panel.
#[test]
#[ignore = "needs a GPU; run with --ignored"]
fn an_agent_designs_over_mcp() {
    use serde_json::json;
    let port = 28500 + (std::process::id() % 1000) as u16;
    let mut h = harness(1440.0, 900.0);
    open_preset(&mut h, "bc-eu");
    h.state_mut().enable_agents_for_tests(port);
    settle(&mut h);
    let saved = std::env::temp_dir().join(format!("cb-agent-{port}.cb"));
    let _ = std::fs::remove_file(&saved);
    let (tx, rx) = std::sync::mpsc::channel::<&'static str>();
    let path = saved.display().to_string();
    let agent = std::thread::spawn(move || {
        let call = |sid: &str, id: u64, name: &str, args: serde_json::Value| mcp_post(port, json!({"jsonrpc": "2.0", "id": id, "method": "tools/call", "params": {"name": name, "arguments": args}}), Some(sid)).1["result"].clone();
        let (sid, init) = mcp_post(port, json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18", "clientInfo": {"name": "claude-code", "version": "2.1.0"}, "capabilities": {}}}), None);
        assert!(init["result"]["instructions"].as_str().unwrap().contains("millimetres"));
        let sid = sid.expect("session");
        mcp_post(port, json!({"jsonrpc": "2.0", "method": "notifications/initialized"}), Some(&sid));
        let design = call(&sid, 2, "get_design", json!({}));
        let text = design["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("\"layers\""), "{text}");
        let added = call(&sid, 3, "add_layer", json!({"type": "text", "side": "front", "text": "Designed with Claude", "y": 14, "size": 9, "color": "#9b7bff"}));
        assert_eq!(added["isError"], false, "{added}");
        tx.send("added").unwrap();
        let id = serde_json::from_str::<serde_json::Value>(added["content"][0]["text"].as_str().unwrap()).unwrap()["added"].as_str().unwrap().to_string();
        let changed = call(&sid, 4, "update_layer", json!({"id": id, "set": {"weight": 800, "uppercase": true}}));
        assert_eq!(changed["isError"], false, "{changed}");
        let seen = call(&sid, 5, "render", json!({"side": "front", "dpi": 100}));
        assert_eq!(seen["content"][1]["type"], "image", "render gives a picture");
        // saving is a File tool: under Make edits, the user is asked
        tx.send("saving").unwrap();
        let save = call(&sid, 6, "save_design", json!({"path": path}));
        assert_eq!(save["isError"], false, "{save}");
        // an unknown layer is an error the agent can read
        let bad = call(&sid, 7, "delete_layer", json!({"id": "nope"}));
        assert_eq!(bad["isError"], true);
    });
    let mut approved = false;
    let mut shot_pointer = false;
    let t0 = std::time::Instant::now();
    while !agent.is_finished() {
        h.step();
        std::thread::sleep(std::time::Duration::from_millis(15));
        if let Ok(m) = rx.try_recv() {
            if m == "added" && !shot_pointer {
                // the pointer glides to the new layer
                for _ in 0..12 {
                    h.step();
                    std::thread::sleep(std::time::Duration::from_millis(30));
                }
                save(&mut h, "70-agent-pointer");
                shot_pointer = true;
            }
        }
        if !approved && h.state().agent_waiting_for_tests() {
            h.run_steps(3);
            save(&mut h, "71-agent-asks");
            h.get_by_label("Allow").click();
            approved = true;
        }
        assert!(t0.elapsed().as_secs() < 60, "the agent's calls didn't finish");
    }
    agent.join().expect("the agent's checks");
    assert!(approved, "saving asked first");
    let d = h.state().doc_for_tests();
    assert!(d.layers.iter().any(|l| l.text.as_deref() == Some("Designed with Claude") && l.weight == Some(800.0)));
    let back = cb_core::doc::load_design(&std::fs::read_to_string(&saved).expect("saved")).unwrap();
    assert_eq!(back.layers.len(), d.layers.len());
    let _ = std::fs::remove_file(&saved);
    // one Ctrl+Z takes the agent's last edit back
    let n = h.state().doc_for_tests().layers.iter().find(|l| l.text.as_deref() == Some("Designed with Claude")).unwrap().weight;
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
    h.run_steps(2);
    let after = h.state().doc_for_tests().layers.iter().find(|l| l.text.as_deref() == Some("Designed with Claude")).unwrap().weight;
    assert_ne!(n, after, "Ctrl+Z undid the agent's change");

    // the Assistant panel under its button
    h.get_by_label("Assistant: AI agents").click();
    h.event(egui::Event::PointerGone);
    settle(&mut h);
    save(&mut h, "72-assistant-panel");
    // Preferences › Assistant (Ctrl+,)
    h.key_press(egui::Key::Escape);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Comma);
    h.run_steps(3);
    // the page switch comes before the page's own heading of the same name
    h.get_all_by_label("Assistant").next().unwrap().click();
    h.event(egui::Event::PointerGone);
    settle(&mut h);
    save(&mut h, "73-preferences-assistant");
    h.get_all_by_label("Graphics card").next().unwrap().click();
    settle(&mut h);
    save(&mut h, "74-preferences-graphics");
}

/// Manual check with a real agent: the app (headless) lets agents connect on the default port
/// for CB_SERVE_SECONDS seconds, e.g. while `claude mcp get cardbuilder` runs.
#[test]
#[ignore = "manual check; set CB_SERVE_SECONDS"]
fn serve_agents_for_manual_checks() {
    let Some(secs) = std::env::var("CB_SERVE_SECONDS").ok().and_then(|s| s.parse::<u64>().ok()) else { return };
    let mut h = harness(1280.0, 800.0);
    open_preset(&mut h, "bc-eu");
    h.state_mut().enable_agents_for_tests(27879);
    let t0 = std::time::Instant::now();
    while t0.elapsed().as_secs() < secs {
        h.step();
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    if let Ok(dir) = std::env::var("CB_SCREENSHOT_DIR") {
        let _ = dir;
        save(&mut h, "80-after-a-real-agent");
    }
}

/// the README's Preferences pictures: Assistant (with Claude Code connected) and Graphics card
#[test]
#[ignore = "needs a GPU; run with --ignored"]
fn readme_preferences() {
    use serde_json::json;
    let mut h = harness(1600.0, 1000.0);
    let mut doc = cb_core::doc::default_doc(cb_core::format::CardFormat::from_preset(cb_core::format::preset("bc-round").unwrap()));
    doc.paper = cb_core::doc::Paper { stock: cb_core::doc::Stock::Gloss, color: "#ffffff".into() };
    h.state_mut().open_json("cards.cb", &cb_core::doc::save_cb(&doc, "test"));
    h.state_mut().enable_agents_for_tests(27879);
    settle(&mut h);
    // an agent connects and looks around
    let agent = std::thread::spawn(|| {
        let (sid, _) = mcp_post(27879, json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18", "clientInfo": {"name": "claude-code", "version": "2.1.328"}, "capabilities": {}}}), None);
        let sid = sid.unwrap();
        for (i, tool) in ["get_design", "list_fonts", "get_design"].into_iter().enumerate() {
            mcp_post(27879, json!({"jsonrpc": "2.0", "id": 2 + i, "method": "tools/call", "params": {"name": tool, "arguments": {}}}), Some(&sid));
        }
    });
    while !agent.is_finished() {
        h.step();
        std::thread::sleep(std::time::Duration::from_millis(15));
    }
    agent.join().unwrap();
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Comma);
    h.run_steps(3);
    h.get_all_by_label("Assistant").next().unwrap().click();
    h.event(egui::Event::PointerGone);
    settle(&mut h);
    save(&mut h, "readme-preferences-assistant");
    h.get_all_by_label("Graphics card").next().unwrap().click();
    h.event(egui::Event::PointerGone);
    settle(&mut h);
    save(&mut h, "readme-preferences-graphics");
}
