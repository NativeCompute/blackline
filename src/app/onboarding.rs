use eframe::egui::{self, Align2, Color32, Key, Modifiers, Pos2, Rect, Sense, Stroke, Vec2};
use web_time::Instant;

use super::App;
use crate::anim;
use crate::chrome;
use crate::keys::Command;
use crate::theme::{self, pal};
use crate::ui;

pub const SEEN: &str = "blue-pages";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Full,
    New,
}

pub struct Tour {
    pub kind: Kind,
    pub step: usize,
    born: Instant,
    dir: f32,
}

#[derive(Clone, Copy)]
enum Sketch {
    Welcome,
    Writing,
    Views,
    Cards,
    Notes,
    Revisions,
    Files,
    Dual,
    Panel,
    Reports,
}

struct Step {
    title: &'static str,
    body: String,
    sketch: Sketch,
}

pub fn wanted(onboarded: &str, fresh: bool) -> Option<Kind> {
    if onboarded == SEEN {
        None
    } else if fresh {
        Some(Kind::Full)
    } else {
        Some(Kind::New)
    }
}

impl App {
    pub(super) fn start_tour(&mut self, kind: Kind) {
        self.tour = Some(Tour { kind, step: 0, born: Instant::now(), dir: 1.0 });
        self.popover = None;
    }

    fn steps(&self, kind: Kind) -> Vec<Step> {
        let k = |c: Command| self.keys_for(c);
        match kind {
            Kind::Full => vec![
                Step {
                    title: "Welcome to Blackline",
                    body: "A screenplay editor that keeps to industry format, so you can keep your mind on the story. This takes a minute, and you can skip it any time.".into(),
                    sketch: Sketch::Welcome,
                },
                Step {
                    title: "Write, and the format follows",
                    body: "Press Enter for the element that usually comes next, and Tab to change it: scene heading, action, character, dialogue. Margins and page breaks are set the way the page will print.".into(),
                    sketch: Sketch::Writing,
                },
                Step {
                    title: "Three ways to see a script",
                    body: format!("Write is where you type. Cards shows each scene as a card you can drag. Read shows the printed pages. Switch with the tabs at the top, or {}.", k(Command::NextView)),
                    sketch: Sketch::Views,
                },
                Step {
                    title: "Cards and storylines",
                    body: "Give each storyline a colour and a name. The timeline above the cards shows the whole script to scale, with guides at the usual turning points.".into(),
                    sketch: Sketch::Cards,
                },
                Step {
                    title: "Notes in the margin",
                    body: format!("Put the cursor on a line and press {} to leave yourself a note. Every note is listed in the panel on the left, and none of them print unless you ask.", k(Command::AddNote)),
                    sketch: Sketch::Notes,
                },
                Step {
                    title: "Revisions and export",
                    body: "When the script goes out, start a revision from the Changes tab. Every change is marked with an asterisk, and each round takes the next colour. Export a PDF, Fountain or Final Draft file from the ribbon.".into(),
                    sketch: Sketch::Revisions,
                },
                Step {
                    title: "Your scripts stay yours",
                    body: "Each script is a plain text file in your library folder, with snapshots kept beside it. The Library button at the top left takes you back to all of them.".into(),
                    sketch: Sketch::Files,
                },
            ],
            Kind::New => vec![
                Step {
                    title: "Dual dialogue",
                    body: format!("Two characters talking at once: put the cursor in the second speech and press {}. The pair sits side by side, on screen and in print.", k(Command::DualDialogue)),
                    sketch: Sketch::Dual,
                },
                Step {
                    title: "Notes, and a new left panel",
                    body: format!("Leave a note on any line with {}. The left panel now lists your notes and changes. Your scripts are one click away, under Library at the top left.", k(Command::AddNote)),
                    sketch: Sketch::Panel,
                },
                Step {
                    title: "Revisions in colour",
                    body: "Start a revision from the Changes tab. Changed lines get an asterisk, locked pages keep their numbers with new material on A-pages, and you can print just the changed pages.".into(),
                    sketch: Sketch::Revisions,
                },
                Step {
                    title: "Reports, and find everywhere",
                    body: format!("{} opens reports on characters, scenes, locations and time of day. {} searches inside every script from Home.", k(Command::Reports), k(Command::FindScripts)),
                    sketch: Sketch::Reports,
                },
            ],
        }
    }

    fn end_tour(&mut self) {
        self.tour = None;
        if self.settings.onboarded != SEEN {
            self.settings.onboarded = SEEN.to_string();
            self.save_settings();
        }
    }

    fn turn(&mut self, by: isize, count: usize) {
        let Some(t) = self.tour.as_mut() else {
            return;
        };
        let next = t.step as isize + by;
        if next < 0 {
            return;
        }
        if next as usize >= count {
            self.end_tour();
            return;
        }
        t.step = next as usize;
        t.dir = by.signum() as f32;
        t.born = Instant::now();
    }

    pub(super) fn tour_keys(&mut self, ctx: &egui::Context) {
        let Some(kind) = self.tour.as_ref().map(|t| t.kind) else {
            return;
        };
        let count = self.steps(kind).len();
        ctx.memory_mut(|m| {
            if let Some(id) = m.focused() {
                m.surrender_focus(id);
            }
        });
        let (skip, back, next) = ctx.input_mut(|i| {
            (
                i.consume_key(Modifiers::NONE, Key::Escape),
                i.consume_key(Modifiers::NONE, Key::ArrowLeft),
                i.consume_key(Modifiers::NONE, Key::ArrowRight) || i.consume_key(Modifiers::NONE, Key::Enter),
            )
        });
        if skip {
            self.end_tour();
        } else if back {
            self.turn(-1, count);
        } else if next {
            self.turn(1, count);
        }
    }

    pub(super) fn tour_overlay(&mut self, ctx: &egui::Context) {
        let Some((kind, step, born, dir)) = self.tour.as_ref().map(|t| (t.kind, t.step, t.born, t.dir)) else {
            return;
        };
        let steps = self.steps(kind);
        let Some(here) = steps.get(step) else {
            self.end_tour();
            return;
        };
        let p = pal();
        let screen = ctx.screen_rect();
        let t = if anim::enabled() { (born.elapsed().as_secs_f32() / 0.30).clamp(0.0, 1.0) } else { 1.0 };
        if t < 1.0 {
            ctx.request_repaint();
        }
        let e = anim::smootherstep(t);
        let mut action: Option<isize> = None;
        let mut skip = false;

        egui::Area::new(egui::Id::new("ns-tour"))
            .order(egui::Order::Foreground)
            .fixed_pos(screen.min)
            .interactable(true)
            .show(ctx, |ui| {
                let (whole, _) = ui.allocate_exact_size(screen.size(), Sense::click());
                ui.painter().rect_filled(
                    whole,
                    egui::Rounding::same(chrome::window_radius(ctx, theme::R_WINDOW)),
                    theme::wash(p.backdrop, 238),
                );
                chrome::column_guides(ui.painter(), whole);

                let size = Vec2::new((screen.width() - 64.0).min(820.0), (screen.height() - 64.0).min(440.0));
                let card = Rect::from_center_size(screen.center(), size);
                ui.painter().rect_filled(card, egui::Rounding::same(theme::R_ISLAND), p.solid);
                ui.painter().rect_stroke(card.shrink(0.5), egui::Rounding::same(theme::R_ISLAND), Stroke::new(1.0_f32, p.line_strong));

                let art_w = (card.width() * 0.42).min(330.0);
                let art = Rect::from_min_size(card.min, Vec2::new(art_w, card.height())).shrink(28.0);
                let shift = (1.0 - e) * 24.0 * dir;
                let fade = |c: Color32| theme::wash(c, (c.a() as f32 * (0.15 + 0.85 * e)) as u8);
                sketch(ui.painter(), art.translate(Vec2::new(shift, 0.0)), here.sketch, &fade);

                let text = Rect::from_min_max(Pos2::new(card.left() + art_w + 8.0, card.top() + 32.0), Pos2::new(card.right() - 36.0, card.bottom() - 28.0));
                let mut col = ui.new_child(egui::UiBuilder::new().max_rect(text.translate(Vec2::new(shift * 0.6, 0.0))).layout(egui::Layout::top_down(egui::Align::Min)));
                col.set_opacity(0.15 + 0.85 * e);
                let label = match kind {
                    Kind::Full => format!("Tour / {:02} of {:02}", step + 1, steps.len()),
                    Kind::New => format!("What's new in Blue Pages / {:02} of {:02}", step + 1, steps.len()),
                };
                ui::hud(&mut col, &label.to_uppercase(), Some(p.primary_light));
                col.add_space(14.0);
                let size_t = 28.0;
                let galley = col.painter().layout(here.title.to_uppercase(), theme::font_display(size_t), p.text, text.width());
                let (r, _) = col.allocate_exact_size(Vec2::new(text.width(), galley.rect.height()), Sense::hover());
                col.painter().galley(r.min, galley, p.text);
                col.add_space(14.0);
                let body = col.painter().layout(here.body.clone(), theme::font(theme::T_BODY), p.text_dim, text.width());
                let (r, _) = col.allocate_exact_size(Vec2::new(text.width(), body.rect.height()), Sense::hover());
                col.painter().galley(r.min, body, p.text_dim);

                let foot = Rect::from_min_max(Pos2::new(text.left(), card.bottom() - 62.0), Pos2::new(card.right() - 28.0, card.bottom() - 22.0));
                for k in 0..steps.len() {
                    let c = Pos2::new(foot.left() + 4.0 + k as f32 * 14.0, foot.center().y);
                    let on = k == step;
                    ui.painter().rect_filled(
                        Rect::from_center_size(c, Vec2::new(if on { 10.0 } else { 6.0 }, 6.0)),
                        egui::Rounding::same(1.0),
                        if on { p.primary_light } else { p.line_strong },
                    );
                }
                let mut buttons = ui.new_child(egui::UiBuilder::new().max_rect(foot).layout(egui::Layout::right_to_left(egui::Align::Center)));
                let last = step + 1 == steps.len();
                if ui::button(&mut buttons, if last { "Start writing" } else { "Next" }, None, true) {
                    action = Some(1);
                }
                if step > 0 && ui::button(&mut buttons, "Back", None, false) {
                    action = Some(-1);
                }
                if !last && ui::quiet_button(&mut buttons, "Skip", None) {
                    skip = true;
                }
            });

        if skip {
            self.end_tour();
        } else if let Some(by) = action {
            self.turn(by, steps.len());
        }
    }

    pub fn debug_tour(&self) -> Option<(Kind, usize)> {
        self.tour.as_ref().map(|t| (t.kind, t.step))
    }
    pub fn debug_skip_tour(&mut self) {
        self.end_tour();
    }
    pub fn debug_start_tour(&mut self) {
        self.start_tour(Kind::Full);
    }
}

fn page(painter: &egui::Painter, r: Rect, fade: &dyn Fn(Color32) -> Color32) -> Rect {
    let p = pal();
    painter.rect_filled(r, egui::Rounding::same(theme::R_SM), fade(if p.dark { p.solid_hi } else { Color32::WHITE }));
    painter.rect_stroke(r.shrink(0.5), egui::Rounding::same(theme::R_SM), Stroke::new(1.0_f32, fade(p.line_strong)));
    r.shrink2(Vec2::new(r.width() * 0.14, r.height() * 0.08))
}

fn bar(painter: &egui::Painter, x: f32, y: f32, w: f32, c: Color32) {
    painter.rect_filled(Rect::from_min_size(Pos2::new(x, y), Vec2::new(w.max(2.0), 3.0)), egui::Rounding::ZERO, c);
}

fn tag(painter: &egui::Painter, at: Pos2, text: &str, c: Color32) {
    theme::tracked_text(painter, at, text, theme::font_hud(theme::T_MICRO), c, 0.8);
}

fn sketch(painter: &egui::Painter, area: Rect, kind: Sketch, fade: &dyn Fn(Color32) -> Color32) {
    let p = pal();
    let ink = fade(theme::mix(p.text_dim, p.text_faint, 0.3));
    let strong = fade(p.text);
    let blue = fade(p.primary_light);
    let sheet = Rect::from_center_size(area.center(), Vec2::new(area.width().min(area.height() * 0.78), area.height()).min(area.size()));
    match kind {
        Sketch::Welcome => {
            let inner = page(painter, sheet, fade);
            let c = inner.center();
            painter.text(c - Vec2::new(0.0, 24.0), Align2::CENTER_CENTER, "FADE IN:", theme::font_page_bold(15.0), strong);
            bar(painter, inner.left(), c.y + 6.0, inner.width() * 0.9, ink);
            bar(painter, inner.left(), c.y + 14.0, inner.width() * 0.6, ink);
            painter.rect_filled(Rect::from_min_size(Pos2::new(inner.left() + inner.width() * 0.6 + 4.0, c.y + 12.0), Vec2::new(2.0, 9.0)), egui::Rounding::ZERO, blue);
        }
        Sketch::Writing => {
            let room = 84.0;
            let sheet = Rect::from_min_max(Pos2::new(area.left() + room, sheet.top()), Pos2::new(area.right(), sheet.bottom()));
            let inner = page(painter, sheet, fade);
            let rows: [(&str, f32, f32, bool); 5] = [
                ("SCENE", 0.0, 0.7, true),
                ("ACTION", 0.0, 0.95, false),
                ("CHARACTER", 0.38, 0.22, true),
                ("DIALOGUE", 0.18, 0.6, false),
                ("ACTION", 0.0, 0.8, false),
            ];
            for (k, (name, x, w, bold)) in rows.iter().enumerate() {
                let y = inner.top() + 14.0 + k as f32 * (inner.height() - 20.0) / 5.0;
                bar(painter, inner.left() + inner.width() * x, y, inner.width() * w, if *bold { strong } else { ink });
                tag(painter, Pos2::new(sheet.left() - 10.0 - theme::tracked_width(painter.ctx(), name, theme::font_hud(theme::T_MICRO), 0.8), y + 1.0), name, if k == 3 { blue } else { fade(p.text_faint) });
            }
            let key = Rect::from_center_size(Pos2::new(sheet.right() - 26.0, sheet.bottom() - 22.0), Vec2::new(38.0, 22.0));
            painter.rect_stroke(key, egui::Rounding::same(4.0), Stroke::new(1.0_f32, blue));
            painter.text(key.center(), Align2::CENTER_CENTER, "TAB", theme::font_mono(theme::T_MICRO), blue);
        }
        Sketch::Views => {
            let w = (area.width() - 16.0) / 3.0;
            for k in 0..3 {
                let r = Rect::from_min_size(Pos2::new(area.left() + k as f32 * (w + 8.0), area.center().y - w * 0.65), Vec2::new(w, w * 1.3));
                let inner = page(painter, r, fade);
                match k {
                    0 => (0..6).for_each(|i| bar(painter, inner.left(), inner.top() + i as f32 * 9.0, inner.width() * if i % 2 == 0 { 0.9 } else { 0.5 }, ink)),
                    1 => (0..4).for_each(|i| {
                        let c = Rect::from_min_size(Pos2::new(inner.left() + (i % 2) as f32 * inner.width() * 0.52, inner.top() + (i / 2) as f32 * inner.height() * 0.5), Vec2::new(inner.width() * 0.46, inner.height() * 0.44));
                        painter.rect_stroke(c, egui::Rounding::same(1.0), Stroke::new(1.0_f32, ink));
                        painter.rect_filled(Rect::from_min_size(c.min, Vec2::new(2.0, c.height())), egui::Rounding::ZERO, fade(p.group(i)));
                    }),
                    _ => (0..7).for_each(|i| bar(painter, inner.left() + if i % 3 == 1 { inner.width() * 0.3 } else { 0.0 }, inner.top() + i as f32 * 8.0, inner.width() * 0.6, ink)),
                }
                let name = ["WRITE", "CARDS", "READ"][k];
                tag(painter, Pos2::new(r.left(), r.bottom() + 14.0), name, if k == 1 { blue } else { fade(p.text_faint) });
            }
        }
        Sketch::Cards => {
            let strip = Rect::from_min_size(Pos2::new(area.left(), area.center().y - 104.0), Vec2::new(area.width(), 12.0));
            let lens = [3.0, 2.0, 4.0, 1.0, 3.0, 2.0, 3.0, 2.0];
            let total: f32 = lens.iter().sum();
            let mut x = strip.left();
            for (k, l) in lens.iter().enumerate() {
                let w = strip.width() * l / total;
                painter.rect_filled(Rect::from_min_size(Pos2::new(x, strip.top()), Vec2::new(w - 1.5, strip.height())), egui::Rounding::ZERO, fade(p.group(k % 3)));
                x += w;
            }
            let mid = strip.left() + strip.width() * 0.5;
            painter.line_segment([Pos2::new(mid, strip.top() - 14.0), Pos2::new(mid, strip.bottom() + 4.0)], Stroke::new(1.0_f32, strong));
            tag(painter, Pos2::new(mid + 4.0, strip.top() - 10.0), "MIDPOINT", strong);
            let cw = (area.width() - 12.0) / 2.0;
            for k in 0..4 {
                let r = Rect::from_min_size(Pos2::new(area.left() + (k % 2) as f32 * (cw + 12.0), strip.bottom() + 22.0 + (k / 2) as f32 * 86.0), Vec2::new(cw, 74.0));
                painter.rect_filled(r, egui::Rounding::same(theme::R_SM), fade(p.solid_hi));
                painter.rect_stroke(r.shrink(0.5), egui::Rounding::same(theme::R_SM), Stroke::new(1.0_f32, fade(p.line)));
                painter.rect_filled(Rect::from_min_size(r.min, Vec2::new(3.0, r.height())), egui::Rounding::ZERO, fade(p.group(k % 3)));
                bar(painter, r.left() + 12.0, r.top() + 14.0, r.width() * 0.6, strong);
                bar(painter, r.left() + 12.0, r.top() + 30.0, r.width() * 0.75, ink);
                bar(painter, r.left() + 12.0, r.top() + 40.0, r.width() * 0.5, ink);
            }
        }
        Sketch::Notes | Sketch::Panel => {
            let sheet = Rect::from_min_size(Pos2::new(area.left(), sheet.top()), Vec2::new(area.width() * 0.66, sheet.height()));
            let inner = page(painter, sheet, fade);
            for i in 0..8 {
                let y = inner.top() + 6.0 + i as f32 * (inner.height() - 10.0) / 8.0;
                bar(painter, inner.left(), y, inner.width() * if i % 3 == 2 { 0.45 } else { 0.85 }, if i == 3 { strong } else { ink });
                if i == 3 {
                    painter.rect_filled(Rect::from_center_size(Pos2::new(sheet.right() - 10.0, y + 1.5), Vec2::splat(6.0)), egui::Rounding::same(1.0), blue);
                }
            }
            let note = Rect::from_min_size(Pos2::new(sheet.right() - 24.0, sheet.top() + sheet.height() * 0.44), Vec2::new(area.right() - sheet.right() + 24.0, 52.0));
            painter.rect_filled(note, egui::Rounding::same(theme::R_SM), fade(p.solid));
            painter.rect_stroke(note.shrink(0.5), egui::Rounding::same(theme::R_SM), Stroke::new(1.0_f32, blue));
            bar(painter, note.left() + 8.0, note.top() + 12.0, note.width() - 20.0, strong);
            bar(painter, note.left() + 8.0, note.top() + 24.0, note.width() * 0.5, ink);
            if matches!(kind, Sketch::Panel) {
                tag(painter, Pos2::new(inner.left(), sheet.top() + 12.0), "NOTES / CHANGES", blue);
            }
        }
        Sketch::Revisions => {
            let tint = crate::revision::paper(0);
            painter.rect_filled(sheet, egui::Rounding::same(theme::R_SM), fade(Color32::from_rgb(tint[0], tint[1], tint[2])));
            let inner = sheet.shrink2(Vec2::new(sheet.width() * 0.14, sheet.height() * 0.08));
            let paper_ink = fade(Color32::from_gray(60));
            painter.text(Pos2::new(inner.right(), inner.top() - 2.0), Align2::RIGHT_TOP, "Blue Rev.  12A.", theme::font_page(10.0), paper_ink);
            for i in 0..9 {
                let y = inner.top() + 22.0 + i as f32 * (inner.height() - 30.0) / 9.0;
                bar(painter, inner.left(), y, inner.width() * if i % 2 == 0 { 0.85 } else { 0.55 }, paper_ink);
                if i == 2 || i == 3 || i == 6 {
                    painter.text(Pos2::new(sheet.right() - 12.0, y + 1.5), Align2::CENTER_CENTER, "*", theme::font_page_bold(13.0), paper_ink);
                }
            }
        }
        Sketch::Files => {
            let names = ["the-long-night.md", "crossfire.md", "snapshots/"];
            for (k, n) in names.iter().enumerate() {
                let r = Rect::from_min_size(Pos2::new(area.left(), area.center().y - 72.0 + k as f32 * 52.0), Vec2::new(area.width(), 40.0));
                painter.rect_stroke(r.shrink(0.5), egui::Rounding::same(theme::R_SM), Stroke::new(1.0_f32, fade(p.line_strong)));
                crate::icons::draw(painter, Rect::from_center_size(Pos2::new(r.left() + 20.0, r.center().y), Vec2::splat(14.0)), if k == 2 { crate::icons::Icon::Folder } else { crate::icons::Icon::File }, if k == 0 { blue } else { ink });
                painter.text(Pos2::new(r.left() + 40.0, r.center().y), Align2::LEFT_CENTER, *n, theme::font_mono(theme::T_SM), if k == 0 { strong } else { ink });
            }
        }
        Sketch::Dual => {
            let inner = page(painter, sheet, fade);
            let half = inner.width() * 0.46;
            for (k, x) in [inner.left(), inner.left() + inner.width() * 0.54].into_iter().enumerate() {
                tag(painter, Pos2::new(x + half * 0.3, inner.top() + 30.0), ["MARIA", "JONAS"][k], strong);
                for i in 0..4 {
                    bar(painter, x, inner.top() + 46.0 + i as f32 * 9.0, half * if i == 3 { 0.5 } else { 0.95 }, ink);
                }
            }
            painter.line_segment([Pos2::new(sheet.left() - 8.0, inner.top() + 24.0), Pos2::new(sheet.left() - 8.0, inner.top() + 82.0)], Stroke::new(1.5_f32, blue));
            bar(painter, inner.left(), inner.top() + 4.0, inner.width() * 0.8, ink);
            bar(painter, inner.left(), inner.top() + 104.0, inner.width() * 0.9, ink);
        }
        Sketch::Reports => {
            let field = Rect::from_min_size(Pos2::new(area.left(), area.center().y - 84.0), Vec2::new(area.width(), 30.0));
            painter.rect_stroke(field.shrink(0.5), egui::Rounding::same(theme::R_SM), Stroke::new(1.0_f32, blue));
            crate::icons::draw(painter, Rect::from_center_size(Pos2::new(field.left() + 16.0, field.center().y), Vec2::splat(12.0)), crate::icons::Icon::Search, blue);
            painter.text(Pos2::new(field.left() + 30.0, field.center().y), Align2::LEFT_CENTER, "kettle", theme::font(theme::T_SM), strong);
            let rows = [("MARIA", 0.39), ("COLE", 0.36), ("JONAS", 0.25)];
            for (k, (name, share)) in rows.iter().enumerate() {
                let y = field.bottom() + 36.0 + k as f32 * 34.0;
                tag(painter, Pos2::new(area.left(), y), name, strong);
                let track = Rect::from_min_size(Pos2::new(area.left() + area.width() * 0.4, y - 3.0), Vec2::new(area.width() * 0.6, 6.0));
                painter.rect_filled(track, egui::Rounding::same(3.0), fade(p.line));
                painter.rect_filled(Rect::from_min_size(track.min, Vec2::new(track.width() * share * 2.0, 6.0)), egui::Rounding::same(3.0), blue);
            }
        }
    }
}
