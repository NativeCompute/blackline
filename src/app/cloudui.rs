use std::path::{Path, PathBuf};

use eframe::egui::{self, Align, Color32, Layout, Pos2, Rect, Sense, Stroke, Vec2};

use super::{App, Ask, Popover};
use crate::alerts::Tone;
use crate::backend::{Source, Space};
use crate::icons::{self, Icon};
use crate::settings::Settings;
use crate::theme::{self, pal};
use crate::ui;

pub(super) const ACCOUNT_TAB: usize = 9;
pub(super) const TEAMS_TAB: usize = 10;

const LOCAL_ONLY_KEYS: [&str; 7] = ["autosave_ms", "dismissed_update", "onboarded", "debug", "cloud_asked", "cloud_sync_settings", "colour_seed"];

pub fn synced_settings(s: &Settings) -> String {
    s.serialize()
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter(|l| l.split_once('=').map(|(k, _)| !LOCAL_ONLY_KEYS.contains(&k.trim())).unwrap_or(false))
        .map(|l| format!("{l}\n"))
        .collect()
}

pub fn with_pulled(s: &Settings, pulled: &str) -> Settings {
    let mut text = s.serialize();
    for line in pulled.lines().map(str::trim) {
        if let Some((k, _)) = line.split_once('=') {
            if !LOCAL_ONLY_KEYS.contains(&k.trim()) {
                text.push_str(line);
                text.push('\n');
            }
        }
    }
    let mut out = Settings::parse(&text);
    out.fresh = s.fresh;
    out
}

pub(super) enum Move {
    To(String),
    Local,
}

pub(super) struct Picker {
    pub rows: Vec<(PathBuf, String, bool)>,
    pub opened_on: u64,
}

fn tick_box(ui: &mut egui::Ui, on: bool, label: &str, w: f32) -> bool {
    let p = pal();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 30.0), Sense::click());
    let hot = crate::anim::ease(ui.ctx(), resp.id, resp.hovered(), crate::anim::HOVER);
    if hot > 0.01 {
        ui.painter().rect_filled(rect, egui::Rounding::same(theme::R_SM), theme::wash(p.raised, (255.0 * hot) as u8));
    }
    let b = Rect::from_center_size(Pos2::new(rect.left() + 14.0, rect.center().y), Vec2::splat(14.0));
    if on {
        ui.painter().rect_filled(b, egui::Rounding::same(2.0), p.primary);
        icons::draw(ui.painter(), b.shrink(2.0), Icon::Check, Color32::WHITE);
    } else {
        ui.painter().rect_stroke(b, egui::Rounding::same(2.0), Stroke::new(1.0_f32, p.text_faint));
    }
    ui.painter().text(
        Pos2::new(rect.left() + 32.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        ui::elide_to_width(ui.ctx(), label, theme::font_med(theme::T_SM), 0.0, w - 44.0),
        theme::font_med(theme::T_SM),
        p.text,
    );
    resp.clicked()
}

impl App {
    pub(super) fn cloud_on(&self) -> bool {
        self.account.is_some()
    }

    pub(super) fn signed_in(&self) -> bool {
        self.account.as_ref().map(|a| a.signed_in()).unwrap_or(false)
    }

    pub(super) fn open_settings_tab(&mut self, tab: usize) {
        self.settings_tab = tab;
        self.popover = Some(Popover::Settings);
        self.popover_frame = self.frame_no;
    }

    pub(super) fn cloud_frame(&mut self) {
        let Some(a) = self.account.as_mut() else {
            return;
        };
        a.poll();
        let notices = a.take_notices();
        let asks = a.take_asks();
        let urls = a.take_urls();
        let pulled = a.take_pulled_settings();
        let sync = a.sync_settings;
        let changed = a.take_changed();
        for (title, body, tone) in notices {
            self.deck.say(&title, &body, tone);
        }
        for q in asks {
            self.deck.ask(&q.title, &q.body, &q.yes, if q.danger { Tone::Danger } else { Tone::Info }, Ask::Cloud(q.action));
        }
        for u in urls {
            match u.strip_prefix("ncf:settings:") {
                Some("Organizations") => self.open_settings_tab(TEAMS_TAB),
                Some(_) => self.open_settings_tab(ACCOUNT_TAB),
                None => self.store.open_url(&u),
            }
        }
        if sync != self.settings.cloud_sync {
            self.settings.cloud_sync = sync;
            self.save_settings();
        }
        if let Some(text) = pulled {
            let before = self.settings.clone();
            self.settings = with_pulled(&self.settings, &text);
            if self.settings != before {
                self.ed.font_px = self.settings.page_px;
                self.theme_dirty = true;
                self.save_settings();
            }
        }
        if let Some(on) = changed {
            self.store.account_changed(on);
            self.refresh_entries();
            if on && !self.settings.cloud_asked {
                self.offer_first_sync();
            }
        }
    }

    pub(super) fn push_settings(&mut self) {
        let text = synced_settings(&self.settings);
        if let Some(a) = self.account.as_mut() {
            a.push_settings(&text);
        }
    }

    pub(super) fn answer_cloud(&mut self, action: &str) {
        if let Some(a) = self.account.as_mut() {
            a.answer(action);
        }
    }

    // first sync here---

    pub(super) fn offer_first_sync(&mut self) {
        let personal = self.store.spaces().into_iter().find(|s| s.personal);
        let local: Vec<(PathBuf, String, bool)> = if self.store.source() == Source::Local {
            self.entries.iter().map(|e| (e.path.clone(), e.title.clone(), false)).collect()
        } else {
            let was = self.store.source();
            self.store.set_source(Source::Local);
            let list = self.store.list().into_iter().map(|e| (e.path, e.title, false)).collect();
            self.store.set_source(was);
            list
        };
        self.settings.cloud_asked = true;
        self.save_settings();
        if local.is_empty() {
            return;
        }
        if personal.is_none() && !self.store.spaces().is_empty() {
            return;
        }
        self.picker = Some(Picker { rows: local, opened_on: self.frame_no });
    }

    pub(super) fn first_sync_picker(&mut self, ctx: &egui::Context) {
        let Some(mut picker) = self.picker.take() else {
            return;
        };
        let p = pal();
        let screen = ctx.screen_rect();
        let size = Vec2::new(480.0_f32.min(screen.width() - 40.0), (200.0 + 30.0 * picker.rows.len() as f32).clamp(260.0, screen.height() - 80.0));
        let rect = Rect::from_center_size(screen.center(), size);
        let mut done: Option<bool> = None;
        egui::Area::new(egui::Id::new("bl-cloud-scrim")).order(egui::Order::Middle).fixed_pos(screen.min).show(ctx, |ui| {
            ui.allocate_exact_size(screen.size(), Sense::click());
            ui.painter().rect_filled(screen, egui::Rounding::ZERO, Color32::from_black_alpha(120));
        });
        egui::Area::new(egui::Id::new("bl-cloud-picker")).order(egui::Order::Foreground).fixed_pos(rect.min).show(ctx, |ui| {
            let (win, _) = ui.allocate_exact_size(rect.size(), Sense::hover());
            ui.painter().rect_filled(win, egui::Rounding::same(theme::R_MD), p.solid_hi);
            ui.painter().rect_stroke(win.shrink(0.5), egui::Rounding::same(theme::R_MD), Stroke::new(1.0_f32, p.line));
            let mut inner = ui.new_child(egui::UiBuilder::new().max_rect(win.shrink2(Vec2::new(22.0, 18.0))).layout(Layout::top_down(Align::Min)));
            ui::section(&mut inner, "NativeCloud");
            inner.label(egui::RichText::new("Which scripts should sync?").font(theme::font_semi(theme::T_H)).color(p.text));
            inner.add_space(2.0);
            inner.label(
                egui::RichText::new("Ticked scripts move to My cloud and sync to every computer you sign in on. The rest stay on this computer only. You can move any script later from its ⋯ menu.")
                    .font(theme::font(theme::T_CAP))
                    .color(p.text_dim),
            );
            inner.add_space(10.0);
            let w = inner.available_width();
            let all = picker.rows.iter().all(|r| r.2);
            if tick_box(&mut inner, all, if all { "All scripts" } else { "Select all" }, w) {
                for r in &mut picker.rows {
                    r.2 = !all;
                }
            }
            ui::separator(&mut inner);
            let list_h = (inner.available_height() - 52.0).max(60.0);
            egui::ScrollArea::vertical().max_height(list_h).auto_shrink([false, true]).show(&mut inner, |ui| {
                for r in &mut picker.rows {
                    if tick_box(ui, r.2, &r.1, w - 10.0) {
                        r.2 = !r.2;
                    }
                }
            });
            inner.add_space(10.0);
            inner.horizontal(|ui| {
                let n = picker.rows.iter().filter(|r| r.2).count();
                let label = match n {
                    0 => "Keep them all here".to_string(),
                    1 => "Move 1 script to My cloud".to_string(),
                    n => format!("Move {n} scripts to My cloud"),
                };
                if ui::button(ui, &label, Some(if n == 0 { Icon::Check } else { Icon::Cloud }), true) {
                    done = Some(true);
                }
                if n > 0 && ui::quiet_button(ui, "Not now", None) {
                    done = Some(false);
                }
            });
        });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) && self.frame_no > picker.opened_on + 1 {
            done = Some(false);
        }
        match done {
            Some(true) => {
                let chosen: Vec<PathBuf> = picker.rows.iter().filter(|r| r.2).map(|r| r.0.clone()).collect();
                self.move_chosen(chosen);
            }
            Some(false) => {}
            None => self.picker = Some(picker),
        }
    }

    pub(super) fn move_chosen(&mut self, chosen: Vec<PathBuf>) {
        let personal = self.store.spaces().into_iter().find(|s| s.personal).map(|s| s.id);
        {
            {
                if let (Some(ws), false) = (personal, chosen.is_empty()) {
                    let mut moved = 0;
                    for path in &chosen {
                        match self.store.move_to(path, Some(&ws)) {
                            Ok(new) => {
                                moved += 1;
                                if self.path.as_deref() == Some(path.as_path()) {
                                    self.path = Some(new);
                                }
                            }
                            Err(e) => {
                                self.deck.say("Couldn't move a script", &e, Tone::Warn);
                                break;
                            }
                        }
                    }
                    if moved > 0 {
                        self.deck.ok(
                            &format!("{moved} {} in My cloud", if moved == 1 { "script is" } else { "scripts are" }),
                            "They sync to every computer you sign in on. A copy of each original is kept on this computer too.",
                        );
                    }
                    self.refresh_entries();
                }
            }
        }
    }

    // moving here---

    pub(super) fn move_items(&mut self, ui: &mut egui::Ui, rects: &mut Vec<Rect>, path: &Path) -> Option<Move> {
        if !self.signed_in() {
            return None;
        }
        let here = self.store.place(path);
        let spaces: Vec<Space> = self.store.spaces().into_iter().filter(|s| s.can_write && Some(&s.id) != here.as_ref()).collect();
        if spaces.is_empty() && here.is_none() {
            return None;
        }
        ui::separator(ui);
        let mut pick = None;
        for s in spaces {
            let label = if here.is_none() && s.personal {
                "Move to NativeCloud".to_string()
            } else if s.personal {
                "Move to My cloud".to_string()
            } else {
                format!("Move to {}", s.name)
            };
            if ui::menu_item(ui, rects, &label, if s.personal { Icon::Cloud } else { Icon::Users }, false) {
                pick = Some(Move::To(s.id.clone()));
            }
        }
        if here.is_some() && ui::menu_item(ui, rects, "Make local only", Icon::Download, false) {
            pick = Some(Move::Local);
        }
        pick
    }

    pub(super) fn do_move(&mut self, path: &Path, m: Move) {
        match m {
            Move::To(ws) => {
                let name = self.store.spaces().into_iter().find(|s| s.id == ws).map(|s| s.name).unwrap_or_else(|| "NativeCloud".into());
                if self.dirty && self.path.as_deref() == Some(path) {
                    self.save(false);
                }
                match self.store.move_to(path, Some(&ws)) {
                    Ok(new) => {
                        if self.path.as_deref() == Some(path) {
                            self.path = Some(new);
                        }
                        self.deck.ok(&format!("Moved to {name}"), "It syncs now. Everyone in that space can open it.");
                        self.refresh_entries();
                    }
                    Err(e) => self.deck.say("Couldn't move it", &e, Tone::Danger),
                }
            }
            Move::Local => {
                let space = self.store.place(path).and_then(|id| self.store.spaces().into_iter().find(|s| s.id == id)).map(|s| s.name).unwrap_or_default();
                let body = if space.is_empty() || space == "My cloud" {
                    "It stays on this computer and stops syncing. NativeCloud keeps it in Recently deleted for 30 days.".to_string()
                } else {
                    format!("It stays on this computer and leaves {space}, so the rest of the team loses it. NativeCloud keeps it in Recently deleted for 30 days.")
                };
                self.deck.ask("Keep this script on this computer only?", &body, "Make local only", Tone::Danger, Ask::MakeLocal(path.to_path_buf()));
            }
        }
    }

    pub(super) fn make_local(&mut self, path: &Path) {
        if self.dirty && self.path.as_deref() == Some(path) {
            self.save(false);
        }
        match self.store.move_to(path, None) {
            Ok(new) => {
                if self.path.as_deref() == Some(path) {
                    self.path = Some(new);
                    self.access = crate::backend::Access::Edit;
                }
                self.store.set_source(Source::Local);
                self.deck.ok("On this computer only", "It no longer syncs. It's under On this computer.");
                self.refresh_entries();
            }
            Err(e) => self.deck.say("Couldn't make it local", &e, Tone::Danger),
        }
    }

    // library sources here---

    pub(super) fn sources_bar(&mut self, ui: &mut egui::Ui, w: f32) {
        let spaces = self.store.spaces();
        if spaces.is_empty() {
            if self.cloud_on() && !self.signed_in() {
                ui.horizontal(|ui| {
                    ui::hud(ui, "On this computer", None);
                    ui.add_space(8.0);
                    if ui::link(ui, "Sign in to NativeCloud to sync and share") {
                        self.open_settings_tab(ACCOUNT_TAB);
                    }
                });
                ui.add_space(10.0);
            }
            return;
        }
        let source = self.store.source();
        let local_n = if source == Source::Local { self.entries.len() } else { usize::MAX };
        let mut labels: Vec<(String, Icon, Source)> = vec![(
            if local_n == usize::MAX { "On this computer".to_string() } else { format!("On this computer · {local_n}") },
            Icon::Folder,
            Source::Local,
        )];
        let orgs: std::collections::BTreeSet<&str> = spaces.iter().filter_map(|s| s.org.as_deref()).collect();
        for s in &spaces {
            let name = if orgs.len() > 1 { &s.name } else { &s.short };
            labels.push((format!("{name} · {}", s.scripts), if s.personal { Icon::Cloud } else { Icon::Users }, Source::Space(s.id.clone())));
        }
        let current = labels.iter().position(|l| l.2 == source).unwrap_or(0);
        let opts: Vec<(&str, Icon)> = labels.iter().map(|(l, i, _)| (l.as_str(), *i)).collect();
        ui.allocate_ui_with_layout(Vec2::new(w, 30.0), Layout::left_to_right(Align::Center), |ui| {
            if let Some(k) = ui::segmented(ui, &opts, current) {
                if k != current {
                    self.store.set_source(labels[k].2.clone());
                    self.home_sel = 0;
                    self.refresh_entries();
                }
            }
        });
        ui.add_space(4.0);
        ui.allocate_ui_with_layout(Vec2::new(w, 22.0), Layout::left_to_right(Align::Center), |ui| {
            if let Some(org) = spaces.iter().find(|s| Source::Space(s.id.clone()) == source).and_then(|s| s.org.clone()).filter(|_| orgs.len() <= 1) {
                ui::hud(ui, &org, None);
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if let Some(path) = self.store.web_path() {
                    if ui::link(ui, "Manage on the web") {
                        if let Some(a) = self.account.as_mut() {
                            a.open_on_web(&path);
                        }
                    }
                    ui.add_space(12.0);
                }
                if let Some(s) = self.store.sync() {
                    let p = pal();
                    let col = if s.warn {
                        p.warn
                    } else if s.busy {
                        p.primary_light
                    } else {
                        p.text_faint
                    };
                    ui::hud(ui, &s.text, Some(col));
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(13.0), Sense::hover());
                    icons::draw(ui.painter(), r, Icon::Cloud, col);
                }
            });
        });
        ui.add_space(12.0);
    }

    pub(super) fn cloud_title(&mut self, ui: &mut egui::Ui) {
        if !self.cloud_on() {
            return;
        }
        let p = pal();
        let (tip, col) = match self.store.sync() {
            Some(s) if self.signed_in() => (format!("NativeCloud · {}", s.text.to_lowercase()), if s.warn { p.warn } else if s.busy { p.primary_light } else { p.text_faint }),
            _ => ("Sign in to NativeCloud".to_string(), p.text_faint),
        };
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(28.0, 28.0), Sense::click());
        ui::ghost_slot(ui, rect, resp.hovered(), false);
        icons::draw(ui.painter(), Rect::from_center_size(rect.center(), Vec2::splat(15.0)), Icon::Cloud, col);
        if resp.on_hover_text(tip).clicked() {
            self.open_settings_tab(ACCOUNT_TAB);
        }
    }

    pub(super) fn account_body(&mut self, ui: &mut egui::Ui, tab: usize) {
        let Some(a) = self.account.as_mut() else {
            return;
        };
        if tab == ACCOUNT_TAB {
            a.account_ui(ui);
        } else {
            a.orgs_ui(ui);
        }
    }
}

impl App {
    pub fn debug_signed_in(&self) -> bool {
        self.signed_in()
    }

    pub fn debug_picker_open(&self) -> bool {
        self.picker.is_some()
    }

    pub fn debug_pick_all(&mut self) {
        if let Some(p) = self.picker.take() {
            self.move_chosen(p.rows.into_iter().map(|r| r.0).collect());
        }
    }

    pub fn debug_spaces(&self) -> Vec<Space> {
        self.store.spaces()
    }

    pub fn debug_set_source(&mut self, source: Source) {
        self.store.set_source(source);
        self.refresh_entries();
    }

    pub fn debug_store_key(&self, title: &str) -> Option<PathBuf> {
        self.entries.iter().find(|e| e.title == title).map(|e| e.path.clone())
    }

    pub fn debug_entries(&self) -> Vec<crate::storage::Entry> {
        self.entries.clone()
    }
}
