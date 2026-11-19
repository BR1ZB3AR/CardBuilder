//! Card formats: every kind of card CardBuilder makes, from business cards to folded greeting
//! cards, and the geometry that follows from a format (the flat sheet, its panels, the fold).
//!
//! A card has two printed sides. A flat card's sides are its front and back. A folded card is
//! printed on one flat sheet: its *outside* (front cover, back cover) and its *inside* spread.
//!
//! - Side fold (opens like a book): outside = [back cover | front cover], inside = [left | right].
//! - Top fold (opens upwards; tent and place cards): outside = [back cover / front cover], the back
//!   cover printed upside down so it reads upright once folded; inside = [top / bottom].

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Fold {
    /// a flat card: front and back
    #[default]
    None,
    /// folded down the middle, opens like a book (greeting cards)
    Side,
    /// folded across the middle, opens upwards (tent and place cards, top-fold greeting cards)
    Top,
}

impl Fold {
    pub fn label(self) -> &'static str {
        match self {
            Fold::None => "Flat",
            Fold::Side => "Side fold",
            Fold::Top => "Top fold",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Category {
    Business,
    Id,
    Game,
    Postcard,
    Greeting,
    Tent,
    Tag,
    Custom,
}

impl Category {
    /// a short stable name for files (templates say which category they belong in)
    pub fn id(self) -> &'static str {
        match self {
            Category::Business => "business",
            Category::Id => "id",
            Category::Game => "game",
            Category::Postcard => "postcard",
            Category::Greeting => "greeting",
            Category::Tent => "tent",
            Category::Tag => "tag",
            Category::Custom => "custom",
        }
    }
    pub fn from_id(id: &str) -> Category {
        Category::ALL.into_iter().find(|c| c.id() == id).unwrap_or(Category::Custom)
    }
    pub const ALL: [Category; 8] = [Category::Business, Category::Id, Category::Game, Category::Postcard, Category::Greeting, Category::Tent, Category::Tag, Category::Custom];
    pub fn name(self) -> &'static str {
        match self {
            Category::Business => "Business cards",
            Category::Id => "ID & gift cards",
            Category::Game => "Playing & game cards",
            Category::Postcard => "Postcards & invitations",
            Category::Greeting => "Greeting cards",
            Category::Tent => "Tent & place cards",
            Category::Tag => "Bookmarks & tags",
            Category::Custom => "Custom size",
        }
    }
    /// what a card of this kind is called in the UI ("business card")
    pub fn noun(self) -> &'static str {
        match self {
            Category::Business => "business card",
            Category::Id => "ID card",
            Category::Game => "game card",
            Category::Postcard => "postcard",
            Category::Greeting => "greeting card",
            Category::Tent => "tent card",
            Category::Tag => "tag",
            Category::Custom => "card",
        }
    }
}

/// a ready-made card size
#[derive(Clone, Copy, Debug)]
pub struct Preset {
    pub id: &'static str,
    pub category: Category,
    pub name: &'static str,
    /// where it's used / what it's for
    pub note: &'static str,
    /// finished (folded) size, mm, as normally held
    pub w: f64,
    pub h: f64,
    pub radius: f64,
    pub fold: Fold,
    /// a group inside the category (shown together under its own heading)
    pub group: Option<&'static str>,
}

const fn p(id: &'static str, category: Category, name: &'static str, note: &'static str, w: f64, h: f64, radius: f64, fold: Fold) -> Preset {
    Preset { id, category, name, note, w, h, radius, fold, group: None }
}

/// a preset in a group
#[allow(clippy::too_many_arguments)]
const fn pg(id: &'static str, category: Category, group: &'static str, name: &'static str, note: &'static str, w: f64, h: f64, radius: f64) -> Preset {
    Preset { id, category, name, note, w, h, radius, fold: Fold::None, group: Some(group) }
}

/// the group of Ingress agent biocards in Playing & game cards
pub const INGRESS_BIOCARD: &str = "Ingress Biocard";

use Category::*;
pub const PRESETS: &[Preset] = &[
    p("bc-eu", Business, "Europe", "85 × 55 mm · most of Europe", 85.0, 55.0, 0.0, Fold::None),
    p("bc-us", Business, "US / Canada", "3.5 × 2 in", 88.9, 50.8, 0.0, Fold::None),
    p("bc-asia", Business, "Asia", "90 × 54 mm · Malaysia, Singapore, China, HK", 90.0, 54.0, 0.0, Fold::None),
    p("bc-jp", Business, "Japan", "91 × 55 mm", 91.0, 55.0, 0.0, Fold::None),
    p("bc-au", Business, "Australia / NZ / India", "90 × 55 mm", 90.0, 55.0, 0.0, Fold::None),
    p("bc-round", Business, "Rounded corners", "85 × 55 mm, 3 mm corners", 85.0, 55.0, 3.0, Fold::None),
    p("bc-square", Business, "Square", "65 × 65 mm", 65.0, 65.0, 0.0, Fold::None),
    p("bc-mini", Business, "Mini / slim", "85 × 28 mm", 85.0, 28.0, 0.0, Fold::None),
    p("id-cr80", Id, "ID / membership card", "CR80 · 85.6 × 54 mm, like a bank card", 85.6, 53.98, 3.18, Fold::None),
    p("id-badge", Id, "Badge (portrait)", "CR80 upright · lanyards, staff passes", 53.98, 85.6, 3.18, Fold::None),
    p("id-gift", Id, "Gift / loyalty card", "CR80 · gift vouchers, stamp cards", 85.6, 53.98, 3.18, Fold::None),
    p("id-key", Id, "Key tag", "CR50 · 54 × 28.6 mm", 54.0, 28.6, 2.0, Fold::None),
    p("game-poker", Game, "Poker", "2.5 × 3.5 in · standard playing cards", 63.5, 88.9, 3.5, Fold::None),
    p("game-bridge", Game, "Bridge", "2.25 × 3.5 in", 57.15, 88.9, 3.5, Fold::None),
    p("game-tcg", Game, "Trading card", "63 × 88 mm · collectible card games", 63.0, 88.0, 3.0, Fold::None),
    p("game-tarot", Game, "Tarot", "70 × 120 mm", 70.0, 120.0, 4.0, Fold::None),
    p("game-mini", Game, "Mini", "44 × 63 mm · board-game cards", 44.0, 63.0, 3.0, Fold::None),
    p("game-square", Game, "Square", "70 × 70 mm", 70.0, 70.0, 3.0, Fold::None),
    pg("game-biocard-enl", Game, INGRESS_BIOCARD, "Enlightened", "2.5 × 3.5 in · green agent card", 63.5, 88.9, 3.0),
    pg("game-biocard-res", Game, INGRESS_BIOCARD, "Resistance", "2.5 × 3.5 in · blue agent card", 63.5, 88.9, 3.0),
    p("post-a6", Postcard, "A6 postcard", "148 × 105 mm", 148.0, 105.0, 0.0, Fold::None),
    p("post-4x6", Postcard, "4 × 6 in postcard", "152 × 102 mm · US", 152.4, 101.6, 0.0, Fold::None),
    p("post-5x7", Postcard, "5 × 7 in invitation", "127 × 178 mm · weddings, parties", 127.0, 177.8, 0.0, Fold::None),
    p("post-a5", Postcard, "A5 card", "148 × 210 mm · menus, invitations", 148.0, 210.0, 0.0, Fold::None),
    p("post-dl", Postcard, "DL card", "99 × 210 mm · compliments slips", 99.0, 210.0, 0.0, Fold::None),
    p("post-sq", Postcard, "Square invitation", "148 × 148 mm", 148.0, 148.0, 0.0, Fold::None),
    p("greet-a6", Greeting, "A6 greeting card", "105 × 148 mm folded · prints on A5", 105.0, 148.0, 0.0, Fold::Side),
    p("greet-a5", Greeting, "A5 greeting card", "148 × 210 mm folded · prints on A4", 148.0, 210.0, 0.0, Fold::Side),
    p("greet-5x7", Greeting, "5 × 7 in greeting card", "127 × 178 mm folded", 127.0, 177.8, 0.0, Fold::Side),
    p("greet-sq", Greeting, "Square greeting card", "145 × 145 mm folded", 145.0, 145.0, 0.0, Fold::Side),
    p("greet-dl", Greeting, "DL greeting card", "99 × 210 mm folded", 99.0, 210.0, 0.0, Fold::Side),
    p("greet-top", Greeting, "A6 top-fold card", "148 × 105 mm, opens upwards", 148.0, 105.0, 0.0, Fold::Top),
    p("tent-place", Tent, "Place card", "90 × 50 mm folded · names at the table", 90.0, 50.0, 0.0, Fold::Top),
    p("tent-table", Tent, "Table tent", "148 × 105 mm folded · menus, table numbers", 148.0, 105.0, 0.0, Fold::Top),
    p("tent-desk", Tent, "Desk name plate", "210 × 74 mm folded", 210.0, 74.0, 0.0, Fold::Top),
    p("tag-bookmark", Tag, "Bookmark", "50 × 150 mm", 50.0, 150.0, 0.0, Fold::None),
    p("tag-hang", Tag, "Hang tag", "50 × 90 mm · clothing, price tags", 50.0, 90.0, 3.0, Fold::None),
    p("tag-gift", Tag, "Gift tag", "55 × 85 mm", 55.0, 85.0, 4.0, Fold::None),
    p("tag-label", Tag, "Square label / sticker", "60 × 60 mm", 60.0, 60.0, 6.0, Fold::None),
];

pub fn preset(id: &str) -> Option<&'static Preset> {
    PRESETS.iter().find(|p| p.id == id)
}

/// the size and shape of a card
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CardFormat {
    /// the preset this came from ("custom" when the size was typed in)
    pub preset: String,
    /// finished (folded) card size, mm
    pub w: f64,
    pub h: f64,
    /// corner radius, mm (outer corners only on folded cards)
    pub radius: f64,
    pub fold: Fold,
    /// bleed: artwork beyond the trim, cut off when the cards are trimmed
    pub bleed: f64,
    /// safe area inset from the trim: keep text inside it
    pub safe: f64,
}

impl Default for CardFormat {
    fn default() -> Self {
        CardFormat::from_preset(preset("bc-eu").unwrap())
    }
}

/// one panel of the flat sheet (the whole sheet on a flat card)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Panel {
    pub name: &'static str,
    /// rectangle in sheet coordinates, mm
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    /// printed upside down (the back cover of a top-fold card)
    pub upside_down: bool,
}

impl CardFormat {
    pub fn from_preset(p: &Preset) -> CardFormat {
        CardFormat { preset: p.id.into(), w: p.w, h: p.h, radius: p.radius, fold: p.fold, bleed: 3.0, safe: if p.w.min(p.h) < 40.0 { 2.5 } else { 4.0 } }
    }

    pub fn category(&self) -> Category {
        preset(&self.preset).map(|p| p.category).unwrap_or(Category::Custom)
    }

    /// size of the flat printed sheet (the card itself when flat), mm
    pub fn sheet(&self) -> (f64, f64) {
        match self.fold {
            Fold::None => (self.w, self.h),
            Fold::Side => (2.0 * self.w, self.h),
            Fold::Top => (self.w, 2.0 * self.h),
        }
    }

    /// the flat sheet with bleed, mm
    pub fn sheet_with_bleed(&self) -> (f64, f64) {
        let (w, h) = self.sheet();
        (w + 2.0 * self.bleed, h + 2.0 * self.bleed)
    }

    /// "Front" / "Back", or "Outside" / "Inside" for folded cards
    pub fn side_name(&self, side: crate::doc::Side) -> &'static str {
        use crate::doc::Side;
        match (self.fold, side) {
            (Fold::None, Side::Front) => "Front",
            (Fold::None, Side::Back) => "Back",
            (_, Side::Front) => "Outside",
            (_, Side::Back) => "Inside",
        }
    }

    pub fn panels(&self, side: crate::doc::Side) -> Vec<Panel> {
        use crate::doc::Side;
        let (w, h) = (self.w, self.h);
        let pn = |name, x, y, upside_down| Panel { name, x, y, w, h, upside_down };
        match (self.fold, side) {
            (Fold::None, Side::Front) => vec![pn("Front", 0.0, 0.0, false)],
            (Fold::None, Side::Back) => vec![pn("Back", 0.0, 0.0, false)],
            (Fold::Side, Side::Front) => vec![pn("Back cover", 0.0, 0.0, false), pn("Front cover", w, 0.0, false)],
            (Fold::Side, Side::Back) => vec![pn("Inside left", 0.0, 0.0, false), pn("Inside right", w, 0.0, false)],
            (Fold::Top, Side::Front) => vec![pn("Back cover", 0.0, 0.0, true), pn("Front cover", 0.0, h, false)],
            (Fold::Top, Side::Back) => vec![pn("Inside top", 0.0, 0.0, false), pn("Inside bottom", 0.0, h, false)],
        }
    }

    /// the fold line in sheet coordinates (start, end), if folded
    pub fn fold_line(&self) -> Option<((f64, f64), (f64, f64))> {
        match self.fold {
            Fold::None => None,
            Fold::Side => Some(((self.w, 0.0), (self.w, self.h))),
            Fold::Top => Some(((0.0, self.h), (self.w, self.h))),
        }
    }

    /// the panel holding the front of the card (front cover)
    pub fn front_panel(&self) -> Panel {
        let ps = self.panels(crate::doc::Side::Front);
        *ps.last().unwrap()
    }

    /// Where a point of the front side's sheet is on the back side's sheet, both read the right way
    /// round: a flat or side-fold card turns over left-to-right, a top-fold card top-to-bottom.
    pub fn to_back(&self, x: f64, y: f64) -> (f64, f64) {
        let (sw, sh) = self.sheet();
        match self.fold {
            Fold::Top => (x, sh - y),
            _ => (sw - x, y),
        }
    }

    /// swap width and height (portrait ⇄ landscape)
    pub fn rotated(&self) -> CardFormat {
        CardFormat { w: self.h, h: self.w, ..self.clone() }
    }

    pub fn landscape(&self) -> bool {
        self.w >= self.h
    }

    /// the trim outline of the flat sheet (rounded outer corners), as a polygon, mm
    pub fn outline(&self, segments: usize) -> Vec<(f64, f64)> {
        let (sw, sh) = self.sheet();
        crate::geom::rounded_rect(0.0, 0.0, sw, sh, self.radius.min(self.w / 2.0).min(self.h / 2.0), segments)
    }

    /// "85 × 55 mm"
    pub fn size_label(&self) -> String {
        format!("{} × {} mm", trim_num(self.w), trim_num(self.h))
    }
}

/// 85.6 → "85.6", 85.0 → "85"
pub fn trim_num(v: f64) -> String {
    let s = format!("{:.1}", v);
    s.strip_suffix(".0").map(str::to_string).unwrap_or(s)
}

/// paper sizes for print sheets
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SheetSize {
    pub id: &'static str,
    pub name: &'static str,
    /// portrait size, mm
    pub w: f64,
    pub h: f64,
}

pub const SHEETS: &[SheetSize] = &[
    SheetSize { id: "a4", name: "A4", w: 210.0, h: 297.0 },
    SheetSize { id: "a3", name: "A3", w: 297.0, h: 420.0 },
    SheetSize { id: "sra3", name: "SRA3", w: 320.0, h: 450.0 },
    SheetSize { id: "a5", name: "A5", w: 148.0, h: 210.0 },
    SheetSize { id: "letter", name: "US Letter", w: 215.9, h: 279.4 },
    SheetSize { id: "legal", name: "US Legal", w: 215.9, h: 355.6 },
    SheetSize { id: "tabloid", name: "Tabloid", w: 279.4, h: 431.8 },
    SheetSize { id: "superb", name: "13 × 19 in", w: 330.2, h: 482.6 },
];

pub fn sheet_size(id: &str) -> &'static SheetSize {
    SHEETS.iter().find(|s| s.id == id).unwrap_or(&SHEETS[0])
}
