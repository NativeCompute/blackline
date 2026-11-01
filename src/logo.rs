use eframe::egui::{self, Rect};

pub fn paint(painter: &egui::Painter, rect: Rect, _glow: f32) {
    crate::MARK.paint(painter, rect);
}

pub fn svg() -> String {
    crate::MARK.svg()
}

pub fn rgba(size: u32) -> Vec<u8> {
    crate::MARK.rgba(size)
}

pub fn ico() -> Vec<u8> {
    crate::MARK.ico()
}
