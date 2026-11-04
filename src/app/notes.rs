use eframe::egui::{self, Key, Modifiers, Pos2, Vec2};

use super::App;
use crate::editor;
use crate::model::Element;
use crate::theme;
use crate::ui;

pub(super) struct NoteEdit {
    pub block: u64,
    pub draft: String,
    pub had: bool,
    pub opened: u64,
}

const NOTE_W: f32 = 320.0;

impl App {
    pub(super) fn open_note(&mut self, id: u64) {
        if !self.guard_edit() {
            return;
        }
        let Some(b) = self.doc.block(id) else {
            return;
        };
        if b.element == Element::Act || b.text.trim().is_empty() {
            self.deck.say("Write the line first", "A note sits beside a line that has words in it.", crate::alerts::Tone::Info);
            return;
        }
        self.note_edit = Some(NoteEdit {
            block: id,
            draft: b.remark.clone(),
            had: !b.remark.trim().is_empty(),
            opened: self.frame_no,
        });
    }

    pub(super) fn set_remark(&mut self, id: u64, text: &str) {
        let tidy = text.replace(['\n', '\r'], " ").trim().to_string();
        let Some(i) = self.doc.index_of(id) else {
            return;
        };
        if self.doc.blocks[i].remark == tidy {
            return;
        }
        self.checkpoint();
        self.doc.blocks[i].remark = tidy;
        self.mark_changed();
    }

    pub(super) fn note_window(&mut self, ctx: &egui::Context) {
        let frame_no = self.frame_no;
        let Some(edit) = self.note_edit.as_mut() else {
            return;
        };
        if self.doc.index_of(edit.block).is_none() {
            self.note_edit = None;
            return;
        }
        let screen = ctx.screen_rect();
        let anchor = ctx.read_response(editor::block_id(edit.block)).map(|r| r.rect);
        let pos = match anchor {
            Some(r) => Pos2::new(
                (r.right() + 24.0).min(screen.right() - NOTE_W - 16.0),
                (r.top() - 8.0).clamp(screen.top() + 60.0, screen.bottom() - 230.0),
            ),
            None => screen.center() - Vec2::new(NOTE_W * 0.5, 100.0),
        };
        let (mut save, mut delete, mut cancel) = (false, false, false);
        let shown = egui::Area::new(egui::Id::new("ns-note-window"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            .show(ctx, |ui| {
                theme::island_frame(theme::R_ISLAND)
                    .inner_margin(egui::Margin::same(14.0))
                    .show(ui, |ui| {
                        ui.set_width(NOTE_W - 28.0);
                        ui::hud(ui, if edit.had { "Edit the note" } else { "Add a note" }, None);
                        ui.add_space(8.0);
                        let field = ui.add(
                            egui::TextEdit::multiline(&mut edit.draft)
                                .id(egui::Id::new("ns-note-text"))
                                .desired_rows(3)
                                .desired_width(f32::INFINITY)
                                .hint_text("What to remember about this line"),
                        );
                        if edit.opened + 2 >= frame_no {
                            field.request_focus();
                        }
                        ui.add_space(10.0);
                        ui.horizontal(|ui| {
                            if ui::button(ui, "Save note", None, true) {
                                save = true;
                            }
                            if edit.had && ui::button(ui, "Delete", None, false) {
                                delete = true;
                            }
                            if ui::quiet_button(ui, "Cancel", None) {
                                cancel = true;
                            }
                        });
                        ui.add_space(6.0);
                        ui::caption(ui, "Ctrl+Enter saves · Esc closes");
                    });
            });
        let _ = shown;
        ctx.input_mut(|i| {
            if i.consume_key(Modifiers::COMMAND, Key::Enter) {
                save = true;
            }
            if i.consume_key(Modifiers::NONE, Key::Escape) {
                cancel = true;
            }
        });
        if save || delete {
            let (id, text) = (edit.block, if delete { String::new() } else { edit.draft.clone() });
            self.note_edit = None;
            self.set_remark(id, &text);
            self.ed.focus(id, editor::Caret::End);
        } else if cancel {
            let id = edit.block;
            self.note_edit = None;
            self.ed.focus(id, editor::Caret::End);
        }
    }
}
