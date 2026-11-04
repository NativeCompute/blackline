use std::collections::HashMap;

use eframe::egui::{self, text::CCursor, Key, Modifiers, Pos2, Rect, Sense, Vec2};

use crate::anim;
use crate::caret;
use crate::model::{act_title, base_character, complete, guess_element, Document, Element, DUAL_COLS, DUAL_RIGHT, PAGE_COLS};
use crate::theme::{self, pal};

pub const GUTTER: f32 = 74.0;
const STAR_X: f32 = 26.0;
const ACT_H: f32 = 118.0;
const ACT_TITLE_PX: f32 = 36.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Caret {
    Start,
    End,
    At(usize),
    Range(usize, usize),
}

pub struct EditorState {
    pub focus_block: Option<u64>,
    pub caret: Option<usize>,
    pub pending_focus: Option<(u64, Caret)>,
    pub scroll_to_focus: bool,
    pub font_px: f32,
    seen: std::collections::HashSet<u64>,
    heights: HashMap<u64, f32>,
    pub fresh: bool,
    pub note_request: Option<u64>,
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            focus_block: None,
            caret: None,
            pending_focus: None,
            scroll_to_focus: false,
            font_px: 16.0,
            seen: Default::default(),
            heights: Default::default(),
            fresh: true,
            note_request: None,
        }
    }
}

impl EditorState {
    pub fn focus(&mut self, id: u64, caret: Caret) {
        self.pending_focus = Some((id, caret));
    }

    pub fn jump_to(&mut self, id: u64) {
        self.pending_focus = Some((id, Caret::End));
        self.scroll_to_focus = true;
    }

    pub fn select(&mut self, id: u64, from: usize, to: usize) {
        self.pending_focus = Some((id, Caret::Range(from, to)));
        self.scroll_to_focus = true;
    }

    pub fn current_element(&self, doc: &Document) -> Option<Element> {
        self.focus_block
            .and_then(|id| doc.block(id))
            .map(|b| b.element)
    }
}

pub struct View<'a> {
    pub page_starts: &'a HashMap<u64, usize>,
    pub page_breaks: bool,
    pub scene_numbers: bool,
    pub smart_type: bool,
    pub typewriter: bool,
    pub focus_mode: bool,
    pub find: Option<&'a str>,
    pub current: Option<(u64, usize)>,
    pub element_colors: bool,
    pub char_colors: Option<&'a HashMap<String, egui::Color32>>,
    pub pages: usize,
    pub margin: f32,
}

pub fn set_element(doc: &mut Document, st: &mut EditorState, element: Element) -> bool {
    let Some(id) = st.focus_block else {
        return false;
    };
    apply(
        doc,
        st,
        Act::SetElement {
            id,
            element,
            at: usize::MAX,
        },
    )
}

enum Act {
    Split { id: u64, at: usize, same: bool },
    SetElement { id: u64, element: Element, at: usize },
    MergePrev { id: u64 },
    MergeNext { id: u64 },
    FocusPrev { id: u64 },
    FocusNext { id: u64 },
    MoveUp { id: u64 },
    MoveDown { id: u64 },
    Explode { id: u64 },
    Accept { id: u64, rest: String },
    Stay { id: u64, at: usize },
}

#[derive(Clone, Copy)]
struct Geo {
    outer: bool,
    first: bool,
    lead: f32,
    origin: f32,
    left: f32,
    right: f32,
    tags: bool,
    hop: bool,
}

fn keep_remark(into: &mut crate::model::Block, from: &crate::model::Block) {
    let extra = from.remark.trim();
    if extra.is_empty() {
        return;
    }
    if into.remark.trim().is_empty() {
        into.remark = extra.to_string();
    } else {
        into.remark = format!("{} / {extra}", into.remark.trim());
    }
}

pub fn block_id(b: u64) -> egui::Id {
    egui::Id::new(("ns-block", b))
}

fn hint_for(e: Element) -> &'static str {
    match e {
        Element::SceneHeading => "INT. LOCATION - DAY",
        Element::Action => "What we see and hear.",
        Element::Character => "CHARACTER",
        Element::Parenthetical => "(under her breath)",
        Element::Dialogue => "What they say.",
        Element::Transition => "CUT TO:",
        Element::Shot => "ANGLE ON",
        Element::Act => "ACT ONE",
    }
}

pub fn column_width(ui: &egui::Ui, font_px: f32) -> f32 {
    let font = theme::font_page(font_px);
    let char_w = ui.fonts(|f| f.glyph_width(&font, 'M')).max(1.0);
    GUTTER + char_w * PAGE_COLS as f32 + 28.0
}

pub fn show(ui: &mut egui::Ui, doc: &mut Document, st: &mut EditorState, view: &View) -> bool {
    let font = theme::font_page(st.font_px);
    let font_bold = theme::font_page_bold(st.font_px);
    let char_w = ui.fonts(|f| f.glyph_width(&font, 'M')).max(1.0);
    let row_h = ui.fonts(|f| f.row_height(&font));

    if st.fresh {
        st.seen.clear();
        st.heights.clear();
        for b in &doc.blocks {
            st.seen.insert(b.id);
            ui.ctx()
                .animate_bool_with_time(egui::Id::new(("ns-grow", b.id)), true, 0.0);
        }
        st.fresh = false;
    }

    let mut acts: Vec<Act> = Vec::new();
    let mut changed = false;
    ui.style_mut().spacing.item_spacing.y = 0.0;
    changed |= page_body(
        ui, doc, st, view, &mut acts, &font, &font_bold, char_w, row_h,
    );

    for act in acts {
        changed |= apply(doc, st, act);
    }
    doc.ensure_not_empty();
    changed
}

#[allow(clippy::too_many_arguments)]
fn page_body(
    outer: &mut egui::Ui,
    doc: &mut Document,
    st: &mut EditorState,
    view: &View,
    acts: &mut Vec<Act>,
    font: &egui::FontId,
    font_bold: &egui::FontId,
    char_w: f32,
    row_h: f32,
) -> bool {
    let p = pal();
    let mut changed = false;
    let col = outer.max_rect();
    let content_left = col.left();
    let mut any_focus = false;

    let live_scene = st.focus_block.and_then(|id| doc.scene_of(id));
    let mut scene_no = 0usize;
    let mut in_scene: Option<usize> = None;

    let needle = view.find.map(|s| s.to_lowercase()).filter(|s| !s.is_empty());
    let act_notes = act_notes(doc, view.page_starts, view.pages);
    let mut speaker: Option<String> = None;
    let pairs = doc.dual_pairs();
    let mut cols: Option<(egui::Ui, egui::Ui, usize, usize, f32)> = None;
    let x0 = content_left + GUTTER;
    let dual_w = DUAL_COLS as f32 * char_w;
    let rx = x0 + DUAL_RIGHT as f32 * char_w;

    for i in 0..doc.blocks.len() {
        // dual pair opens here---
        if let Some(pair) = pairs.iter().find(|p| p.left == i).copied() {
            if view.page_breaks {
                if let Some(n) = (pair.left..pair.end).find_map(|k| view.page_starts.get(&doc.blocks[k].id)) {
                    page_rule(outer, content_left + GUTTER, col.right() - 10.0, *n);
                }
            }
            if i > 0 {
                outer.add_space(Element::Character.blank_lines_before() as f32 * row_h * 0.62 + 2.0);
            }
            let top = outer.cursor().top();
            let mk = |outer: &mut egui::Ui, x: f32| {
                outer.new_child(
                    egui::UiBuilder::new()
                        .max_rect(Rect::from_min_size(Pos2::new(x, top), Vec2::new(dual_w + 4.0, 100_000.0)))
                        .layout(egui::Layout::top_down(egui::Align::Min)),
                )
            };
            let left_ui = mk(outer, x0);
            let right_ui = mk(outer, rx);
            cols = Some((left_ui, right_ui, pair.right, pair.end, top));
        }
        let geo = match &cols {
            Some((_, _, right, _, _)) if i < *right => Geo {
                outer: false,
                first: i + 1 == 1 || doc.blocks[i].element == Element::Character,
                lead: 0.0,
                origin: x0,
                left: content_left + 2.0,
                right: x0 + dual_w + 6.0,
                tags: true,
                hop: i + 1 == *right,
            },
            Some((_, _, _, end, _)) if i < *end => Geo {
                outer: false,
                first: doc.blocks[i].element == Element::Character,
                lead: 0.0,
                origin: rx,
                left: rx - 6.0,
                right: col.right(),
                tags: false,
                hop: false,
            },
            _ => Geo {
                outer: true,
                first: false,
                lead: GUTTER,
                origin: x0,
                left: content_left + 2.0,
                right: col.right(),
                tags: true,
                hop: false,
            },
        };
        let ui: &mut egui::Ui = match cols.as_mut() {
            Some((l, _, right, _, _)) if i < *right => l,
            Some((_, r, _, end, _)) if i < *end => r,
            _ => &mut *outer,
        };
        let bid = doc.blocks[i].id;
        let element = doc.blocks[i].element;
        match element {
            Element::Character => speaker = Some(base_character(&doc.blocks[i].text)),
            Element::Parenthetical | Element::Dialogue => {}
            _ => speaker = None,
        }
        let voice = speaker
            .as_ref()
            .and_then(|n| view.char_colors.and_then(|m| m.get(n)))
            .copied();
        let id = block_id(bid);
        let focused = ui.memory(|m| m.has_focus(id));
        if element == Element::SceneHeading {
            in_scene = Some(scene_no);
            scene_no += 1;
        }
        let dimmed = view.focus_mode && live_scene.is_some() && in_scene != live_scene;

        // printed page begins here---
        if view.page_breaks && geo.outer {
            if let Some(n) = view.page_starts.get(&bid) {
                page_rule(ui, content_left + GUTTER, col.right() - 10.0, *n);
            }
        }

        // vertical rhythm between here---
        if i > 0 && !geo.first {
            ui.add_space(element.blank_lines_before() as f32 * row_h * 0.62 + 2.0);
        }

        // smarttype would finish here---
        let len = caret::char_len(&doc.blocks[i].text);
        let pos = if focused { caret::caret(ui.ctx(), id) } else { None };
        let ghost = if focused && view.smart_type && pos == Some(len) {
            complete(doc, bid, element, &doc.blocks[i].text)
        } else {
            None
        };

        // structural keys before here---
        if focused {
            any_focus = true;
            st.focus_block = Some(bid);
            st.caret = pos;
            let at = pos.unwrap_or(len);
            let ghost = ghost.clone();
            let is_act = element == Element::Act;
            let split_at = if is_act { len } else { at };

            ui.input_mut(|inp| {
                if inp.consume_key(Modifiers::SHIFT, Key::Enter) {
                    acts.push(Act::Split {
                        id: bid,
                        at: split_at,
                        same: !is_act,
                    });
                } else if geo.hop && pos == Some(len) && inp.consume_key(Modifiers::NONE, Key::Enter) {
                    acts.push(Act::FocusNext { id: bid });
                } else if inp.consume_key(Modifiers::NONE, Key::Enter) {
                    acts.push(Act::Split {
                        id: bid,
                        at: split_at,
                        same: false,
                    });
                } else if is_act
                    && (inp.consume_key(Modifiers::SHIFT, Key::Tab) || inp.consume_key(Modifiers::NONE, Key::Tab))
                {
                    acts.push(Act::Stay { id: bid, at });
                } else if inp.consume_key(Modifiers::SHIFT, Key::Tab) {
                    acts.push(Act::SetElement {
                        id: bid,
                        element: element.cycle(false),
                        at,
                    });
                } else if let Some(rest) = ghost.filter(|_| inp.consume_key(Modifiers::NONE, Key::Tab)) {
                    acts.push(Act::Accept { id: bid, rest });
                } else if inp.consume_key(Modifiers::NONE, Key::Tab) {
                    acts.push(Act::SetElement {
                        id: bid,
                        element: element.cycle(true),
                        at,
                    });
                } else if inp.consume_key(Modifiers::ALT, Key::ArrowUp) {
                    acts.push(Act::MoveUp { id: bid });
                } else if inp.consume_key(Modifiers::ALT, Key::ArrowDown) {
                    acts.push(Act::MoveDown { id: bid });
                } else if pos == Some(0) && inp.consume_key(Modifiers::NONE, Key::Backspace) {
                    acts.push(Act::MergePrev { id: bid });
                } else if pos == Some(len) && inp.consume_key(Modifiers::NONE, Key::Delete) {
                    acts.push(Act::MergeNext { id: bid });
                } else if pos == Some(0) && inp.consume_key(Modifiers::NONE, Key::ArrowUp) {
                    acts.push(Act::FocusPrev { id: bid });
                } else if pos == Some(len) && inp.consume_key(Modifiers::NONE, Key::ArrowDown) {
                    acts.push(Act::FocusNext { id: bid });
                }
            });
        }

        // new block springs here---
        let grow_id = egui::Id::new(("ns-grow", bid));
        if st.seen.insert(bid) {
            ui.ctx().animate_bool_with_time(grow_id, false, 0.0);
        }
        let raw = if anim::enabled() {
            ui.ctx().animate_bool_with_time(grow_id, true, 0.5)
        } else {
            1.0
        };
        let opening = raw < 0.999;

        // row here---
        let bg_idx = ui.painter().add(egui::Shape::Noop);
        let (indent_cols, width_cols) = if geo.outer {
            (element.indent_cols(), element.width_cols())
        } else {
            (element.dual_indent(), element.dual_width())
        };
        let indent_px = indent_cols as f32 * char_w;
        let width_px = width_cols as f32 * char_w;
        let ink = match voice {
            Some(v) if element == Element::Parenthetical => theme::mix(v, p.text_dim, 0.35),
            Some(v) => v,
            None => theme::element_ink(element),
        };
        let ink = if dimmed { theme::wash(ink, 70) } else { ink };

        let row_top = ui.cursor().top();
        let lay_row = |ui: &mut egui::Ui, doc: &mut Document| {
            ui.horizontal(|ui| {
                ui.add_space(geo.lead + indent_px);
                let block = &mut doc.blocks[i];
                egui::TextEdit::multiline(&mut block.text)
                    .id(id)
                    .font(if element == Element::SceneHeading {
                        font_bold.clone()
                    } else {
                        font.clone()
                    })
                    .text_color(ink)
                    .desired_width(width_px)
                    .desired_rows(1)
                    .frame(false)
                    .margin(egui::Margin::symmetric(0.0, 1.0))
                    .lock_focus(false)
                    .hint_text(
                        egui::RichText::new(hint_for(element))
                            .color(theme::wash(p.text_faint, if dimmed { 60 } else { 160 })),
                    )
                    .show(ui)
            })
            .inner
        };
        let output = if element == Element::Act {
            let note = act_notes.get(&bid).cloned().unwrap_or_default();
            let ink = if dimmed { 0.35 } else { 1.0 };
            act_row(ui, doc, i, id, col, focused, raw, &note, ink)
        } else if opening {
            let known = st.heights.get(&bid).copied().unwrap_or(row_h + 2.0);
            let grow = anim::bounce(raw);
            let wide = ui.max_rect().width();
            let (slot, _) = ui.allocate_exact_size(
                Vec2::new(wide, (known * grow).max(0.0)),
                Sense::hover(),
            );
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(Rect::from_min_size(slot.min, Vec2::new(wide, known.max(row_h) + 40.0)))
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );
            child.set_clip_rect(slot.intersect(ui.clip_rect()));
            let out = lay_row(&mut child, doc);
            st.heights.insert(bid, child.min_rect().height());
            anim::keep_going(ui.ctx());
            out
        } else {
            let out = lay_row(ui, doc);
            st.heights.insert(bid, ui.cursor().top() - row_top);
            out
        };
        let response = output.response.clone();

        if response.changed() {
            changed = true;
            let block = &mut doc.blocks[i];
            if block.text.contains('\n') {
                acts.push(Act::Explode { id: bid });
            } else if element.is_upper() {
                let upper = block.text.to_uppercase();
                if upper != block.text {
                    block.text = upper;
                }
            }
            if view.typewriter {
                response.scroll_to_me(Some(egui::Align::Center));
            }
        }

        // row's own light here---
        let row = response.rect;
        let lit = anim::ease(ui.ctx(), ("ns-wash", bid), focused, 0.24);
        let mut under: Vec<egui::Shape> = Vec::new();
        if view.element_colors && !dimmed && element != Element::Act {
            let words = output.galley.rect.translate(output.galley_pos.to_vec2());
            let text_left = geo.origin + indent_px;
            let right = if doc.blocks[i].text.is_empty() {
                text_left + char_w * 8.0
            } else {
                words.right().max(text_left + char_w * 4.0)
            };
            let band = Rect::from_min_max(
                Pos2::new(text_left - 7.0, row.top() - 1.5),
                Pos2::new((right + 7.0).min(geo.right - 2.0), row.bottom() + 1.5),
            );
            let (fill, tick) = match voice {
                Some(v) => (theme::tint(v, if p.dark { 0.035 } else { 0.05 }), v),
                None => (theme::element_band(element), theme::element_color(element)),
            };
            under.push(egui::Shape::rect_filled(band, egui::Rounding::same(5.0), fill));
            if element != Element::Action || voice.is_some() {
                under.push(egui::Shape::rect_filled(
                    Rect::from_min_size(
                        Pos2::new(band.left(), band.top() + 4.0),
                        Vec2::new(1.5, (band.height() - 8.0).max(4.0)),
                    ),
                    egui::Rounding::same(0.75),
                    theme::wash(tick, if p.dark { 85 } else { 105 }),
                ));
            }
        }
        if lit > 0.01 && element != Element::Act {
            let plate = Rect::from_min_max(
                Pos2::new(geo.left, row.top() - 6.0),
                Pos2::new(geo.right, row.bottom() + 6.0),
            );
            under.push(theme::grad_poly_shape(
                &theme::rounded_poly(plate, theme::R_CARD),
                theme::wash(p.primary, (34.0 * lit) as u8),
                theme::wash(p.primary, 0),
                Vec2::new(1.0, 0.0),
            ));
        }
        if let Some(n) = &needle {
            let text_lower = doc.blocks[i].text.to_lowercase();
            if text_lower.contains(n.as_str()) {
                let n_len = n.chars().count();
                let chars: Vec<char> = text_lower.chars().collect();
                let pat: Vec<char> = n.chars().collect();
                let mut k = 0;
                while k + n_len <= chars.len() {
                    if chars[k..k + n_len] == pat[..] {
                        let a = output.galley.pos_from_ccursor(CCursor::new(k));
                        let b = output.galley.pos_from_ccursor(CCursor::new(k + n_len));
                        let r = if (a.min.y - b.min.y).abs() < 1.0 {
                            Rect::from_min_max(a.min, Pos2::new(b.min.x, a.max.y))
                        } else {
                            Rect::from_min_size(a.min, Vec2::new(char_w * n_len as f32, a.height()))
                        };
                        let r = r.translate(output.galley_pos.to_vec2()).expand2(Vec2::new(1.5, 0.0));
                        if view.current == Some((bid, k)) {
                            under.push(egui::Shape::rect_filled(
                                r.expand(1.0),
                                egui::Rounding::same(4.0),
                                theme::wash(p.sec_light, if p.dark { 150 } else { 120 }),
                            ));
                        } else {
                            under.push(egui::Shape::rect_filled(
                                r,
                                egui::Rounding::same(3.0),
                                theme::wash(p.sec, if p.dark { 80 } else { 60 }),
                            ));
                        }
                        k += n_len;
                    } else {
                        k += 1;
                    }
                }
            }
        }
        if !under.is_empty() {
            ui.painter().set(bg_idx, egui::Shape::Vec(under));
        }

        // smarttype's suggestion ghost here---
        if let Some(rest) = &ghost {
            let end = output.galley.pos_from_ccursor(CCursor::new(len));
            let at = output.galley_pos + end.min.to_vec2();
            let g = ui.painter().layout_no_wrap(
                rest.clone(),
                if element == Element::SceneHeading {
                    font_bold.clone()
                } else {
                    font.clone()
                },
                theme::wash(p.text_faint, 170),
            );
            let w = g.rect.width();
            ui.painter().galley(at, g, theme::wash(p.text_faint, 170));
            let chip = Rect::from_min_size(
                Pos2::new(at.x + w + 8.0, at.y + (end.height() - 15.0) * 0.5),
                Vec2::new(28.0, 15.0),
            );
            ui.painter().rect_filled(
                chip,
                egui::Rounding::same(theme::R_PILL.min(7.5)),
                theme::wash(p.primary, 34),
            );
            ui.painter().text(
                chip.center(),
                egui::Align2::CENTER_CENTER,
                "tab",
                theme::font_mono(theme::T_MICRO - 1.0),
                p.primary_light,
            );
        }

        // gutter here---
        let mid_y = row.top() + row_h * 0.5;
        if element == Element::SceneHeading {
            let live = live_scene == in_scene && live_scene.is_some();
            scene_star(ui, Pos2::new(content_left + STAR_X, mid_y), bid, live, dimmed);
            if view.scene_numbers {
                ui.painter().text(
                    Pos2::new(content_left + STAR_X + 16.0, mid_y),
                    egui::Align2::LEFT_CENTER,
                    format!("{}", scene_no),
                    theme::font_mono(theme::T_CAP),
                    if dimmed { theme::wash(p.text_faint, 80) } else { p.text_faint },
                );
            }
        } else {
            let tag = anim::ease(ui.ctx(), ("ns-tag", bid), focused || response.hovered(), anim::HOVER);
            if tag > 0.02 && geo.tags {
                let color = theme::element_color(element);
                theme::tracked_text(
                    ui.painter(),
                    Pos2::new(content_left + 10.0, mid_y),
                    element.short(),
                    theme::font_semi(theme::T_MICRO - 1.0),
                    theme::wash(color, (235.0 * tag) as u8),
                    0.9,
                );
            }
        }

        // note in the margin here---
        let remark = doc.blocks[i].remark.trim().to_string();
        let mut wants_note = false;
        response.context_menu(|ui| {
            let label = if remark.is_empty() { "Add a note" } else { "Edit the note" };
            if crate::ui::menu_item(ui, &mut Vec::new(), label, crate::icons::Icon::Pencil, false) {
                wants_note = true;
                ui.close_menu();
            }
        });
        if !remark.is_empty() && element != Element::Act {
            let dot = Rect::from_center_size(Pos2::new(col.right() - 12.0, mid_y), Vec2::splat(16.0));
            let hit = ui.interact(dot, egui::Id::new(("ns-note", bid)), Sense::click());
            let hot = anim::ease(ui.ctx(), hit.id, hit.hovered(), anim::HOVER);
            ui.painter().rect_filled(
                Rect::from_center_size(dot.center(), Vec2::splat(7.0 + 2.0 * hot)),
                egui::Rounding::same(1.5),
                theme::mix(p.primary, p.primary_light, hot),
            );
            if hit.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if view.margin >= 170.0 {
                let wrap_w = (view.margin - 34.0).min(260.0);
                let galley = ui.painter().layout(
                    remark.clone(),
                    theme::font(theme::T_SM),
                    theme::wash(p.text_dim, if dimmed { 90 } else { 230 }),
                    wrap_w,
                );
                let at = Pos2::new(col.right() + 18.0, row.top());
                ui.painter().rect_filled(
                    Rect::from_min_size(Pos2::new(at.x - 9.0, at.y + 2.0), Vec2::new(2.0, galley.rect.height().max(row_h) - 4.0)),
                    egui::Rounding::same(1.0),
                    theme::wash(p.primary, 140),
                );
                ui.painter().galley(at, galley, p.text_dim);
            }
            if hit.on_hover_text(&remark).clicked() {
                wants_note = true;
            }
        }
        if wants_note {
            st.note_request = Some(bid);
        }

        // deferred focus here---
        if let Some((fid, target)) = st.pending_focus {
            if fid == bid {
                response.request_focus();
                let len = caret::char_len(&doc.blocks[i].text);
                match target {
                    Caret::Range(a, b) => caret::select(ui.ctx(), id, a.min(len), b.min(len)),
                    other => {
                        let index = match other {
                            Caret::Start => 0,
                            Caret::End => len,
                            Caret::At(n) => n.min(len),
                            Caret::Range(..) => len,
                        };
                        caret::set(ui.ctx(), id, index);
                    }
                }
                st.pending_focus = None;
                if st.scroll_to_focus {
                    response.scroll_to_me(Some(egui::Align::Center));
                    st.scroll_to_focus = false;
                }
            }
        }

        // dual pair closes here---
        if let Some((l, r, _, end, top)) = &cols {
            if i + 1 == *end {
                let bottom = l.min_rect().bottom().max(r.min_rect().bottom()).max(*top);
                let top = *top;
                cols = None;
                outer.allocate_exact_size(Vec2::new(col.width(), bottom - top), Sense::hover());
                let bracket = Rect::from_min_max(Pos2::new(content_left + 8.0, top + 2.0), Pos2::new(content_left + 10.0, bottom - 2.0));
                outer.painter().rect_filled(bracket, egui::Rounding::same(1.0), theme::wash(p.primary_light, 120));
                theme::tracked_text(
                    outer.painter(),
                    Pos2::new(content_left + 16.0, top + row_h * 0.5),
                    "DUAL",
                    theme::font_semi(theme::T_MICRO - 1.0),
                    theme::wash(p.primary_light, 200),
                    0.9,
                );
            }
        }
    }

    if !any_focus && st.focus_block.and_then(|id| doc.index_of(id)).is_none() {
        st.focus_block = doc.blocks.last().map(|b| b.id);
    }

    let filler = outer.allocate_response(
        egui::vec2(outer.available_width().max(1.0), 90.0),
        Sense::click(),
    );
    if filler.clicked() {
        if let Some(last) = doc.blocks.last() {
            st.pending_focus = Some((last.id, Caret::End));
        }
    }

    changed
}

pub fn act_notes(doc: &Document, page_starts: &HashMap<u64, usize>, pages: usize) -> HashMap<u64, String> {
    let acts = doc.acts();
    let mut out = HashMap::new();
    if acts.is_empty() {
        return out;
    }
    let first_page = |id: u64| page_starts.get(&id).copied().unwrap_or(1);
    for (k, a) in acts.iter().enumerate() {
        let scenes = doc.blocks[a.start..a.end]
            .iter()
            .filter(|b| b.element == Element::SceneHeading)
            .count();
        let from = first_page(a.id);
        let to = match acts.get(k + 1) {
            Some(next) => first_page(next.id).saturating_sub(1).max(from),
            None => pages.max(from),
        };
        let pages = if to > from {
            format!("pp {from}\u{2013}{to}")
        } else {
            format!("page {from}")
        };
        out.insert(
            a.id,
            format!(
                "Act {:02} \u{2014} {} scene{} \u{2014} {pages}",
                a.number,
                scenes,
                if scenes == 1 { "" } else { "s" }
            ),
        );
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn act_row(
    ui: &mut egui::Ui,
    doc: &mut Document,
    i: usize,
    id: egui::Id,
    col: Rect,
    focused: bool,
    raw: f32,
    note: &str,
    fade: f32,
) -> egui::text_edit::TextEditOutput {
    let p = pal();
    let ctx = ui.ctx().clone();
    let bid = doc.blocks[i].id;
    let number = doc.blocks[..=i].iter().filter(|b| b.element == Element::Act).count();

    let grow = if raw < 0.999 { anim::ease_out_expo(raw).max(0.0) } else { 1.0 };
    let extra = {
        let words = if doc.blocks[i].text.is_empty() { act_title(number) } else { doc.blocks[i].text.clone() };
        let mut job = egui::text::LayoutJob::single_section(
            words.to_uppercase(),
            egui::TextFormat {
                font_id: theme::font_display(ACT_TITLE_PX),
                extra_letter_spacing: -0.5,
                ..Default::default()
            },
        );
        job.wrap.max_width = col.width() - GUTTER - 28.0;
        let g = ctx.fonts(|f| f.layout_job(job));
        let row = g.rows.first().map(|r| r.rect.height()).unwrap_or(0.0);
        g.rows.len().saturating_sub(1) as f32 * row
    };
    let band_h = ACT_H + extra;
    let (slot, _) = ui.allocate_exact_size(Vec2::new(col.width(), band_h * grow), Sense::hover());
    let band = Rect::from_min_size(slot.min, Vec2::new(col.width(), band_h));
    let clip = slot.intersect(ui.clip_rect());
    let painter = ui.painter().with_clip_rect(clip);
    if raw < 0.999 {
        anim::keep_going(&ctx);
    }

    let lit = anim::ease(&ctx, ("ns-act", bid), focused, 0.15);
    let a = |c: egui::Color32, k: f32| theme::wash(c, (c.a() as f32 * k * fade).clamp(0.0, 255.0) as u8);

    let char_w = (col.width() - GUTTER - 28.0) / PAGE_COLS as f32;
    let left = col.left() + GUTTER;
    let title_w = char_w * PAGE_COLS as f32;
    let right = (left + title_w).min(col.right() - 4.0);

    // rule over chapter here---
    let rule_y = band.top() + 14.5;
    painter.line_segment(
        [Pos2::new(col.left() + 8.0, rule_y), Pos2::new(right, rule_y)],
        egui::Stroke::new(1.0_f32, a(theme::mix(p.line_strong, p.primary_light, lit), 1.0)),
    );

    // readout signal square here---
    let hud_y = band.top() + 32.0;
    let readout = if note.is_empty() {
        format!("Act {number:02}")
    } else {
        note.to_string()
    };
    painter.rect_filled(
        Rect::from_center_size(Pos2::new(left + 3.0, hud_y), Vec2::splat(6.0)),
        egui::Rounding::ZERO,
        a(theme::mix(p.text_faint, p.primary_light, 0.6 + 0.4 * lit), 1.0),
    );
    theme::tracked_text(
        &painter,
        Pos2::new(left + 14.0, hud_y),
        &readout.to_uppercase(),
        theme::font_hud(theme::T_MICRO + 0.5),
        a(theme::mix(p.text_faint, p.text_dim, lit), 1.0),
        0.8,
    );

    // title still just here---
    let font = theme::font_display(ACT_TITLE_PX);
    let ink = a(p.text, 1.0);
    let shout = |text: &str| -> String {
        text.chars()
            .map(|c| {
                let mut up = c.to_uppercase();
                match (up.next(), up.next()) {
                    (Some(u), None) => u,
                    _ => c,
                }
            })
            .collect()
    };
    let job = |text: &str, color: egui::Color32, wrap: f32| {
        let mut job = egui::text::LayoutJob::single_section(
            shout(text),
            egui::TextFormat {
                font_id: font.clone(),
                color,
                extra_letter_spacing: -0.5,
                ..Default::default()
            },
        );
        job.wrap.max_width = wrap;
        job
    };
    let mut layouter = |ui: &egui::Ui, text: &str, wrap: f32| ui.fonts(|f| f.layout_job(job(text, ink, wrap)));
    let title_rect = Rect::from_min_size(Pos2::new(left, band.top() + 46.0), Vec2::new(title_w, 46.0 + extra));
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(title_rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    child.set_clip_rect(clip);
    let hint = job(&act_title(number), theme::wash(p.text_faint, (150.0 * fade) as u8), title_w);
    let output = egui::TextEdit::multiline(&mut doc.blocks[i].text)
        .id(id)
        .font(font.clone())
        .desired_width(title_w)
        .desired_rows(1)
        .frame(false)
        .margin(egui::Margin::symmetric(0.0, 1.0))
        .lock_focus(false)
        .layouter(&mut layouter)
        .hint_text(egui::WidgetText::LayoutJob(hint))
        .show(&mut child);

    // quiet rule under here---
    let under = band.bottom() - 14.5;
    painter.line_segment(
        [Pos2::new(left, under), Pos2::new(right, under)],
        egui::Stroke::new(1.0_f32, a(p.line, 1.0)),
    );

    output
}

fn scene_star(ui: &egui::Ui, c: Pos2, bid: u64, live: bool, dimmed: bool) {
    let p = pal();
    let lit = anim::ease(ui.ctx(), ("ns-star", bid), live, 0.15);
    let fade = if dimmed { 0.35 } else { 1.0 };
    let side = 8.0;
    let r = Rect::from_center_size(c, Vec2::splat(side));
    ui.painter().rect_filled(
        r,
        egui::Rounding::ZERO,
        theme::wash(theme::mix(p.text_faint, p.primary_light, lit), (255.0 * fade) as u8),
    );
}

fn page_rule(ui: &mut egui::Ui, x0: f32, x1: f32, page: usize) {
    let p = pal();
    ui.add_space(6.0);
    let (slot, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 18.0), Sense::hover());
    let y = slot.center().y;
    let col = theme::wash(p.text_faint, 110);
    let label = format!("{page}.");
    let lw = ui
        .painter()
        .layout_no_wrap(label.clone(), theme::font_mono(theme::T_MICRO), col)
        .rect
        .width();
    let end = x1 - lw - 10.0;
    let mid = (x0 + end) * 0.5;
    for (a, b, ca, cb) in [
        (x0, mid, theme::wash(col, 0), col),
        (mid, end, col, theme::wash(col, 0)),
    ] {
        theme::fill_grad_poly(
            ui.painter(),
            &[
                Pos2::new(a, y - 0.5),
                Pos2::new(b, y - 0.5),
                Pos2::new(b, y + 0.5),
                Pos2::new(a, y + 0.5),
            ],
            ca,
            cb,
            Vec2::new(1.0, 0.0),
        );
    }
    ui.painter().text(
        Pos2::new(x1, y),
        egui::Align2::RIGHT_CENTER,
        label,
        theme::font_mono(theme::T_MICRO),
        p.text_faint,
    );
    ui.add_space(4.0);
}

fn apply(doc: &mut Document, st: &mut EditorState, act: Act) -> bool {
    match act {
        Act::Stay { id, at } => {
            st.focus(id, Caret::At(at));
            false
        }
        Act::Accept { id, rest } => {
            let Some(i) = doc.index_of(id) else {
                return false;
            };
            doc.blocks[i].text.push_str(&rest);
            let upper = doc.blocks[i].element.is_upper();
            if upper {
                doc.blocks[i].text = doc.blocks[i].text.to_uppercase();
            }
            st.focus(id, Caret::End);
            true
        }
        Act::Split { id, at, same } => {
            let Some(i) = doc.index_of(id) else {
                return false;
            };
            let element = doc.blocks[i].element;
            let (head, tail) = caret::split_at_char(&doc.blocks[i].text, at);
            let splitting_mid = !tail.trim().is_empty();
            doc.blocks[i].text = head;
            let new_element = if same || splitting_mid {
                element
            } else {
                element.on_enter()
            };
            let nb = doc.new_block(new_element, &tail);
            let nid = nb.id;
            doc.blocks.insert(i + 1, nb);
            st.focus(nid, Caret::Start);
            true
        }
        Act::SetElement { id, element, at } => {
            let Some(i) = doc.index_of(id) else {
                return false;
            };
            if doc.blocks[i].element == element {
                return false;
            }
            doc.blocks[i].element = element;
            if element != Element::Character {
                doc.blocks[i].dual = false;
            }
            if element.is_upper() {
                let upper = doc.blocks[i].text.to_uppercase();
                doc.blocks[i].text = upper;
            }
            st.focus(id, Caret::At(at));
            true
        }
        Act::MergePrev { id } => {
            let Some(i) = doc.index_of(id) else {
                return false;
            };
            if i == 0 {
                return false;
            }
            let cur_empty = doc.blocks[i].text.trim().is_empty();
            let same_kind = doc.blocks[i].element == doc.blocks[i - 1].element;
            let prev_id = doc.blocks[i - 1].id;
            let prev_len = caret::char_len(&doc.blocks[i - 1].text);

            if cur_empty {
                let cur = doc.blocks.remove(i);
                keep_remark(&mut doc.blocks[i - 1], &cur);
                st.focus(prev_id, Caret::At(prev_len));
                doc.ensure_not_empty();
                true
            } else if same_kind {
                let cur = doc.blocks.remove(i);
                doc.blocks[i - 1].text.push_str(&cur.text);
                keep_remark(&mut doc.blocks[i - 1], &cur);
                st.focus(prev_id, Caret::At(prev_len));
                true
            } else {
                st.focus(prev_id, Caret::At(prev_len));
                false
            }
        }
        Act::MergeNext { id } => {
            let Some(i) = doc.index_of(id) else {
                return false;
            };
            if i + 1 >= doc.blocks.len() {
                return false;
            }
            let next_empty = doc.blocks[i + 1].text.trim().is_empty();
            let same_kind = doc.blocks[i].element == doc.blocks[i + 1].element;
            if next_empty || same_kind {
                let next = doc.blocks.remove(i + 1);
                let at = caret::char_len(&doc.blocks[i].text);
                doc.blocks[i].text.push_str(&next.text);
                keep_remark(&mut doc.blocks[i], &next);
                st.focus(id, Caret::At(at));
                true
            } else {
                false
            }
        }
        Act::FocusPrev { id } => {
            if let Some(i) = doc.index_of(id) {
                if i > 0 {
                    let pid = doc.blocks[i - 1].id;
                    st.focus(pid, Caret::End);
                }
            }
            false
        }
        Act::FocusNext { id } => {
            if let Some(i) = doc.index_of(id) {
                if i + 1 < doc.blocks.len() {
                    let nid = doc.blocks[i + 1].id;
                    st.focus(nid, Caret::Start);
                }
            }
            false
        }
        Act::MoveUp { id } => {
            if let Some(i) = doc.index_of(id) {
                if i > 0 {
                    doc.blocks.swap(i, i - 1);
                    st.focus(id, Caret::End);
                    return true;
                }
            }
            false
        }
        Act::MoveDown { id } => {
            if let Some(i) = doc.index_of(id) {
                if i + 1 < doc.blocks.len() {
                    doc.blocks.swap(i, i + 1);
                    st.focus(id, Caret::End);
                    return true;
                }
            }
            false
        }
        Act::Explode { id } => {
            let Some(i) = doc.index_of(id) else {
                return false;
            };
            let raw = std::mem::take(&mut doc.blocks[i].text);
            let mut lines = raw.split('\n');
            let first = lines.next().unwrap_or("").trim_end().to_string();
            let element = doc.blocks[i].element;
            doc.blocks[i].text = if element.is_upper() {
                first.to_uppercase()
            } else {
                first
            };

            let mut prev = element;
            let mut at = i;
            let mut last_id = id;
            for line in lines {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    prev = Element::Action;
                    continue;
                }
                let e = guess_element(trimmed, Some(prev));
                let text = if e.is_upper() {
                    trimmed.to_uppercase()
                } else {
                    trimmed.to_string()
                };
                let nb = doc.new_block(e, &text);
                last_id = nb.id;
                at += 1;
                doc.blocks.insert(at, nb);
                prev = e;
            }
            st.focus(last_id, Caret::End);
            true
        }
    }
}
