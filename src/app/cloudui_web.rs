use std::path::{Path, PathBuf};

use eframe::egui::{self, Rect};

use super::App;

pub(super) const ACCOUNT_TAB: usize = 9;
pub(super) const TEAMS_TAB: usize = 10;

pub(super) enum Move {}

impl App {
    pub(super) fn cloud_on(&self) -> bool {
        false
    }

    pub(super) fn cloud_frame(&mut self) {}

    pub(super) fn push_settings(&mut self) {}

    pub(super) fn answer_cloud(&mut self, _action: &str) {}

    pub(super) fn first_sync_picker(&mut self, _ctx: &egui::Context) {}

    pub(super) fn move_items(&mut self, _ui: &mut egui::Ui, _rects: &mut Vec<Rect>, _path: &Path) -> Option<Move> {
        None
    }

    pub(super) fn do_move(&mut self, _path: &Path, m: Move) {
        match m {}
    }

    pub(super) fn make_local(&mut self, _path: &PathBuf) {}

    pub(super) fn sources_bar(&mut self, _ui: &mut egui::Ui, _w: f32) {}

    pub(super) fn cloud_title(&mut self, _ui: &mut egui::Ui) {}

    pub(super) fn account_body(&mut self, _ui: &mut egui::Ui, _tab: usize) {}
}
