pub use ncf::chrome::*;

use eframe::egui;

pub fn title_bar(ui: &mut egui::Ui, ctx: &egui::Context, extra: impl FnOnce(&mut egui::Ui)) {
    ncf::chrome::title_bar(ui, ctx, &crate::INFO, extra);
}
