use eframe::egui::{self, Key, Modifiers, Pos2, Rect, Sense, Vec2};
use web_time::Instant;

use crate::anim;
use crate::export::{compose, paginate, Line};
use crate::icons::Icon;
use crate::model::{base_character, Document, Element, LINES_PER_PAGE, PAGE_COLS};
use crate::theme::{self, pal};
use crate::ui;

const ASPECT: f32 = 11.0 / 8.5;
const FOOT: f32 = 58.0;
const WHEEL_STEP: f32 = 90.0;

pub struct PagesState {
    pub current: usize,
    pub sheets: usize,
    pub show_block: Option<u64>,
    pub sheet_rect: Rect,
    turned: Option<(Instant, f32)>,
    wheel: f32,
}

impl Default for PagesState {
    fn default() -> Self {
        PagesState {
            current: 0,
            sheets: 1,
            show_block: None,
            sheet_rect: Rect::NOTHING,
            turned: None,
            wheel: 0.0,
        }
    }
}

impl PagesState {
    pub fn turn_to(&mut self, sheet: usize) {
        let sheet = sheet.min(self.sheets.saturating_sub(1));
        if sheet != self.current {
            let dir = if sheet > self.current { 1.0 } else { -1.0 };
            self.current = sheet;
            self.turned = Some((Instant::now(), dir));
        }
    }
}

pub fn show(
    ui: &mut egui::Ui,
    doc: &Document,
    st: &mut PagesState,
    scene_numbers: bool,
    voices: Option<&std::collections::HashMap<String, egui::Color32>>,
) -> Option<u64> {
    let p = pal();
    let pages = paginate(&compose(doc));
    st.sheets = 1 + pages.len();
    if st.current >= st.sheets {
        st.current = st.sheets - 1;
    }
    if let Some(block) = st.show_block.take() {
        if let Some(k) = pages
            .iter()
            .position(|pg| pg.iter().any(|l| l.block == Some(block)))
        {
            st.current = k + 1;
            st.turned = None;
        }
    }

    type Voice = Option<egui::Color32>;
    let mut speaking: Vec<Vec<(Voice, Voice)>> = Vec::with_capacity(pages.len());
    {
        let mut who: Option<String> = None;
        let mut left_who: Option<String> = None;
        let mut right_who: Option<String> = None;
        let colour = |w: &Option<String>| w.as_ref().and_then(|n| voices.and_then(|m| m.get(n))).copied();
        let track = |w: &mut Option<String>, line: &Line| {
            if line.element == Some(Element::Character) {
                let n = base_character(&line.text);
                if !n.is_empty() {
                    *w = Some(n);
                }
            }
        };
        for page in &pages {
            let mut row = Vec::with_capacity(page.len());
            for line in page {
                if line.dual {
                    who = None;
                    track(&mut left_who, line);
                    if let Some(r) = &line.right {
                        track(&mut right_who, r);
                    }
                    row.push((colour(&left_who), colour(&right_who)));
                    continue;
                }
                left_who = None;
                right_who = None;
                match line.element {
                    Some(Element::Character) => {
                        let n = base_character(&line.text);
                        if !n.is_empty() {
                            who = Some(n);
                        }
                    }
                    Some(Element::Dialogue) | Some(Element::Parenthetical) | None => {}
                    _ => who = None,
                }
                row.push((colour(&who), None));
            }
            speaking.push(row);
        }
    }

    // turning page here---
    let area = ui.max_rect();
    let typing = ui.ctx().memory(|m| m.focused().is_some());
    if !typing {
        let (fwd, back, first, last) = ui.input_mut(|i| {
            (
                i.consume_key(Modifiers::NONE, Key::ArrowRight)
                    || i.consume_key(Modifiers::NONE, Key::PageDown)
                    || i.consume_key(Modifiers::NONE, Key::ArrowDown)
                    || i.consume_key(Modifiers::NONE, Key::Space),
                i.consume_key(Modifiers::NONE, Key::ArrowLeft)
                    || i.consume_key(Modifiers::NONE, Key::PageUp)
                    || i.consume_key(Modifiers::NONE, Key::ArrowUp),
                i.consume_key(Modifiers::NONE, Key::Home),
                i.consume_key(Modifiers::NONE, Key::End),
            )
        });
        if fwd {
            st.turn_to(st.current + 1);
        }
        if back && st.current > 0 {
            st.turn_to(st.current - 1);
        }
        if first {
            st.turn_to(0);
        }
        if last {
            st.turn_to(st.sheets - 1);
        }
    }
    if ui.rect_contains_pointer(area) {
        let dy = ui.input(|i| i.raw_scroll_delta.y);
        if dy != 0.0 {
            st.wheel += dy;
            if st.wheel <= -WHEEL_STEP {
                st.wheel = 0.0;
                st.turn_to(st.current + 1);
            } else if st.wheel >= WHEEL_STEP {
                st.wheel = 0.0;
                if st.current > 0 {
                    st.turn_to(st.current - 1);
                }
            }
        }
    }

    // sheet whole view here---
    let room = Rect::from_min_max(
        area.min + Vec2::new(24.0, 24.0),
        Pos2::new(area.right() - 24.0, area.bottom() - FOOT),
    );
    let page_h = room.height().min(room.width() * ASPECT).max(120.0);
    let page_w = page_h / ASPECT;
    let rest = Rect::from_center_size(room.center(), Vec2::new(page_w, page_h));

    let (shift, fade) = match st.turned {
        Some((at, dir)) if anim::enabled() => {
            let t = (at.elapsed().as_secs_f32() / 0.30).clamp(0.0, 1.0);
            if t >= 1.0 {
                st.turned = None;
            } else {
                anim::keep_going(ui.ctx());
            }
            let e = anim::ease_out_expo(t);
            ((1.0 - e) * 26.0 * dir, 0.25 + 0.75 * e)
        }
        _ => (0.0, 1.0),
    };
    let sheet = rest.translate(Vec2::new(shift, 0.0));
    st.sheet_rect = rest;

    let inch = page_w / 8.5;
    let line_h = inch / 6.0;
    let font_px = {
        let probe = theme::font_page(10.0);
        let w10 = ui.fonts(|f| f.glyph_width(&probe, 'M')).max(0.1);
        (6.0 * inch / PAGE_COLS as f32) / w10 * 10.0
    };
    let char_w = 6.0 * inch / PAGE_COLS as f32;
    let font = theme::font_page(font_px);
    let bold = theme::font_page_bold(font_px);
    let paper = if p.dark { p.solid_hi } else { p.solid };
    let ink = if p.dark { theme::mix(p.text, p.text_dim, 0.1) } else { p.text };
    let a = |c: egui::Color32| theme::wash(c, (c.a() as f32 * fade) as u8);

    let mut jump = None;
    {
        let painter = ui.painter();
        painter.rect_filled(sheet, egui::Rounding::same(theme::R_SM), a(paper));
        painter.rect_stroke(sheet.shrink(0.5), egui::Rounding::same(theme::R_SM), egui::Stroke::new(1.0_f32, a(p.line)));
    }
    let text_left = sheet.left() + 1.5 * inch;
    let body_top = sheet.top() + inch;

    if st.current == 0 {
        title_page(ui.painter(), doc, sheet, inch, line_h, char_w, &font, &bold, a(ink));
    } else {
        let n = st.current;
        if n > 1 {
            ui.painter().text(
                Pos2::new(text_left + PAGE_COLS as f32 * char_w, sheet.top() + 0.5 * inch + line_h * 0.5),
                egui::Align2::RIGHT_CENTER,
                format!("{n}."),
                font.clone(),
                a(ink),
            );
        }
        let page: &Vec<Line> = &pages[n - 1];
        for (row, whole) in page.iter().enumerate().take(LINES_PER_PAGE + 5) {
            let y = body_top + row as f32 * line_h;
            let (left_voice, right_voice) = speaking[n - 1][row];
            let mut halves: Vec<(&Line, Option<egui::Color32>, usize)> = vec![(whole, left_voice, 0)];
            if let Some(r) = &whole.right {
                halves.push((r, right_voice, 1));
            }
            for (line, voice, half) in halves {
                if line.text.trim().is_empty() {
                    continue;
                }
                let x = text_left + line.indent as f32 * char_w;
                let is_heading = matches!(line.element, Some(Element::SceneHeading) | Some(Element::Act));
                let color = match (voice, line.element) {
                    (Some(v), _) => v,
                    (None, Some(Element::Parenthetical)) | (None, Some(Element::Transition)) => {
                        theme::mix(ink, p.text_dim, 0.4)
                    }
                    _ => ink,
                };
                if let Some(block) = line.block {
                    let hit = Rect::from_min_size(
                        Pos2::new(x, y),
                        Vec2::new(line.text.chars().count() as f32 * char_w, line_h),
                    );
                    let resp = ui.interact(hit, egui::Id::new(("ns-page-line", n, row, half)), Sense::click());
                    let hot = anim::ease(ui.ctx(), resp.id, resp.hovered(), anim::HOVER);
                    if hot > 0.01 {
                        ui.painter().rect_filled(
                            hit.expand2(Vec2::new(3.0, 0.0)),
                            egui::Rounding::ZERO,
                            theme::wash(p.primary, (40.0 * hot) as u8),
                        );
                    }
                    if resp.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if resp.on_hover_text("Click to write this line").clicked() {
                        jump = Some(block);
                    }
                }
                let drawn = ui.painter().text(
                    Pos2::new(x, y),
                    egui::Align2::LEFT_TOP,
                    &line.text,
                    if is_heading { bold.clone() } else { font.clone() },
                    a(color),
                );
                if line.element == Some(Element::Act) {
                    let under = y + line_h * 0.92;
                    ui.painter().line_segment(
                        [Pos2::new(drawn.left(), under), Pos2::new(drawn.right(), under)],
                        egui::Stroke::new((line_h * 0.06).max(0.8), a(color)),
                    );
                }
                if scene_numbers {
                    if let Some(num) = line.scene {
                        let label = format!("{num}");
                        ui.painter().text(
                            Pos2::new(text_left - 0.5 * inch, y),
                            egui::Align2::RIGHT_TOP,
                            &label,
                            bold.clone(),
                            a(ink),
                        );
                        ui.painter().text(
                            Pos2::new(text_left + PAGE_COLS as f32 * char_w + 0.2 * inch, y),
                            egui::Align2::LEFT_TOP,
                            &label,
                            bold.clone(),
                            a(ink),
                        );
                    }
                }
            }
        }
    }

    // hud strip foot here---
    let label = if st.current == 0 {
        format!("TITLE PAGE / {:02} PP", st.sheets - 1)
    } else {
        format!("PAGE {:02} / {:02}", st.current, st.sheets - 1)
    };
    let hud = theme::font_hud(theme::T_CAP);
    let text_w = theme::tracked_width(ui.ctx(), &label, hud.clone(), 0.9);
    let bar_w = text_w.max(118.0) + 2.0 * 30.0 + 28.0;
    let bar = Rect::from_center_size(
        Pos2::new(area.center().x, area.bottom() - FOOT * 0.5 - 2.0),
        Vec2::new(bar_w, 34.0),
    );
    theme::glass_surface(ui.painter(), bar, theme::R_SM, 1.0);
    let mut foot = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(bar.shrink2(Vec2::new(6.0, 2.0)))
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    foot.spacing_mut().item_spacing.x = 4.0;
    let can_back = st.current > 0;
    let can_fwd = st.current + 1 < st.sheets;
    if ui::icon_button(&mut foot, Icon::ChevronLeft, "Previous page  ·  ←", 28.0) && can_back {
        st.turn_to(st.current - 1);
    }
    let (mid, _) = foot.allocate_exact_size(
        Vec2::new(foot.available_width() - 32.0, 28.0),
        Sense::hover(),
    );
    theme::tracked_text(
        foot.painter(),
        Pos2::new(mid.center().x - text_w * 0.5, mid.center().y),
        &label,
        hud,
        p.text_dim,
        0.9,
    );
    if ui::icon_button(&mut foot, Icon::ChevronRight, "Next page  ·  →", 28.0) && can_fwd {
        st.turn_to(st.current + 1);
    }
    ui.allocate_rect(area, Sense::hover());
    jump
}

#[allow(clippy::too_many_arguments)]
fn title_page(
    painter: &egui::Painter,
    doc: &Document,
    sheet: Rect,
    inch: f32,
    line_h: f32,
    char_w: f32,
    font: &egui::FontId,
    bold: &egui::FontId,
    ink: egui::Color32,
) {
    let text_left = sheet.left() + 1.5 * inch;
    let body_top = sheet.top() + inch;
    let centre = text_left + PAGE_COLS as f32 * char_w * 0.5;
    let put = |text: &str, line: f32, f: &egui::FontId| {
        painter.text(
            Pos2::new(centre, body_top + line * line_h),
            egui::Align2::CENTER_TOP,
            text,
            f.clone(),
            ink,
        );
    };
    let title = if doc.meta.title.trim().is_empty() {
        "UNTITLED SCRIPT".to_string()
    } else {
        doc.meta.title.trim().to_uppercase()
    };
    put(&title, 19.0, bold);
    if !doc.meta.author.trim().is_empty() {
        put("written by", 23.0, font);
        put(doc.meta.author.trim(), 25.0, font);
    }
    if !doc.meta.draft.trim().is_empty() {
        put(doc.meta.draft.trim(), 43.0, font);
    }
    if !doc.meta.contact.trim().is_empty() {
        painter.text(
            Pos2::new(text_left, body_top + 47.0 * line_h),
            egui::Align2::LEFT_TOP,
            doc.meta.contact.trim(),
            font.clone(),
            ink,
        );
    }
}
