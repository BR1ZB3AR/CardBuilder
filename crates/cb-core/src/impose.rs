//! Print-sheet imposition: as many cards as fit on a sheet, crop marks for the guillotine, and
//! where each card's back goes on the reverse so double-sided printing lines up.
//!
//! Sheet coordinates are mm from the sheet's top-left corner, y down, as seen from the side
//! being printed.

use crate::doc::Side;
use crate::format::CardFormat;

/// a 2D affine map: x' = a·x + c·y + e, y' = b·x + d·y + f (PDF's `cm` order)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Affine {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub e: f64,
    pub f: f64,
}

impl Affine {
    pub const IDENTITY: Affine = Affine { a: 1.0, b: 0.0, c: 0.0, d: 1.0, e: 0.0, f: 0.0 };
    pub fn translate(x: f64, y: f64) -> Affine {
        Affine { e: x, f: y, ..Affine::IDENTITY }
    }
    pub fn scale(sx: f64, sy: f64) -> Affine {
        Affine { a: sx, d: sy, ..Affine::IDENTITY }
    }
    /// first `self`, then `next`
    pub fn then(self, next: Affine) -> Affine {
        let m = self;
        let n = next;
        Affine {
            a: n.a * m.a + n.c * m.b,
            b: n.b * m.a + n.d * m.b,
            c: n.a * m.c + n.c * m.d,
            d: n.b * m.c + n.d * m.d,
            e: n.a * m.e + n.c * m.f + n.e,
            f: n.b * m.e + n.d * m.f + n.f,
        }
    }
    pub fn apply(&self, x: f64, y: f64) -> (f64, f64) {
        (self.a * x + self.c * y + self.e, self.b * x + self.d * y + self.f)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImposeOptions {
    /// sheet size as chosen (portrait); the better orientation is picked
    pub sheet_w: f64,
    pub sheet_h: f64,
    pub margin: f64,
    pub gap: f64,
    /// bleed printed round each card (0 = none)
    pub bleed: f64,
    pub allow_rotate: bool,
    /// the printer turns sheets over on their short edge
    pub short_edge: bool,
}

/// cards laid out on a sheet
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    pub sheet_w: f64,
    pub sheet_h: f64,
    /// the flat card (trim) size
    pub card_w: f64,
    pub card_h: f64,
    pub bleed: f64,
    /// cards turned 90° clockwise on the sheet
    pub rotated: bool,
    pub cols: usize,
    pub rows: usize,
    /// top-left corner of each card's trim box on the sheet, row by row
    pub slots: Vec<(f64, f64)>,
    pub short_edge: bool,
}

/// lay cards out on a sheet: the orientation of the sheet (and of the cards, if allowed) that fits most wins
pub fn impose(card_w: f64, card_h: f64, o: &ImposeOptions) -> Layout {
    let (pw, ph) = (o.sheet_w.min(o.sheet_h), o.sheet_w.max(o.sheet_h));
    let mut best: Option<Layout> = None;
    for (sw, sh) in [(pw, ph), (ph, pw)] {
        for rotated in [false, true] {
            if rotated && !o.allow_rotate {
                continue;
            }
            let (cw, ch) = if rotated { (card_h, card_w) } else { (card_w, card_h) };
            let (cell_w, cell_h) = (cw + 2.0 * o.bleed, ch + 2.0 * o.bleed);
            let fit = |avail: f64, cell: f64| if avail < cell { 0 } else { ((avail + o.gap) / (cell + o.gap) + 1e-9).floor() as usize };
            let cols = fit(sw - 2.0 * o.margin, cell_w);
            let rows = fit(sh - 2.0 * o.margin, cell_h);
            let n = cols * rows;
            if best.as_ref().is_some_and(|b| b.cols * b.rows >= n) {
                continue;
            }
            let grid_w = cols as f64 * cell_w + (cols.saturating_sub(1)) as f64 * o.gap;
            let grid_h = rows as f64 * cell_h + (rows.saturating_sub(1)) as f64 * o.gap;
            let (x0, y0) = ((sw - grid_w) / 2.0, (sh - grid_h) / 2.0);
            let mut slots = Vec::with_capacity(n);
            for r in 0..rows {
                for c in 0..cols {
                    slots.push((x0 + c as f64 * (cell_w + o.gap) + o.bleed, y0 + r as f64 * (cell_h + o.gap) + o.bleed));
                }
            }
            best = Some(Layout { sheet_w: sw, sheet_h: sh, card_w, card_h, bleed: o.bleed, rotated, cols, rows, slots, short_edge: o.short_edge });
        }
    }
    best.unwrap_or(Layout { sheet_w: pw, sheet_h: ph, card_w, card_h, bleed: o.bleed, rotated: false, cols: 0, rows: 0, slots: vec![], short_edge: o.short_edge })
}

impl Layout {
    pub fn per_sheet(&self) -> usize {
        self.slots.len()
    }

    /// size of a slot's trim box on the sheet
    pub fn slot_size(&self) -> (f64, f64) {
        if self.rotated {
            (self.card_h, self.card_w)
        } else {
            (self.card_w, self.card_h)
        }
    }

    /// how the sheet is turned over: maps a point seen from the front to the same point seen from the back
    pub fn flip(&self) -> Affine {
        let long_edge_vertical = self.sheet_h >= self.sheet_w;
        // turning over on the long edge of a portrait sheet = mirror left-right
        let mirror_x = long_edge_vertical != self.short_edge;
        if mirror_x {
            Affine { a: -1.0, e: self.sheet_w, ..Affine::IDENTITY }
        } else {
            Affine { d: -1.0, f: self.sheet_h, ..Affine::IDENTITY }
        }
    }

    /// Where side `side` of the card in slot `i` goes: maps that side's design coordinates (mm)
    /// to sheet coordinates as seen when printing that side. Backs are placed behind their
    /// fronts, turned so they read the right way round once the sheet is flipped (`fmt.to_back`).
    pub fn place(&self, i: usize, side: Side, fmt: &CardFormat) -> Affine {
        let (sx, sy) = self.slots[i];
        let turn = if self.rotated {
            // 90° clockwise inside the slot: (x, y) → (card_h − y, x)
            Affine { a: 0.0, b: 1.0, c: -1.0, d: 0.0, e: self.card_h, f: 0.0 }
        } else {
            Affine::IDENTITY
        };
        let front = turn.then(Affine::translate(sx, sy));
        match side {
            Side::Front => front,
            Side::Back => {
                // back design point → the same paper point in front design coordinates → front sheet → seen from the back
                let (sw, sh) = fmt.sheet();
                let to_front = match fmt.fold {
                    crate::format::Fold::Top => Affine { d: -1.0, f: sh, ..Affine::IDENTITY },
                    _ => Affine { a: -1.0, e: sw, ..Affine::IDENTITY },
                };
                to_front.then(front).then(self.flip())
            }
        }
    }

    /// Crop marks round the grid: a short line in the margin at every trim edge, kept clear of
    /// the bleed. Each mark is (start, end) in front-side sheet coordinates.
    pub fn crop_marks(&self) -> Vec<((f64, f64), (f64, f64))> {
        if self.slots.is_empty() {
            return vec![];
        }
        let (w, h) = self.slot_size();
        let mut xs: Vec<f64> = Vec::new();
        let mut ys: Vec<f64> = Vec::new();
        for &(x, y) in &self.slots {
            for v in [x, x + w] {
                if !xs.iter().any(|o| (o - v).abs() < 1e-6) {
                    xs.push(v);
                }
            }
            for v in [y, y + h] {
                if !ys.iter().any(|o| (o - v).abs() < 1e-6) {
                    ys.push(v);
                }
            }
        }
        let left = self.slots.iter().map(|s| s.0).fold(f64::MAX, f64::min) - self.bleed;
        let top = self.slots.iter().map(|s| s.1).fold(f64::MAX, f64::min) - self.bleed;
        let right = self.slots.iter().map(|s| s.0 + w).fold(f64::MIN, f64::max) + self.bleed;
        let bottom = self.slots.iter().map(|s| s.1 + h).fold(f64::MIN, f64::max) + self.bleed;
        let gap = 1.5; // clear of the artwork
        let want = 5.0;
        let mut out = Vec::new();
        let len_before = |edge: f64| (edge - gap - 1.0).min(want).max(0.0); // room between the sheet edge and the art
        let (lt, lb) = (len_before(top), len_before(self.sheet_h - bottom));
        let (ll, lr) = (len_before(left), len_before(self.sheet_w - right));
        for x in &xs {
            if lt > 0.5 {
                out.push(((*x, top - gap - lt), (*x, top - gap)));
            }
            if lb > 0.5 {
                out.push(((*x, bottom + gap), (*x, bottom + gap + lb)));
            }
        }
        for y in &ys {
            if ll > 0.5 {
                out.push(((left - gap - ll, *y), (left - gap, *y)));
            }
            if lr > 0.5 {
                out.push(((right + gap, *y), (right + gap + lr, *y)));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::{preset, sheet_size};

    fn opts(sheet: &str, bleed: f64) -> ImposeOptions {
        let s = sheet_size(sheet);
        ImposeOptions { sheet_w: s.w, sheet_h: s.h, margin: 10.0, gap: 0.0, bleed, allow_rotate: true, short_edge: false }
    }

    #[test]
    fn ten_business_cards_on_a4() {
        // 85 × 55 trim, no bleed: 2 × 5 portrait (190 / 85 = 2, 277 / 55 = 5)
        let l = impose(85.0, 55.0, &opts("a4", 0.0));
        assert_eq!(l.per_sheet(), 10, "{l:?}");
        // with 3 mm bleed: 91 × 61 cells → 2 × 4 portrait, or rotated 61 × 91: 3 × 3 = 9
        let l = impose(85.0, 55.0, &opts("a4", 3.0));
        assert_eq!(l.per_sheet(), 9, "{l:?}");
        assert!(l.rotated);
        // everything inside the sheet margins
        let (w, h) = l.slot_size();
        for &(x, y) in &l.slots {
            assert!(x - 3.0 >= 10.0 - 1e-9 && y - 3.0 >= 10.0 - 1e-9 && x + w + 3.0 <= l.sheet_w - 10.0 + 1e-9 && y + h + 3.0 <= l.sheet_h - 10.0 + 1e-9);
        }
    }

    #[test]
    fn backs_land_behind_their_fronts() {
        for (id, sheet) in [("bc-eu", "a4"), ("greet-a6", "a4"), ("greet-top", "a4"), ("tent-place", "a3"), ("game-poker", "letter")] {
            let fmt = CardFormat::from_preset(preset(id).unwrap());
            let (sw, sh) = fmt.sheet();
            for short_edge in [false, true] {
                let mut o = opts(sheet, 3.0);
                o.short_edge = short_edge;
                let l = impose(sw, sh, &o);
                assert!(l.per_sheet() > 0, "{id}");
                let flip = l.flip();
                for i in 0..l.per_sheet() {
                    // the same point of paper: front design (x, y) and back design to_back(x, y)
                    for (x, y) in [(1.0, 2.0), (sw - 3.0, 4.0), (sw / 2.0, sh - 1.0)] {
                        let on_front = l.place(i, Side::Front, &fmt).apply(x, y);
                        let (bx, by) = fmt.to_back(x, y);
                        let on_back = l.place(i, Side::Back, &fmt).apply(bx, by);
                        let seen_from_front = flip.apply(on_back.0, on_back.1);
                        assert!((seen_from_front.0 - on_front.0).abs() < 1e-9 && (seen_from_front.1 - on_front.1).abs() < 1e-9, "{id} slot {i}: {on_front:?} vs {seen_from_front:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn crop_marks_stay_on_the_sheet_and_off_the_art() {
        let l = impose(85.0, 55.0, &opts("a4", 3.0));
        let marks = l.crop_marks();
        assert!(!marks.is_empty());
        let (w, h) = l.slot_size();
        for ((x0, y0), (x1, y1)) in marks {
            for (x, y) in [(x0, y0), (x1, y1)] {
                assert!(x >= 0.0 && y >= 0.0 && x <= l.sheet_w && y <= l.sheet_h);
                for &(sx, sy) in &l.slots {
                    let inside = x > sx - 3.0 + 1e-6 && x < sx + w + 3.0 - 1e-6 && y > sy - 3.0 + 1e-6 && y < sy + h + 3.0 - 1e-6;
                    assert!(!inside, "mark point {x},{y} on a card");
                }
            }
        }
    }

    #[test]
    fn affine_composition() {
        let m = Affine::translate(1.0, 2.0).then(Affine::scale(2.0, 3.0));
        assert_eq!(m.apply(1.0, 1.0), (4.0, 9.0));
    }
}
