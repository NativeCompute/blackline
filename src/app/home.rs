use std::path::PathBuf;

use eframe::egui::{self, Align, Layout, Pos2, Rect, Sense, Stroke, Vec2};
use web_time::Instant;

use super::{new_script_button, relative_time, youtrack_button, App, Popover};
use crate::anim;
use crate::icons::{self, Icon};
use crate::people;
use crate::storage::{Entry, Found};
use crate::theme::{self, pal};
use crate::ui;

const CARD_W: f32 = 268.0;
const CARD_H: f32 = 132.0;
const T_CONTINUE: f32 = 22.0;
const CONTINUE_RIGHT: f32 = 150.0;
const HERO_T: f32 = 34.0;
const HERO_T_MIN: f32 = 22.0;
const HERO_BUTTONS: f32 = 300.0;

impl App {
    pub(super) fn go_home(&mut self) {
        if self.dirty {
            self.save(false);
        }
        if let Some(path) = self.path.clone() {
            self.store.close(&path);
        }
        self.home = true;
        self.home_born = Instant::now();
        self.popover = None;
        self.find.open = false;
        self.focus_mode = false;
        self.refresh_entries();
    }

    pub(super) fn home_screen(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default()
            .frame(egui::Frame::none().inner_margin(egui::Margin {
                left: theme::GAP,
                right: theme::GAP,
                top: theme::GAP * 0.5,
                bottom: theme::GAP,
            }))
            .show(ctx, |ui| {
                let full = ui.available_rect_before_wrap();
                let w = (full.width() - 2.0 * theme::GAP).clamp(300.0, 1100.0);
                let col = Rect::from_min_size(
                    Pos2::new(full.center().x - w * 0.5, full.top()),
                    Vec2::new(w, full.height()),
                );
                let mut inner = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(col)
                        .layout(Layout::top_down(Align::Min)),
                );
                let t = (self.home_born.elapsed().as_secs_f32() / 0.32).clamp(0.0, 1.0);
                if t < 1.0 && anim::enabled() {
                    inner.set_opacity(0.25 + 0.75 * anim::smootherstep(t));
                    anim::keep_going(ctx);
                }
                egui::ScrollArea::vertical()
                    .id_salt("ns-home")
                    .auto_shrink([false, false])
                    .show(&mut inner, |ui| {
                        ui.set_width(w - 14.0);
                        self.home_body(ui);
                    });
            });
    }

    fn home_body(&mut self, ui: &mut egui::Ui) {
        let p = pal();
        let team = self.store.team().cloned();
        let w = ui.available_width();

        // greeting straight glass here---
        ui.add_space(14.0);
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                let hello = match team.as_ref().map(|t| t.me.first_name().to_string()).or_else(local_name) {
                    Some(name) => format!("{}, {name}", greeting()),
                    None => greeting().to_string(),
                };
                let about = match &team {
                    Some(team) => {
                        let people = team.members.iter().filter(|m| !m.pending).count().max(1);
                        format!(
                            "{} · {} · {}",
                            team.name,
                            plural(self.entries.len(), "script", "scripts"),
                            plural(people, "person", "people"),
                        )
                    }
                    None => format!("Your library · {}", plural(self.entries.len(), "script", "scripts")),
                };
                ui::hud(ui, &about.replace(" · ", " / "), None);
                ui.add_space(6.0);
                let buttons = HERO_BUTTONS + if team.is_some() { 38.0 } else { 0.0 };
                let room = (w - buttons).max(120.0);
                let shout = hello.to_uppercase();
                let at = |size: f32| theme::tracked_width(ui.ctx(), &shout, theme::font_display(size), 0.0);
                let full = at(HERO_T);
                let size = if full <= room { HERO_T } else { (HERO_T * room / full * 0.97).max(HERO_T_MIN) };
                let shown = ui::elide_to_width(ui.ctx(), &shout, theme::font_display(size), 0.0, room);
                ui.add(
                    egui::Label::new(egui::RichText::new(shown).font(theme::font_display(size)).color(p.text))
                        .extend(),
                );
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if self.can_write() {
                    if new_script_button(ui, 150.0, &self.keys_for(crate::keys::Command::NewScript)) {
                        self.new_script();
                        self.home = false;
                    }
                    ui.add_space(4.0);
                    if ui::button(ui, "Import", Some(Icon::Upload), false) {
                        self.start_import();
                    }
                }
                if team.is_some() && ui::icon_button(ui, Icon::Trash, "Recently deleted", 30.0) {
                    self.toggle_popover(Popover::Trash);
                }
            });
        });
        ui.add_space(14.0);

        // search here---
        let search = ui.add(
            egui::TextEdit::singleline(&mut self.home_search)
                .id(egui::Id::new("ns-home-search"))
                .hint_text(if team.is_some() {
                    "Search the team's scripts  ·  Ctrl+F"
                } else {
                    "Search titles and every line  ·  Ctrl+F"
                })
                .desired_width(w.min(380.0))
                .margin(egui::Margin::symmetric(12.0, 8.0)),
        );
        if self.focus_home_search {
            search.request_focus();
            self.focus_home_search = false;
        }
        let enter = search.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        ui.add_space(16.0);

        let needle = self.home_search.trim().to_lowercase();
        let mut list: Vec<Entry> = self
            .entries
            .iter()
            .filter(|e| {
                needle.is_empty()
                    || e.title.to_lowercase().contains(&needle)
                    || e.author.to_lowercase().contains(&needle)
                    || e.preview.to_lowercase().contains(&needle)
            })
            .cloned()
            .collect();
        list.sort_by_key(|a| std::cmp::Reverse(a.modified));

        if self.entries.is_empty() {
            self.home_empty(ui);
            self.home_foot(ui);
            return;
        }

        // you left off here---
        let me = team.as_ref().map(|t| t.me.clone());
        if needle.is_empty() {
            let mine = list
                .iter()
                .find(|e| match (&me, &e.edited_by) {
                    (Some(me), Some(by)) => by.id == me.id,
                    _ => false,
                })
                .or_else(|| list.first())
                .cloned();
            if let Some(e) = mine {
                self.continue_card(ui, &e, w);
                ui.add_space(22.0);
            }
        }

        // every script cards here---
        let (starred, rest): (Vec<Entry>, Vec<Entry>) = list.into_iter().partition(|e| e.starred);
        let mut order: Vec<PathBuf> = Vec::new();
        let mut open = None;
        let mut star = None;
        let mut menu = None;
        for (label, icon, items) in [
            ("Starred", Icon::StarFilled, &starred),
            (if team.is_some() { "Team scripts" } else { "Scripts" }, Icon::File, &rest),
        ] {
            if items.is_empty() {
                continue;
            }
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                icons::show(ui, icon, 12.0, if label == "Starred" { p.warn } else { p.text_faint });
                ui::hud(ui, &format!("{label} / {:02}", items.len()), Some(p.text_dim));
            });
            ui.add_space(8.0);
            let gap = theme::GAP;
            let cols = (((w + gap) / (CARD_W + gap)).floor() as usize).max(1);
            let card_w = (w - gap * (cols - 1) as f32) / cols as f32;
            ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("ns-home-cols"), cols));
            for row in items.chunks(cols) {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = gap;
                    for e in row {
                        let selected = self.home_keyed && self.home_sel == order.len();
                        order.push(e.path.clone());
                        match self.script_card(ui, e, card_w, selected) {
                            CardAction::Open => open = Some(e.path.clone()),
                            CardAction::Star => star = Some(e.path.clone()),
                            CardAction::Menu(at) => menu = Some((e.path.clone(), at)),
                            CardAction::None => {}
                        }
                    }
                });
                ui.add_space(gap);
            }
            ui.add_space(10.0);
        }
        let found = self.found_for(&needle);
        let mut jump = None;
        self.hit_rows.clear();
        if !found.is_empty() {
            jump = self.found_section(ui, &found, w);
        }
        if enter && open.is_none() && jump.is_none() && !needle.is_empty() {
            match (order.first(), found.first().and_then(|f| f.hits.first().map(|h| (f.path.clone(), h.block)))) {
                (Some(path), _) => open = Some(path.clone()),
                (None, Some(hit)) => jump = Some(hit),
                (None, None) => {}
            }
        }
        if order.is_empty() && found.is_empty() {
            ui.label(
                egui::RichText::new("Nothing matches.")
                    .font(theme::font(theme::T_SM))
                    .color(p.text_faint),
            );
        }
        self.home_order = order;

        self.home_foot(ui);

        if let Some(path) = star {
            self.toggle_star(&path);
        }
        if let Some((path, at)) = menu {
            self.rail.menu = Some((path, at, self.frame_no));
        }
        if let Some(path) = open {
            self.open_from_home(path);
        } else if let Some((path, k)) = jump {
            self.pending_jump = Some((path.clone(), k));
            self.open_from_home(path);
        }
    }

    fn found_for(&mut self, needle: &str) -> Vec<Found> {
        if needle.chars().count() < 2 {
            return Vec::new();
        }
        if self.home_hits.as_ref().map(|(n, _)| n.as_str()) != Some(needle) {
            let found = self.store.search(needle).unwrap_or_default();
            self.home_hits = Some((needle.to_string(), found));
        }
        self.home_hits.as_ref().map(|(_, f)| f.clone()).unwrap_or_default()
    }

    fn found_section(&mut self, ui: &mut egui::Ui, found: &[Found], w: f32) -> Option<(PathBuf, usize)> {
        let p = pal();
        let mut picked = None;
        let all: usize = found.iter().map(|f| f.total).sum();
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            icons::show(ui, Icon::Search, 12.0, p.text_faint);
            ui::hud(ui, &format!("In the pages / {all:02}"), Some(p.text_dim));
        });
        ui.add_space(8.0);
        let font = theme::font_page(theme::T_SM);
        for f in found {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                ui.label(egui::RichText::new(&f.title).font(theme::font_med(theme::T_SM)).color(p.text));
                let more = if f.total == 1 { "1 match".to_string() } else { format!("{} matches", f.total) };
                ui.label(egui::RichText::new(more).font(theme::font_mono(theme::T_CAP)).color(p.text_faint));
            });
            ui.add_space(4.0);
            for h in &f.hits {
                let chars: Vec<char> = h.line.chars().collect();
                let cut = |a: usize, b: usize| chars[a.min(chars.len())..b.min(chars.len())].iter().collect::<String>();
                let mut job = egui::text::LayoutJob::default();
                let plain = egui::TextFormat { font_id: font.clone(), color: p.text_dim, ..Default::default() };
                if h.note {
                    job.append("NOTE  ", 0.0, egui::TextFormat { font_id: theme::font_mono(theme::T_CAP), color: p.primary_light, ..Default::default() });
                }
                job.append(&cut(0, h.start), 0.0, plain.clone());
                job.append(
                    &cut(h.start, h.start + h.len),
                    0.0,
                    egui::TextFormat { font_id: font.clone(), color: p.text, background: theme::wash(p.primary, 90), ..Default::default() },
                );
                job.append(&cut(h.start + h.len, chars.len()), 0.0, plain);
                job.wrap.max_width = w - 28.0;
                job.wrap.max_rows = 1;
                let galley = ui.painter().layout_job(job);
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, galley.rect.height() + 14.0), Sense::click());
                let hot = anim::ease(ui.ctx(), resp.id, resp.hovered(), anim::HOVER);
                theme::hover_surface(ui.painter(), rect, theme::R_SM, p.primary, hot, false);
                ui.painter().galley(Pos2::new(rect.left() + 14.0, rect.center().y - galley.rect.height() * 0.5), galley, p.text);
                self.hit_rows.push((f.path.clone(), h.block, rect));
                if resp.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                if resp.clicked() {
                    picked = Some((f.path.clone(), h.block));
                }
            }
            if f.total > f.hits.len() {
                ui.label(
                    egui::RichText::new(format!("and {} more in this script", f.total - f.hits.len()))
                        .font(theme::font(theme::T_CAP))
                        .color(p.text_faint),
                );
            }
            ui.add_space(12.0);
        }
        picked
    }

    pub(super) fn open_from_home(&mut self, path: PathBuf) {
        self.mode = super::Mode::Write;
        self.open(path);
    }

    fn continue_card(&mut self, ui: &mut egui::Ui, e: &Entry, w: f32) {
        let p = pal();
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 98.0), Sense::click());
        let hot = anim::ease(ui.ctx(), resp.id, resp.hovered(), anim::HOVER);
        ui.painter().rect_filled(rect, egui::Rounding::same(theme::R_SM), theme::mix(p.solid, p.solid_hi, hot));
        ui.painter().rect_stroke(
            rect.shrink(0.5),
            egui::Rounding::same(theme::R_SM),
            Stroke::new(1.0_f32, p.primary_light),
        );
        theme::top_edge(ui.painter(), rect);

        let x = rect.left() + 26.0;
        ui.painter().rect_filled(
            Rect::from_center_size(Pos2::new(x + 3.0, rect.top() + 22.0), Vec2::splat(6.0)),
            egui::Rounding::ZERO,
            p.primary_light,
        );
        theme::tracked_text(
            ui.painter(),
            Pos2::new(x + 14.0, rect.top() + 22.0),
            "CONTINUE WRITING",
            theme::font_hud(theme::T_MICRO + 0.5),
            p.primary_light,
            0.8,
        );
        let room = (rect.right() - CONTINUE_RIGHT - x).max(40.0);
        let clip = ui.painter().with_clip_rect(rect.shrink(1.0));
        clip.text(
            Pos2::new(x, rect.top() + 48.0),
            egui::Align2::LEFT_CENTER,
            ui::elide_to_width(ui.ctx(), &e.title.to_uppercase(), theme::font_display(T_CONTINUE), 0.0, room),
            theme::font_display(T_CONTINUE),
            p.text,
        );
        let me = self.me();
        let mut meta = format!(
            "{} / {}",
            plural(e.pages, "page", "pages"),
            plural(e.scenes, "scene", "scenes")
        );
        if let Some(by) = &e.edited_by {
            meta.push_str(&format!(" / saved by {}", people::by_line(by, me.as_ref(), &relative_time(e.modified))));
        }
        let hud = theme::font_hud(theme::T_MICRO + 0.5);
        theme::tracked_text(
            &clip,
            Pos2::new(x, rect.top() + 76.0),
            &ui::elide_to_width(ui.ctx(), &meta.replace(" · ", " / ").to_uppercase(), hud.clone(), 0.8, room),
            hud,
            p.text_faint,
            0.8,
        );
        let right = Pos2::new(rect.right() - 24.0, rect.center().y);
        if self.is_opening(&e.path) {
            ui.painter().text(right, egui::Align2::RIGHT_CENTER, "Opening…", theme::font_med(theme::T_SM), p.text_dim);
        } else if let Some(who) = &e.editing {
            editing_badge(ui, right, who);
        } else {
            let ink = theme::mix(p.text_dim, p.primary_light, hot);
            let arrow = Rect::from_center_size(Pos2::new(right.x - 7.0, right.y), Vec2::splat(14.0));
            icons::draw(ui.painter(), arrow, Icon::ArrowRight, ink);
            ui.painter().text(
                Pos2::new(arrow.left() - 6.0, right.y),
                egui::Align2::RIGHT_CENTER,
                "Open",
                theme::font_med(theme::T_SM),
                ink,
            );
        }
        if resp.clicked() {
            self.open_from_home(e.path.clone());
        }
    }

    fn script_card(&self, ui: &mut egui::Ui, e: &Entry, w: f32, selected: bool) -> CardAction {
        let p = pal();
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, CARD_H), Sense::click());
        let hot = anim::ease(ui.ctx(), resp.id, resp.hovered(), anim::HOVER);
        ui.painter().rect_filled(rect, egui::Rounding::same(theme::R_SM), theme::mix(p.solid, p.solid_hi, hot));
        ui.painter().rect_stroke(
            rect.shrink(0.5),
            egui::Rounding::same(theme::R_SM),
            Stroke::new(1.0_f32, theme::mix(p.line, p.text_faint, hot)),
        );
        theme::top_edge(ui.painter(), rect);
        if selected {
            ui.painter().rect_stroke(
                rect.expand(3.0),
                egui::Rounding::same(theme::R_SM),
                Stroke::new(2.0_f32, p.primary_light),
            );
        }

        let pad = 16.0;
        let x = rect.left() + pad;
        let cp = ui.painter().with_clip_rect(rect.shrink(1.0));
        let inner = w - pad * 2.0;
        let title_font = theme::font_bold(theme::T_BODY + 0.5);
        cp.text(
            Pos2::new(x, rect.top() + 24.0),
            egui::Align2::LEFT_CENTER,
            ui::elide_to_width(ui.ctx(), &e.title, title_font.clone(), 0.0, inner - 28.0),
            title_font,
            p.text,
        );
        let author = if e.author.trim().is_empty() {
            "No author yet".to_string()
        } else {
            format!("by {}", e.author.trim())
        };
        cp.text(
            Pos2::new(x, rect.top() + 46.0),
            egui::Align2::LEFT_CENTER,
            ui::elide_to_width(ui.ctx(), &author, theme::font(theme::T_CAP), 0.0, inner),
            theme::font(theme::T_CAP),
            p.text_dim,
        );
        theme::tracked_text(
            ui.painter(),
            Pos2::new(x, rect.top() + 68.0),
            &format!("{:02} pg / {:02} sc", e.pages, e.scenes).to_uppercase(),
            theme::font_hud(theme::T_MICRO + 0.5),
            p.text_faint,
            0.8,
        );

        let foot_y = rect.bottom() - 24.0;
        let me = self.me();
        if self.is_opening(&e.path) {
            ui.painter().text(
                Pos2::new(x, foot_y),
                egui::Align2::LEFT_CENTER,
                "Opening…",
                theme::font_med(theme::T_CAP),
                p.text_dim,
            );
        } else if let Some(who) = &e.editing {
            editing_badge(ui, Pos2::new(rect.right() - pad, foot_y), who);
            if let Some(by) = &e.edited_by {
                people::avatar(ui, Rect::from_center_size(Pos2::new(x + 9.0, foot_y), Vec2::splat(18.0)), by);
            }
        } else if let Some(by) = &e.edited_by {
            people::avatar(ui, Rect::from_center_size(Pos2::new(x + 9.0, foot_y), Vec2::splat(18.0)), by);
            cp.text(
                Pos2::new(x + 26.0, foot_y),
                egui::Align2::LEFT_CENTER,
                ui::elide_to_width(
                    ui.ctx(),
                    &people::by_line(by, me.as_ref(), &relative_time(e.modified)),
                    theme::font(theme::T_CAP),
                    0.0,
                    inner - 26.0,
                ),
                theme::font(theme::T_CAP),
                p.text_dim,
            );
        } else {
            ui.painter().text(
                Pos2::new(x, foot_y),
                egui::Align2::LEFT_CENTER,
                relative_time(e.modified),
                theme::font(theme::T_CAP),
                p.text_faint,
            );
        }

        let star_rect = Rect::from_center_size(Pos2::new(rect.right() - 22.0, rect.top() + 24.0), Vec2::splat(24.0));
        let star_resp = ui.interact(star_rect, resp.id.with("star"), Sense::click());
        let star_hot = anim::ease(ui.ctx(), star_resp.id, star_resp.hovered(), anim::HOVER);
        if e.starred || hot > 0.02 {
            icons::draw(
                ui.painter(),
                star_rect.shrink(6.0),
                if e.starred { Icon::StarFilled } else { Icon::Star },
                if e.starred {
                    theme::lighten(p.warn, 0.2 * star_hot)
                } else {
                    theme::mix(theme::wash(p.text_faint, (200.0 * hot) as u8), p.warn, star_hot)
                },
            );
        }
        let star_clicked = star_resp
            .on_hover_text(if e.starred { "Unstar — stars are yours alone" } else { "Star it, for you" })
            .clicked();

        let resp = if e.preview.is_empty() {
            resp
        } else {
            resp.on_hover_text(format!("{}\nright click for more", e.preview))
        };
        if star_clicked {
            CardAction::Star
        } else if resp.secondary_clicked() {
            ui.ctx()
                .pointer_latest_pos()
                .map(CardAction::Menu)
                .unwrap_or(CardAction::None)
        } else if resp.clicked() {
            CardAction::Open
        } else {
            CardAction::None
        }
    }

    fn home_empty(&mut self, ui: &mut egui::Ui) {
        let p = pal();
        theme::island_frame(theme::R_ISLAND)
            .inner_margin(egui::Margin::symmetric(28.0, 26.0))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(
                    egui::RichText::new("NO SCRIPTS YET")
                        .font(theme::font_display(26.0))
                        .color(p.text),
                );
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(if self.can_write() {
                        "Start one, or bring one in: Fountain, Final Draft and Blackline's own files all import."
                    } else {
                        "When someone on the team starts a script, it shows up here."
                    })
                    .font(theme::font(theme::T_SM))
                    .color(p.text_dim),
                );
                if self.can_write() {
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        if new_script_button(ui, 150.0, &self.keys_for(crate::keys::Command::NewScript)) {
                            self.new_script();
                            self.home = false;
                        }
                        ui.add_space(4.0);
                        if ui::button(ui, "Import", Some(Icon::Upload), false) {
                            self.start_import();
                        }
                    });
                }
            });
        ui.add_space(18.0);
    }

    fn home_foot(&mut self, ui: &mut egui::Ui) {
        ui.add_space(20.0);
        ui.vertical_centered(|ui| {
            ui.set_width(260.0);
            if !self.settings.youtrack_url.trim().is_empty() && youtrack_button(ui, &self.settings.youtrack_url) {
                let url = self.settings.youtrack_url.trim().to_string();
                if url.starts_with("http://") || url.starts_with("https://") {
                    self.store.open_url(&url);
                } else {
                    self.deck.warn("That is not a web address", "Set the YouTrack link in Settings.");
                }
            }
            ui.add_space(10.0);
            let (r, _) = ui.allocate_exact_size(Vec2::new(260.0, 14.0), Sense::hover());
            if ui::maker_line(ui, r, true) {
                self.store.open_url(crate::NATIVECOMPUTE_URL);
            }
        });
        ui.add_space(12.0);
    }

    pub(super) fn home_keys(&mut self, ctx: &egui::Context) {
        use egui::{Key, KeyboardShortcut, Modifiers};
        let typing = ctx.memory(|m| m.focused()).is_some();
        let n = self.home_order.len();
        let cols = ctx.data(|d| d.get_temp::<usize>(egui::Id::new("ns-home-cols"))).unwrap_or(3).max(1);
        if ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::F))) {
            self.focus_home_search = true;
        }
        if typing || n == 0 {
            return;
        }
        let mut sel = self.home_sel.min(n - 1);
        let before = sel;
        let mut moved = false;
        ctx.input(|i| {
            moved = [Key::ArrowRight, Key::ArrowLeft, Key::ArrowDown, Key::ArrowUp]
                .iter()
                .any(|k| i.key_pressed(*k));
            if i.key_pressed(Key::ArrowRight) {
                sel = (sel + 1).min(n - 1);
            }
            if i.key_pressed(Key::ArrowLeft) {
                sel = sel.saturating_sub(1);
            }
            if i.key_pressed(Key::ArrowDown) {
                sel = (sel + cols).min(n - 1);
            }
            if i.key_pressed(Key::ArrowUp) {
                sel = sel.saturating_sub(cols);
            }
        });
        if moved && !self.home_keyed {
            self.home_keyed = true;
            sel = before;
        }
        self.home_sel = sel;
        if self.home_keyed && ctx.input(|i| i.key_pressed(Key::Enter)) {
            if let Some(path) = self.home_order.get(sel).cloned() {
                self.open_from_home(path);
            }
        }
    }
}

enum CardAction {
    None,
    Open,
    Star,
    Menu(Pos2),
}

fn editing_badge(ui: &egui::Ui, right: Pos2, who: &crate::backend::Person) {
    let p = pal();
    let text = format!("{} is editing", who.first_name());
    let galley = ui
        .painter()
        .layout_no_wrap(text, theme::font_med(theme::T_CAP), p.text);
    let left = right.x - galley.rect.width();
    ui.painter().circle_filled(Pos2::new(left - 9.0, right.y), 3.2, p.ok);
    ui.painter()
        .galley(Pos2::new(left, right.y - galley.rect.height() * 0.5), galley, p.text);
}

fn greeting() -> &'static str {
    use chrono::Timelike;
    match chrono::Local::now().hour() {
        5..=11 => "Good morning",
        12..=17 => "Good afternoon",
        18..=22 => "Good evening",
        _ => "Up late",
    }
}

fn local_name() -> Option<String> {
    if crate::WEB {
        return None;
    }
    let raw = std::env::var("USER").or_else(|_| std::env::var("USERNAME")).ok()?;
    let first = raw.split(['.', '_', '-', ' ']).next()?.trim();
    if first.is_empty() || first == "root" || !first.chars().all(|c| c.is_alphabetic()) {
        return None;
    }
    let mut c = first.chars();
    Some(c.next()?.to_uppercase().collect::<String>() + &c.as_str().to_lowercase())
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}
