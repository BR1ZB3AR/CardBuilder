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
        "0.0.15",
        "2026-11-20",
        "Preferences and the Assistant",
        &[
            ("new", "Preferences (the gear in the top bar, or Ctrl+,): settings for CardBuilder on this computer. The Graphics card settings moved here from the Scene tab."),
            ("new", "The Assistant: AI agents such as Claude Code can design with you. Turn on Let AI agents connect, add CardBuilder to your agent with the line Preferences › Assistant gives you, and ask it for a card. It can read the design, add and change layers, set backgrounds, the card size and the data, look at the card, and open, save or export. Every change is one Ctrl+Z away."),
            ("new", "You decide what an agent may do: Ask first, Make edits (changes go ahead, opening, saving and exporting ask) or Do everything. When it asks, a card shows what it wants, with Allow, Always allow and Deny."),
            ("new", "The agent's pointer glides to whatever the agent works on, with its name beside it. The Assistant button in the top bar shows who is connected and what they did; custom instructions tell agents how you like to work."),
        ],
    ),
    (
        "0.0.14",
        "2026-11-20",
        "CardBuilder in your browser",
        &[
            ("new", "CardBuilder runs in a web browser too, nothing to install: br1zb3ar.github.io/CardBuilder, or br1zb3ar.codeberg.page/CardBuilder on Codeberg. It's the same app. Saving and exports download to your downloads folder, Open and dropped files bring designs, pictures, spreadsheets and fonts in, and the design you're working on stays in the browser between visits."),
        ],
    ),
    (
        "0.0.13",
        "2026-11-20",
        "Sharper 3D and a view gizmo",
        &[
            ("new", "A view gizmo in the 3D preview's top right corner, with the X, Y and Z axes as CAD apps show them (Z up). Click an axis to look along it, and click it again to look from the other end. Drag the gizmo to turn the card. The house button below it goes back to the starting view."),
            ("improved", "The card in 3D is much sharper: small text, thin lines and dotted patterns stay crisp instead of going soft. The print is drawn at about 600 dpi, so it holds up close too, and fine detail is sampled and sharpened like a photo, without halos round the card."),
            ("fixed", "On screens scaled by a fraction (125 %, 150 %), the 3D picture could be stretched by a pixel and come out slightly blurred all over."),
        ],
    ),
    (
        "0.0.12",
        "2026-11-20",
        "Vector shapes, dotted outlines and photo windows",
        &[
            ("new", "Vector shapes: artwork of any outline — a frame, a logo, a pattern of lines or dots — as a layer you can recolour, outline, resize, move or delete, sharp at any size and printed as vectors in the PDF."),
            ("new", "Outlines can be dotted or dashed (Shape › Outline style), with the dots kept round and evenly spaced however big the shape gets."),
            ("new", "Clip to the layer below (Arrange): a layer shows only inside the shape under it, like a photo in a window. Replace the photo and the new one stays in the window."),
            ("improved", "Vector shapes and outline-only shapes are picked where they are drawn, so a click through a frame's window or inside a dotted box reaches the layer underneath."),
            ("fixed", "Clicking a layer could nudge it onto a nearby guide and mark the design as changed. A click now only selects; a layer moves once you drag it."),
        ],
    ),
    (
        "0.0.11",
        "2026-11-19",
        "Named text layers",
        &[
            ("improved", "The Layers list shows what a template's text layers are for along with their words, such as Number · 1 or Title · <INSERT NAME>, so the biocard numbers are easy to find. Editing the words keeps that name."),
        ],
    ),
    (
        "0.0.10",
        "2026-11-19",
        "Outlined text",
        &[
            ("new", "Text can have an outline: Text › Outline sets its colour and width. Turn Fill off for outline-only lettering, like the big numbers on the Ingress biocards, which are now text you can retype or delete."),
        ],
    ),
    (
        "0.0.9",
        "2026-11-19",
        "A new business card, and fresh cards from the picker",
        &[
            ("new", "A new business card to start from: white, with the company name in two weights, a teal-to-blue wave along the bottom with the website, and on the back the name, job title and contact rows with icons. It fits every business card size."),
            ("improved", "Leaving a card that has a look of its own, such as an Ingress biocard template, for another kind of card starts a fresh card of that kind, so the biocard's artwork and Faction buttons don't come along. It asks first if there are unsaved changes, and Ctrl+Z brings the old card back. Other size changes still keep your layout."),
            ("fixed", "Each kind of card keeps its own place in the card picker's list. Before, switching kinds could open the new list scrolled down, past its first cards."),
        ],
    ),
    (
        "0.0.8",
        "2026-11-19",
        "Faction buttons",
        &[
            ("new", "A design can offer a choice as coloured buttons at the top of the Design tab. The Ingress biocard templates use it for the faction: RES, ENL or XFAC each show their own logo layers and set the faction colour, in one click that Ctrl+Z undoes."),
        ],
    ),
    (
        "0.0.7",
        "2026-11-19",
        "Sharper card editor, Exo font",
        &[
            ("improved", "The card editor is sharper. Once you stop zooming, it draws the card at exactly the screen's resolution, one pixel for one pixel, so text and fine lines look crisp instead of slightly soft."),
            ("new", "The Exo font comes with CardBuilder, in every weight from Thin to Black. It's the lettering of the 2024 Ingress biocards."),
            ("improved", "Opening a group in the card picker scrolls it into view, so all of its cards show. The picker also opens scrolled to the card you're working on."),
        ],
    ),
    (
        "0.0.6",
        "2026-11-19",
        "Card fronts in the picker",
        &[
            ("new", "The card picker shows each template's front, and the starter design of each card in a group, as a small picture next to its name, so you can see an Ingress Biocard before you pick it."),
            ("new", "The Coda font comes with CardBuilder (regular and extra bold), the lettering of the classic agent cards."),
            ("improved", "A template on this computer named like a built-in card takes its place in the picker. For example, an Enlightened or Resistance biocard template replaces the plain starter of the same name."),
            ("fixed", "A long card name in the sidebar ran under the Change… button. It is now cut short, and hovering it shows the whole name."),
        ],
    ),
    (
        "0.0.5",
        "2026-11-19",
        "Ingress Biocard group, templates and your own fonts",
        &[
            ("new", "Playing & game cards has an Ingress Biocard group — click its heading to open it: Enlightened, Resistance, and any biocard templates on this computer, such as the 2023 Origin card."),
            ("new", "Local templates: designs in CardBuilder's templates folder show in the card picker, in their own category and group. Picking one starts a new card from it; New (Ctrl+N) starts afresh from the same template. Export › Save as template… adds your own."),
            ("new", "Your own fonts: Font › Add a font file… takes a .ttf or .otf; it travels inside the .cb design, so the card opens with it anywhere."),
            ("new", "Text frames: a text box can keep its height with the text at its top, middle or bottom (Text › Text box) — for description panels that shouldn't move when the text gets longer. Drag its top and bottom handles to resize it."),
            ("improved", "The Enlightened and Resistance biocards are named by faction inside their group."),
            ("fixed", "A click on the last row of the card picker could close it instead of choosing the row, right after a group was opened."),
        ],
    ),
    (
        "0.0.4",
        "2026-11-19",
        "Front and back on one page",
        &[
            ("new", "The card editor shows both sides at once — front and back, or the outside and inside of a folded card — side by side or one above the other, whichever shows them bigger. Click anything on either side to edit it; the side you are working on is highlighted, and the sidebar's Add, Layers and Background follow it. The Front / Back switch is gone."),
            ("improved", "On folded cards the panel names (Front cover, Inside left …) sit inside each panel."),
            ("fixed", "Trying other sizes in the card picker shrank the artwork a little more with every switch (a business card, then a portrait card, then back left the name tiny and pushed aside). Browsing sizes now always starts from your layout, so coming back restores it exactly."),
            ("fixed", "The card picker stretched to the bottom of the window and, with longer lists, crept upwards so a click could hit the row below. It is as tall as its contents again (so is the Print dialog's divider)."),
        ],
    ),
    (
        "0.0.3",
        "2026-11-19",
        "The back of the card in 3D",
        &[
            ("new", "The 3D preview shows flat cards as a pair, like a business-card mockup: one card shows the front, the one beside it is turned to show the back — so both sides are in view at once. The new button in the 3D bar (or Scene › Front and back side by side) switches back to one card."),
            ("fixed", "Turning a card round in 3D showed its back dim and grey, because the studio light stayed in front of the card. The light now turns with the camera, so whichever side you look at is lit."),
        ],
    ),
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
