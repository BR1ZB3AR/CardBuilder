//! CardBuilder renderer: draws the printed sides of a card — background, text, images, shapes,
//! QR codes, foil and spot UV — as pixels (tiny-skia: editor, PNG, 3D textures) or as vector
//! PDF (card files and imposed print sheets).

pub mod canvas;
pub mod color;
pub mod draw;
pub mod images;
pub mod pdf;
pub mod qr;
pub mod sheets;
pub mod text;

pub use draw::{draw_side, render_finish_mask, render_side, DrawOptions, Mode};
pub use images::ImageCache;
pub use resvg::tiny_skia;

/// image sources used by a design (layers and backgrounds)
pub fn image_srcs(doc: &cb_core::Doc) -> impl Iterator<Item = &str> {
    doc.layers.iter().filter_map(|l| l.src.as_deref()).chain(doc.front.src.as_deref()).chain(doc.back.src.as_deref())
}

/// premultiplied RGBA → straight RGBA (for PNG files, PDF images and GPU textures)
pub fn unpremultiply(data: &mut [u8]) {
    for px in data.chunks_exact_mut(4) {
        let a = px[3] as u32;
        if a > 0 && a < 255 {
            for c in &mut px[..3] {
                *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
}
