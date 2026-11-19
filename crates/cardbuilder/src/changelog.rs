//! Release notes, newest first. Every update or bugfix bumps the patch version
//! (v0.0.1 → v0.0.2 …) — keep all three crates' `Cargo.toml` in sync.

pub type Release = (
    &'static str,
    &'static str,
    &'static str,
    &'static [(&'static str, &'static str)],
);

pub const CHANGELOG: &[Release] = &[
    (
        "0.0.2",
        "2026-11-19",
        "Ingress agent biocards",
        &[
            ("new", "Ingress Biocard templates under Playing & game cards, in Enlightened green and Resistance blue: trading-card size (2.5 × 3.5 in) with your codename, faction, level badge, a photo to replace (Image › Replace…), a few stats and a QR code to your Telegram; the back carries a faction ring. Only the faction names and colours are used — no game logos — so the cards are yours to print."),
        ],
    ),
    (
    "0.0.1",
    "2026-11-19",
    "CardBuilder — first release",
    &[
        ("new", "Design every kind of card: business cards (Europe, US, Asia, Japan, Australia, rounded, square, mini), ID, badge and gift cards (CR80), playing, trading and tarot cards, postcards and invitations, folded greeting cards (side fold and top fold), tent and place cards, bookmarks and tags — or any custom size, flat or folded, with rounded corners."),
        ("new", "Both sides of the card — front and back, or outside and inside of a folded card — with bleed, trim, safe-area and fold guides on the card."),
        ("new", "Text in 15 fonts (sans, serif, script, display, monospace) with sizes in points, bold, italic, capitals, alignment, line and letter spacing; lines wrap at the box width or long lines shrink to fit."),
        ("new", "Pictures (PNG, JPEG, WebP, GIF, SVG), rectangles, ellipses and lines, backgrounds in colour, gradient or a picture."),
        ("new", "QR codes for a website, a contact card (vCard), Wi-Fi, a phone number, an email address or any text — drawn as sharp vectors, with a warning when a code gets too small to scan."),
        ("new", "Batch cards: import a spreadsheet (CSV) and put {Column} fields in text and QR codes — one card per row, previewed row by row, editable in the Data tab."),
        ("new", "Gold, silver, copper, rose-gold and holographic foil, and spot UV varnish — shown in 3D, and as separate pages for the print shop."),
        ("new", "3D preview on the graphics card: the card in its paper stock (matte, silk, gloss, soft-touch, linen, kraft, pearl, black, plastic) with studio lighting and soft shadows; folded cards stand open at any angle."),
        ("new", "Print sheets as a vector PDF: as many cards per page as fit (A4, A3, SRA3, A5, Letter, Legal, Tabloid, 13 × 19 in), crop marks, bleed, and the backs lined up on the reverse for double-sided printing."),
        ("new", "Card PDF (one page per side with bleed and trim boxes, as print shops want it), PNG pictures at 300 dpi and a 2000 px 3D snapshot."),
        ("new", "Design files: New, Open and Save (.cb files, pictures and data included); unsaved changes are never thrown away without asking, and everything can be undone."),
        ("new", "31 themes with CardBuilder's own Card Studio, a custom title bar, and the CardBuilder logo."),
    ],
),
];
