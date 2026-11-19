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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ShapeKind {
    #[default]
    Rect,
    Ellipse,
    Line,
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
            src: None,
            shape: None,
            fill: None,
            stroke: None,
            stroke_w: None,
            radius: None,
            qr: None,
            foil: None,
            spot_uv: false,
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
}

impl Default for SceneSettings {
    fn default() -> Self {
        SceneSettings { backdrop: Backdrop::Studio, bg_color: "#c9cbe0".into(), light: 1.0, light_angle: -35.0, shadows: true, open: 110.0, auto_rotate: false }
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
}

impl Doc {
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
        format,
    };
    let f = doc.format.clone();
    doc.scene.open = default_open(f.fold);
    let front = f.front_panel();
    let (pw, ph) = (front.w, front.h);
    let k = pw.min(ph) / 55.0; // scale from a business card
    let ink = "#1f2347";
    let accent = "#ff7a59";
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
