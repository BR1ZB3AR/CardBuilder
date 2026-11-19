//! CardBuilder — design cards of every kind, with a live 3D preview.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // no console window on Windows release builds

fn main() -> eframe::Result {
    // a couple of command-line flags, handy for packaging checks; anything else is a design file
    // to open (the file manager passes it when a .cb is double-clicked)
    let mut open: Option<std::path::PathBuf> = None;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--version" | "-V" => {
                println!("CardBuilder {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            "--help" | "-h" => {
                println!("CardBuilder {} — design business cards, greeting cards, game cards and more\n\nUsage: cardbuilder [--version] [--help] [design.cb]\n\nOpens the design file if one is given. Designs are saved as .cb files (Ctrl+S).", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            a if !a.starts_with('-') && open.is_none() => open = Some(a.into()),
            _ => {}
        }
    }
    env_logger::init();
    show_startup_errors();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("CardBuilder")
            .with_app_id("cardbuilder") // matches cardbuilder.desktop (dock icon on Wayland)
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([980.0, 620.0])
            .with_decorations(false) // the top bar is the title bar (titlebar.rs)
            .with_drag_and_drop(true)
            .with_icon(eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon-256.png")).unwrap_or_default()),
        wgpu_options: cardbuilder::gpu::wgpu_options(cardbuilder::gpu::load_choice()),
        ..Default::default()
    };
    eframe::run_native(
        "CardBuilder",
        options,
        Box::new(move |cc| {
            let mut app = cardbuilder::CardBuilderApp::new(cc);
            if let Some(path) = open {
                app.request_open_path(path, 0.0);
            }
            Ok(Box::new(app))
        }),
    )
}

/// A crash in the release build would otherwise vanish without a word (the Windows build has no
/// console, and the app launcher hides the terminal): show the reason in a message box as well.
fn show_startup_errors() {
    if cfg!(debug_assertions) {
        return;
    }
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        default_hook(info);
        if std::thread::current().name() != Some("main") {
            return;
        }
        let what = info.payload().downcast_ref::<String>().map(String::as_str).or_else(|| info.payload().downcast_ref::<&str>().copied()).unwrap_or("unknown error");
        let what: String = what.chars().take(1200).collect();
        rfd::MessageDialog::new()
            .set_level(rfd::MessageLevel::Error)
            .set_title("CardBuilder stopped")
            .set_description(format!("CardBuilder ran into a problem and has to close.\n\n{what}\n\nIf this keeps happening, please report it at https://github.com/BR1ZB3AR/CardBuilder/issues"))
            .set_buttons(rfd::MessageButtons::Ok)
            .show();
    }));
}
