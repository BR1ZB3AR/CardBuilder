//! QR codes: the module grid (qrcode crate) turned into one path, dark modules merged into
//! row runs so the print has no hairline seams between them.

use cb_core::doc::QrEcc;
use resvg::tiny_skia::{Path, PathBuilder, Rect};

/// the module grid of `data`: (modules per side, dark modules row by row)
pub fn matrix(data: &str, ecc: QrEcc) -> Option<(usize, Vec<bool>)> {
    let level = match ecc {
        QrEcc::L => qrcode::EcLevel::L,
        QrEcc::M => qrcode::EcLevel::M,
        QrEcc::Q => qrcode::EcLevel::Q,
        QrEcc::H => qrcode::EcLevel::H,
    };
    let code = qrcode::QrCode::with_error_correction_level(data.as_bytes(), level).ok()?;
    let n = code.width();
    Some((n, code.to_colors().into_iter().map(|c| c == qrcode::Color::Dark).collect()))
}

/// The dark modules as a path filling a square of side `size` centred on the origin, with
/// `quiet` light modules of margin inside it.
pub fn path(n: usize, dark: &[bool], size: f32, quiet: usize) -> Option<Path> {
    let total = (n + 2 * quiet) as f32;
    let m = size / total;
    let o = -size / 2.0 + quiet as f32 * m;
    let mut pb = PathBuilder::new();
    for y in 0..n {
        let mut x = 0;
        while x < n {
            if !dark[y * n + x] {
                x += 1;
                continue;
            }
            let start = x;
            while x < n && dark[y * n + x] {
                x += 1;
            }
            // a hair of overlap between rows hides anti-aliasing seams
            if let Some(r) = Rect::from_xywh(o + start as f32 * m, o + y as f32 * m, (x - start) as f32 * m, m * 1.002) {
                pb.push_rect(r);
            }
        }
    }
    pb.finish()
}

/// smallest module size (mm) that reliably scans from a phone: about 0.35 mm
pub const MIN_MODULE_MM: f64 = 0.35;

/// module size of a QR code `size` mm wide holding `data`
pub fn module_mm(data: &str, ecc: QrEcc, size: f64, quiet: usize) -> Option<f64> {
    matrix(data, ecc).map(|(n, _)| size / (n + 2 * quiet) as f64)
}
