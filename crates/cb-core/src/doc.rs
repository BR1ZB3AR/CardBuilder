//! The design document: card format, the two printed sides (background + layers), paper,
//! 3D scene, data for batch cards and print settings. Saved as `.cb` files.

use crate::format::{CardFormat, Category, Fold, Panel};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    /// front of a flat card, outside of a folded one
    #[default]
    Front,
    /// back of a flat card, inside of a folded one
    Back,
}

impl Side {
    pub fn other(self) -> Side {
        match self {
            Side::Front => Side::Back,
            Side::Back => Side::Front,
        }
    }
    pub fn index(self) -> usize {
        self as usize
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LayerType {
    Text,
    Image,
    Shape,
    Qr,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Align {
    Left,
    #[default]
    Center,
    Right,
}

/// where text sits in a text frame (a text box with a fixed height)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VAlign {
    Top,
    Middle,
    Bottom,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ShapeKind {
    #[default]
    Rect,
    Ellipse,
    Line,
    /// any outline (`Layer::path`): artwork from a template or an SVG
    Path,
}

/// A vector outline in SVG path syntax, drawn into its layer's box: the path's own box
/// (0, 0)–(w, h) is stretched onto the layer's.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VectorPath {
    /// SVG path data (`M`, `L`, `C`, `A`, `Z` …)
    pub d: String,
    /// the size of the box the path data spans, in its own units
    pub w: f64,
    pub h: f64,
    /// holes by the even-odd rule (otherwise by winding direction)
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub even_odd: bool,
}

/// how an outline is drawn: solid unless dashed or dotted
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Dash {
    Dashed,
    Dotted,
}

impl Dash {
    /// on/off lengths along the outline for a line `width` wide (mm); a dot is a
    /// round-capped dash of (almost) no length
    pub fn intervals(self, width: f64) -> [f64; 2] {
        let w = width.max(0.01);
        match self {
            Dash::Dashed => [3.0 * w, 2.0 * w],
            Dash::Dotted => [w * 0.001, w * 1.999],
        }
    }
}

/// metallic foil stamped on top of the print
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Foil {
    Gold,
    Silver,
    Copper,
    Rose,
    Holo,
}

impl Foil {
    pub const ALL: [Foil; 5] = [Foil::Gold, Foil::Silver, Foil::Copper, Foil::Rose, Foil::Holo];
    pub fn name(self) -> &'static str {
        match self {
            Foil::Gold => "Gold",
            Foil::Silver => "Silver",
            Foil::Copper => "Copper",
            Foil::Rose => "Rose gold",
            Foil::Holo => "Holographic",
        }
    }
    /// (dark, light) colours of the foil as printed in previews and proofs
    pub fn colors(self) -> (&'static str, &'static str) {
        match self {
            Foil::Gold => ("#a87c2a", "#f6dc8c"),
            Foil::Silver => ("#8e9399", "#f1f3f5"),
            Foil::Copper => ("#94512c", "#f0a878"),
            Foil::Rose => ("#b46f6a", "#f6cbc0"),
            Foil::Holo => ("#8fb8ff", "#ffb8ec"),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QrKind {
    #[default]
    Url,
    Text,
    Phone,
    Email,
    Vcard,
    Wifi,
}

impl QrKind {
    pub const ALL: [QrKind; 6] = [QrKind::Url, QrKind::Vcard, QrKind::Wifi, QrKind::Phone, QrKind::Email, QrKind::Text];
    pub fn name(self) -> &'static str {
        match self {
            QrKind::Url => "Website",
            QrKind::Text => "Text",
            QrKind::Phone => "Phone",
            QrKind::Email => "Email",
            QrKind::Vcard => "Contact",
            QrKind::Wifi => "Wi-Fi",
        }
    }
}

/// error correction: how much of the code can be damaged (or covered by a logo) and still scan
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum QrEcc {
    L,
    #[default]
    M,
    Q,
    H,
}

/// what a QR code layer encodes
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct QrSpec {
    pub kind: QrKind,
    /// website / text / phone number / email address
    pub text: String,
    // contact (vCard)
    pub name: String,
    pub org: String,
    pub title: String,
    pub phone: String,
    pub email: String,
    pub url: String,
    pub address: String,
    // Wi-Fi
    pub ssid: String,
    pub password: String,
    /// "WPA", "WEP" or "nopass"
    pub security: String,
    pub hidden: bool,
    pub ecc: QrEcc,
}

fn vcard_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace(',', "\\,").replace(';', "\\;").replace('\n', "\\n")
}
fn wifi_escape(s: &str) -> String {
    let mut o = String::new();
    for c in s.chars() {
        if matches!(c, '\\' | ';' | ',' | ':' | '"') {
            o.push('\\');
        }
        o.push(c);
    }
    o
}

impl QrSpec {
    pub fn url(u: &str) -> QrSpec {
        QrSpec { kind: QrKind::Url, text: u.into(), ..Default::default() }
    }

    /// the text the QR code holds; `fill` substitutes data placeholders in each field first
    pub fn payload(&self, fill: &dyn Fn(&str) -> String) -> String {
        match self.kind {
            QrKind::Url | QrKind::Text => fill(&self.text),
            QrKind::Phone => {
                let t: String = fill(&self.text).chars().filter(|c| c.is_ascii_digit() || *c == '+').collect();
                format!("tel:{t}")
            }
            QrKind::Email => format!("mailto:{}", fill(&self.text).trim()),
            QrKind::Vcard => {
                let name = fill(&self.name);
                let name = name.trim();
                let (first, last) = name.rsplit_once(' ').map(|(f, l)| (f.trim(), l.trim())).unwrap_or((name, ""));
                let mut v = String::from("BEGIN:VCARD\nVERSION:3.0\n");
                v += &format!("N:{};{};;;\nFN:{}\n", vcard_escape(last), vcard_escape(first), vcard_escape(name));
                for (key, val) in [("ORG", &self.org), ("TITLE", &self.title), ("TEL", &self.phone), ("EMAIL", &self.email), ("URL", &self.url)] {
                    let val = fill(val);
                    if !val.trim().is_empty() {
                        v += &format!("{key}:{}\n", vcard_escape(val.trim()));
                    }
                }
                let adr = fill(&self.address);
                if !adr.trim().is_empty() {
                    v += &format!("ADR:;;{};;;;\n", vcard_escape(adr.trim()));
                }
                v + "END:VCARD"
            }
            QrKind::Wifi => {
                let sec = if self.security.is_empty() { "WPA" } else { self.security.as_str() };
                let mut w = format!("WIFI:T:{sec};S:{};", wifi_escape(&fill(&self.ssid)));
                if sec != "nopass" {
                    w += &format!("P:{};", wifi_escape(&fill(&self.password)));
                }
                if self.hidden {
                    w += "H:true;";
                }
                w + ";"
            }
        }
    }

    /// short description for the layer list
    pub fn summary(&self) -> String {
        match self.kind {
            QrKind::Vcard => format!("Contact · {}", self.name),
            QrKind::Wifi => format!("Wi-Fi · {}", self.ssid),
            k => format!("{} · {}", k.name(), self.text),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Layer {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: LayerType,
    pub side: Side,
    /// centre, mm, in the side's sheet coordinates
    pub x: f64,
    pub y: f64,
    /// box size, mm (a text box's height follows its text)
    pub w: f64,
    pub h: f64,
    /// degrees, clockwise
    pub rot: f64,
    pub opacity: f64,
    pub visible: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub locked: bool,
    // text
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weight: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub italic: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub align: Option<Align>,
    /// font size, pt
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_height: Option<f64>,
    /// extra space after every character, em
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub letter_spacing: Option<f64>,
    /// wrap lines at the box width (otherwise long lines shrink to fit it)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wrap: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uppercase: Option<bool>,
    /// a text frame: the box keeps its height (`h`) and the text sits at its top / middle /
    /// bottom; None = the box follows the text, centred on (x, y)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valign: Option<VAlign>,
    // image
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub src: Option<String>,
    // shape
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shape: Option<ShapeKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke: Option<String>,
    /// outline width, mm
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke_w: Option<f64>,
    /// corner radius, mm
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radius: Option<f64>,
    /// the outline of a vector shape (`ShapeKind::Path`)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<VectorPath>,
    /// a dashed or dotted outline (shapes)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dash: Option<Dash>,
    /// shown only inside the layer below it (a photo in a window shape)
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub clip: bool,
    // QR code (colours: `color` = modules, `fill` = background, none = transparent)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub qr: Option<QrSpec>,
    // finishing
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub foil: Option<Foil>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub spot_uv: bool,
}

impl Layer {
    /// an empty layer of a kind (callers fill in the rest)
    pub fn new(name: &str, kind: LayerType, side: Side, x: f64, y: f64, w: f64, h: f64) -> Layer {
        Layer {
            id: uid(),
            name: name.into(),
            kind,
            side,
            x,
            y,
            w,
            h,
            rot: 0.0,
            opacity: 1.0,
            visible: true,
            locked: false,
            text: None,
            font: None,
            weight: None,
            italic: None,
            color: None,
            align: None,
            size: None,
            line_height: None,
            letter_spacing: None,
            wrap: None,
            uppercase: None,
            valign: None,
            src: None,
            shape: None,
            fill: None,
            stroke: None,
            stroke_w: None,
            radius: None,
            path: None,
            dash: None,
            clip: false,
            qr: None,
            foil: None,
            spot_uv: false,
        }
    }

    /// a vector shape filled with `fill` (callers add an outline if it has one)
    pub fn vector(side: Side, x: f64, y: f64, w: f64, h: f64, path: VectorPath, fill: Option<&str>) -> Layer {
        let mut l = Layer::new("Vector shape", LayerType::Shape, side, x, y, w, h);
        l.shape = Some(ShapeKind::Path);
        l.path = Some(path);
        l.fill = fill.map(Into::into);
        l
    }

    /// set the layer's main colour: a shape's fill (or its outline when it has no fill),
    /// otherwise its `color` (text, QR code)
    pub fn set_main_color(&mut self, c: &str) {
        if self.kind == LayerType::Shape {
            if self.fill.is_some() || (self.shape != Some(ShapeKind::Line) && self.stroke.is_none()) {
                self.fill = Some(c.into());
            } else {
                self.stroke = Some(c.into());
            }
        } else {
            self.color = Some(c.into());
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn text(side: Side, x: f64, y: f64, w: f64, text: &str, font: &str, weight: f64, size: f64, color: &str, align: Align) -> Layer {
        let mut l = Layer::new(text.lines().next().unwrap_or("Text"), LayerType::Text, side, x, y, w, size * crate::MM_PER_PT * 1.2);
        l.text = Some(text.into());
        l.font = Some(font.into());
        l.weight = Some(weight);
        l.italic = Some(false);
        l.color = Some(color.into());
        l.align = Some(align);
        l.size = Some(size);
        l.line_height = Some(1.2);
        l.letter_spacing = Some(0.0);
        l.wrap = Some(false);
        l
    }

    pub fn rect(side: Side, x: f64, y: f64, w: f64, h: f64, fill: &str) -> Layer {
        let mut l = Layer::new("Shape", LayerType::Shape, side, x, y, w, h);
        l.shape = Some(ShapeKind::Rect);
        l.fill = Some(fill.into());
        l.radius = Some(0.0);
        l
    }

    pub fn qr(side: Side, x: f64, y: f64, size: f64, spec: QrSpec, color: &str) -> Layer {
        let mut l = Layer::new("QR code", LayerType::Qr, side, x, y, size, size);
        l.qr = Some(spec);
        l.color = Some(color.into());
        l
    }

    /// the text as printed for one data record (placeholders filled, uppercase applied)
    pub fn text_for(&self, fill: &dyn Fn(&str) -> String) -> String {
        let t = fill(self.text.as_deref().unwrap_or(""));
        if self.uppercase.unwrap_or(false) {
            t.to_uppercase()
        } else {
            t
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BgMode {
    /// nothing printed: the paper shows
    #[default]
    Paper,
    Solid,
    Gradient,
    Image,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Background {
    pub mode: BgMode,
    pub color: String,
    pub color2: String,
    /// gradient direction, degrees (0 = left to right, 90 = top to bottom)
    pub angle: f64,
    /// picture filling the side (cover), as a data URL
    pub src: Option<String>,
}

impl Default for Background {
    fn default() -> Self {
        Background { mode: BgMode::Paper, color: "#1f2347".into(), color2: "#6b3fa0".into(), angle: 35.0, src: None }
    }
}

/// card stock: colour, feel and thickness
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Stock {
    #[default]
    Matte,
    Silk,
    Gloss,
    SoftTouch,
    Linen,
    Kraft,
    Pearl,
    Black,
    Plastic,
}

/// how a stock looks and feels
#[derive(Clone, Copy, Debug)]
pub struct StockInfo {
    pub name: &'static str,
    pub note: &'static str,
    pub color: &'static str,
    /// mm
    pub thickness: f64,
    /// 0 mirror … 1 chalk
    pub roughness: f64,
    /// clear varnish / lamination highlight strength
    pub gloss: f64,
    /// 0 smooth, 1 linen weave, 2 kraft fibres, 3 pearl shimmer
    pub grain: u32,
}

impl Stock {
    pub const ALL: [Stock; 9] = [Stock::Matte, Stock::Silk, Stock::Gloss, Stock::SoftTouch, Stock::Linen, Stock::Kraft, Stock::Pearl, Stock::Black, Stock::Plastic];
    pub fn info(self) -> StockInfo {
        let s = |name, note, color, thickness, roughness, gloss, grain| StockInfo { name, note, color, thickness, roughness, gloss, grain };
        match self {
            Stock::Matte => s("Matte", "350 gsm uncoated, chalky white", "#f7f6f2", 0.38, 0.92, 0.0, 0),
            Stock::Silk => s("Silk", "400 gsm satin coated", "#fbfbf9", 0.42, 0.6, 0.25, 0),
            Stock::Gloss => s("Gloss", "350 gsm gloss laminated", "#ffffff", 0.40, 0.18, 1.0, 0),
            Stock::SoftTouch => s("Soft-touch", "450 gsm velvet laminate", "#f5f4f1", 0.50, 0.97, 0.0, 0),
            Stock::Linen => s("Linen", "300 gsm textured linen", "#f4f0e6", 0.36, 0.95, 0.0, 1),
            Stock::Kraft => s("Kraft", "350 gsm recycled brown", "#b98b5e", 0.40, 0.96, 0.0, 2),
            Stock::Pearl => s("Pearl", "300 gsm shimmer", "#f3efe9", 0.36, 0.45, 0.3, 3),
            Stock::Black => s("Black", "450 gsm black card", "#1b1b1d", 0.50, 0.85, 0.0, 0),
            Stock::Plastic => s("Plastic (PVC)", "0.76 mm, like a bank card", "#ffffff", 0.76, 0.3, 0.7, 0),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Paper {
    pub stock: Stock,
    /// paper colour (shows where nothing is printed, and on the edges)
    pub color: String,
}

impl Default for Paper {
    fn default() -> Self {
        Paper { stock: Stock::Matte, color: Stock::Matte.info().color.into() }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Backdrop {
    #[default]
    Studio,
    Dark,
    Desk,
    Solid,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SceneSettings {
    pub backdrop: Backdrop,
    pub bg_color: String,
    /// key light strength
    pub light: f64,
    /// key light direction round the card, degrees
    pub light_angle: f64,
    pub shadows: bool,
    /// how far a folded card is opened, degrees (0 closed … 180 flat)
    pub open: f64,
    pub auto_rotate: bool,
    /// flat cards: show two cards side by side, the second turned to show its back
    pub pair: bool,
}

impl Default for SceneSettings {
    fn default() -> Self {
        SceneSettings { backdrop: Backdrop::Studio, bg_color: "#c9cbe0".into(), light: 1.0, light_angle: -35.0, shadows: true, open: 110.0, auto_rotate: false, pair: true }
    }
}

/// rows of data: one card per row, `{Column}` placeholders filled from it
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DataSet {
    /// where it came from (file name)
    pub name: String,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

impl DataSet {
    pub fn is_empty(&self) -> bool {
        self.columns.is_empty() || self.rows.is_empty()
    }
    /// the value of `column` (case-insensitive) in `row`
    pub fn value(&self, row: usize, column: &str) -> Option<&str> {
        let c = self.columns.iter().position(|c| c.trim().eq_ignore_ascii_case(column.trim()))?;
        Some(self.rows.get(row)?.get(c).map(String::as_str).unwrap_or(""))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PrintSettings {
    /// sheet size id (`format::SHEETS`)
    pub sheet: String,
    /// mm kept clear round the sheet edge (printer's non-printable border + crop marks)
    pub margin: f64,
    /// mm between cards
    pub gap: f64,
    pub crop_marks: bool,
    pub bleed: bool,
    /// backs on the reverse of each sheet, lined up for double-sided printing
    pub duplex: bool,
    /// the printer turns the sheet over on its short edge
    pub short_edge: bool,
    /// cards to print when there is no data (0 = fill one sheet)
    pub copies: u32,
    /// turn cards 90° when more fit that way
    pub allow_rotate: bool,
    /// resolution of photos and gradients in the PDF
    pub dpi: u32,
}

impl Default for PrintSettings {
    fn default() -> Self {
        PrintSettings { sheet: "a4".into(), margin: 10.0, gap: 0.0, crop_marks: true, bleed: true, duplex: true, short_edge: false, copies: 0, allow_rotate: true, dpi: 300 }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Doc {
    pub version: u32,
    pub format: CardFormat,
    #[serde(default)]
    pub front: Background,
    #[serde(default)]
    pub back: Background,
    pub layers: Vec<Layer>,
    #[serde(default)]
    pub paper: Paper,
    #[serde(default)]
    pub scene: SceneSettings,
    #[serde(default)]
    pub data: DataSet,
    #[serde(default)]
    pub print: PrintSettings,
    /// fonts the design brings along (beyond the bundled ones), so it opens anywhere
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fonts: Vec<FontFile>,
    /// the local template this design was started from (its id)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
    /// a choice the design offers as buttons (e.g. a biocard's faction), if any
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variants: Option<Variants>,
}

/// One choice a design (usually a template) offers as a row of buttons, such as a biocard's
/// faction: each option shows its own layers (the other options' layers are hidden) and sets
/// colours.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Variants {
    /// what is being chosen ("Faction")
    pub label: String,
    /// a line under the heading, saying what the buttons change
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    pub options: Vec<VariantOption>,
    /// the option picked
    #[serde(default)]
    pub active: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VariantOption {
    /// the button's text ("RES")
    pub name: String,
    /// the button's colour
    pub color: String,
    /// layers (ids) shown with this option only
    #[serde(default)]
    pub show: Vec<String>,
    /// (layer id, colour) set when this option is picked
    #[serde(default)]
    pub recolor: Vec<(String, String)>,
}

/// a font file carried in a design
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FontFile {
    /// the name layers use for it
    pub family: String,
    /// the font file as a data URL
    pub src: String,
}

impl Doc {
    /// Pick option `i` of the design's choice: its layers show, the other options' layers
    /// hide, and its colours are set.
    pub fn apply_variant(&mut self, i: usize) {
        let Some(v) = self.variants.as_mut() else { return };
        if i >= v.options.len() {
            return;
        }
        v.active = Some(i);
        let v = v.clone();
        for (j, o) in v.options.iter().enumerate() {
            for id in &o.show {
                if let Some(l) = self.layers.iter_mut().find(|l| &l.id == id) {
                    l.visible = j == i;
                }
            }
        }
        // a layer two options share stays shown when the picked one has it
        for id in &v.options[i].show {
            if let Some(l) = self.layers.iter_mut().find(|l| &l.id == id) {
                l.visible = true;
            }
        }
        for (id, c) in &v.options[i].recolor {
            if let Some(l) = self.layers.iter_mut().find(|l| &l.id == id) {
                l.set_main_color(c);
            }
        }
    }

    pub fn background(&self, side: Side) -> &Background {
        match side {
            Side::Front => &self.front,
            Side::Back => &self.back,
        }
    }
    pub fn background_mut(&mut self, side: Side) -> &mut Background {
        match side {
            Side::Front => &mut self.front,
            Side::Back => &mut self.back,
        }
    }
    pub fn layers_on(&self, side: Side) -> impl Iterator<Item = &Layer> {
        self.layers.iter().filter(move |l| l.side == side)
    }
    pub fn layer(&self, id: &str) -> Option<&Layer> {
        self.layers.iter().find(|l| l.id == id)
    }
    pub fn layer_mut(&mut self, id: &str) -> Option<&mut Layer> {
        self.layers.iter_mut().find(|l| l.id == id)
    }
    /// any layer has foil or spot UV
    pub fn has_finishes(&self) -> bool {
        self.layers.iter().any(|l| l.visible && (l.foil.is_some() || l.spot_uv))
    }
    /// placeholder filler for data record `row` (None = leave placeholders as typed)
    pub fn filler(&self, row: Option<usize>) -> impl Fn(&str) -> String + '_ {
        move |t: &str| match row {
            Some(r) if !self.data.is_empty() => crate::data::fill(t, &self.data, r),
            _ => t.to_string(),
        }
    }
    /// how many different cards there are (data rows, else one)
    pub fn records(&self) -> usize {
        if self.data.is_empty() {
            1
        } else {
            self.data.rows.len()
        }
    }
}

static UID_SEQ: AtomicU64 = AtomicU64::new(1);
static UID_SEED: AtomicU64 = AtomicU64::new(0xcb);

/// seed layer ids (e.g. with the start-up time) so ids from different sessions don't collide
pub fn seed_uids(seed: u64) {
    UID_SEED.store(seed, Ordering::Relaxed);
}

fn base36(mut n: u64) -> String {
    const D: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    if n == 0 {
        return "0".into();
    }
    let mut s = vec![];
    while n > 0 {
        s.push(D[(n % 36) as usize]);
        n /= 36;
    }
    s.reverse();
    String::from_utf8(s).unwrap()
}

/// a new layer id
pub fn uid() -> String {
    format!("l{}{}", base36(UID_SEED.load(Ordering::Relaxed)), base36(UID_SEQ.fetch_add(1, Ordering::Relaxed)))
}

/// A fresh design for a format: a ready-made layout that suits the kind of card, so there is
/// something to start from (and to see in 3D) straight away.
pub fn default_doc(format: CardFormat) -> Doc {
    let mut doc = Doc {
        version: 1,
        front: Background::default(),
        back: Background::default(),
        layers: vec![],
        paper: Paper::default(),
        scene: SceneSettings::default(),
        data: DataSet::default(),
        print: PrintSettings::default(),
        fonts: vec![],
        template: None,
        variants: None,
        format,
    };
    let f = doc.format.clone();
    doc.scene.open = default_open(f.fold);
    let front = f.front_panel();
    let (pw, ph) = (front.w, front.h);
    let k = pw.min(ph) / 55.0; // scale from a business card
    let ink = "#1f2347";
    let accent = "#ff7a59";
    if let Some(faction) = Faction::of_preset(&f.preset) {
        biocard(&mut doc, faction);
        return doc;
    }
    match f.category() {
        Category::Greeting | Category::Postcard | Category::Tent => {
            doc.front = Background { mode: BgMode::Gradient, color: "#1f2347".into(), color2: "#6b3fa0".into(), angle: 60.0, src: None };
            let (cx, cy) = (front.x + pw / 2.0, front.y + ph / 2.0);
            let (title, sub) = match f.category() {
                Category::Tent => ("Table 7", "Welcome"),
                Category::Postcard => ("You're invited", "Saturday 12 December · 7 pm"),
                _ => ("Happy Birthday", "with love"),
            };
            let size = 30.0 * k;
            let mut t = Layer::text(Side::Front, cx, cy - 4.0 * k, pw * 0.86, title, "Great Vibes", 400.0, size, "#ffffff", Align::Center);
            t.foil = Some(Foil::Gold);
            if front.upside_down {
                t.rot = 180.0;
            }
            doc.layers.push(t);
            doc.layers.push(Layer::text(Side::Front, cx, cy + 12.0 * k, pw * 0.8, sub, "Montserrat", 500.0, 9.0 * k, "#d9d4ff", Align::Center));
            if f.fold != Fold::None {
                // a message inside, on the panel you read last
                let inside = *f.panels(Side::Back).last().unwrap();
                let mut m = Layer::text(Side::Back, inside.x + inside.w / 2.0, inside.y + inside.h / 2.0, inside.w * 0.75, "Wishing you a wonderful year\nfull of good things.", "Lora", 400.0, 11.0 * k.min(1.4), ink, Align::Center);
                m.italic = Some(true);
                m.line_height = Some(1.4);
                doc.layers.push(m);
                let back = f.panels(Side::Front)[0];
                let mut by = Layer::text(Side::Front, back.x + back.w / 2.0, back.y + back.h * 0.82, back.w * 0.6, "Made with CardBuilder", "Montserrat", 500.0, 6.0, "#d9d4ff", Align::Center);
                if back.upside_down {
                    by.rot = 180.0;
                    by.y = back.y + back.h * 0.18;
                }
                doc.layers.push(by);
            } else {
                doc.layers.push(Layer::text(Side::Back, pw / 2.0, ph * 0.5, pw * 0.7, "Dear friend,\nwrite your message here.", "Lora", 400.0, 11.0 * k.min(1.4), ink, Align::Left));
            }
        }
        Category::Game => {
            // frame, title bar, art box and rules text, like a game card
            doc.front = Background { mode: BgMode::Gradient, color: "#2b2f5c".into(), color2: "#141633".into(), angle: 90.0, src: None };
            let m = 3.5 * k;
            let mut frame = Layer::rect(Side::Front, pw / 2.0, ph / 2.0, pw - 2.0 * m, ph - 2.0 * m, "#00000000");
            frame.fill = None;
            frame.stroke = Some("#e8c46a".into());
            frame.stroke_w = Some(0.8);
            frame.radius = Some(2.0);
            frame.name = "Frame".into();
            doc.layers.push(frame);
            let mut art = Layer::rect(Side::Front, pw / 2.0, ph * 0.38, pw - 4.0 * m, ph * 0.42, "#6b3fa0");
            art.radius = Some(1.5);
            art.name = "Art".into();
            doc.layers.push(art);
            doc.layers.push(Layer::text(Side::Front, pw / 2.0, m + 5.0 * k, pw - 4.0 * m, "Star Forger", "Bebas Neue", 400.0, 14.0 * k, "#ffffff", Align::Left));
            let mut rules = Layer::text(Side::Front, pw / 2.0, ph * 0.76, pw - 5.0 * m, "When this card enters play, draw two cards. Forge one star for every card in your hand.", "Lora", 400.0, 6.5 * k, "#e9e6ff", Align::Left);
            rules.wrap = Some(true);
            rules.line_height = Some(1.3);
            doc.layers.push(rules);
            doc.back = Background { mode: BgMode::Solid, color: "#1f2347".into(), ..Default::default() };
            let mut emblem = Layer::rect(Side::Back, pw / 2.0, ph / 2.0, pw * 0.5, pw * 0.5, "#6b3fa0");
            emblem.shape = Some(ShapeKind::Ellipse);
            emblem.foil = Some(Foil::Gold);
            emblem.name = "Emblem".into();
            doc.layers.push(emblem);
        }
        Category::Business => business_card(&mut doc),
        _ => {
            // business-card style: name, title and an accent bar on the front; contact details and a QR code on the back
            doc.front = Background { mode: BgMode::Solid, color: "#1f2347".into(), ..Default::default() };
            let m = 6.0 * k;
            let landscape = pw >= ph;
            let left = m;
            let w = pw - 2.0 * m;
            let name_y = if landscape { ph * 0.42 } else { ph * 0.55 };
            doc.layers.push(Layer::text(Side::Front, left + w / 2.0, name_y, w, "Alex Morgan", "Montserrat", 700.0, 15.0 * k, "#ffffff", Align::Left));
            doc.layers.push(Layer::text(Side::Front, left + w / 2.0, name_y + 7.0 * k, w, "Card designer", "Montserrat", 400.0, 8.0 * k, "#b8b4ff", Align::Left));
            let mut bar = Layer::rect(Side::Front, left + 7.0 * k, name_y - 8.5 * k, 14.0 * k, 1.2 * k, accent);
            bar.name = "Accent bar".into();
            doc.layers.push(bar);
            let qr = 18.0 * k;
            let mut info = Layer::text(Side::Back, m + (pw - 3.0 * m - qr) / 2.0, ph / 2.0, pw - 3.0 * m - qr, "+60 12-345 6789\nalex@example.com\nexample.com", "Inter", 400.0, 7.0 * k, ink, Align::Left);
            info.line_height = Some(1.5);
            doc.layers.push(info);
            doc.layers.push(Layer::qr(Side::Back, pw - m - qr / 2.0, ph / 2.0, qr, QrSpec::url("https://example.com"), ink));
        }
    }
    doc
}

impl Default for Doc {
    fn default() -> Self {
        default_doc(CardFormat::default())
    }
}

/// JavaScript's `encodeURIComponent`, for SVG data URLs
fn encode_uri_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 2);
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// an SVG as a data URL (how pictures live in a design)
pub fn svg_data_url(svg: &str) -> String {
    format!("data:image/svg+xml;charset=utf-8,{}", encode_uri_component(svg))
}

/// The business card starter: white, the company name in two weights in the middle and a
/// teal-to-blue wave along the bottom with the website; on the back the company name again, a
/// soft divider, the person's name, title and contact rows with icons.
fn business_card(doc: &mut Doc) {
    let f = doc.format.clone();
    let p = f.front_panel();
    let (ox, oy, w, h) = (p.x, p.y, p.w, p.h);
    // sizes follow the card, against an 85 × 55 mm one
    let k = (w / 85.0).min(h / 55.0);
    let blue = "#2b87d1";
    let teal = "#1fa7b8";
    let dark = "#3b3f45";
    let ink = "#33475b";
    let icon_c = "#25507a";
    doc.front = Background { mode: BgMode::Solid, color: "#ffffff".into(), ..Default::default() };
    doc.back = doc.front.clone();
    // Inter: how far a line's baseline sits below its middle, and word widths (em)
    const M2B: f64 = 0.3638;
    const COMPANY_W: f64 = 5.3296; // bold
    const NAME_W: f64 = 2.9478;
    const SPACE_W: f64 = 0.2812;
    const JOHN_W: f64 = 2.8643; // bold
    let em = |pt: f64| pt * crate::MM_PER_PT;
    let middle = |pt: f64, base: f64| base - M2B * em(pt);
    // two words in two weights meeting at `join`: the first ends there, the second starts a space on
    #[allow(clippy::too_many_arguments)]
    let pair = |side: Side, a: &str, b: &str, pt: f64, join: f64, base: f64, room: (f64, f64), color: &str| {
        let y = middle(pt, base);
        let first = Layer::text(side, join - room.0 / 2.0, y, room.0, a, "Inter", 800.0, pt, color, Align::Right);
        let start = join + SPACE_W * em(pt);
        let second = Layer::text(side, start + room.1 / 2.0, y, room.1, b, "Inter", 400.0, pt, color, Align::Left);
        [first, second]
    };
    let band = 0.12 * h;
    let wave = |side: Side| {
        let bl = f.bleed;
        let top = h - band;
        let (x0, y0, x1, y1) = (-bl, top - 0.5, w + bl, h + bl);
        let (lw, lh) = (x1 - x0, y1 - y0);
        let pt = |x: f64, y: f64| format!("{:.3} {:.3}", x - x0, y - y0);
        let d = format!(
            "M{} L{} L{} C{} {} {} L{} C{} {} {} L{} L{} Z",
            pt(-bl, h + bl),
            pt(-bl, h),
            pt(0.0, h),
            pt(0.13 * w, h),
            pt(0.12 * w, top),
            pt(0.27 * w, top),
            pt(0.80 * w, top),
            pt(0.91 * w, top),
            pt(0.97 * w, top + 0.12 * band),
            pt(w, top + 0.42 * band),
            pt(w + bl, top + 0.5 * band),
            pt(w + bl, h + bl),
        );
        let svg = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {lw:.3} {lh:.3}" width="{lw:.3}mm" height="{lh:.3}mm"><defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="0"><stop offset="0" stop-color="#16b39b"/><stop offset="1" stop-color="#2a7fd6"/></linearGradient></defs><path d="{d}" fill="url(#g)"/></svg>"##
        );
        let mut l = Layer::new("Wave", LayerType::Image, side, ox + x0 + lw / 2.0, oy + y0 + lh / 2.0, lw, lh);
        l.src = Some(svg_data_url(&svg));
        l
    };
    let website = |side: Side| {
        let pt = 6.0 * k;
        // lower-case letters' middle (x-height / 2) in the middle of the wave
        let base = oy + h - 0.47 * band + 0.273 * em(pt);
        Layer::text(side, ox + w / 2.0, middle(pt, base), w * 0.6, "www.website.com", "Inter", 500.0, pt, "#ffffff", Align::Center)
    };

    // ---- front
    doc.layers.push(wave(Side::Front));
    let pt = 10.5 * k;
    let total = (COMPANY_W + SPACE_W + NAME_W) * em(pt);
    let join = ox + w / 2.0 - total / 2.0 + COMPANY_W * em(pt);
    let base = oy + h * 0.45 + 0.3638 * em(pt); // capitals' middle at 45 % of the height
    let room = (join - ox - 3.0 * k, ox + w - 3.0 * k - join - SPACE_W * em(pt));
    doc.layers.extend(pair(Side::Front, "COMPANY", "NAME", pt, join, base, room, blue));
    doc.layers.push(website(Side::Front));

    // ---- back
    doc.layers.push(wave(Side::Back));
    let line_x = ox + 0.505 * w;
    // as on the front, made smaller when the left column is narrower than that
    let pt = pt.min((line_x - ox - 5.0 * k) / ((COMPANY_W + SPACE_W + NAME_W) * crate::MM_PER_PT));
    let total = (COMPANY_W + SPACE_W + NAME_W) * em(pt);
    let join = ox + (line_x - ox) / 2.0 - total / 2.0 + COMPANY_W * em(pt);
    let base = oy + h * 0.48 + 0.3638 * em(pt);
    let room = (join - ox - 2.0 * k, line_x - 1.5 * k - join - SPACE_W * em(pt));
    doc.layers.extend(pair(Side::Back, "COMPANY", "NAME", pt, join, base, room, blue));
    // a soft divider: a thin line with a shadow to its left, fading out at both ends
    let (dw, dh) = (3.5 * k, 0.71 * h);
    let svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {dw:.3} {dh:.3}" width="{dw:.3}mm" height="{dh:.3}mm"><defs><linearGradient id="s" x1="0" y1="0" x2="1" y2="0"><stop offset="0" stop-color="#000" stop-opacity="0"/><stop offset="1" stop-color="#000" stop-opacity="0.12"/></linearGradient><linearGradient id="v" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#fff" stop-opacity="0"/><stop offset="0.25" stop-color="#fff"/><stop offset="0.75" stop-color="#fff"/><stop offset="1" stop-color="#fff" stop-opacity="0"/></linearGradient><mask id="m"><rect width="{dw:.3}" height="{dh:.3}" fill="url(#v)"/></mask></defs><g mask="url(#m)"><rect width="{dw:.3}" height="{dh:.3}" fill="url(#s)"/><rect x="{lx:.3}" width="0.18" height="{dh:.3}" fill="#cfd5dc"/></g></svg>"##,
        lx = dw - 0.18
    );
    let mut divider = Layer::new("Divider", LayerType::Image, Side::Back, line_x - dw / 2.0, oy + 0.09 * h + dh / 2.0, dw, dh);
    divider.src = Some(svg_data_url(&svg));
    doc.layers.push(divider);
    // the person
    let x0 = ox + 0.572 * w;
    let xr = ox + w - 4.0 * k;
    const DOE_W: f64 = 2.0874;
    let pt = (11.0 * k).min((xr - x0) / ((JOHN_W + SPACE_W + DOE_W) * crate::MM_PER_PT) * 0.98);
    let y = middle(pt, oy + 0.24 * h);
    doc.layers.push(Layer::text(Side::Back, (x0 + xr) / 2.0, y, xr - x0, "JOHN", "Inter", 800.0, pt, dark, Align::Left));
    let start = x0 + (JOHN_W + SPACE_W) * em(pt);
    doc.layers.push(Layer::text(Side::Back, (start + xr) / 2.0, y, xr - start, "DOE", "Inter", 400.0, pt, dark, Align::Left));
    let pt = 7.5 * k;
    doc.layers.push(Layer::text(Side::Back, (x0 + xr) / 2.0, middle(pt, oy + 0.335 * h), xr - x0, "Creative Designer", "Inter", 400.0, pt, teal, Align::Left));
    // contact rows: an icon, then the text
    let icons = [
        ("Address icon", format!(r##"<path d="M12 2.5c-3.9 0-7 3-7 6.9 0 5 7 12.1 7 12.1s7-7.1 7-12.1c0-3.9-3.1-6.9-7-6.9z" fill="{icon_c}"/><circle cx="12" cy="9.4" r="2.6" fill="#fff"/>"##)),
        ("Phone icon", format!(r##"<rect x="7" y="2.5" width="10" height="19" rx="2.2" fill="{icon_c}"/><rect x="8.6" y="5" width="6.8" height="12.4" rx="0.6" fill="#fff"/><circle cx="12" cy="19.3" r="0.95" fill="#fff"/>"##)),
        ("Website icon", format!(r##"<circle cx="12" cy="12" r="9.2" fill="{icon_c}"/><g fill="none" stroke="#fff" stroke-width="1.3"><ellipse cx="12" cy="12" rx="3.9" ry="9.2"/><path d="M2.8 12h18.4M4.4 7.4h15.2M4.4 16.6h15.2"/></g>"##)),
        ("Email icon", format!(r##"<rect x="2.5" y="5.5" width="19" height="13" rx="1.8" fill="{icon_c}"/><path d="M3.6 7.2l8.4 6.4 8.4-6.4" fill="none" stroke="#fff" stroke-width="1.6" stroke-linejoin="round" stroke-linecap="round"/>"##)),
    ];
    let rows = [("123 Anywhere Street, Any City, ST 12345", 0.44), ("+123 456 7890", 0.532), ("www.website.com", 0.615), ("hello@website.com", 0.693)];
    let pt = 5.6 * k;
    let tx = x0 + 4.5 * k;
    let s = 2.6 * k;
    for ((name, art), (text, at)) in icons.into_iter().zip(rows) {
        let y = oy + at * h;
        let mut icon = Layer::new(name, LayerType::Image, Side::Back, x0 + 1.3 * k, y, s, s);
        icon.src = Some(svg_data_url(&format!(r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="48" height="48">{art}</svg>"##)));
        doc.layers.push(icon);
        let mut t = Layer::text(Side::Back, (tx + xr) / 2.0, y, xr - tx, text, "Inter", 400.0, pt, ink, Align::Left);
        if text.len() > 30 {
            t.wrap = Some(true);
            t.line_height = Some(1.25);
            t.h = 2.0 * 1.25 * em(pt);
        }
        doc.layers.push(t);
    }
    doc.layers.push(website(Side::Back));
}

/// the two Ingress factions, for the agent biocard templates
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Faction {
    Enlightened,
    Resistance,
}

impl Faction {
    pub fn of_preset(id: &str) -> Option<Faction> {
        match id {
            "game-biocard-enl" => Some(Faction::Enlightened),
            "game-biocard-res" => Some(Faction::Resistance),
            _ => None,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Faction::Enlightened => "ENLIGHTENED",
            Faction::Resistance => "RESISTANCE",
        }
    }
    /// (faction colour, dark background, darker background, light text)
    pub fn colors(self) -> (&'static str, &'static str, &'static str, &'static str) {
        match self {
            Faction::Enlightened => ("#03dc03", "#0c2a14", "#020905", "#c9f7cf"),
            Faction::Resistance => ("#0088ff", "#0b2140", "#02060e", "#cfe6ff"),
        }
    }
}

/// An Ingress agent biocard (the cards agents trade at events): codename, faction, level, a photo
/// to replace, a few stats and a QR code to the agent's Telegram — the back carries a faction ring.
/// Only the faction names and colours are used; no game logos or artwork.
fn biocard(doc: &mut Doc, faction: Faction) {
    let f = doc.format.clone();
    let (pw, ph) = (f.w, f.h);
    // laid out on a 63.5 × 88.9 mm card, scaled to the format
    let (kx, ky) = (pw / 63.5, ph / 88.9);
    let k = kx.min(ky);
    let at = |x: f64, y: f64| (x * kx, y * ky);
    let (glow, dark, darker, light) = faction.colors();
    doc.paper = Paper { stock: Stock::Silk, color: Stock::Silk.info().color.into() };
    doc.front = Background { mode: BgMode::Gradient, color: dark.into(), color2: darker.into(), angle: 90.0, src: None };
    doc.back = Background { mode: BgMode::Gradient, color: dark.into(), color2: darker.into(), angle: 120.0, src: None };
    let mut push = |l: Layer| doc.layers.push(l);

    // ---- front
    let (cx, cy) = at(31.75, 44.45);
    let mut frame = Layer::rect(Side::Front, cx, cy, pw - 6.0 * k, ph - 6.0 * k, glow);
    frame.name = "Frame".into();
    frame.fill = None;
    frame.stroke = Some(glow.into());
    frame.stroke_w = Some(0.45);
    frame.radius = Some(2.0 * k);
    push(frame);
    let (x, y) = at(31.75, 9.0);
    let mut head = Layer::text(Side::Front, x, y, 50.0 * kx, "AGENT BIOCARD", "Oswald", 500.0, 6.5 * k, glow, Align::Center);
    head.letter_spacing = Some(0.3);
    push(head);
    let photo_svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="400" height="400" viewBox="0 0 100 100"><rect width="100" height="100" rx="4" fill="{darker}"/><rect x="1" y="1" width="98" height="98" rx="3.5" fill="none" stroke="{glow}" stroke-width="1.2" stroke-opacity="0.7"/><circle cx="50" cy="38" r="17" fill="{glow}" fill-opacity="0.85"/><path d="M17 99 C17 72 33 61 50 61 C67 61 83 72 83 99 Z" fill="{glow}" fill-opacity="0.85"/><text x="50" y="93" font-family="Inter, sans-serif" font-size="4.1" font-weight="600" text-anchor="middle" fill="{darker}">REPLACE WITH YOUR PHOTO</text></svg>"##
    );
    let (x, y) = at(31.75, 30.0);
    let mut photo = Layer::new("Agent photo — Replace…", LayerType::Image, Side::Front, x, y, 34.0 * k, 34.0 * k);
    photo.src = Some(svg_data_url(&photo_svg));
    push(photo);
    let (x, y) = at(47.5, 15.0);
    let mut badge = Layer::rect(Side::Front, x, y, 10.0 * k, 5.4 * k, glow);
    badge.name = "Level badge".into();
    badge.radius = Some(1.0 * k);
    push(badge);
    let (x, y) = at(47.5, 15.0);
    push(Layer::text(Side::Front, x, y, 9.0 * k, "L16", "Oswald", 600.0, 8.0 * k, darker, Align::Center));
    let (x, y) = at(31.75, 53.5);
    push(Layer::text(Side::Front, x, y, 52.0 * kx, "AgentCodename", "Oswald", 600.0, 16.0 * k, "#ffffff", Align::Center));
    let (x, y) = at(31.75, 60.3);
    let mut fac = Layer::text(Side::Front, x, y, 52.0 * kx, faction.name(), "Oswald", 400.0, 7.0 * k, glow, Align::Center);
    fac.letter_spacing = Some(0.28);
    push(fac);
    let (x, y) = at(23.0, 73.5);
    let mut stats = Layer::text(Side::Front, x, y, 31.0 * kx, "Level 16\nAgent since 2014\nKuala Lumpur\n@agentcodename", "Inter", 500.0, 5.6 * k, light, Align::Left);
    stats.line_height = Some(1.5);
    stats.name = "Stats".into();
    push(stats);
    let (x, y) = at(50.0, 73.5);
    let mut qr = Layer::qr(Side::Front, x, y, 17.0 * k, QrSpec::url("https://t.me/agentcodename"), "#0a0f0c");
    qr.fill = Some("#ffffff".into());
    qr.radius = Some(1.0);
    push(qr);

    // ---- back: a faction ring
    let (bx, by) = at(31.75, 38.0);
    let mut ring = Layer::rect(Side::Back, bx, by, 40.0 * k, 40.0 * k, glow);
    ring.shape = Some(ShapeKind::Ellipse);
    ring.fill = None;
    ring.stroke = Some(glow.into());
    ring.stroke_w = Some(1.3 * k);
    ring.name = "Outer ring".into();
    push(ring);
    let mut inner = Layer::rect(Side::Back, bx, by, 27.0 * k, 27.0 * k, glow);
    inner.shape = Some(ShapeKind::Ellipse);
    inner.fill = None;
    inner.stroke = Some(glow.into());
    inner.stroke_w = Some(0.5 * k);
    inner.opacity = 0.6;
    inner.name = "Inner ring".into();
    push(inner);
    let mut core = Layer::rect(Side::Back, bx, by, 9.0 * k, 9.0 * k, glow);
    core.rot = 45.0;
    core.name = "Core".into();
    push(core);
    let mut frame_b = Layer::rect(Side::Back, cx, cy, pw - 6.0 * k, ph - 6.0 * k, glow);
    frame_b.name = "Frame".into();
    frame_b.fill = None;
    frame_b.stroke = Some(glow.into());
    frame_b.stroke_w = Some(0.45);
    frame_b.radius = Some(2.0 * k);
    push(frame_b);
    let (x, y) = at(31.75, 68.0);
    let mut name = Layer::text(Side::Back, x, y, 52.0 * kx, faction.name(), "Oswald", 600.0, 11.0 * k, glow, Align::Center);
    name.letter_spacing = Some(0.2);
    push(name);
    let (x, y) = at(31.75, 76.0);
    push(Layer::text(Side::Back, x, y, 52.0 * kx, "Trade me at the next anomaly", "Inter", 400.0, 5.5 * k, light, Align::Center));
}

/// how far a folded card stands open in 3D at first: a greeting card like a book on a shelf,
/// a tent card like a little roof
pub fn default_open(fold: Fold) -> f64 {
    match fold {
        Fold::Top => 62.0,
        _ => 110.0,
    }
}

#[derive(Debug)]
pub enum LoadError {
    Json(serde_json::Error),
    NotADesign,
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadError::Json(e) => write!(f, "not valid JSON: {e}"),
            LoadError::NotADesign => write!(f, "not a CardBuilder design"),
        }
    }
}

/// File extension of CardBuilder design files.
pub const CB_EXT: &str = "cb";
/// `format` value that marks a `.cb` file.
pub const CB_FORMAT: &str = "cardbuilder-design";

/// A `.cb` design file: JSON with a small header around the design (images embedded as data
/// URLs), so one file reopens exactly as it was saved. `app` names the program that wrote it.
pub fn save_cb(doc: &Doc, app: &str) -> String {
    let v = serde_json::json!({
        "format": CB_FORMAT,
        "formatVersion": 1,
        "app": app,
        "design": doc,
    });
    serde_json::to_string_pretty(&v).expect("documents always serialise")
}

/// What a template file says about itself: where it shows in the card picker.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateInfo {
    pub id: String,
    pub name: String,
    /// `format::Category::id`
    pub category: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(default)]
    pub note: String,
}

/// a `.cb` file that is also a template (it shows in the card picker)
pub fn save_cb_template(doc: &Doc, app: &str, info: &TemplateInfo) -> String {
    let mut d = doc.clone();
    d.template = Some(info.id.clone());
    let v = serde_json::json!({
        "format": CB_FORMAT,
        "formatVersion": 1,
        "app": app,
        "template": info,
        "design": d,
    });
    serde_json::to_string_pretty(&v).expect("documents always serialise")
}

/// the template header of a `.cb` file, if it is a template
pub fn template_info(text: &str) -> Option<TemplateInfo> {
    let v: serde_json::Value = serde_json::from_str(text).ok()?;
    if v.get("format").and_then(|f| f.as_str()) != Some(CB_FORMAT) {
        return None;
    }
    serde_json::from_value(v.get("template")?.clone()).ok()
}

/// the bare design JSON (autosave)
pub fn save_doc(doc: &Doc) -> String {
    serde_json::to_string(doc).expect("documents always serialise")
}

pub fn load_doc(json: &str) -> Result<Doc, LoadError> {
    let v: serde_json::Value = serde_json::from_str(json).map_err(LoadError::Json)?;
    if v.get("version").and_then(|x| x.as_u64()) != Some(1) || !v.get("layers").is_some_and(|l| l.is_array()) {
        return Err(LoadError::NotADesign);
    }
    let mut doc: Doc = serde_json::from_value(v).map_err(LoadError::Json)?;
    sanitize(&mut doc);
    Ok(doc)
}

/// Read a design: a `.cb` file, or the bare design JSON.
pub fn load_design(text: &str) -> Result<Doc, LoadError> {
    let v: serde_json::Value = serde_json::from_str(text).map_err(LoadError::Json)?;
    if v.get("format").and_then(|f| f.as_str()) == Some(CB_FORMAT) {
        let design = v.get("design").ok_or(LoadError::NotADesign)?;
        return load_doc(&design.to_string());
    }
    load_doc(text)
}

/// keep numbers in range so a hand-edited or damaged file can't break drawing
fn sanitize(doc: &mut Doc) {
    let f = &mut doc.format;
    f.w = f.w.clamp(10.0, 1000.0);
    f.h = f.h.clamp(10.0, 1000.0);
    f.radius = f.radius.clamp(0.0, f.w.min(f.h) / 2.0);
    f.bleed = f.bleed.clamp(0.0, 10.0);
    f.safe = f.safe.clamp(0.0, f.w.min(f.h) / 3.0);
    doc.scene.open = doc.scene.open.clamp(0.0, 180.0);
    for l in &mut doc.layers {
        l.w = l.w.max(0.1);
        l.h = l.h.max(0.1);
        l.opacity = l.opacity.clamp(0.0, 1.0);
        if let Some(s) = &mut l.size {
            *s = s.clamp(1.0, 500.0);
        }
    }
    let cols = doc.data.columns.len();
    for r in &mut doc.data.rows {
        r.resize(cols, String::new());
    }
}

/// which part of the card a panel is, so artwork can move between flat and folded formats
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Role {
    FrontCover,
    BackCover,
    InsideFirst,
    InsideSecond,
}

fn roles(f: &CardFormat) -> Vec<(Side, Panel, Role)> {
    let mut out = Vec::new();
    for side in [Side::Front, Side::Back] {
        let ps = f.panels(side);
        for (i, p) in ps.iter().enumerate() {
            let role = match (f.fold, side, i) {
                (Fold::None, Side::Front, _) => Role::FrontCover,
                (Fold::None, Side::Back, _) => Role::BackCover,
                (_, Side::Front, 0) => Role::BackCover,
                (_, Side::Front, _) => Role::FrontCover,
                (_, Side::Back, 0) => Role::InsideFirst,
                (_, Side::Back, _) => Role::InsideSecond,
            };
            out.push((side, *p, role));
        }
    }
    out
}

/// Carry artwork over to another format: each layer keeps its place on its panel (front cover,
/// back cover, inside), scaled to the new panel size; text sizes scale with it.
pub fn remap_layers(layers: &[Layer], from: &CardFormat, to: &CardFormat) -> Vec<Layer> {
    let old = roles(from);
    let new = roles(to);
    layers
        .iter()
        .map(|l| {
            // the old panel holding the layer's centre (else the nearest one on its side)
            let (_, op, role) = old
                .iter()
                .filter(|(s, _, _)| *s == l.side)
                .min_by(|a, b| {
                    let d = |p: &Panel| {
                        let dx = (l.x - (p.x + p.w / 2.0)).abs() - p.w / 2.0;
                        let dy = (l.y - (p.y + p.h / 2.0)).abs() - p.h / 2.0;
                        dx.max(0.0) + dy.max(0.0)
                    };
                    d(&a.1).total_cmp(&d(&b.1))
                })
                .copied()
                .unwrap_or(old[0]);
            let target = new.iter().find(|(_, _, r)| *r == role).or_else(|| {
                // flat formats have no inside panels: inside artwork goes to the back
                let fallback = if matches!(role, Role::InsideFirst | Role::InsideSecond) { Role::BackCover } else { Role::FrontCover };
                new.iter().find(|(_, _, r)| *r == fallback)
            });
            let Some(&(nside, np, _)) = target else { return l.clone() };
            let (mut u, mut v) = ((l.x - op.x) / op.w, (l.y - op.y) / op.h);
            let mut rot = l.rot;
            if op.upside_down != np.upside_down {
                u = 1.0 - u;
                v = 1.0 - v;
                rot = (rot + 180.0) % 360.0;
            }
            let k = (np.w / op.w).min(np.h / op.h);
            let mut n = l.clone();
            n.side = nside;
            n.x = np.x + u * np.w;
            n.y = np.y + v * np.h;
            n.w = l.w * k;
            n.h = l.h * k;
            n.rot = rot;
            if let Some(s) = n.size {
                n.size = Some((s * k).clamp(1.0, 500.0));
            }
            n
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::{preset, PRESETS};

    #[test]
    fn variants_show_their_layers_and_set_colours() {
        let mut doc = default_doc(CardFormat::from_preset(preset("game-poker").unwrap()));
        let mut logo = |id: &str| {
            let mut l = Layer::new(id, LayerType::Image, Side::Back, 10.0, 10.0, 5.0, 5.0);
            l.id = id.into();
            doc.layers.push(l);
        };
        logo("res"); logo("enl"); logo("xfac");
        let mut name = Layer::new("Agent name", LayerType::Text, Side::Back, 10.0, 10.0, 5.0, 5.0);
        name.id = "name".into();
        doc.layers.push(name);
        let opt = |n: &str, c: &str, show: &[&str]| VariantOption { name: n.into(), color: c.into(), show: show.iter().map(|s| s.to_string()).collect(), recolor: vec![("name".into(), c.into())] };
        doc.variants = Some(Variants { label: "Faction".into(), hint: None, options: vec![opt("RES", "#0097ca", &["res"]), opt("ENL", "#23c316", &["enl"]), opt("XFAC", "#e0262b", &["xfac"])], active: None });
        let shown = |d: &Doc| d.layers.iter().filter(|l| ["res", "enl", "xfac"].contains(&l.id.as_str()) && l.visible).map(|l| l.id.clone()).collect::<Vec<_>>();
        let colour = |d: &Doc| d.layers.iter().find(|l| l.id == "name").unwrap().color.clone();
        doc.apply_variant(1);
        assert_eq!(shown(&doc), ["enl"]);
        assert_eq!(colour(&doc).as_deref(), Some("#23c316"));
        doc.apply_variant(2);
        assert_eq!(shown(&doc), ["xfac"]);
        assert_eq!(doc.variants.as_ref().unwrap().active, Some(2));
        // saved and opened again, the choice is kept
        let back = load_design(&save_cb(&doc, "test")).unwrap();
        assert_eq!(back.variants, doc.variants);
        // a design without a choice saves as before
        let plain = default_doc(CardFormat::from_preset(preset("game-poker").unwrap()));
        assert!(!save_cb(&plain, "test").contains("variants"));
    }

    #[test]
    fn vector_shapes_take_the_faction_colour_and_round_trip() {
        let mut doc = default_doc(CardFormat::from_preset(preset("game-poker").unwrap()));
        let dots = VectorPath { d: "M0 0h1v1h-1zM2 0h1v1h-1z".into(), w: 3.0, h: 1.0, even_odd: false };
        let mut logo = Layer::vector(Side::Back, 30.0, 40.0, 9.0, 3.0, dots, Some("#03bfff"));
        logo.id = "logo".into();
        let mut ring = Layer::vector(Side::Back, 30.0, 40.0, 9.0, 3.0, VectorPath { d: "M0 0L5 5".into(), w: 5.0, h: 5.0, even_odd: true }, None);
        ring.id = "ring".into();
        ring.stroke = Some("#000000".into());
        ring.stroke_w = Some(0.3);
        ring.dash = Some(Dash::Dotted);
        let mut photo = Layer::new("Photo", LayerType::Image, Side::Back, 30.0, 40.0, 9.0, 3.0);
        photo.clip = true;
        doc.layers.extend([logo, ring, photo]);
        let opt = |n: &str, c: &str| VariantOption { name: n.into(), color: c.into(), show: vec![], recolor: vec![("logo".into(), c.into()), ("ring".into(), c.into())] };
        doc.variants = Some(Variants { label: "Faction".into(), hint: None, options: vec![opt("RES", "#03bfff"), opt("XFAC", "#ed1c24")], active: None });
        doc.apply_variant(1);
        let get = |d: &Doc, id: &str| d.layers.iter().find(|l| l.id == id).unwrap().clone();
        // a filled shape takes the colour as its fill, an outline-only one as its outline
        assert_eq!(get(&doc, "logo").fill.as_deref(), Some("#ed1c24"));
        assert_eq!((get(&doc, "ring").fill, get(&doc, "ring").stroke.as_deref()), (None, Some("#ed1c24")));
        let back = load_design(&save_cb(&doc, "test")).unwrap();
        assert_eq!(back.layers, doc.layers);
        let json = save_cb(&doc, "test");
        assert!(json.contains("\"dash\": \"dotted\"") && json.contains("\"clip\": true") && json.contains("\"evenOdd\": true"), "{json}");
        // layers without the new settings save as before
        assert!(!save_cb(&default_doc(CardFormat::from_preset(preset("game-poker").unwrap())), "test").contains("\"clip\""));
        assert_eq!(Dash::Dotted.intervals(0.5)[1] + Dash::Dotted.intervals(0.5)[0], 1.0, "dots one width apart");
    }

    #[test]
    fn every_preset_gets_a_design_inside_its_sheet() {
        for p in PRESETS {
            let d = default_doc(CardFormat::from_preset(p));
            assert!(!d.layers.is_empty(), "{}", p.id);
            let (sw, sh) = d.format.sheet();
            for l in &d.layers {
                assert!(l.x > 0.0 && l.x < sw && l.y > 0.0 && l.y < sh, "{}: {} at {},{}", p.id, l.name, l.x, l.y);
            }
        }
    }

    #[test]
    fn biocards_in_both_factions() {
        for (id, colour, name) in [("game-biocard-enl", "#03dc03", "ENLIGHTENED"), ("game-biocard-res", "#0088ff", "RESISTANCE")] {
            let d = default_doc(CardFormat::from_preset(preset(id).unwrap()));
            assert_eq!((d.format.w, d.format.h), (63.5, 88.9), "{id}: trading-card size, portrait");
            assert!(d.layers.iter().any(|l| l.text.as_deref() == Some("AgentCodename") && l.side == Side::Front), "{id}");
            assert!(d.layers.iter().any(|l| l.text.as_deref() == Some(name)), "{id}: faction named");
            assert!(d.layers.iter().any(|l| l.kind == LayerType::Qr && l.qr.as_ref().unwrap().text.contains("t.me/")), "{id}: Telegram QR");
            assert!(d.layers.iter().any(|l| l.kind == LayerType::Image && l.src.as_deref().is_some_and(|s| s.starts_with("data:image/svg+xml"))), "{id}: photo to replace");
            assert!(d.layers.iter().any(|l| l.stroke.as_deref() == Some(colour)), "{id}: faction colour");
            assert!(d.layers_on(Side::Back).count() >= 4, "{id}: the back has the faction ring");
        }
    }

    #[test]
    fn templates_say_where_they_belong() {
        let d = default_doc(CardFormat::from_preset(preset("game-tcg").unwrap()));
        let info = TemplateInfo { id: "t1".into(), name: "Mine".into(), category: "game".into(), group: Some("Ingress Biocard".into()), note: "63 × 88".into() };
        let text = save_cb_template(&d, "test", &info);
        assert_eq!(template_info(&text), Some(info));
        let back = load_design(&text).unwrap();
        assert_eq!(back.template.as_deref(), Some("t1"));
        assert_eq!(template_info(&save_cb(&d, "test")), None);
    }

    #[test]
    fn cb_files_round_trip() {
        let mut d = default_doc(CardFormat::from_preset(preset("greet-a6").unwrap()));
        d.data = DataSet { name: "guests.csv".into(), columns: vec!["Name".into()], rows: vec![vec!["Ana".into()], vec!["Ben".into()]] };
        let text = save_cb(&d, "test");
        assert!(text.contains("\"format\": \"cardbuilder-design\""));
        assert_eq!(load_design(&text).unwrap(), d);
        assert_eq!(load_design(&save_doc(&d)).unwrap(), d);
        assert!(load_design("{\"hello\": 1}").is_err());
    }

    #[test]
    fn remapping_flat_to_folded_and_back_keeps_layers_on_their_panel() {
        let flat = CardFormat::from_preset(preset("post-a6").unwrap());
        let folded = CardFormat::from_preset(preset("greet-top").unwrap());
        let d = default_doc(flat.clone());
        let there = remap_layers(&d.layers, &flat, &folded);
        let fp = folded.front_panel();
        for (a, b) in d.layers.iter().zip(&there) {
            // the front goes to the front cover; the back of a flat card becomes the back cover,
            // which is on the outside of a folded card
            assert_eq!(b.side, Side::Front, "{}", a.name);
            if a.side == Side::Front {
                assert!(b.y > fp.y && b.y < fp.y + fp.h, "on the front cover");
            } else {
                assert!(b.y < folded.h, "back artwork on the (upside-down) back cover");
                assert!((b.rot - (a.rot + 180.0) % 360.0).abs() < 1e-9);
            }
        }
        let back = remap_layers(&there, &folded, &flat);
        for (a, b) in d.layers.iter().zip(&back) {
            assert!((a.x - b.x).abs() < 1e-6 && (a.y - b.y).abs() < 1e-6 && (a.rot - b.rot).abs() < 1e-6, "{} came back to {},{} from {},{}", a.name, b.x, b.y, a.x, a.y);
        }
    }

    #[test]
    fn qr_payloads() {
        let id = |s: &str| s.to_string();
        let mut q = QrSpec { kind: QrKind::Vcard, name: "Alex Morgan".into(), phone: "+60 12".into(), org: "A, B; C".into(), ..Default::default() };
        let v = q.payload(&id);
        assert!(v.starts_with("BEGIN:VCARD\nVERSION:3.0\nN:Morgan;Alex;;;\nFN:Alex Morgan\n"), "{v}");
        assert!(v.contains("ORG:A\\, B\\; C\n") && v.ends_with("END:VCARD"));
        q = QrSpec { kind: QrKind::Wifi, ssid: "Cafe;1".into(), password: "p:w".into(), ..Default::default() };
        assert_eq!(q.payload(&id), "WIFI:T:WPA;S:Cafe\\;1;P:p\\:w;;");
        q = QrSpec { kind: QrKind::Phone, text: "+60 (12) 345".into(), ..Default::default() };
        assert_eq!(q.payload(&id), "tel:+6012345");
    }
}
