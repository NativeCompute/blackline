pub use ncf::ui::*;

use eframe::egui::{self, Rect};

pub fn maker_line(ui: &mut egui::Ui, rect: Rect, centre: bool) -> bool {
    ncf::ui::maker_line(ui, rect, centre, crate::INFO.name)
}
