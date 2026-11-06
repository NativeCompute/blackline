use eframe::egui::{self, Align, Color32, Layout, Pos2, Rect, Sense, Stroke, Vec2};

use super::{App, Mode, Popover};
use crate::anim;
use crate::chrome;
use crate::icons::{self, Icon};
use crate::model::eighths_label;
use crate::report::{self, Report};
use crate::theme::{self, pal};
use crate::ui;

const TABS: [&str; 4] = ["Characters", "Scenes", "Locations", "Day and night"];

struct Col {
    head: &'static str,
    w: f32,
    right: bool,
}

const fn col(head: &'static str, w: f32, right: bool) -> Col {
    Col { head, w, right }
}

impl App {
    pub(super) fn open_reports(&mut self) {
        self.report = report::build(&self.doc);
        self.popover = Some(Popover::Reports);
        self.popover_frame = self.frame_no;
    }

    pub(super) fn reports_window(&mut self, ctx: &egui::Context) {
        let p = pal();
        let screen = ctx.screen_rect();
        let mut close = false;
        let mut go: Option<u64> = None;

        let veil = anim::ease(ctx, "ns-reports-veil", true, 0.24);
        egui::Area::new(egui::Id::new("ns-reports-scrim"))
            .order(egui::Order::Middle)
            .fixed_pos(screen.min)
            .interactable(true)
            .show(ctx, |ui| {
                let (rect, resp) = ui.allocate_exact_size(screen.size(), Sense::click());
                ui.painter().rect_filled(
                    rect,
                    egui::Rounding::same(chrome::window_radius(ctx, theme::R_WINDOW)),
                    Color32::from_black_alpha((120.0 * veil) as u8),
                );
                if resp.clicked() && self.frame_no > self.popover_frame + 1 {
                    close = true;
                }
            });

        let t = anim::ease(ctx, "ns-reports-rise", true, 0.30);
        let size = Vec2::new((screen.width() - 80.0).clamp(420.0, 860.0), (screen.height() - 90.0).clamp(320.0, 620.0));
        let rect = Rect::from_center_size(screen.center(), size * (0.97 + 0.03 * t));
        let title = self.title_or_untitled();

        egui::Area::new(egui::Id::new("ns-reports"))
            .order(egui::Order::Foreground)
            .fixed_pos(rect.min)
            .show(ctx, |ui| {
                ui.set_opacity(0.2 + 0.8 * t);
                let (win, _) = ui.allocate_exact_size(rect.size(), Sense::click());
                theme::lift_shadow(ui.painter(), win, theme::R_ISLAND, 1.0);
                ui.painter().rect_filled(win, egui::Rounding::same(theme::R_ISLAND), p.solid);
                ui.painter().rect_stroke(
                    win.shrink(0.5),
                    egui::Rounding::same(theme::R_ISLAND),
                    Stroke::new(1.0_f32, theme::wash(Color32::WHITE, if p.dark { 18 } else { 120 })),
                );
                let mut ui = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(win.shrink2(Vec2::new(22.0, 18.0)))
                        .layout(Layout::top_down(Align::Min)),
                );
                let ui = &mut ui;

                // head here---
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 10.0;
                    icons::show(ui, Icon::Graph, 16.0, p.primary_light);
                    ui.label(egui::RichText::new("Reports").font(theme::font_semi(theme::T_H)).color(p.text));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui::icon_button(ui, Icon::Close, "Close · Esc", 26.0) {
                            close = true;
                        }
                    });
                });
                let r = &self.report;
                ui.label(
                    egui::RichText::new(format!(
                        "{}  ·  {}  ·  {}  ·  {}",
                        ui::elide(&title, 48),
                        count(r.pages, "page", "pages"),
                        count(r.scenes.len(), "scene", "scenes"),
                        count(r.speakers.len(), "speaking part", "speaking parts")
                    ))
                    .font(theme::font(theme::T_CAP))
                    .color(p.text_faint),
                );
                ui.add_space(12.0);
                if let Some(k) = ui::tabs(ui, &TABS, self.report_tab) {
                    self.report_tab = k;
                }
                ui.add_space(12.0);

                // table here---
                let foot = 44.0;
                let (area, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), (ui.available_height() - foot).max(80.0)), Sense::hover());
                let mut body = ui.new_child(egui::UiBuilder::new().max_rect(area).layout(Layout::top_down(Align::Min)));
                body.set_clip_rect(area);
                self.report_rows.clear();
                go = table(&mut body, &self.report, self.report_tab, area.width(), &mut self.report_rows);

                // foot here---
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui::button(ui, "Copy as text", Some(Icon::Copy), false) {
                        ui.ctx().copy_text(self.report.text(self.report_tab, &title));
                        self.deck.ok("Copied", &format!("{} report, ready to paste.", TABS[self.report_tab]));
                    }
                    ui.add_space(8.0);
                    ui::caption(ui, match self.report_tab {
                        1 => "Lengths in eighths of a page, as printed. Click a scene to go to it.",
                        0 => "Share is each character's part of all spoken words.",
                        _ => "Lengths in eighths of a page, as printed.",
                    });
                });
            });

        if let Some(id) = go {
            self.popover = None;
            if self.mode != Mode::Write && self.access.can_edit() {
                self.mode = Mode::Write;
            }
            self.ed.jump_to(id);
            self.pages.show_block = Some(id);
        }
        if close {
            self.popover = None;
        }
    }

    pub fn debug_report(&self) -> &Report {
        &self.report
    }
    pub fn debug_report_tab(&mut self, tab: usize) {
        self.report_tab = tab;
    }
    pub fn debug_report_rows(&self) -> Vec<(u64, Rect)> {
        self.report_rows.clone()
    }
}

fn table(ui: &mut egui::Ui, r: &Report, tab: usize, width: f32, seen: &mut Vec<(u64, Rect)>) -> Option<u64> {
    let p = pal();
    let cols: Vec<Col> = match tab {
        0 => vec![col("CHARACTER", 0.0, false), col("CUES", 60.0, true), col("LINES", 60.0, true), col("WORDS", 70.0, true), col("SHARE", 130.0, false), col("SCENES", 66.0, true), col("FIRST", 120.0, true)],
        1 => vec![col("#", 36.0, true), col("HEADING", 0.0, false), col("PAGE", 54.0, true), col("LENGTH", 74.0, true), col("CAST", 230.0, false)],
        2 => vec![col("LOCATION", 0.0, false), col("INT/EXT", 130.0, false), col("SCENES", 70.0, true), col("LENGTH", 80.0, true), col("FIRST PAGE", 96.0, true)],
        _ => vec![col("TIME", 0.0, false), col("SCENES", 80.0, true), col("LENGTH", 90.0, true), col("", 260.0, false)],
    };
    let fixed: f32 = cols.iter().map(|c| c.w).sum();
    let flex = (width - fixed - 16.0).max(120.0);
    let widths: Vec<f32> = cols.iter().map(|c| if c.w == 0.0 { flex } else { c.w }).collect();

    let head_font = theme::font_hud(theme::T_MICRO);
    let (head, _) = ui.allocate_exact_size(Vec2::new(width, 22.0), Sense::hover());
    let mut x = head.left();
    for (c, w) in cols.iter().zip(&widths) {
        let at = if c.right { Pos2::new(x + w - 10.0, head.center().y) } else { Pos2::new(x + 10.0, head.center().y) };
        let align = if c.right { egui::Align2::RIGHT_CENTER } else { egui::Align2::LEFT_CENTER };
        ui.painter().text(at, align, c.head, head_font.clone(), p.text_faint);
        x += w;
    }
    ui.painter().line_segment([head.left_bottom(), head.right_bottom()], Stroke::new(1.0_f32, p.line));

    let rows: Vec<(Option<u64>, Vec<String>, Option<f32>)> = match tab {
        0 => r
            .speakers
            .iter()
            .map(|s| {
                (
                    None,
                    vec![
                        s.name.clone(),
                        s.cues.to_string(),
                        s.lines.to_string(),
                        s.words.to_string(),
                        format!("{:.0}%", s.share * 100.0),
                        s.scenes.to_string(),
                        format!("sc. {} · p. {}", s.first_scene, s.first_page),
                    ],
                    Some(s.share),
                )
            })
            .collect(),
        1 => r
            .scenes
            .iter()
            .map(|s| (Some(s.id), vec![s.number.to_string(), s.heading.clone(), s.page.to_string(), eighths_label(s.eighths), s.cast.join(", ")], None))
            .collect(),
        2 => r
            .places
            .iter()
            .map(|pl| (None, vec![pl.name.clone(), pl.sides.join("  "), pl.scenes.to_string(), eighths_label(pl.eighths), pl.first_page.to_string()], None))
            .collect(),
        _ => {
            let all = r.eighths.max(1) as f32;
            r.times
                .iter()
                .map(|t| (None, vec![t.name.clone(), t.scenes.to_string(), eighths_label(t.eighths), String::new()], Some(t.eighths as f32 / all)))
                .collect()
        }
    };
    let bar_col = if tab == 0 { 4 } else { 3 };

    let mut picked = None;
    egui::ScrollArea::vertical()
        .id_salt(("ns-report-rows", tab))
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            if rows.is_empty() {
                ui.add_space(14.0);
                ui::body(ui, match tab {
                    0 => "Nobody speaks yet. Character cues show up here.",
                    _ => "No scenes yet. Scene headings show up here.",
                });
            }
            let font = theme::font(theme::T_SM);
            for (k, (id, cells, bar)) in rows.iter().enumerate() {
                let sense = if id.is_some() { Sense::click() } else { Sense::hover() };
                let (row, resp) = ui.allocate_exact_size(Vec2::new(width, 30.0), sense);
                if k % 2 == 1 {
                    ui.painter().rect_filled(row, egui::Rounding::ZERO, theme::mix(p.solid, p.solid_hi, 0.55));
                }
                if let Some(id) = id {
                    seen.push((*id, row));
                }
                if id.is_some() {
                    let hot = anim::ease(ui.ctx(), resp.id, resp.hovered(), anim::HOVER);
                    theme::hover_surface(ui.painter(), row, theme::R_SM, p.primary, hot, false);
                    if resp.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if resp.clicked() {
                        picked = *id;
                    }
                }
                let mut x = row.left();
                for (c, (cell, w)) in cells.iter().zip(&widths).enumerate() {
                    let ink = if c == 0 { p.text } else { p.text_dim };
                    if c == bar_col {
                        if let Some(v) = bar {
                            let track = Rect::from_min_size(Pos2::new(x + 10.0, row.center().y - 3.0), Vec2::new((w - 60.0).max(20.0), 6.0));
                            ui.painter().rect_filled(track, egui::Rounding::same(3.0), theme::wash(p.text, 18));
                            let fill = Rect::from_min_size(track.min, Vec2::new(track.width() * v.clamp(0.0, 1.0), track.height()));
                            ui.painter().rect_filled(fill, egui::Rounding::same(3.0), p.primary_light);
                            if !cell.is_empty() {
                                ui.painter().text(Pos2::new(x + w - 10.0, row.center().y), egui::Align2::RIGHT_CENTER, cell, theme::font_mono(theme::T_CAP), p.text_dim);
                            }
                            x += w;
                            continue;
                        }
                    }
                    let shown = ui::elide_to_width(ui.ctx(), cell, font.clone(), 0.0, w - 20.0);
                    let (at, align) = if c_right(&cols, c) {
                        (Pos2::new(x + w - 10.0, row.center().y), egui::Align2::RIGHT_CENTER)
                    } else {
                        (Pos2::new(x + 10.0, row.center().y), egui::Align2::LEFT_CENTER)
                    };
                    ui.painter().text(at, align, shown, font.clone(), ink);
                    x += w;
                }
            }
            ui.add_space(8.0);
        });
    picked
}

fn c_right(cols: &[Col], k: usize) -> bool {
    cols.get(k).map(|c| c.right).unwrap_or(false)
}

fn count(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}
