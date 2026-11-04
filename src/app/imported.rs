use eframe::egui::{self, Align, Color32, Layout, Pos2, Rect, Sense, Stroke, Vec2};

use super::{App, Mode, Popover};
use crate::anim;
use crate::chrome;
use crate::fountain::Issue;
use crate::icons::{self, Icon};
use crate::theme::{self, pal};
use crate::ui;

pub(super) struct Imported {
    pub title: String,
    pub issues: Vec<Issue>,
}

impl App {
    pub(super) fn show_import_report(&mut self, title: String, issues: Vec<Issue>) {
        self.imported = Some(Imported { title, issues });
        self.popover = Some(Popover::Imported);
        self.popover_frame = self.frame_no;
    }

    pub(super) fn imported_window(&mut self, ctx: &egui::Context) {
        let Some(report) = self.imported.as_ref() else {
            self.popover = None;
            return;
        };
        let p = pal();
        let screen = ctx.screen_rect();
        let mut close = false;
        let mut go: Option<usize> = None;

        let veil = anim::ease(ctx, "ns-imported-veil", true, 0.24);
        egui::Area::new(egui::Id::new("ns-imported-scrim"))
            .order(egui::Order::Middle)
            .fixed_pos(screen.min)
            .interactable(true)
            .show(ctx, |ui| {
                let (rect, resp) = ui.allocate_exact_size(screen.size(), Sense::click());
                ui.painter().rect_filled(
                    rect,
                    egui::Rounding::same(if chrome::maximized(ctx) { 0.0 } else { theme::R_WINDOW }),
                    Color32::from_black_alpha((120.0 * veil) as u8),
                );
                if resp.clicked() && self.frame_no > self.popover_frame + 1 {
                    close = true;
                }
            });

        let t = anim::ease(ctx, "ns-imported-rise", true, 0.30);
        let rows = report.issues.len() as f32;
        let size = Vec2::new(520.0_f32.min(screen.width() - 40.0), (196.0 + rows * 52.0).min(screen.height() - 80.0));
        let rect = Rect::from_center_size(screen.center(), size * (0.97 + 0.03 * t));
        let total: usize = report.issues.iter().map(|i| i.count).sum();

        egui::Area::new(egui::Id::new("ns-imported"))
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
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 10.0;
                    icons::show(ui, Icon::Warning, 16.0, p.warn);
                    let things = if report.issues.len() == 1 { "1 thing".to_string() } else { format!("{} things", report.issues.len()) };
                    ui.label(egui::RichText::new(format!("Imported, with {things} to check")).font(theme::font_semi(theme::T_H)).color(p.text));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui::icon_button(ui, Icon::Close, "Close · Esc", 26.0) {
                            close = true;
                        }
                    });
                });
                ui.label(
                    egui::RichText::new(format!(
                        "{} came in. The original file is untouched; {} not carried over exactly.",
                        ui::elide(&report.title, 40),
                        if total == 1 { "this one place was" } else { "these places were" }
                    ))
                    .font(theme::font(theme::T_CAP))
                    .color(p.text_faint),
                );
                ui.add_space(12.0);
                let foot = 46.0;
                let (area, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), (ui.available_height() - foot).max(60.0)), Sense::hover());
                let mut body = ui.new_child(egui::UiBuilder::new().max_rect(area).layout(Layout::top_down(Align::Min)));
                egui::ScrollArea::vertical().id_salt("ns-imported-rows").auto_shrink([false; 2]).show(&mut body, |ui| {
                    for issue in &report.issues {
                        let (row, _) = ui.allocate_exact_size(Vec2::new(area.width(), 48.0), Sense::hover());
                        ui.painter().line_segment([row.left_bottom(), row.right_bottom()], Stroke::new(1.0_f32, p.line));
                        let count = format!("×{}", issue.count);
                        ui.painter().text(Pos2::new(row.left() + 2.0, row.center().y), egui::Align2::LEFT_CENTER, &count, theme::font_mono(theme::T_SM), p.warn);
                        let text_w = row.width() - 56.0 - if issue.at.is_some() { 96.0 } else { 0.0 };
                        let galley = ui.painter().layout_job(marked(&issue.what, text_w));
                        ui.painter().galley(Pos2::new(row.left() + 52.0, row.center().y - galley.rect.height() * 0.5), galley, p.text);
                        if let Some(at) = issue.at {
                            let btn = Rect::from_min_size(Pos2::new(row.right() - 90.0, row.center().y - 14.0), Vec2::new(90.0, 28.0));
                            let mut b = ui.new_child(egui::UiBuilder::new().max_rect(btn).layout(Layout::right_to_left(Align::Center)));
                            if ui::quiet_button(&mut b, "Show me", Some(Icon::ArrowRight)) {
                                go = Some(at);
                            }
                        }
                    }
                });
                ui.add_space(10.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui::button(ui, "Got it", None, true) {
                        close = true;
                    }
                });
            });

        if let Some(at) = go {
            if let Some(id) = self.doc.blocks.get(at.min(self.doc.blocks.len().saturating_sub(1))).map(|b| b.id) {
                self.popover = None;
                self.mode = Mode::Write;
                self.ed.jump_to(id);
                self.ed.focus_block = Some(id);
            }
        }
        if close {
            self.popover = None;
        }
    }

    pub fn debug_imported_text(&self) -> Vec<String> {
        self.imported.as_ref().map(|r| r.issues.iter().map(|i| marked(&i.what, 400.0).text).collect()).unwrap_or_default()
    }

    pub fn debug_imported(&self) -> Option<Vec<Issue>> {
        self.imported.as_ref().map(|r| r.issues.clone())
    }
}

fn marked(what: &str, width: f32) -> egui::text::LayoutJob {
    let p = pal();
    let plain = egui::TextFormat { font_id: theme::font(theme::T_SM), color: p.text, ..Default::default() };
    let code = egui::TextFormat { font_id: theme::font_mono(theme::T_SM), color: p.warn, ..Default::default() };
    let mut job = egui::text::LayoutJob::default();
    let mut rest = what;
    while let Some(a) = rest.find('(') {
        let Some(b) = rest[a..].find(')') else { break };
        job.append(&rest[..a + 1], 0.0, plain.clone());
        job.append(&rest[a + 1..a + b], 0.0, code.clone());
        job.append(")", 0.0, plain.clone());
        rest = &rest[a + b + 1..];
    }
    job.append(rest, 0.0, plain);
    job.wrap.max_width = width;
    job
}
