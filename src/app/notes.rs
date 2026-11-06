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
                        ui::caption(ui, &format!("{}+Enter saves · Esc closes", crate::keys::command_key()));
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

// left panel here---

struct Entry {
    id: u64,
    top: String,
    text: String,
    sub: String,
}

impl App {
    pub(super) fn ensure_rev_base(&mut self) {
        if !self.doc.meta.revising() {
            self.rev_base = None;
            return;
        }
        let name = self.doc.meta.revision_base.clone();
        if self.rev_base.as_ref().map(|(n, _)| *n == name).unwrap_or(false) {
            return;
        }
        let Some(key) = self.path.clone() else {
            return;
        };
        let Some(snaps) = self.store.snapshots(&key) else {
            return;
        };
        let Some(snap) = snaps.iter().find(|s| s.path.file_name().and_then(|f| f.to_str()) == Some(name.as_str())) else {
            return;
        };
        if let crate::backend::Load::Ready(doc, _) = self.store.load_snapshot(&snap.path.clone()) {
            self.rev_base = Some((name, *doc));
        }
    }

    pub(super) fn start_revision(&mut self) {
        if !self.guard_edit() || self.doc.meta.revising() {
            return;
        }
        let Some(key) = self.path.clone() else {
            return;
        };
        let colour = self.doc.meta.revision.map(|c| c + 1).unwrap_or(0);
        let label = format!("{} revision begins", crate::revision::name(colour));
        if let Err(e) = self.store.take_snapshot(&key, &self.doc, Some(&label)) {
            self.deck.say("Could not start the revision", &e, crate::alerts::Tone::Danger);
            return;
        }
        let newest = self
            .store
            .snapshots(&key)
            .and_then(|v| v.into_iter().max_by_key(|s| s.when.clone()))
            .and_then(|s| s.path.file_name().and_then(|f| f.to_str()).map(str::to_string));
        let Some(base) = newest else {
            self.deck.say("Could not start the revision", "The starting copy was not found.", crate::alerts::Tone::Danger);
            return;
        };
        self.checkpoint();
        self.rev_base = Some((base.clone(), self.doc.clone()));
        self.doc.meta.revision = Some(colour);
        self.doc.meta.revision_date = chrono::Local::now().format("%Y-%m-%d").to_string();
        self.doc.meta.revision_base = base;
        self.doc.meta.locked = false;
        self.mark_changed();
        self.layout_stale = true;
        self.deck.ok(
            &format!("{} revision started", crate::revision::name(colour)),
            "Every line you change from now on is marked with an asterisk.",
        );
    }

    pub(super) fn end_revision(&mut self) {
        if !self.guard_edit() || !self.doc.meta.revising() {
            return;
        }
        self.checkpoint();
        self.doc.meta.revision_base.clear();
        self.doc.meta.locked = false;
        self.rev_base = None;
        self.mark_changed();
        self.layout_stale = true;
        self.deck.ok("Revision ended", "The marks are cleared. The next revision takes the next colour.");
    }

    pub(super) fn left_panel(&mut self, ctx: &egui::Context) {
        if !self.settings.show_library || self.focus_mode {
            return;
        }
        egui::SidePanel::left("ns-rail")
            .exact_width(theme::RAIL_W + theme::GAP)
            .resizable(false)
            .show_separator_line(false)
            .frame(egui::Frame::none().inner_margin(egui::Margin {
                left: theme::GAP,
                right: 0.0,
                top: theme::GAP,
                bottom: theme::GAP,
            }))
            .show(ctx, |ui| {
                theme::island_frame(theme::R_ISLAND)
                    .inner_margin(egui::Margin::symmetric(12.0, 14.0))
                    .show(ui, |ui| {
                        ui.set_min_size(ui.available_size());
                        self.left_rect = ui.max_rect();
                        if let Some(k) = ui::tabs(ui, &["Notes", "Changes"], self.left_tab) {
                            self.left_tab = k;
                        }
                        ui.add_space(12.0);

                        const TRACKER: f32 = 34.0;
                        const FOOT: f32 = 24.0;
                        let tracker = !self.settings.youtrack_url.trim().is_empty();
                        let gaps = ui.spacing().item_spacing.y * if tracker { 2.0 } else { 1.0 };
                        let list_h = (ui.available_height() - if tracker { TRACKER } else { 0.0 } - FOOT - gaps).max(60.0);
                        let (list, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), list_h), egui::Sense::hover());
                        let mut list_ui = ui.new_child(egui::UiBuilder::new().max_rect(list).layout(egui::Layout::top_down(egui::Align::Min)));
                        self.left_rows.clear();
                        if self.left_tab == 0 {
                            self.notes_list(&mut list_ui);
                        } else {
                            self.changes_list(&mut list_ui);
                        }

                        if tracker && super::youtrack_button(ui, &self.settings.youtrack_url) {
                            let url = self.settings.youtrack_url.trim().to_string();
                            if url.starts_with("http://") || url.starts_with("https://") {
                                self.store.open_url(&url);
                            } else {
                                self.deck.warn("That is not a web address", "Set the YouTrack link in Settings.");
                            }
                        }
                        let (foot, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), FOOT), egui::Sense::hover());
                        self.foot_rect = foot;
                        if ui::maker_line(ui, foot.translate(Vec2::new(0.0, 2.0)), false) {
                            self.store.open_url(crate::NATIVECOMPUTE_URL);
                        }
                    });
            });
    }

    fn entry_list(&mut self, ui: &mut egui::Ui, salt: &str, entries: &[Entry]) -> Option<u64> {
        let p = crate::theme::pal();
        let mut picked = None;
        let room = ui.available_height();
        egui::ScrollArea::vertical()
            .id_salt(salt)
            .auto_shrink([false; 2])
            .max_height(room)
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 4.0;
                let w = ui.available_width() - 6.0;
                for e in entries {
                    let body = ui.painter().layout(e.text.clone(), theme::font(theme::T_SM), p.text, w - 20.0);
                    let h = 22.0 + body.rect.height() + if e.sub.is_empty() { 8.0 } else { 22.0 };
                    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), egui::Sense::click());
                    let hot = crate::anim::ease(ui.ctx(), resp.id, resp.hovered(), crate::anim::HOVER);
                    theme::hover_surface(ui.painter(), rect, theme::R_SM, p.primary, hot, false);
                    let top = ui::elide_to_width(ui.ctx(), &e.top, theme::font_hud(theme::T_MICRO), 0.8, w - 20.0);
                    theme::tracked_text(ui.painter(), Pos2::new(rect.left() + 10.0, rect.top() + 12.0), &top, theme::font_hud(theme::T_MICRO), p.text_faint, 0.8);
                    ui.painter().galley(Pos2::new(rect.left() + 10.0, rect.top() + 22.0), body, p.text);
                    if !e.sub.is_empty() {
                        let sub = ui::elide_to_width(ui.ctx(), &e.sub, theme::font_page(theme::T_CAP + 1.0), 0.0, w - 20.0);
                        ui.painter().text(
                            Pos2::new(rect.left() + 10.0, rect.bottom() - 13.0),
                            egui::Align2::LEFT_CENTER,
                            sub,
                            theme::font_page(theme::T_CAP + 1.0),
                            p.text_faint,
                        );
                    }
                    if resp.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    self.left_rows.push((e.id, rect));
                    if resp.clicked() {
                        picked = Some(e.id);
                    }
                }
            });
        picked
    }

    fn where_is(&self, id: u64) -> String {
        let page = self.layout.block_pages.get(&id).map(|p| format!("P.{}", self.page_label(*p))).unwrap_or_else(|| "P.–".into());
        let scene = self
            .doc
            .scene_of(id)
            .and_then(|k| self.doc.scenes().get(k).map(|s| s.heading.clone()))
            .filter(|h| !h.trim().is_empty());
        match scene {
            Some(h) => format!("{page} · {h}"),
            None => page,
        }
    }

    fn jump(&mut self, id: u64) {
        self.mode = super::Mode::Write;
        self.ed.jump_to(id);
    }

    fn notes_list(&mut self, ui: &mut egui::Ui) {
        let entries: Vec<Entry> = self
            .doc
            .blocks
            .iter()
            .filter(|b| !b.remark.trim().is_empty())
            .map(|b| Entry { id: b.id, top: self.where_is(b.id), text: b.remark.trim().to_string(), sub: b.text.trim().to_string() })
            .collect();
        ui::hud(ui, &format!("Notes / {:02}", entries.len()), None);
        ui.add_space(8.0);
        if entries.is_empty() {
            ui::body(ui, "No notes yet.");
            ui.add_space(4.0);
            ui::caption(ui, &format!("Select a line and press {}, or right-click it, to leave yourself a note.", self.keys_for(crate::keys::Command::AddNote)));
        }
        let foot = 40.0;
        let (room, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), (ui.available_height() - foot).max(40.0)), egui::Sense::hover());
        let mut list = ui.new_child(egui::UiBuilder::new().max_rect(room).layout(egui::Layout::top_down(egui::Align::Min)));
        if let Some(id) = self.entry_list(&mut list, "ns-notes-list", &entries) {
            self.jump(id);
        }
        ui.add_space(6.0);
        if ui::button(ui, "Add a note", Some(crate::icons::Icon::Plus), false) {
            match self.ed.focus_block {
                Some(id) => self.open_note(id),
                None => self.deck.say("Pick a line first", "Click the line the note is about, then add it.", crate::alerts::Tone::Info),
            }
        }
    }

    fn changes_list(&mut self, ui: &mut egui::Ui) {
        if !self.doc.meta.revising() {
            let next = self.doc.meta.revision.map(|c| c + 1).unwrap_or(0);
            ui::hud(ui, "Revision mode", None);
            ui.add_space(8.0);
            ui::body(ui, "Once a script is out, every change is marked with an asterisk in the margin, and each round of changes takes the next colour: blue, then pink, then yellow.");
            ui.add_space(12.0);
            if ui::button(ui, &format!("Start the {} revision", crate::revision::name(next)), Some(crate::icons::Icon::Star4), true) {
                self.start_revision();
            }
            return;
        }
        let colour = self.doc.meta.revision.unwrap_or(0);
        let ids: Vec<u64> = self.doc.blocks.iter().filter(|b| self.layout.changes.contains(&b.id)).map(|b| b.id).collect();
        let entries: Vec<Entry> = ids
            .iter()
            .filter_map(|id| self.doc.block(*id))
            .map(|b| Entry { id: b.id, top: format!("{} · {}", b.element.short(), self.where_is(b.id)), text: b.text.trim().to_string(), sub: String::new() })
            .collect();
        ui::hud(ui, &format!("{} revision / {:02} changed", crate::revision::name(colour), entries.len()), Some(crate::theme::pal().primary_light));
        ui.add_space(4.0);
        ui::caption(ui, &format!("{} · since {}", crate::revision::header(colour, &self.doc.meta.revision_date), self.doc.meta.revision_date));
        ui.add_space(6.0);
        let mut locked = self.doc.meta.locked;
        if ui::toggle_row(ui, "Lock pages", "Pages keep their numbers; new material goes on A-pages.", &mut locked) && self.guard_edit() {
            self.checkpoint();
            self.doc.meta.locked = locked;
            self.mark_changed();
            self.layout_stale = true;
            self.refresh_layout(true);
        }
        ui.add_space(8.0);
        if entries.is_empty() {
            ui::body(ui, "Nothing changed yet. Edit any line and it is listed here and marked in the margin.");
        }
        let foot = 40.0;
        let (room, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), (ui.available_height() - foot).max(40.0)), egui::Sense::hover());
        let mut list = ui.new_child(egui::UiBuilder::new().max_rect(room).layout(egui::Layout::top_down(egui::Align::Min)));
        if let Some(id) = self.entry_list(&mut list, "ns-changes-list", &entries) {
            self.jump(id);
        }
        ui.add_space(6.0);
        if ui::button(ui, "End the revision", Some(crate::icons::Icon::Check), false) {
            self.end_revision();
        }
    }
}
