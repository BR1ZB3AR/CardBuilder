//! The tools agents get, and what each does to the open design. A tool is Read (looks only),
//! Edit (changes the design: one Ctrl+Z undoes it) or File (opens, saves, exports or replaces
//! the design), which decides when the user is asked first (`needs_asking`).
//!
//! Layers travel as the same JSON as in a .cb file (camelCase fields), with big pictures and
//! long outlines left out ("(kept: …)" placeholders, which updates leave alone).

use super::mcp::Outcome;
use crate::app::CardBuilderApp;
use base64::Engine as _;
use cb_core::doc::{Align, Background, BgMode, Layer, LayerType, QrSpec, ShapeKind, Side};
use cb_core::format::{preset, CardFormat, PRESETS};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Read,
    Edit,
    File,
}

struct Tool {
    name: &'static str,
    title: &'static str,
    kind: Kind,
    description: &'static str,
}

const TOOLS: &[Tool] = &[
    Tool { name: "get_design", title: "Read the design", kind: Kind::Read, description: "The design open in CardBuilder: the card (size, fold, bleed, safe area, panels), paper, both backgrounds, every layer in drawing order (bottom first, each with its id), the design's choice buttons (variants), its data columns and the selected layer. Start here." },
    Tool { name: "render", title: "Look at the card", kind: Kind::Read, description: "A picture (PNG) of the front, the back or both, as printed: look at it after changing things. dpi 30–300 (default 150); bleed true includes the bleed area; row picks a data row for {Column} fields." },
    Tool { name: "list_card_formats", title: "Card sizes", kind: Kind::Read, description: "Every ready-made card size (business, ID, game, postcard, greeting, tent, bookmark …) with its id, for new_design and set_card." },
    Tool { name: "list_fonts", title: "Fonts", kind: Kind::Read, description: "The fonts text layers can use: the bundled families and any the design carries." },
    Tool { name: "add_layer", title: "Add a layer", kind: Kind::Edit, description: "Add a layer: type text, shape, qr or image, on side front or back (outside / inside on folded cards). Position (x, y = centre) and size (w, h) in mm default to the middle of the side. Any layer field can be given too: text, font, weight (100–900), size (pt), color, align (left / center / right), uppercase, letterSpacing, lineHeight; shape (rect / ellipse / line / path), fill, stroke, strokeW, radius, dash (dotted / dashed), path {d, w, h, evenOdd}; qr {kind: url / text / phone / email / wifi / vcard, text, …}; opacity, rot, clip, foil (gold / silver / copper / rosegold / holo), spotUv. An image comes from image_path (a file on this computer) or src (a data URL). Answers with the new layer and its id." },
    Tool { name: "update_layer", title: "Change a layer", kind: Kind::Edit, description: "Change fields of the layer with this id: put them in set (only those change; null removes an optional one). image_path replaces an image layer's picture, keeping its width." },
    Tool { name: "delete_layer", title: "Delete layers", kind: Kind::Edit, description: "Delete the layer with this id (or every id in ids)." },
    Tool { name: "arrange_layer", title: "Arrange a layer", kind: Kind::Edit, description: "Move a layer up or down the stack on its side: to = front (on top), back (at the bottom), forward or backward (one step)." },
    Tool { name: "set_background", title: "Change a background", kind: Kind::Edit, description: "The background of a side: mode paper (nothing printed), solid (color), gradient (color to color2 at angle degrees, 0 = left to right, 90 = top to bottom) or image (image_path or src, covering the side and its bleed)." },
    Tool { name: "set_card", title: "Change the card", kind: Kind::Edit, description: "Change the card: preset (an id from list_card_formats) or w, h, radius, fold (none / side / top), bleed, safe in mm; orientation portrait / landscape; paper {stock: matte / silk / gloss / softtouch / linen / kraft / pearl / black / plastic, color}. Artwork is scaled to the new size." },
    Tool { name: "choose_variant", title: "Pick an option", kind: Kind::Edit, description: "Pick one of the design's choice buttons (variants), such as a biocard's faction, by its name or number: it shows that option's layers and sets its colours." },
    Tool { name: "set_data", title: "Set the card data", kind: Kind::Edit, description: "Replace the design's data with this CSV text (first line = column names). Text and QR codes with {Column} fields then make one card per row." },
    Tool { name: "undo", title: "Undo", kind: Kind::Edit, description: "Undo the last change (yours or the user's)." },
    Tool { name: "redo", title: "Redo", kind: Kind::Edit, description: "Redo the last undone change." },
    Tool { name: "new_design", title: "Start a new design", kind: Kind::File, description: "Replace the design with a fresh one: preset (an id from list_card_formats) or a custom w and h in mm (fold none / side / top, radius). The user can bring the old design back with Ctrl+Z." },
    Tool { name: "open_design", title: "Open a design", kind: Kind::File, description: "Open a CardBuilder design (.cb) from this computer, replacing the one on screen (Ctrl+Z brings it back)." },
    Tool { name: "save_design", title: "Save the design", kind: Kind::File, description: "Save the design as a .cb file: to path, or into the file it came from." },
    Tool { name: "export", title: "Export", kind: Kind::File, description: "Make a file for printing or sharing at path: kind card_pdf (one page per side, bleed and trim boxes), print_sheets (many cards per page with crop marks), png_front / png_back (300 dpi with bleed) or csv (the data)." },
];

pub fn kind(name: &str) -> Option<Kind> {
    TOOLS.iter().find(|t| t.name == name).map(|t| t.kind)
}

/// a tool's name as people read it ("Add a layer")
pub fn title(name: &str) -> &str {
    TOOLS.iter().find(|t| t.name == name).map(|t| t.title).unwrap_or(name)
}

fn schema(name: &str) -> Value {
    let side = json!({"type": "string", "enum": ["front", "back", "outside", "inside"], "description": "front / back (outside / inside on folded cards)"});
    let mm = |d: &str| json!({"type": "number", "description": d});
    let obj = |props: Value, required: &[&str]| json!({"type": "object", "properties": props, "required": required});
    match name {
        "render" => obj(json!({"side": {"type": "string", "enum": ["front", "back", "both", "outside", "inside"]}, "dpi": {"type": "number", "minimum": 30, "maximum": 300}, "bleed": {"type": "boolean"}, "row": {"type": "integer", "minimum": 0}}), &[]),
        "add_layer" => json!({"type": "object", "properties": {
            "type": {"type": "string", "enum": ["text", "shape", "qr", "image"]},
            "side": side, "x": mm("centre, mm from the trim's left edge"), "y": mm("centre, mm from the trim's top edge"), "w": mm("width, mm"), "h": mm("height, mm"),
            "text": {"type": "string"}, "name": {"type": "string"}, "image_path": {"type": "string", "description": "a picture file on this computer"}, "src": {"type": "string", "description": "a picture as a data URL"}
        }, "required": ["type"], "additionalProperties": true}),
        "update_layer" => obj(json!({"id": {"type": "string"}, "set": {"type": "object", "description": "the fields to change", "additionalProperties": true}, "image_path": {"type": "string"}}), &["id"]),
        "delete_layer" => obj(json!({"id": {"type": "string"}, "ids": {"type": "array", "items": {"type": "string"}}}), &[]),
        "arrange_layer" => obj(json!({"id": {"type": "string"}, "to": {"type": "string", "enum": ["front", "back", "forward", "backward"]}}), &["id", "to"]),
        "set_background" => obj(json!({"side": side, "mode": {"type": "string", "enum": ["paper", "solid", "gradient", "image"]}, "color": {"type": "string"}, "color2": {"type": "string"}, "angle": {"type": "number"}, "image_path": {"type": "string"}, "src": {"type": "string"}}), &["side"]),
        "set_card" => obj(json!({"preset": {"type": "string"}, "w": mm("finished width"), "h": mm("finished height"), "radius": mm("corner radius"), "fold": {"type": "string", "enum": ["none", "side", "top"]}, "bleed": mm("bleed"), "safe": mm("safe area inset"), "orientation": {"type": "string", "enum": ["portrait", "landscape"]}, "paper": {"type": "object", "properties": {"stock": {"type": "string"}, "color": {"type": "string"}}}}), &[]),
        "choose_variant" => obj(json!({"option": {"type": ["string", "integer"], "description": "the option's name (\"ENL\") or number from 0"}}), &["option"]),
        "set_data" => obj(json!({"csv": {"type": "string"}, "name": {"type": "string"}}), &["csv"]),
        "list_card_formats" => obj(json!({"category": {"type": "string", "description": "only this kind of card (business, id, game, postcard, greeting, tent, bookmark …)"}}), &[]),
        "new_design" => obj(json!({"preset": {"type": "string"}, "w": mm("finished width"), "h": mm("finished height"), "fold": {"type": "string", "enum": ["none", "side", "top"]}, "radius": mm("corner radius")}), &[]),
        "open_design" => obj(json!({"path": {"type": "string", "description": "a .cb file on this computer"}}), &["path"]),
        "save_design" => obj(json!({"path": {"type": "string", "description": "where to save (.cb); leave out to save into the design's own file"}}), &[]),
        "export" => obj(json!({"kind": {"type": "string", "enum": ["card_pdf", "print_sheets", "png_front", "png_back", "csv"]}, "path": {"type": "string"}}), &["kind", "path"]),
        _ => obj(json!({}), &[]),
    }
}

/// every tool, as MCP's tools/list gives them
pub fn list() -> Vec<Value> {
    TOOLS
        .iter()
        .map(|t| {
            json!({
                "name": t.name,
                "title": t.title,
                "description": t.description,
                "inputSchema": schema(t.name),
                "annotations": {"title": t.title, "readOnlyHint": t.kind == Kind::Read, "destructiveHint": t.kind == Kind::File, "idempotentHint": false, "openWorldHint": false},
            })
        })
        .collect()
}

/// what agents are told when they connect (CardBuilder's own words, then the user's)
pub fn instructions(custom: &str) -> String {
    let mut s = String::from(
        "CardBuilder is a card designer (business cards, ID and game cards, postcards, folded greeting cards …) and you are working on the design open in it, live, while the user watches.\n\
         - Lengths are millimetres. A layer's x and y are its centre, measured from the top-left corner of the trim (of the flat sheet on folded cards). Text size is in points; colours are #rrggbb.\n\
         - Start with get_design: it lists the card, both sides and every layer with its id. Use render to look at a side after changing it.\n\
         - add_layer makes layers; update_layer changes only the fields you send. Later layers are drawn on top.\n\
         - Keep text inside the safe area; artwork meant to reach the edge of the card must run into the bleed.\n\
         - Every change you make is one Ctrl+Z away for the user. Opening, saving and exporting may ask the user first; if they say no, don't try again unless they ask.",
    );
    let custom = custom.trim();
    if !custom.is_empty() {
        s.push_str("\n\nThe user's own instructions:\n");
        s.push_str(custom);
    }
    s
}

/// a tool's result before it becomes MCP content
pub struct ToolOut {
    pub text: String,
    pub images: Vec<Vec<u8>>,
    pub is_error: bool,
    /// where on the card it happened (side, x, y) and the layer, for the agent's pointer
    pub focus: Option<(Side, f64, f64, Option<String>)>,
}

impl ToolOut {
    fn fail(msg: impl Into<String>) -> ToolOut {
        ToolOut { text: msg.into(), images: vec![], is_error: true, focus: None }
    }
    pub fn into_outcome(self) -> Outcome {
        let mut content = vec![json!({"type": "text", "text": self.text})];
        for png in self.images {
            content.push(json!({"type": "image", "data": base64::engine::general_purpose::STANDARD.encode(png), "mimeType": "image/png"}));
        }
        Outcome { content, is_error: self.is_error }
    }
}

const KEPT: &str = "(kept: ";

/// a JSON merge patch (RFC 7386) that leaves "(kept: …)" placeholders alone
fn merge(dst: &mut Value, src: &Value) {
    match (dst, src) {
        (Value::Object(d), Value::Object(s)) => {
            for (k, v) in s {
                if v.as_str().is_some_and(|t| t.starts_with(KEPT)) {
                    continue;
                }
                if v.is_null() {
                    d.remove(k);
                } else if v.is_object() && d.get(k).is_some_and(Value::is_object) {
                    merge(d.get_mut(k).unwrap(), v);
                } else {
                    d.insert(k.clone(), v.clone());
                }
            }
        }
        (d, s) => *d = s.clone(),
    }
}

/// `v` with `set` merged in (an error says which field didn't fit)
fn patched<T: Serialize + DeserializeOwned>(v: &T, set: &Value) -> Result<T, String> {
    let mut j = serde_json::to_value(v).map_err(|e| e.to_string())?;
    merge(&mut j, set);
    serde_json::from_value(j).map_err(|e| format!("that doesn't fit: {e}"))
}

/// a layer as agents see it: big pictures and long outlines left out
fn layer_json(l: &Layer, compact: bool) -> Value {
    let mut v = serde_json::to_value(l).unwrap_or(Value::Null);
    if let Some(src) = v.get("src").and_then(Value::as_str) {
        let kb = src.len() * 3 / 4 / 1024;
        v["src"] = json!(format!("{KEPT}picture, {kb} KB)"));
    }
    if let Some(d) = v.pointer("/path/d").and_then(Value::as_str) {
        if d.len() > 1500 {
            let n = d.len();
            v["path"]["d"] = json!(format!("{KEPT}outline, {n} characters)"));
        }
    }
    if compact {
        if let Some(o) = v.as_object_mut() {
            if o.get("rot").and_then(Value::as_f64) == Some(0.0) {
                o.remove("rot");
            }
            if o.get("opacity").and_then(Value::as_f64) == Some(1.0) {
                o.remove("opacity");
            }
            if o.get("visible").and_then(Value::as_bool) == Some(true) {
                o.remove("visible");
            }
        }
    }
    v
}

fn bg_json(b: &Background) -> Value {
    let mut v = serde_json::to_value(b).unwrap_or(Value::Null);
    if let Some(src) = v.get("src").and_then(Value::as_str) {
        let kb = src.len() * 3 / 4 / 1024;
        v["src"] = json!(format!("{KEPT}picture, {kb} KB)"));
    }
    v
}

fn side_of(v: Option<&Value>, default: Side) -> Result<Side, String> {
    match v.and_then(Value::as_str).map(str::to_ascii_lowercase).as_deref() {
        None => Ok(default),
        Some("front" | "outside") => Ok(Side::Front),
        Some("back" | "inside") => Ok(Side::Back),
        Some(s) => Err(format!("side is front or back (outside or inside on folded cards), not {s}")),
    }
}

fn num(args: &Value, k: &str) -> Option<f64> {
    args.get(k).and_then(Value::as_f64)
}

fn path_arg(args: &Value, k: &str) -> Option<std::path::PathBuf> {
    let p = args.get(k).and_then(Value::as_str)?.trim();
    if p.is_empty() {
        return None;
    }
    Some(match (p.strip_prefix("~/"), std::env::var_os("HOME")) {
        (Some(rest), Some(home)) => std::path::PathBuf::from(home).join(rest),
        _ => std::path::PathBuf::from(p),
    })
}

/// a picture from image_path or a data URL in src: (data URL, width px, height px)
fn picture(args: &Value) -> Result<Option<(String, f64, f64)>, String> {
    if let Some(p) = path_arg(args, "image_path") {
        let bytes = std::fs::read(&p).map_err(|e| format!("couldn't read {}: {e}", p.display()))?;
        let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        return crate::exports::image_data_url(&name, &bytes).map(Some);
    }
    if let Some(src) = args.get("src").and_then(Value::as_str).filter(|s| s.starts_with("data:")) {
        let (head, data) = src.split_once(',').ok_or("src isn't a data URL")?;
        let bytes = if head.ends_with(";base64") { base64::engine::general_purpose::STANDARD.decode(data.trim()).map_err(|e| format!("src: {e}"))? } else { data.as_bytes().to_vec() };
        let name = if head.contains("svg") { "picture.svg" } else if head.contains("jpeg") { "picture.jpg" } else { "picture.png" };
        return crate::exports::image_data_url(name, &bytes).map(Some);
    }
    Ok(None)
}

impl CardBuilderApp {
    /// a dark or light ink that shows on `side`'s background
    fn ink_for(&self, side: Side) -> &'static str {
        let bg = self.doc.background(side);
        let hex = match bg.mode {
            BgMode::Paper | BgMode::Image => self.doc.paper.color.as_str(),
            _ => bg.color.as_str(),
        };
        if cb_print::color::luminance(hex) < 0.5 {
            "#ffffff"
        } else {
            "#1f2347"
        }
    }

    fn layer_name(&self, id: &str) -> String {
        self.doc.layer(id).map(|l| l.name.clone()).unwrap_or_else(|| id.to_string())
    }

    /// a sentence for the approval card and the activity list ("add a text layer …")
    pub(crate) fn describe_call(&self, tool: &str, a: &Value) -> String {
        let s = |k: &str| a.get(k).and_then(Value::as_str).unwrap_or("");
        let side_name = |k: &str| side_of(a.get(k), self.side).map(|sd| self.doc.format.side_name(sd).to_lowercase()).unwrap_or_default();
        let short = |t: &str| {
            let t: String = t.lines().next().unwrap_or("").chars().take(40).collect();
            t
        };
        match tool {
            "get_design" => "read the design".into(),
            "render" => format!("look at the {}", if s("side").is_empty() || s("side") == "both" { "card".to_string() } else { side_name("side") }),
            "list_card_formats" => "look up card sizes".into(),
            "list_fonts" => "look up fonts".into(),
            "add_layer" => {
                let what = if s("text").is_empty() { String::new() } else { format!(" “{}”", short(s("text"))) };
                format!("add {} layer{what} on the {}", match s("type") { "image" => "an image", "qr" => "a QR code", "shape" => "a shape", _ => "a text" }, side_name("side"))
            }
            "update_layer" => {
                let keys: Vec<String> = a.get("set").and_then(Value::as_object).map(|o| o.keys().cloned().collect()).unwrap_or_default();
                format!("change “{}”{}", self.layer_name(s("id")), if keys.is_empty() { String::new() } else { format!(" ({})", keys.join(", ")) })
            }
            "delete_layer" => match a.get("ids").and_then(Value::as_array) {
                Some(ids) => format!("delete {} layers", ids.len()),
                None => format!("delete “{}”", self.layer_name(s("id"))),
            },
            "arrange_layer" => format!("move “{}” {}", self.layer_name(s("id")), match s("to") { "front" => "to the top", "back" => "to the bottom", "forward" => "up", _ => "down" }),
            "set_background" => format!("change the {} background", side_name("side")),
            "set_card" => {
                let what = if !s("preset").is_empty() { preset(s("preset")).map(|p| p.name.to_string()).unwrap_or_else(|| s("preset").into()) } else { "its size or paper".into() };
                format!("change the card ({what})")
            }
            "choose_variant" => format!("pick {}", a.get("option").map(|o| o.as_str().map(str::to_string).unwrap_or_else(|| o.to_string())).unwrap_or_default()),
            "set_data" => format!("replace the card data ({} rows)", s("csv").lines().count().saturating_sub(1)),
            "undo" => "undo the last change".into(),
            "redo" => "redo the last change".into(),
            "new_design" => format!("start a new design{}", preset(s("preset")).map(|p| format!(" ({})", p.name)).unwrap_or_default()),
            "open_design" => format!("open {}", s("path")),
            "save_design" => if s("path").is_empty() { "save the design".into() } else { format!("save the design to {}", s("path")) },
            "export" => format!("export {} to {}", s("kind").replace('_', " "), s("path")),
            t => title(t).to_lowercase(),
        }
    }

    /// run tool `name` with `args` on the open design
    pub(crate) fn run_tool(&mut self, name: &str, args: &Value, now: f64, compact: bool) -> ToolOut {
        let r = match name {
            "get_design" => Ok(self.t_get_design(compact)),
            "render" => self.t_render(args),
            "list_card_formats" => Ok(self.t_formats(args, compact)),
            "list_fonts" => Ok(self.t_fonts()),
            "add_layer" => self.t_add_layer(args, now, compact),
            "update_layer" => self.t_update_layer(args, now, compact),
            "delete_layer" => self.t_delete(args, now),
            "arrange_layer" => self.t_arrange(args, now),
            "set_background" => self.t_background(args, now),
            "set_card" => self.t_card(args, now),
            "choose_variant" => self.t_variant(args, now),
            "set_data" => self.t_data(args, now),
            "undo" | "redo" => {
                let can = if name == "undo" { self.history.can_undo() } else { self.history.can_redo() };
                if !can {
                    Err(format!("there is nothing to {name}"))
                } else {
                    if name == "undo" { self.undo() } else { self.redo() };
                    Ok(ToolOut { text: format!("{}: done", title(name)), images: vec![], is_error: false, focus: None })
                }
            }
            "new_design" => self.t_new(args, now),
            "open_design" => self.t_open(args, now),
            "save_design" => self.t_save(args, now),
            "export" => self.t_export(args),
            _ => Err(format!("CardBuilder has no tool called {name}")),
        };
        r.unwrap_or_else(ToolOut::fail)
    }

    fn json_out(&self, v: Value, compact: bool, focus: Option<(Side, f64, f64, Option<String>)>) -> ToolOut {
        let text = if compact { v.to_string() } else { serde_json::to_string_pretty(&v).unwrap_or_default() };
        ToolOut { text, images: vec![], is_error: false, focus }
    }

    fn t_get_design(&self, compact: bool) -> ToolOut {
        let d = &self.doc;
        let f = &d.format;
        let (sw, sh) = f.sheet();
        let mut card = json!({
            "preset": f.preset, "name": self.format_title(), "w": f.w, "h": f.h, "radius": f.radius, "fold": f.fold,
            "bleed": f.bleed, "safe": f.safe, "sheet": {"w": sw, "h": sh},
            "sides": {"front": f.side_name(Side::Front), "back": f.side_name(Side::Back)},
        });
        if f.fold != cb_core::Fold::None || !compact {
            let panels: Vec<Value> = [Side::Front, Side::Back]
                .iter()
                .flat_map(|&s| f.panels(s).into_iter().map(move |p| json!({"side": s, "name": p.name, "x": p.x, "y": p.y, "w": p.w, "h": p.h, "upsideDown": p.upside_down})))
                .collect();
            card["panels"] = json!(panels);
        }
        let variants = d.variants.as_ref().map(|v| {
            json!({"label": v.label, "active": v.active.map(|i| v.options.get(i).map(|o| o.name.clone())), "options": v.options.iter().map(|o| o.name.clone()).collect::<Vec<_>>()})
        });
        let v = json!({
            "file": self.file_name(),
            "unsavedChanges": self.has_unsaved_changes(),
            "units": "mm; a layer's x, y are its centre from the trim's top-left corner; text size in pt",
            "card": card,
            "paper": d.paper,
            "background": {"front": bg_json(&d.front), "back": bg_json(&d.back)},
            "layers": d.layers.iter().map(|l| layer_json(l, compact)).collect::<Vec<_>>(),
            "variants": variants,
            "data": {"name": d.data.name, "columns": d.data.columns, "rows": d.data.rows.len()},
            "designFonts": d.fonts.iter().map(|f| f.family.clone()).collect::<Vec<_>>(),
            "selected": self.selected,
        });
        self.json_out(v, compact, None)
    }

    fn t_render(&self, args: &Value) -> Result<ToolOut, String> {
        let which = args.get("side").and_then(Value::as_str).unwrap_or("both").to_ascii_lowercase();
        let sides: Vec<Side> = if which == "both" { vec![Side::Front, Side::Back] } else { vec![side_of(args.get("side"), Side::Front)?] };
        let dpi = num(args, "dpi").unwrap_or(150.0).clamp(30.0, 300.0);
        let bleed = args.get("bleed").and_then(Value::as_bool).unwrap_or(false);
        let record = match args.get("row").and_then(Value::as_u64) {
            Some(r) if (r as usize) < self.doc.data.rows.len() => Some(r as usize),
            Some(r) => return Err(format!("the data has {} rows; row {r} isn't one of them", self.doc.data.rows.len())),
            None => self.current_record(),
        };
        let mut imgs = cb_print::ImageCache::default();
        imgs.prepare(cb_print::image_srcs(&self.doc));
        let mut images = Vec::new();
        let mut lines = Vec::new();
        for s in sides {
            let pm = cb_print::render_side(&self.doc, s, &cb_print::DrawOptions::preview(record), dpi / 25.4, bleed, &imgs).ok_or("couldn't draw the card")?;
            lines.push(format!("{}: {} × {} px at {dpi:.0} dpi{}", self.doc.format.side_name(s), pm.width(), pm.height(), if bleed { ", with bleed" } else { "" }));
            images.push(cb_print::sheets::png_bytes(&pm)?);
        }
        Ok(ToolOut { text: lines.join("\n"), images, is_error: false, focus: None })
    }

    fn t_formats(&self, args: &Value, compact: bool) -> ToolOut {
        let want = args.get("category").and_then(Value::as_str).map(str::to_ascii_lowercase);
        let rows: Vec<Value> = PRESETS
            .iter()
            .filter(|p| want.as_deref().is_none_or(|w| p.category.noun().to_ascii_lowercase().contains(w) || p.category.name().to_ascii_lowercase().contains(w)))
            .map(|p| json!({"id": p.id, "kind": p.category.name(), "group": p.group, "name": p.name, "note": p.note, "w": p.w, "h": p.h, "fold": p.fold}))
            .collect();
        self.json_out(json!(rows), compact, None)
    }

    fn t_fonts(&self) -> ToolOut {
        let mut v: Vec<Value> = cb_print::text::FONTS.iter().map(|(f, k)| json!({"family": f, "kind": format!("{k:?}").to_lowercase()})).collect();
        for f in &self.doc.fonts {
            v.push(json!({"family": f.family, "kind": "from the design"}));
        }
        self.json_out(json!(v), true, None)
    }

    fn t_add_layer(&mut self, args: &Value, now: f64, compact: bool) -> Result<ToolOut, String> {
        let side = side_of(args.get("side"), self.side)?;
        let ps = self.doc.format.panels(side);
        let p = *ps.last().ok_or("the card has no panels")?;
        let (x, y) = (num(args, "x").unwrap_or(p.x + p.w / 2.0), num(args, "y").unwrap_or(p.y + p.h / 2.0));
        let k = (p.w.min(p.h) / 55.0).clamp(0.6, 4.0);
        let ink = self.ink_for(side);
        let kind = args.get("type").and_then(Value::as_str).unwrap_or("");
        let mut l = match kind {
            "text" => {
                let text = args.get("text").and_then(Value::as_str).unwrap_or("Your text");
                let size = num(args, "size").unwrap_or((12.0 * k).round());
                Layer::text(side, x, y, num(args, "w").unwrap_or(p.w * 0.7), text, "Montserrat", 600.0, size, ink, Align::Center)
            }
            "shape" => {
                let shape: ShapeKind = match args.get("shape") {
                    Some(s) => serde_json::from_value(s.clone()).map_err(|_| "shape is rect, ellipse, line or path")?,
                    None => ShapeKind::Rect,
                };
                let mut l = Layer::rect(side, x, y, 20.0 * k, if shape == ShapeKind::Line { 2.0 } else { 12.0 * k }, "#ff7a59");
                l.shape = Some(shape);
                l.name = match shape {
                    ShapeKind::Rect => "Rectangle",
                    ShapeKind::Ellipse => "Ellipse",
                    ShapeKind::Line => "Line",
                    ShapeKind::Path => "Vector shape",
                }
                .into();
                if shape == ShapeKind::Ellipse {
                    l.h = l.w;
                }
                if shape == ShapeKind::Line {
                    l.fill = None;
                    l.stroke = Some(ink.into());
                    l.stroke_w = Some(0.5);
                }
                l
            }
            "qr" => {
                let size = (p.w.min(p.h) * 0.4).clamp(14.0, 60.0);
                let mut l = Layer::qr(side, x, y, size, QrSpec::url("https://example.com"), ink);
                if ink == "#ffffff" {
                    l.color = Some("#111111".into());
                    l.fill = Some("#ffffff".into());
                    l.radius = Some(1.0);
                }
                l
            }
            "image" => {
                let (src, iw, ih) = picture(args)?.ok_or("an image layer needs image_path (a file) or src (a data URL)")?;
                let aspect = ih / iw.max(1.0);
                let mut w = num(args, "w").unwrap_or(p.w * 0.5);
                if args.get("w").is_none() && w * aspect > p.h * 0.7 {
                    w = p.h * 0.7 / aspect;
                }
                let name = path_arg(args, "image_path").and_then(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned())).unwrap_or_else(|| "Image".into());
                let mut l = Layer::new(&name, LayerType::Image, side, x, y, w, w * aspect);
                l.src = Some(src);
                l
            }
            other => return Err(format!("type is text, shape, qr or image, not “{other}”")),
        };
        if self.doc.format.panels(side).iter().any(|q| q.upside_down && x >= q.x && x <= q.x + q.w && y >= q.y && y <= q.y + q.h) && args.get("rot").is_none() {
            l.rot = 180.0;
        }
        // every other field as given
        let mut set = args.clone();
        if let Some(o) = set.as_object_mut() {
            for k in ["type", "side", "image_path", "id"] {
                o.remove(k);
            }
            if kind == "image" {
                o.remove("src");
            }
        }
        let l = patched(&l, &set)?;
        let (id, focus) = (l.id.clone(), (l.side, l.x, l.y, Some(l.id.clone())));
        let shown = layer_json(&l, compact);
        self.edit(now, None, |d| d.layers.push(l));
        Ok(self.json_out(json!({"added": id, "layer": shown}), compact, Some(focus)))
    }

    fn t_update_layer(&mut self, args: &Value, now: f64, compact: bool) -> Result<ToolOut, String> {
        let id = args.get("id").and_then(Value::as_str).ok_or("give the layer's id")?.to_string();
        let old = self.doc.layer(&id).ok_or_else(|| format!("there is no layer {id}: get_design lists them"))?.clone();
        let mut set = match args.get("set") {
            Some(s) if s.is_object() => s.clone(),
            _ => {
                let mut s = args.clone();
                if let Some(o) = s.as_object_mut() {
                    o.remove("id");
                    o.remove("image_path");
                }
                s
            }
        };
        if let Some(o) = set.as_object_mut() {
            o.remove("id");
        }
        let mut l = patched(&old, &set)?;
        if old.kind == LayerType::Image {
            if let Some((src, iw, ih)) = picture(args)? {
                l.h = l.w * ih / iw.max(1.0);
                l.src = Some(src);
            }
        }
        let focus = (l.side, l.x, l.y, Some(id.clone()));
        let shown = layer_json(&l, compact);
        self.edit(now, None, |d| {
            if let Some(x) = d.layer_mut(&id) {
                *x = l;
            }
        });
        Ok(self.json_out(json!({"updated": id, "layer": shown}), compact, Some(focus)))
    }

    fn t_delete(&mut self, args: &Value, now: f64) -> Result<ToolOut, String> {
        let ids: Vec<String> = match (args.get("ids").and_then(Value::as_array), args.get("id").and_then(Value::as_str)) {
            (Some(a), _) => a.iter().filter_map(Value::as_str).map(str::to_string).collect(),
            (None, Some(i)) => vec![i.to_string()],
            _ => return Err("give the layer's id (or ids)".into()),
        };
        let missing: Vec<&String> = ids.iter().filter(|i| self.doc.layer(i).is_none()).collect();
        if !missing.is_empty() {
            return Err(format!("no such layer: {}", missing.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")));
        }
        let focus = self.doc.layer(&ids[0]).map(|l| (l.side, l.x, l.y, None));
        self.edit(now, None, |d| d.layers.retain(|l| !ids.contains(&l.id)));
        self.fix_selection();
        Ok(ToolOut { text: format!("deleted {}", ids.join(", ")), images: vec![], is_error: false, focus })
    }

    fn t_arrange(&mut self, args: &Value, now: f64) -> Result<ToolOut, String> {
        let id = args.get("id").and_then(Value::as_str).ok_or("give the layer's id")?.to_string();
        let l = self.doc.layer(&id).ok_or_else(|| format!("there is no layer {id}"))?.clone();
        match args.get("to").and_then(Value::as_str).unwrap_or("") {
            "forward" => self.move_layer(now, &id, 1),
            "backward" => self.move_layer(now, &id, -1),
            to @ ("front" | "back") => {
                let top = to == "front";
                self.edit(now, None, |d| {
                    if let Some(i) = d.layers.iter().position(|x| x.id == id) {
                        let x = d.layers.remove(i);
                        if top {
                            d.layers.push(x);
                        } else {
                            d.layers.insert(0, x);
                        }
                    }
                });
            }
            t => return Err(format!("to is front, back, forward or backward, not “{t}”")),
        }
        let order: Vec<String> = self.doc.layers.iter().filter(|x| x.side == l.side).map(|x| x.name.clone()).collect();
        Ok(ToolOut { text: format!("{} side, bottom to top: {}", self.doc.format.side_name(l.side), order.join(" · ")), images: vec![], is_error: false, focus: Some((l.side, l.x, l.y, Some(id))) })
    }

    fn t_background(&mut self, args: &Value, now: f64) -> Result<ToolOut, String> {
        let side = side_of(args.get("side"), self.side)?;
        let mut set = args.clone();
        if let Some(o) = set.as_object_mut() {
            for k in ["side", "image_path", "src"] {
                o.remove(k);
            }
        }
        let mut bg = patched(self.doc.background(side), &set)?;
        if let Some((src, _, _)) = picture(args)? {
            bg.src = Some(src);
            if args.get("mode").is_none() {
                bg.mode = BgMode::Image;
            }
        }
        if bg.mode == BgMode::Image && bg.src.is_none() {
            return Err("mode image needs a picture: image_path or src".into());
        }
        let shown = bg_json(&bg);
        self.edit(now, None, |d| *d.background_mut(side) = bg);
        let (sw, sh) = self.doc.format.sheet();
        Ok(self.json_out(json!({"side": side, "background": shown}), true, Some((side, sw / 2.0, sh / 2.0, None))))
    }

    fn t_card(&mut self, args: &Value, now: f64) -> Result<ToolOut, String> {
        let mut f = match args.get("preset").and_then(Value::as_str) {
            Some(id) => CardFormat::from_preset(preset(id).ok_or_else(|| format!("no card size {id}: list_card_formats lists them"))?),
            None => self.doc.format.clone(),
        };
        let mut set = json!({});
        for k in ["w", "h", "radius", "fold", "bleed", "safe"] {
            if let Some(v) = args.get(k) {
                set[k] = v.clone();
            }
        }
        if ["w", "h", "fold"].iter().any(|k| args.get(*k).is_some()) {
            set["preset"] = json!("custom");
        }
        f = patched(&f, &set)?;
        match args.get("orientation").and_then(Value::as_str) {
            Some("portrait") if f.w > f.h => std::mem::swap(&mut f.w, &mut f.h),
            Some("landscape") if f.h > f.w => std::mem::swap(&mut f.w, &mut f.h),
            _ => {}
        }
        if f.w < 10.0 || f.h < 10.0 || f.w > 600.0 || f.h > 600.0 {
            return Err("a card is 10 to 600 mm on each side".into());
        }
        self.set_format(now, f);
        if let Some(p) = args.get("paper") {
            let paper = patched(&self.doc.paper, p)?;
            self.edit(now, None, |d| d.paper = paper);
        }
        let f = &self.doc.format;
        Ok(ToolOut { text: format!("the card is now {} ({} × {} mm, {}), on {:?} paper {}", self.format_title(), f.w, f.h, f.fold.label().to_lowercase(), self.doc.paper.stock, self.doc.paper.color), images: vec![], is_error: false, focus: None })
    }

    fn t_variant(&mut self, args: &Value, now: f64) -> Result<ToolOut, String> {
        let v = self.doc.variants.as_ref().ok_or("this design has no choice buttons")?;
        let names: Vec<String> = v.options.iter().map(|o| o.name.clone()).collect();
        let i = match args.get("option") {
            Some(Value::Number(n)) => n.as_u64().map(|n| n as usize),
            Some(Value::String(s)) => names.iter().position(|n| n.eq_ignore_ascii_case(s.trim())),
            _ => None,
        }
        .filter(|i| *i < names.len())
        .ok_or_else(|| format!("the options are {}", names.join(", ")))?;
        self.edit(now, None, |d| d.apply_variant(i));
        Ok(ToolOut { text: format!("picked {}", names[i]), images: vec![], is_error: false, focus: None })
    }

    fn t_data(&mut self, args: &Value, now: f64) -> Result<ToolOut, String> {
        let csv = args.get("csv").and_then(Value::as_str).ok_or("give the data as csv text")?;
        let parsed = cb_core::data::parse_csv(csv)?;
        let name = args.get("name").and_then(Value::as_str).unwrap_or("data.csv");
        self.import_data_text(name, csv, now);
        Ok(ToolOut { text: format!("{} rows; columns: {}. Put {{Column}} in text or a QR code to use them.", parsed.rows.len(), parsed.columns.join(", ")), images: vec![], is_error: false, focus: None })
    }

    fn t_new(&mut self, args: &Value, now: f64) -> Result<ToolOut, String> {
        let f = match (args.get("preset").and_then(Value::as_str), num(args, "w"), num(args, "h")) {
            (Some(id), _, _) => CardFormat::from_preset(preset(id).ok_or_else(|| format!("no card size {id}: list_card_formats lists them"))?),
            (None, Some(w), Some(h)) => {
                let mut set = json!({"preset": "custom", "w": w, "h": h});
                for k in ["fold", "radius"] {
                    if let Some(v) = args.get(k) {
                        set[k] = v.clone();
                    }
                }
                patched(&self.doc.format, &set)?
            }
            _ => return Err("give a preset id, or w and h in mm".into()),
        };
        self.fresh_design(f, now);
        Ok(ToolOut { text: format!("a new {} design (Ctrl+Z brings the old one back)", self.format_title()), images: vec![], is_error: false, focus: None })
    }

    fn t_open(&mut self, args: &Value, now: f64) -> Result<ToolOut, String> {
        let p = path_arg(args, "path").ok_or("give the path of a .cb file")?;
        let text = std::fs::read_to_string(&p).map_err(|e| format!("couldn't read {}: {e}", p.display()))?;
        cb_core::doc::load_design(&text).map_err(|e| format!("{} isn't a design CardBuilder can open: {e}", p.display()))?;
        let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        self.open_design_text(&name, &text, Some(p), now);
        Ok(ToolOut { text: format!("opened {name}"), images: vec![], is_error: false, focus: None })
    }

    fn t_save(&mut self, args: &Value, now: f64) -> Result<ToolOut, String> {
        let p = match path_arg(args, "path").or_else(|| self.file.path.clone()) {
            Some(p) => p,
            None => return Err("this design hasn't been saved yet: give a path ending in .cb".into()),
        };
        let saved = self.save_to(p, now)?;
        Ok(ToolOut { text: format!("saved {}", saved.display()), images: vec![], is_error: false, focus: None })
    }

    fn t_export(&mut self, args: &Value) -> Result<ToolOut, String> {
        use crate::exports::Export;
        let e = match args.get("kind").and_then(Value::as_str).unwrap_or("") {
            "card_pdf" => Export::CardPdf,
            "print_sheets" => Export::PrintSheets,
            "png_front" => Export::Png { side: Side::Front },
            "png_back" => Export::Png { side: Side::Back },
            "csv" => Export::Csv,
            k => return Err(format!("kind is card_pdf, print_sheets, png_front, png_back or csv, not “{k}”")),
        };
        let p = path_arg(args, "path").ok_or("give the path to write")?;
        let mut imgs = cb_print::ImageCache::default();
        imgs.prepare(cb_print::image_srcs(&self.doc));
        let (bytes, note) = crate::exports::make_export(&self.doc, e, self.current_record(), &imgs, &|_| {})?;
        std::fs::write(&p, &bytes).map_err(|e| format!("couldn't write {}: {e}", p.display()))?;
        Ok(ToolOut { text: format!("wrote {} ({} KB){note}", p.display(), bytes.len() / 1024), images: vec![], is_error: false, focus: None })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merging_keeps_what_was_left_out() {
        let mut v = json!({"src": "data:image/png;base64,AAAA", "path": {"d": "M0 0", "w": 1}, "fill": "#fff", "stroke": "#000"});
        merge(&mut v, &json!({"src": "(kept: picture, 1 KB)", "path": {"d": "(kept: outline)", "w": 2}, "fill": "#f00", "stroke": null}));
        assert_eq!(v, json!({"src": "data:image/png;base64,AAAA", "path": {"d": "M0 0", "w": 2}, "fill": "#f00"}));
    }

    #[test]
    fn every_tool_has_a_schema_and_a_kind() {
        for t in list() {
            let name = t["name"].as_str().unwrap();
            assert_eq!(t["inputSchema"]["type"], "object", "{name}");
            assert!(kind(name).is_some());
        }
        assert_eq!(kind("render"), Some(Kind::Read));
        assert_eq!(kind("save_design"), Some(Kind::File));
        assert!(instructions("Use British spelling").ends_with("Use British spelling"));
    }
}
