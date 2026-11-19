//! CardBuilder core — card formats, the design document, data merge, print-sheet imposition,
//! undo/redo and the 3D card meshes.
//!
//! No UI and no platform code. All lengths are millimetres; design coordinates have their origin
//! at the top-left corner of the trimmed card (or of the flat sheet of a folded card), y down.

pub mod card3d;
pub mod data;
pub mod doc;
pub mod format;
pub mod geom;
pub mod history;
pub mod impose;

pub use doc::{Doc, Layer, LayerType, Side};
pub use format::{CardFormat, Fold};

/// millimetres per typographic point
pub const MM_PER_PT: f64 = 25.4 / 72.0;
/// millimetres per inch
pub const MM_PER_IN: f64 = 25.4;
