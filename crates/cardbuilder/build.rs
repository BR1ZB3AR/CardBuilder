//! Windows builds: embed the app icon and version information in CardBuilder.exe (shown by
//! Explorer, the taskbar and "Apps & features"). Nothing to do for other targets.

fn main() {
    println!("cargo:rerun-if-changed=assets/cardbuilder.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set_icon("assets/cardbuilder.ico")
        .set("ProductName", "CardBuilder")
        .set("FileDescription", "CardBuilder — design cards of every kind")
        .set("CompanyName", "BR1ZB3AR")
        .set("LegalCopyright", "MIT License — github.com/BR1ZB3AR/CardBuilder")
        .set("OriginalFilename", "CardBuilder.exe");
    res.compile().expect("couldn't embed the Windows icon and version information");
}
