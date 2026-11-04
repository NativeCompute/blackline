use eframe::egui::{self, Pos2, Rect, Sense, Vec2};

use crate::anim;
use crate::icons::Icon;
use crate::model::{act_title, eighths_label, ActSpan, Document, Scene};
use crate::theme::{self, pal};
use crate::ui;

pub const CARD_W: f32 = 236.0;
pub const CARD_H: f32 = 156.0;
const GAP: f32 = 18.0;
const HEAD_H: f32 = 36.0;
const RIBBON_H: f32 = 40.0;
const RIBBON_GAP: f32 = 12.0;

#[derive(Default)]
pub struct CardsState {
    pub dragging: Option<usize>,
    pub drop_at: Option<usize>,
    pub menu: Option<(usize, Pos2, u64)>,
    pub last_rects: Vec<Rect>,
    pub new_card_rect: Option<Rect>,
    pub only: Vec<usize>,
    pub naming: Option<[String; STORYLINES]>,
    pub scroll_to: Option<usize>,
    pub segments: Vec<(usize, Rect)>,
    pub chips: Vec<(usize, Rect)>,
}

pub const STORYLINES: usize = 6;

pub const BEATS: [(&str, f32); 5] = [
    ("Inciting incident", 0.10),
    ("Break into two", 0.25),
    ("Midpoint", 0.50),
    ("Break into three", 0.75),
    ("Climax", 0.90),
];

pub fn beat_pages(total_eighths: usize) -> Vec<(&'static str, f32, usize)> {
    BEATS
        .iter()
        .map(|(name, f)| (*name, *f, ((f * total_eighths as f32) / 8.0).floor() as usize + 1))
        .collect()
}

pub fn spans(scenes: &[Scene], lengths: &std::collections::HashMap<u64, (usize, usize)>) -> (Vec<(usize, usize)>, usize) {
    let mut at = 0usize;
    let mut out = Vec::with_capacity(scenes.len());
    for sc in scenes {
        let len = lengths.get(&sc.id).map(|l| l.1).unwrap_or(0).max(1);
        out.push((at, len));
        at += len;
    }
    (out, at)
}

pub fn beat_scene(spans: &[(usize, usize)], total: usize, f: f32) -> Option<usize> {
    let mark = f * total as f32;
    spans.iter().position(|(a, l)| (*a as f32) <= mark && mark < (*a + *l) as f32)
}

pub enum Act {
    Open(u64),
    Move(usize, usize),
    Tint(u64, Option<usize>),
    Delete(usize),
    NewScene,
    OpenAct(u64),
}

pub struct Out {
    pub changed: bool,
    pub act: Option<Act>,
}

#[allow(clippy::too_many_arguments)]
pub fn show(
    ui: &mut egui::Ui,
    doc: &mut Document,
    st: &mut CardsState,
    lengths: &std::collections::HashMap<u64, (usize, usize)>,
    act_notes: &std::collections::HashMap<u64, String>,
    live: Option<u64>,
    frame_no: u64,
    guides: bool,
) -> Out {
    let p = pal();
    let scenes = doc.scenes();
    let acts = doc.acts();
    let mut out = Out {
        changed: false,
        act: None,
    };

    let avail = ui.available_width();
    let cols = (((avail - 2.0 * GAP) + GAP) / (CARD_W + GAP)).floor().max(1.0) as usize;
    let grid_w = cols as f32 * CARD_W + (cols as f32 - 1.0) * GAP;
    let left = ui.min_rect().left() + ((avail - grid_w) * 0.5).max(GAP);
    let (spans, total) = spans(&scenes, lengths);
    let bar_top = ui.cursor().top() + 18.0;
    let bar_h = if scenes.is_empty() {
        0.0
    } else {
        beat_bar(ui, doc, st, &scenes, &acts, &spans, total, live, guides, Rect::from_min_size(Pos2::new(left, bar_top), Vec2::new(grid_w, 0.0)), &mut out)
    };
    let top = bar_top + bar_h + 6.0;

    let mut slots: Vec<Rect> = Vec::with_capacity(scenes.len() + 1);
    let mut ribbons: Vec<(Rect, &ActSpan)> = Vec::new();
    let (mut y, mut c) = (top, 0usize);
    let mut next_act = 0usize;
    let mut ribbon = |y: &mut f32, c: &mut usize, a| {
        if *c > 0 {
            *y += CARD_H + GAP;
            *c = 0;
        }
        ribbons.push((Rect::from_min_size(Pos2::new(left, *y), Vec2::new(grid_w, RIBBON_H)), a));
        *y += RIBBON_H + RIBBON_GAP;
    };
    for sc in &scenes {
        while let Some(a) = acts.get(next_act).filter(|a| a.start < sc.start) {
            ribbon(&mut y, &mut c, a);
            next_act += 1;
        }
        slots.push(Rect::from_min_size(
            Pos2::new(left + c as f32 * (CARD_W + GAP), y),
            Vec2::new(CARD_W, CARD_H),
        ));
        c += 1;
        if c == cols {
            c = 0;
            y += CARD_H + GAP;
        }
    }
    for a in &acts[next_act.min(acts.len())..] {
        ribbon(&mut y, &mut c, a);
    }
    slots.push(Rect::from_min_size(
        Pos2::new(left + c as f32 * (CARD_W + GAP), y),
        Vec2::new(CARD_W, CARD_H),
    ));
    let slot = |k: usize| slots[k];
    let bottom = slots.last().map(|r| r.bottom()).unwrap_or(top) + GAP;
    let (whole, _) = ui.allocate_exact_size(
        Vec2::new(avail, (top - bar_top) + 24.0 + (bottom - top) + 60.0),
        Sense::hover(),
    );
    let _ = whole;
    if let Some(k) = st.scroll_to.take() {
        if k < scenes.len() {
            ui.scroll_to_rect(slot(k).expand(GAP), Some(egui::Align::Center));
        }
    }
    let beat_tags: Vec<(usize, &str)> = if guides {
        BEATS.iter().filter_map(|(name, f)| beat_scene(&spans, total, *f).map(|k| (k, *name))).collect()
    } else {
        Vec::new()
    };

    for (rect, a) in &ribbons {
        if act_ribbon(ui, *rect, a, act_notes.get(&a.id).map(String::as_str).unwrap_or("")) {
            out.act = Some(Act::OpenAct(a.id));
        }
    }

    if scenes.is_empty() {
        ui.painter().text(
            Pos2::new(ui.min_rect().center().x, top - 6.0),
            egui::Align2::CENTER_BOTTOM,
            "No scenes yet — every scene heading becomes a card.",
            theme::font(theme::T_BODY),
            p.text_faint,
        );
    }

    st.last_rects.clear();
    let pointer = ui.ctx().pointer_latest_pos();
    let mut drag_started: Option<usize> = None;

    for (k, sc) in scenes.iter().enumerate() {
        let rect = slot(k);
        st.last_rects.push(rect);
        let being_dragged = st.dragging == Some(k);
        let head = Rect::from_min_size(rect.min, Vec2::new(rect.width(), HEAD_H));
        let body_resp = ui.interact(rect, egui::Id::new(("ns-card", sc.id)), Sense::click());
        let head_resp = ui.interact(head, egui::Id::new(("ns-card-head", sc.id)), Sense::click_and_drag());

        let hovered = body_resp.hovered() || head_resp.hovered();
        let selected = live == Some(sc.id);
        let lift = anim::ease(ui.ctx(), ("ns-card-lift", sc.id), hovered || selected, 0.20);
        let r = rect.expand(1.2 * lift);
        let tint = sc.tint.map(|t| p.group(t));
        let accent = tint.unwrap_or(p.primary);
        let outside = !st.only.is_empty() && !sc.tint.map(|t| st.only.contains(&t)).unwrap_or(false);
        let fade: f32 = if being_dragged { 0.35 } else if outside { 0.22 } else { 1.0 };

        draw_card(ui, r, sc, accent, tint, lift, selected, fade, lengths);
        for (_, name) in beat_tags.iter().filter(|(b, _)| *b == k) {
            let font = theme::font_hud(theme::T_MICRO);
            let up = name.to_uppercase();
            let w = theme::tracked_width(ui.ctx(), &up, font.clone(), 0.8);
            let at = Pos2::new(r.right() - w - 4.0, r.top() - 8.0);
            ui.painter().rect_filled(Rect::from_center_size(Pos2::new(at.x - 8.0, at.y), Vec2::splat(5.0)), egui::Rounding::ZERO, theme::wash(p.primary_light, (255.0 * fade) as u8));
            theme::tracked_text(ui.painter(), at, &up, font, theme::wash(p.primary_light, (255.0 * fade) as u8), 0.8);
        }

        let syn_rect = Rect::from_min_max(
            Pos2::new(r.left() + 14.0, r.top() + HEAD_H + 2.0),
            Pos2::new(r.right() - 14.0, r.bottom() - 30.0),
        );
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(syn_rect)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        child.set_clip_rect(syn_rect.intersect(ui.clip_rect()));
        if let Some(i) = doc.index_of(sc.id) {
            let resp = child.add(
                egui::TextEdit::multiline(&mut doc.blocks[i].note)
                    .id(egui::Id::new(("ns-card-synopsis", sc.id)))
                    .frame(false)
                    .font(theme::font(theme::T_CAP + 1.0))
                    .text_color(theme::wash(p.text_dim, (255.0 * fade) as u8))
                    .desired_width(syn_rect.width())
                    .desired_rows(4)
                    .margin(egui::Margin::ZERO)
                    .hint_text(
                        egui::RichText::new("What happens in this scene?")
                            .color(theme::wash(p.text_faint, 150)),
                    ),
            );
            if resp.changed() {
                let b = &mut doc.blocks[i];
                if b.note.contains('\n') {
                    b.note = b.note.replace('\n', " ");
                }
                out.changed = true;
            }
        }

        if head_resp.drag_started() {
            drag_started = Some(k);
        }
        if head_resp.clicked() {
            out.act = Some(Act::Open(sc.id));
        }
        let heading_tip = if sc.heading.trim().is_empty() {
            "Untitled scene".to_string()
        } else {
            sc.heading.clone()
        };
        let _ = head_resp.on_hover_text(format!("{heading_tip}\nClick to open · drag to move"));
        if body_resp.secondary_clicked() {
            if let Some(q) = pointer {
                st.menu = Some((k, q, frame_no));
            }
        }
    }

    // card makes new here---
    let nk = scenes.len();
    let nr = slot(nk);
    st.new_card_rect = Some(nr);
    let resp = ui.interact(nr, egui::Id::new("ns-card-new"), Sense::click());
    let hot = anim::ease(ui.ctx(), resp.id, resp.hovered(), anim::HOVER);
    ui::wipe(ui.painter(), nr, hot, if p.dark { p.raised } else { p.sunken });
    ui.painter().rect_stroke(
        nr.shrink(0.5),
        egui::Rounding::same(theme::R_CARD),
        egui::Stroke::new(1.0_f32, theme::mix(p.line, p.text_faint, hot)),
    );
    let c = nr.center();
    crate::icons::draw(
        ui.painter(),
        Rect::from_center_size(c - Vec2::new(0.0, 12.0), Vec2::splat(16.0)),
        Icon::Plus,
        theme::mix(p.text_dim, p.text, hot),
    );
    let label = "NEW SCENE";
    let font = theme::font_hud(theme::T_CAP);
    let lw = theme::tracked_width(ui.ctx(), label, font.clone(), 0.9);
    theme::tracked_text(
        ui.painter(),
        Pos2::new(c.x - lw * 0.5, c.y + 14.0),
        label,
        font,
        theme::mix(p.text_dim, p.text, hot),
        0.9,
    );
    if resp.on_hover_text("A new scene at the end · Ctrl+Enter in the script").clicked() {
        out.act = Some(Act::NewScene);
    }

    // dragging card new here---
    if let Some(k) = drag_started {
        st.dragging = Some(k);
    }
    let still_down = ui.input(|i| i.pointer.primary_down());
    if let Some(from) = st.dragging {
        if let Some(q) = pointer {
            let mut best = (f32::MAX, from);
            for k in 0..=scenes.len() {
                let edge = if k < scenes.len() {
                    let r = slot(k);
                    Pos2::new(r.left() - GAP * 0.5, r.center().y)
                } else {
                    let r = slot(scenes.len().saturating_sub(1));
                    Pos2::new(r.right() + GAP * 0.5, r.center().y)
                };
                let d = edge.distance(q);
                if d < best.0 {
                    best = (d, k);
                }
            }
            st.drop_at = Some(best.1);

            let at = best.1;
            let marker_x = if at < scenes.len() {
                slot(at).left() - GAP * 0.5
            } else {
                slot(scenes.len().saturating_sub(1)).right() + GAP * 0.5
            };
            let marker_row = if at < scenes.len() { slot(at) } else { slot(scenes.len().saturating_sub(1)) };
            let bar = Rect::from_center_size(
                Pos2::new(marker_x, marker_row.center().y),
                Vec2::new(3.0, CARD_H - 24.0),
            );
            let moving = at != from && at != from + 1;
            if moving {
                ui.painter().rect_filled(bar, egui::Rounding::ZERO, p.primary_light);
            }

            if let Some(sc) = scenes.get(from) {
                let ghost = Rect::from_min_size(q - Vec2::new(40.0, 18.0), Vec2::new(CARD_W, HEAD_H + 8.0));
                egui::Area::new(egui::Id::new("ns-card-ghost"))
                    .order(egui::Order::Tooltip)
                    .fixed_pos(ghost.min)
                    .interactable(false)
                    .show(ui.ctx(), |ui| {
                        let (r, _) = ui.allocate_exact_size(ghost.size(), Sense::hover());
                        theme::glass_surface(ui.painter(), r, theme::R_CARD, 1.0);
                        ui.painter().rect_stroke(r.shrink(0.5), egui::Rounding::same(theme::R_CARD), egui::Stroke::new(1.0_f32, p.primary_light));
                        ui.painter().rect_filled(
                            Rect::from_center_size(Pos2::new(r.left() + 20.0, r.center().y), Vec2::splat(7.0)),
                            egui::Rounding::ZERO,
                            p.primary_light,
                        );
                        ui.painter().text(
                            Pos2::new(r.left() + 34.0, r.center().y),
                            egui::Align2::LEFT_CENTER,
                            ui::elide_to_width(ui.ctx(), &sc.heading, theme::font_med(theme::T_SM), 0.0, r.width() - 48.0),
                            theme::font_med(theme::T_SM),
                            p.text,
                        );
                    });
            }
            anim::keep_going(ui.ctx());
        }
        if !still_down {
            if let Some(to) = st.drop_at.take() {
                if to != from && to != from + 1 {
                    out.act = Some(Act::Move(from, to));
                }
            }
            st.dragging = None;
        }
    }

    // card's menu here---
    if let Some((k, at, opened)) = st.menu {
        if let Some(sc) = scenes.get(k) {
            let mut close = false;
            let area = egui::Area::new(egui::Id::new("ns-card-menu"))
                .order(egui::Order::Foreground)
                .fixed_pos(at)
                .show(ui.ctx(), |ui| {
                    ui::popover_frame().show(ui, |ui| {
                        ui.set_width(ui::menu_width(ui, &["Open in the script", "Delete this scene"]).max(176.0));
                        let mut rects = Vec::new();
                        if ui::menu_item(ui, &mut rects, "Open in the script", Icon::Pencil, false) {
                            out.act = Some(Act::Open(sc.id));
                            close = true;
                        }
                        ui::section(ui, "Storyline");
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 3.0;
                            ui.add_space(6.0);
                            for t in 0..STORYLINES {
                                if ui::swatch(ui, p.group(t), doc.meta.storyline(t).unwrap_or(""), sc.tint == Some(t), None) {
                                    out.act = Some(Act::Tint(sc.id, Some(t)));
                                    close = true;
                                }
                            }
                            if ui::icon_button(ui, Icon::Eraser, "No colour", 22.0) {
                                out.act = Some(Act::Tint(sc.id, None));
                                close = true;
                            }
                        });
                        ui::separator(ui);
                        if ui::menu_item(ui, &mut rects, "Delete this scene", Icon::Trash, true) {
                            out.act = Some(Act::Delete(k));
                            close = true;
                        }
                    });
                })
                .response;
            let d = ui::Dismisser { opened_on: opened };
            if close
                || d.should_close(ui.ctx(), frame_no, &[area.rect])
                || ui.input(|i| i.key_pressed(egui::Key::Escape))
            {
                st.menu = None;
            }
        } else {
            st.menu = None;
        }
    }

    out
}

fn storyline_label(doc: &Document, t: usize) -> String {
    doc.meta.storyline(t).map(str::to_string).unwrap_or_else(|| format!("{} story", (b'A' + t as u8) as char))
}

#[allow(clippy::too_many_arguments)]
fn beat_bar(
    ui: &mut egui::Ui,
    doc: &mut Document,
    st: &mut CardsState,
    scenes: &[Scene],
    acts: &[ActSpan],
    spans: &[(usize, usize)],
    total: usize,
    live: Option<u64>,
    guides: bool,
    frame: Rect,
    out: &mut Out,
) -> f32 {
    let p = pal();
    let left = frame.left();
    let w = frame.width();
    let mut y = frame.top();
    let used: Vec<usize> = (0..STORYLINES)
        .filter(|t| scenes.iter().any(|s| s.tint == Some(*t)) || doc.meta.storyline(*t).is_some())
        .collect();

    // storylines legend here---
    let row = Rect::from_min_size(Pos2::new(left, y), Vec2::new(w, 26.0));
    let hud = theme::font_hud(theme::T_MICRO);
    let mut x = left + theme::tracked_text(ui.painter(), Pos2::new(left, row.center().y), "STORYLINES", hud.clone(), p.text_faint, 0.8) + 14.0;
    let mut right = ui.new_child(egui::UiBuilder::new().max_rect(row).layout(egui::Layout::right_to_left(egui::Align::Center)));
    let naming = st.naming.is_some();
    if ui::quiet_button(&mut right, if naming { "Done" } else { "Name" }, Some(if naming { Icon::Check } else { Icon::Pencil })) {
        st.naming = if naming {
            None
        } else {
            Some(std::array::from_fn(|t| doc.meta.storyline(t).unwrap_or("").to_string()))
        };
    }
    if !st.only.is_empty() && ui::quiet_button(&mut right, "All", None) {
        st.only.clear();
    }
    let stop = right.min_rect().left() - 12.0;
    st.chips.clear();
    if used.is_empty() {
        ui.painter().text(
            Pos2::new(x, row.center().y),
            egui::Align2::LEFT_CENTER,
            "Right click a card to give it a storyline colour.",
            theme::font(theme::T_CAP),
            p.text_faint,
        );
    }
    for t in &used {
        let label = storyline_label(doc, *t);
        let font = theme::font_med(theme::T_CAP + 1.0);
        let tw = ui.painter().layout_no_wrap(label.clone(), font.clone(), p.text).rect.width();
        let chip = Rect::from_min_size(Pos2::new(x, row.top() + 1.0), Vec2::new(tw + 30.0, 24.0));
        if chip.right() > stop {
            break;
        }
        let resp = ui.interact(chip, egui::Id::new(("ns-storyline", *t)), Sense::click());
        let on = st.only.contains(t);
        let hot = anim::ease(ui.ctx(), resp.id, resp.hovered(), anim::HOVER);
        theme::hover_surface(ui.painter(), chip, theme::R_SM, p.group(*t), hot, on);
        let dim = !st.only.is_empty() && !on;
        ui.painter().rect_filled(
            Rect::from_center_size(Pos2::new(chip.left() + 12.0, chip.center().y), Vec2::splat(8.0)),
            egui::Rounding::ZERO,
            if dim { theme::wash(p.group(*t), 90) } else { p.group(*t) },
        );
        ui.painter().text(
            Pos2::new(chip.left() + 22.0, chip.center().y),
            egui::Align2::LEFT_CENTER,
            label,
            font,
            if dim { p.text_faint } else { p.text },
        );
        st.chips.push((*t, chip));
        if resp.on_hover_text("Show only this storyline · click again to add or remove").clicked() {
            if on {
                st.only.retain(|x| x != t);
            } else {
                st.only.push(*t);
            }
        }
        x = chip.right() + 4.0;
    }
    y += 30.0;

    // naming storylines here---
    if let Some(names) = st.naming.as_mut() {
        let per = 3usize;
        let cell_w = (w / per as f32).floor();
        for (t, name) in names.iter_mut().enumerate() {
            let cell = Rect::from_min_size(Pos2::new(left + (t % per) as f32 * cell_w, y + (t / per) as f32 * 34.0), Vec2::new(cell_w - 12.0, 30.0));
            ui.painter().rect_filled(
                Rect::from_center_size(Pos2::new(cell.left() + 6.0, cell.center().y), Vec2::splat(9.0)),
                egui::Rounding::ZERO,
                p.group(t),
            );
            let mut field = ui.new_child(egui::UiBuilder::new().max_rect(cell.with_min_x(cell.left() + 18.0)).layout(egui::Layout::left_to_right(egui::Align::Center)));
            let resp = field.add(
                egui::TextEdit::singleline(name)
                    .id(egui::Id::new(("ns-storyline-name", t)))
                    .hint_text(format!("{} story", (b'A' + t as u8) as char))
                    .desired_width(cell.width() - 18.0)
                    .margin(egui::Margin::symmetric(8.0, 5.0)),
            );
            if resp.changed() {
                doc.meta.set_storyline(t, name);
                out.changed = true;
            }
        }
        y += 2.0 * 34.0 + 8.0;
    }

    // timeline here---
    let label_rows = if guides { 2.0 * 13.0 } else { 0.0 };
    let track = Rect::from_min_size(Pos2::new(left, y + label_rows + 4.0), Vec2::new(w, 20.0));
    ui.painter().rect_filled(track, egui::Rounding::same(theme::R_SM), if p.dark { p.raised } else { p.sunken });
    let total_f = total.max(1) as f32;
    let px = |e: f32| track.left() + track.width() * (e / total_f);
    st.segments.clear();
    for (k, (sc, (at, len))) in scenes.iter().zip(spans).enumerate() {
        let seg = Rect::from_min_max(Pos2::new(px(*at as f32), track.top()), Pos2::new(px((*at + *len) as f32), track.bottom()));
        let inner = seg.shrink2(Vec2::new(if seg.width() > 3.0 { 0.75 } else { 0.0 }, 0.0));
        let resp = ui.interact(seg, egui::Id::new(("ns-timeline", sc.id)), Sense::click());
        let hot = anim::ease(ui.ctx(), resp.id, resp.hovered(), anim::HOVER);
        let base = sc.tint.map(|t| p.group(t)).unwrap_or(if p.dark { p.line_strong } else { p.line });
        let outside = !st.only.is_empty() && !sc.tint.map(|t| st.only.contains(&t)).unwrap_or(false);
        let col = if outside { theme::wash(base, 50) } else { theme::mix(base, p.text, 0.25 * hot) };
        ui.painter().rect_filled(inner, egui::Rounding::ZERO, col);
        if live == Some(sc.id) {
            ui.painter().rect_stroke(inner.expand(1.0), egui::Rounding::ZERO, egui::Stroke::new(1.5_f32, p.text));
        }
        st.segments.push((k, seg));
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        let page = (*at / 8) + 1;
        let heading = if sc.heading.trim().is_empty() { "Untitled scene" } else { sc.heading.trim() };
        let tip = match sc.tint {
            Some(t) => format!("{:02} · {heading}\np.{page} · {} pg · {}", sc.number, eighths_label(*len), storyline_label(doc, t)),
            None => format!("{:02} · {heading}\np.{page} · {} pg", sc.number, eighths_label(*len)),
        };
        if resp.on_hover_text(tip).clicked() {
            st.scroll_to = Some(k);
        }
    }

    // act breaks here---
    for a in acts.iter().skip(1) {
        if let Some(k) = scenes.iter().position(|s| s.start > a.start) {
            let x = px(spans[k].0 as f32).round() + 0.5;
            ui.painter().line_segment([Pos2::new(x, track.top() - 3.0), Pos2::new(x, track.bottom() + 3.0)], egui::Stroke::new(2.0_f32, p.text));
        }
    }

    // beat guides here---
    if guides {
        for (i, (name, f, page)) in beat_pages(total).into_iter().enumerate() {
            let x = px(f * total_f).round() + 0.5;
            let mut yy = track.top() - 2.0;
            while yy < track.bottom() + 2.0 {
                ui.painter().line_segment([Pos2::new(x, yy), Pos2::new(x, (yy + 3.0).min(track.bottom() + 2.0))], egui::Stroke::new(1.0_f32, p.primary_light));
                yy += 5.0;
            }
            let label = format!("{} · P.{page}", name.to_uppercase());
            let lw = theme::tracked_width(ui.ctx(), &label, hud.clone(), 0.7);
            let ly = y + 6.0 + if i % 2 == 0 { 0.0 } else { 13.0 };
            let lx = (x - lw * 0.5).clamp(left, left + w - lw);
            ui.painter().line_segment([Pos2::new(x, ly + 6.0), Pos2::new(x, track.top() - 2.0)], egui::Stroke::new(1.0_f32, theme::wash(p.primary_light, 120)));
            theme::tracked_text(ui.painter(), Pos2::new(lx, ly), &label, hud.clone(), p.primary_light, 0.7);
        }
    }

    // ruler here---
    let pages = (total as f32 / 8.0).ceil().max(1.0) as usize;
    let step = [1usize, 5, 10, 20, 50].into_iter().find(|s| (pages / s) as f32 * 34.0 < w).unwrap_or(100);
    let ry = track.bottom() + 4.0;
    let mut n = 1usize;
    while n <= pages {
        let x = px(((n - 1) * 8) as f32).round() + 0.5;
        ui.painter().line_segment([Pos2::new(x, ry), Pos2::new(x, ry + 4.0)], egui::Stroke::new(1.0_f32, p.text_faint));
        ui.painter().text(Pos2::new(x + 3.0, ry + 2.0), egui::Align2::LEFT_TOP, n.to_string(), theme::font_mono(theme::T_MICRO), p.text_faint);
        n = if n == 1 && step > 1 { step } else { n + step };
    }
    (ry + 18.0) - frame.top()
}

fn act_ribbon(ui: &mut egui::Ui, rect: Rect, a: &ActSpan, note: &str) -> bool {
    let p = pal();
    let resp = ui.interact(rect, egui::Id::new(("ns-act-ribbon", a.id)), Sense::click());
    let hot = anim::ease(ui.ctx(), resp.id, resp.hovered(), anim::HOVER);
    let cy = rect.center().y;
    ui.painter().rect_filled(
        Rect::from_center_size(Pos2::new(rect.left() + 10.0, cy), Vec2::splat(7.0)),
        egui::Rounding::ZERO,
        theme::mix(p.primary_light, p.text, hot),
    );
    let title = if a.title.trim().is_empty() { act_title(a.number) } else { a.title.trim().to_string() };
    let title = ui::elide_to_width(
        ui.ctx(),
        &title.to_uppercase(),
        theme::font_display(20.0),
        0.0,
        (rect.width() - 40.0).max(40.0),
    );
    let g = ui.painter().layout_no_wrap(title, theme::font_display(20.0), p.text);
    let w = g.rect.width();
    ui.painter().galley(Pos2::new(rect.left() + 24.0, cy - g.rect.height() * 0.5), g, p.text);
    let mut x = rect.left() + 24.0 + w + 16.0;
    if !note.is_empty() {
        let font = theme::font_hud(theme::T_MICRO + 0.5);
        let up = note.to_uppercase();
        let nw = theme::tracked_width(ui.ctx(), &up, font.clone(), 0.8);
        if x + nw + 40.0 < rect.right() {
            theme::tracked_text(ui.painter(), Pos2::new(x, cy), &up, font, p.text_faint, 0.8);
            x += nw + 16.0;
        }
    }
    let end = rect.right() - 8.0;
    if end - x > 24.0 {
        let y = cy.round() + 0.5;
        ui.painter().line_segment(
            [Pos2::new(x, y), Pos2::new(end, y)],
            egui::Stroke::new(1.0_f32, theme::mix(p.line, p.line_strong, hot)),
        );
    }
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp.on_hover_text("Go to this act in the script").clicked()
}

#[allow(clippy::too_many_arguments)]
fn draw_card(
    ui: &egui::Ui,
    r: Rect,
    sc: &Scene,
    accent: egui::Color32,
    tint: Option<egui::Color32>,
    lift: f32,
    selected: bool,
    fade: f32,
    lengths: &std::collections::HashMap<u64, (usize, usize)>,
) {
    let p = pal();
    let painter = ui.painter();
    let radius = theme::R_CARD;

    let _ = accent;
    let w = |c: egui::Color32| theme::wash(c, (c.a() as f32 * fade) as u8);
    painter.rect_filled(r, egui::Rounding::same(radius), w(theme::mix(p.solid_hi, p.raised, 0.6 * lift)));
    painter.rect_stroke(
        r.shrink(0.5),
        egui::Rounding::same(radius),
        egui::Stroke::new(
            1.0_f32,
            w(if selected { p.primary_light } else { theme::mix(p.line, p.text_faint, lift) }),
        ),
    );
    theme::top_edge(painter, r);
    if let Some(t) = tint {
        painter.rect_filled(
            Rect::from_min_size(r.min + Vec2::new(1.0, 1.0), Vec2::new(3.0, r.height() - 2.0)),
            egui::Rounding::ZERO,
            w(t),
        );
    }

    let cy = r.top() + HEAD_H * 0.5 + 2.0;
    let num_col = theme::wash(p.primary_light, (255.0 * fade) as u8);
    let ng = painter.layout_no_wrap(format!("{:02}", sc.number), theme::font_display(18.0), num_col);
    let num_w = ng.rect.width();
    painter.galley(Pos2::new(r.left() + 14.0, cy - ng.rect.height() * 0.5), ng, num_col);
    let heading = if sc.heading.trim().is_empty() {
        "Untitled scene".to_string()
    } else {
        sc.heading.clone()
    };
    let head_x = r.left() + 14.0 + num_w + 10.0;
    let clip = painter.with_clip_rect(r.shrink(1.0));
    clip.text(
        Pos2::new(head_x, cy),
        egui::Align2::LEFT_CENTER,
        ui::elide_to_width(ui.ctx(), &heading, theme::font_semi(theme::T_SM), 0.0, r.right() - head_x - 14.0),
        theme::font_semi(theme::T_SM),
        theme::wash(p.text, (255.0 * fade) as u8),
    );

    let (page, eighths) = lengths.get(&sc.id).copied().unwrap_or((0, 0));
    let mut foot = if page > 0 {
        format!("p.{page} / {} pg", eighths_label(eighths))
    } else {
        "not on the page yet".to_string()
    };
    if !sc.cast.is_empty() {
        foot.push_str(" / ");
        foot.push_str(&sc.cast.join(", "));
    }
    let foot_font = theme::font_hud(theme::T_MICRO);
    theme::tracked_text(
        &clip,
        Pos2::new(r.left() + 14.0, r.bottom() - 16.0),
        &ui::elide_to_width(ui.ctx(), &foot.to_uppercase(), foot_font.clone(), 0.6, r.width() - 28.0),
        foot_font,
        theme::wash(p.text_faint, (255.0 * fade) as u8),
        0.6,
    );
}
