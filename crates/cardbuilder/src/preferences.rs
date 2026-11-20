//! Preferences (the gear in the top bar, or Ctrl+,): settings of this computer's CardBuilder
//! rather than of a design. Graphics card (which card draws the window and the 3D), and on the
//! desktop the Assistant (AI agents, `assistant/ui.rs`).

use crate::app::CardBuilderApp;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PrefsPage {
    #[default]
    Graphics,
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    Assistant,
}

impl CardBuilderApp {
    pub(crate) fn preferences_dialog(&mut self, ctx: &egui::Context, now: f64) {
        let mut open = self.popups.prefs;
        if !open {
            return;
        }
        crate::topbar::dialog(ctx, "prefs-dialog", "Preferences", Some("Settings for CardBuilder on this computer"), 720.0, &mut open, |ui| {
            #[cfg(not(target_arch = "wasm32"))]
            {
                let mut page = self.popups.prefs_page;
                let opts = [(PrefsPage::Graphics, "Graphics card"), (PrefsPage::Assistant, "Assistant")];
                ui.add_space(4.0);
                crate::widgets::seg(ui, &mut page, &opts, false);
                self.popups.prefs_page = page;
                ui.add_space(4.0);
            }
            match self.popups.prefs_page {
                PrefsPage::Graphics => self.gpu_section(ui, now),
                #[cfg(not(target_arch = "wasm32"))]
                PrefsPage::Assistant => egui::Frame::NONE.inner_margin(egui::Margin::symmetric(16, 8)).show(ui, |ui| self.assistant_page(ui, now)).inner,
                #[cfg(target_arch = "wasm32")]
                PrefsPage::Assistant => self.gpu_section(ui, now),
            }
        });
        self.popups.prefs = open;
    }
}
