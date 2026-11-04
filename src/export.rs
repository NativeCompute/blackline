use std::collections::HashMap;
use std::io;
#[cfg(not(target_arch = "wasm32"))]
use std::path::{Path, PathBuf};

use crate::model::{act_title, end_of, wrap, Document, Element, DUAL_RIGHT, LINES_PER_PAGE, PAGE_COLS};
use crate::storage;

#[derive(Clone, Debug)]
pub struct Line {
    pub indent: usize,
    pub text: String,
    pub element: Option<Element>,
    pub block: Option<u64>,
    pub scene: Option<usize>,
    pub right: Option<Box<Line>>,
    pub dual: bool,
}

impl Line {
    pub fn is_act_title(&self) -> bool {
        self.element == Some(Element::Act) && self.block.is_some()
    }

    pub fn is_act_end(&self) -> bool {
        self.element == Some(Element::Act) && self.block.is_none()
    }

    fn blank() -> Self {
        Line {
            indent: 0,
            text: String::new(),
            element: None,
            block: None,
            scene: None,
            right: None,
            dual: false,
        }
    }

    pub fn is_blank(&self) -> bool {
        self.text.trim().is_empty() && self.right.as_ref().map(|r| r.text.trim().is_empty()).unwrap_or(true)
    }
}

fn centred(text: &str, block: Option<u64>) -> Vec<Line> {
    wrap(text, PAGE_COLS)
        .into_iter()
        .map(|l| Line {
            indent: PAGE_COLS.saturating_sub(l.chars().count()) / 2,
            text: l,
            element: Some(Element::Act),
            block,
            ..Line::blank()
        })
        .collect()
}

fn act_heading(text: &str, number: usize) -> String {
    let t = text.trim();
    if t.is_empty() {
        act_title(number)
    } else {
        t.to_uppercase()
    }
}

fn close_act(out: &mut Vec<Line>, title: &str) {
    out.push(Line::blank());
    out.push(Line::blank());
    out.extend(centred(&end_of(title), None));
}

fn body_of(e: Element, text: &str) -> String {
    if e == Element::Parenthetical {
        let inner = text.trim_matches(|c| c == '(' || c == ')').trim();
        format!("({})", inner)
    } else if e.is_upper() {
        text.to_uppercase()
    } else {
        text.to_string()
    }
}

fn column(doc: &Document, from: usize, to: usize, offset: usize) -> Vec<Line> {
    let mut out = Vec::new();
    for block in &doc.blocks[from..to] {
        let text = block.text.trim();
        if text.is_empty() {
            continue;
        }
        let e = block.element;
        for l in wrap(&body_of(e, text), e.dual_width()) {
            out.push(Line {
                indent: offset + e.dual_indent(),
                text: l,
                element: Some(e),
                block: Some(block.id),
                ..Line::blank()
            });
        }
    }
    out
}

pub fn compose(doc: &Document) -> Vec<Line> {
    let mut out: Vec<Line> = Vec::new();
    let mut scene_no = 0usize;
    let mut act: Option<String> = None;
    let mut act_no = 0usize;
    let pairs = doc.dual_pairs();

    let mut i = 0usize;
    while i < doc.blocks.len() {
        let block = &doc.blocks[i];
        if let Some(pair) = pairs.iter().find(|p| p.left == i) {
            let left = column(doc, pair.left, pair.right, 0);
            let right = column(doc, pair.right, pair.end, DUAL_RIGHT);
            if !left.is_empty() || !right.is_empty() {
                if !out.is_empty() {
                    for _ in 0..Element::Character.blank_lines_before() {
                        out.push(Line::blank());
                    }
                }
                let anchor = left.first().or(right.first()).and_then(|l| l.block);
                for k in 0..left.len().max(right.len()) {
                    let mut row = left.get(k).cloned().unwrap_or_else(|| Line {
                        block: anchor,
                        ..Line::blank()
                    });
                    row.right = right.get(k).cloned().map(Box::new);
                    row.dual = true;
                    out.push(row);
                }
            }
            i = pair.end;
            continue;
        }
        i += 1;
        if block.element == Element::SceneHeading {
            scene_no += 1;
        }
        if block.element == Element::Act {
            act_no += 1;
            if let Some(t) = act.take() {
                close_act(&mut out, &t);
            }
            let title = act_heading(&block.text, act_no);
            out.extend(centred(&title, Some(block.id)));
            act = Some(title);
            continue;
        }
        let text = block.text.trim();
        if text.is_empty() {
            continue;
        }
        let e = block.element;

        if !out.is_empty() {
            for _ in 0..e.blank_lines_before() {
                out.push(Line::blank());
            }
        }

        for (k, l) in wrap(&body_of(e, text), e.width_cols()).into_iter().enumerate() {
            out.push(Line {
                indent: e.indent_cols(),
                text: l,
                element: Some(e),
                block: Some(block.id),
                scene: if k == 0 && e == Element::SceneHeading {
                    Some(scene_no)
                } else {
                    None
                },
                ..Line::blank()
            });
        }
    }
    if let Some(t) = act {
        close_act(&mut out, &t);
    }

    out
}

pub fn paginate(lines: &[Line]) -> Vec<Vec<Line>> {
    let mut pages: Vec<Vec<Line>> = Vec::new();
    let mut page: Vec<Line> = Vec::new();
    let mut speaker: Option<String> = None;

    for (i, line) in lines.iter().enumerate() {
        if line.dual {
            speaker = None;
            let opens = i == 0 || !lines[i - 1].dual;
            if opens {
                let rows = lines[i..].iter().take_while(|l| l.dual).count();
                let used = page.len();
                if used > 0 && used + rows > LINES_PER_PAGE && rows <= LINES_PER_PAGE {
                    while page.last().map(|l| l.is_blank()).unwrap_or(false) {
                        page.pop();
                    }
                    let mut carry: Vec<Line> = Vec::new();
                    while page.len() > 4
                        && page.last().map(|l| !l.dual).unwrap_or(false)
                        && matches!(page.last().and_then(|l| l.element), Some(Element::Character) | Some(Element::Parenthetical))
                    {
                        carry.insert(0, page.pop().unwrap());
                    }
                    pages.push(std::mem::take(&mut page));
                    page = carry;
                }
            }
        }
        match line.element {
            _ if line.dual => {}
            Some(Element::Character) if line.block.is_some() => {
                let first = i == 0 || lines[i - 1].block != line.block;
                if first {
                    speaker = Some(line.text.trim().to_string());
                }
            }
            Some(Element::Dialogue) | Some(Element::Parenthetical) | Some(Element::Character) => {}
            _ if line.text.trim().is_empty() => {}
            _ => speaker = None,
        }
        if page.is_empty() && line.is_blank() {
            continue;
        }
        let opens_act = line.is_act_title() && (i == 0 || lines[i - 1].block != line.block);
        if opens_act && page.iter().any(|l| !l.text.trim().is_empty()) {
            pages.push(std::mem::take(&mut page));
        }
        if line.is_act_end() && page.is_empty() {
            if let Some(prev) = pages.last_mut() {
                page = carry_into_next(prev);
            }
        }
        page.push(line.clone());

        if page.len() >= LINES_PER_PAGE {
            let mut carry: Vec<Line> = Vec::new();
            while matches!(
                page.last().and_then(|l| l.element),
                Some(Element::Character) | Some(Element::Parenthetical)
            ) && page.len() > 4
                && page.last().map(|l| !l.dual).unwrap_or(false)
            {
                carry.insert(0, page.pop().unwrap());
            }
            let next = carry.first().or_else(|| lines.get(i + 1));
            let runs_on = carry.is_empty()
                && page.last().map(|l| l.element == Some(Element::Dialogue) && !l.dual).unwrap_or(false)
                && next
                    .map(|l| matches!(l.element, Some(Element::Dialogue) | Some(Element::Parenthetical)))
                    .unwrap_or(false);
            if let (true, Some(cue)) = (runs_on, speaker.clone()) {
                carry.insert(0, page.pop().unwrap());
                let kept = page
                    .iter()
                    .rev()
                    .take_while(|l| matches!(l.element, Some(Element::Dialogue) | Some(Element::Parenthetical)))
                    .count();
                if kept < 2 {
                    while page.len() > 4
                        && matches!(
                            page.last().and_then(|l| l.element),
                            Some(Element::Dialogue) | Some(Element::Parenthetical) | Some(Element::Character)
                        )
                    {
                        carry.insert(0, page.pop().unwrap());
                    }
                } else {
                    page.push(layout_cue(MORE.to_string()));
                    carry.insert(0, layout_cue(format!("{} {CONTD}", cue_without_contd(&cue))));
                }
            }
            pages.push(std::mem::take(&mut page));
            page = carry;
        }
    }

    if !page.is_empty() {
        pages.push(page);
    }
    if pages.is_empty() {
        pages.push(Vec::new());
    }
    pages
}

pub const MORE: &str = "(MORE)";
pub const CONTD: &str = "(CONT'D)";

fn layout_cue(text: String) -> Line {
    Line {
        indent: Element::Character.indent_cols(),
        text,
        element: Some(Element::Character),
        ..Line::blank()
    }
}

fn cue_without_contd(cue: &str) -> String {
    let up = cue.to_uppercase();
    match up.rfind(CONTD) {
        Some(at) if at + CONTD.len() == up.trim_end().len() => up[..at].trim_end().to_string(),
        _ => cue.trim().to_string(),
    }
}

fn carry_into_next(prev: &mut Vec<Line>) -> Vec<Line> {
    while prev.last().map(|l| l.text.trim().is_empty()).unwrap_or(false) {
        prev.pop();
    }
    let mut carry: Vec<Line> = Vec::new();
    let Some(block) = prev.last().and_then(|l| l.block) else {
        return carry;
    };
    while prev.len() > 4 && prev.last().map(|l| l.block == Some(block)).unwrap_or(false) {
        carry.insert(0, prev.pop().unwrap());
    }
    while prev.len() > 4
        && matches!(
            prev.last().and_then(|l| l.element),
            Some(Element::Character) | Some(Element::Parenthetical)
        )
    {
        carry.insert(0, prev.pop().unwrap());
    }
    if !carry.is_empty() {
        carry.push(Line::blank());
        carry.push(Line::blank());
    }
    carry
}

pub fn page_count(doc: &Document) -> usize {
    paginate(&compose(doc)).len()
}

#[allow(dead_code)]
pub fn runtime_minutes(doc: &Document) -> usize {
    page_count(doc)
}

pub fn page_starts(doc: &Document) -> Vec<(usize, u64)> {
    let pages = paginate(&compose(doc));
    let mut out = Vec::new();
    for (i, page) in pages.iter().enumerate().skip(1) {
        if let Some(id) = page.iter().find_map(|l| l.block) {
            out.push((i + 1, id));
        }
    }
    out
}

pub fn scene_lengths(doc: &Document) -> Vec<(u64, usize, usize)> {
    let pages = paginate(&compose(doc));
    let mut starts: Vec<(u64, usize, usize)> = Vec::new();
    let mut total = 0usize;
    let heading_ids: Vec<u64> = doc
        .blocks
        .iter()
        .filter(|b| b.element == Element::SceneHeading)
        .map(|b| b.id)
        .collect();
    for (pi, page) in pages.iter().enumerate() {
        for l in page {
            if let (Some(id), Some(Element::SceneHeading)) = (l.block, l.element) {
                if !starts.iter().any(|(s, _, _)| *s == id) {
                    starts.push((id, pi + 1, total));
                }
            }
            total += 1;
        }
    }
    let mut out = Vec::new();
    for id in heading_ids {
        match starts.iter().position(|(s, _, _)| *s == id) {
            Some(k) => {
                let (_, page, at) = starts[k];
                let end = starts.get(k + 1).map(|x| x.2).unwrap_or(total);
                let lines = end.saturating_sub(at) as f32;
                let eighths = ((lines / LINES_PER_PAGE as f32) * 8.0).round().max(1.0) as usize;
                out.push((id, page, eighths));
            }
            None => out.push((id, 0, 0)),
        }
    }
    out
}

// plain text here---

pub fn to_plain_text(doc: &Document) -> String {
    let pages = paginate(&compose(doc));
    let mut s = String::new();

    for (i, page) in pages.iter().enumerate() {
        if i > 0 {
            s.push('\u{000C}');
            s.push('\n');
            let num = format!("{}.", i + 1);
            let pad = PAGE_COLS.saturating_sub(num.chars().count());
            s.push_str(&" ".repeat(pad));
            s.push_str(&num);
            s.push_str("\n\n");
        }
        for line in page {
            if line.is_blank() {
                s.push('\n');
            } else {
                s.push_str(&plain_row(line));
                s.push('\n');
            }
        }
    }
    s
}

pub fn plain_row(line: &Line) -> String {
    let mut row = String::new();
    if !line.text.trim().is_empty() {
        row.push_str(&" ".repeat(line.indent));
        row.push_str(&line.text);
    }
    if let Some(r) = &line.right {
        let at = row.chars().count();
        row.push_str(&" ".repeat(r.indent.saturating_sub(at).max(if at > 0 { 1 } else { 0 })));
        row.push_str(&r.text);
    }
    row.trim_end().to_string()
}

// fountain here---

pub fn to_fountain(doc: &Document) -> String {
    let mut s = String::new();
    if !doc.meta.title.trim().is_empty() {
        s.push_str(&format!("Title: {}\n", doc.meta.title.trim()));
    }
    if !doc.meta.author.trim().is_empty() {
        s.push_str(&format!("Credit: written by\nAuthor: {}\n", doc.meta.author.trim()));
    }
    if !doc.meta.draft.trim().is_empty() {
        s.push_str(&format!("Draft date: {}\n", doc.meta.draft.trim()));
    }
    if !doc.meta.contact.trim().is_empty() {
        s.push_str(&format!("Contact: {}\n", doc.meta.contact.trim()));
    }
    if !s.is_empty() {
        s.push('\n');
    }

    let body_from = s.len();
    let mut act: Option<String> = None;
    let mut act_no = 0usize;
    for b in &doc.blocks {
        let text = b.text.trim();
        if b.element == Element::Act {
            act_no += 1;
            if let Some(t) = act.take() {
                s.push_str(&format!(">{}<\n\n", end_of(&t)));
            }
            let title = act_heading(text, act_no);
            if s.len() > body_from {
                s.push_str("===\n\n");
            }
            s.push_str(&format!("# {title}\n\n>{title}<\n\n"));
            act = Some(title);
            continue;
        }
        if text.is_empty() {
            continue;
        }
        let n = if b.remark.trim().is_empty() {
            String::new()
        } else {
            format!(" [[{}]]", b.remark.trim().replace("]]", "] ]"))
        };
        match b.element {
            Element::SceneHeading => {
                s.push_str(&format!("\n.{}{n}\n\n", text.to_uppercase()));
                if !b.note.trim().is_empty() {
                    s.push_str(&format!("= {}\n\n", b.note.trim()));
                }
            }
            Element::Shot => s.push_str(&format!("\n{}{n}\n\n", text.to_uppercase())),
            Element::Action => s.push_str(&format!("{}{n}\n\n", text)),
            Element::Character if b.dual => s.push_str(&format!("@{} ^{n}\n", text.to_uppercase())),
            Element::Character => s.push_str(&format!("@{}{n}\n", text.to_uppercase())),
            Element::Parenthetical => {
                let inner = text.trim_matches(|c| c == '(' || c == ')').trim();
                s.push_str(&format!("({}){n}\n", inner));
            }
            Element::Dialogue => s.push_str(&format!("{}{n}\n\n", text)),
            Element::Transition => s.push_str(&format!("> {}{n}\n\n", text.to_uppercase())),
            Element::Act => {}
        }
    }
    if let Some(t) = act {
        s.push_str(&format!(">{}<\n\n", end_of(&t)));
    }
    s
}

// pdf here---

const PAGE_W_MM: f32 = 215.9;
const PAGE_H_MM: f32 = 279.4;
const LEFT_MARGIN_MM: f32 = 38.1;
const TOP_MARGIN_MM: f32 = 25.4;
const CHAR_W_MM: f32 = 2.54;
const LINE_H_MM: f32 = 25.4 / 6.0;

#[cfg(not(target_arch = "wasm32"))]
#[allow(dead_code)]
pub fn to_pdf(doc: &Document, path: &Path) -> Result<(), String> {
    to_pdf_opts(doc, path, &PdfOptions::default())
}

#[cfg(not(target_arch = "wasm32"))]
#[allow(dead_code)]
pub fn to_pdf_with(doc: &Document, path: &Path, scene_numbers: bool) -> Result<(), String> {
    to_pdf_opts(
        doc,
        path,
        &PdfOptions {
            scene_numbers,
            ..PdfOptions::default()
        },
    )
}

pub type Ink = [u8; 3];

#[derive(Clone, Debug, PartialEq)]
pub struct PdfOptions {
    pub title_page: bool,
    pub scene_numbers: bool,
    pub pages: Option<Vec<usize>>,
    pub voices: Option<HashMap<String, Ink>>,
    pub notes: bool,
}

impl Default for PdfOptions {
    fn default() -> Self {
        PdfOptions {
            title_page: true,
            scene_numbers: false,
            pages: None,
            voices: None,
            notes: false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct PlannedPage {
    pub number: usize,
    pub lines: Vec<(Line, Option<Ink>)>,
    pub right_inks: Vec<Option<Ink>>,
}

pub fn pdf_plan(doc: &Document, opts: &PdfOptions) -> Vec<PlannedPage> {
    let pages = paginate(&compose(doc));
    let mut who: Option<String> = None;
    let mut left_who: Option<String> = None;
    let mut right_who: Option<String> = None;
    let mut out = Vec::new();
    let ink_of = |who: &Option<String>, line: &Line| match (&opts.voices, who) {
        (Some(v), Some(n)) if line.element.is_some() => v.get(n).copied(),
        _ => None,
    };
    let track = |who: &mut Option<String>, line: &Line| {
        if line.element == Some(Element::Character) {
            let n = crate::model::base_character(&line.text);
            if !n.is_empty() {
                *who = Some(n);
            }
        }
    };
    for (i, page) in pages.iter().enumerate() {
        let number = i + 1;
        let mut lines = Vec::with_capacity(page.len());
        let mut right_inks = Vec::with_capacity(page.len());
        for line in page {
            if line.dual {
                who = None;
                track(&mut left_who, line);
                let left_ink = ink_of(&left_who, line);
                let right_ink = line.right.as_ref().and_then(|r| {
                    track(&mut right_who, r);
                    ink_of(&right_who, r)
                });
                lines.push((line.clone(), left_ink));
                right_inks.push(right_ink);
                continue;
            }
            left_who = None;
            right_who = None;
            right_inks.push(None);
            match line.element {
                Some(Element::Character) => {
                    let n = crate::model::base_character(&line.text);
                    if !n.is_empty() {
                        who = Some(n);
                    }
                }
                Some(Element::Dialogue) | Some(Element::Parenthetical) | None => {}
                _ => who = None,
            }
            let ink = match (&opts.voices, &who) {
                (Some(v), Some(n)) if line.element.is_some() => v.get(n).copied(),
                _ => None,
            };
            lines.push((line.clone(), ink));
        }
        let wanted = opts.pages.as_ref().map(|w| w.contains(&number)).unwrap_or(true);
        if wanted {
            out.push(PlannedPage { number, lines, right_inks });
        }
    }
    out
}

pub fn parse_page_range(text: &str, max: usize) -> Result<Vec<usize>, String> {
    let mut out: Vec<usize> = Vec::new();
    let t = text.trim();
    if t.is_empty() {
        return Err("Name at least one page".into());
    }
    for part in t.split([',', ';', ' ']).map(str::trim).filter(|p| !p.is_empty()) {
        let num = |s: &str| -> Result<usize, String> {
            s.trim()
                .parse::<usize>()
                .map_err(|_| format!("\u{201c}{s}\u{201d} is not a page number"))
        };
        let (lo, hi) = match part.split_once(['-', '\u{2013}']) {
            Some((a, b)) => {
                let lo = if a.trim().is_empty() { 1 } else { num(a)? };
                let hi = if b.trim().is_empty() { max } else { num(b)? };
                (lo, hi)
            }
            None => {
                let n = num(part)?;
                (n, n)
            }
        };
        if lo == 0 || hi == 0 {
            return Err("Pages start at 1".into());
        }
        if lo > hi {
            return Err(format!("{lo}-{hi} runs backwards"));
        }
        if lo > max {
            return Err(format!(
                "There {} only {max} page{}",
                if max == 1 { "is" } else { "are" },
                if max == 1 { "" } else { "s" }
            ));
        }
        for n in lo..=hi.min(max) {
            if !out.contains(&n) {
                out.push(n);
            }
        }
    }
    out.sort_unstable();
    Ok(out)
}

pub fn endnotes(doc: &Document) -> Vec<(bool, String)> {
    let pages = paginate(&compose(doc));
    let page_of = |id: u64| {
        pages
            .iter()
            .position(|pg| pg.iter().any(|l| l.block == Some(id) || l.right.as_ref().map(|r| r.block == Some(id)).unwrap_or(false)))
            .map(|k| k + 1)
    };
    let mut rows: Vec<(bool, String)> = Vec::new();
    for b in &doc.blocks {
        let note = b.remark.trim();
        if note.is_empty() {
            continue;
        }
        if rows.is_empty() {
            rows.push((true, "NOTES".into()));
            rows.push((false, String::new()));
        }
        let line: String = b.text.trim().chars().take(44).collect();
        let more = if b.text.trim().chars().count() > 44 { "…" } else { "" };
        let at = page_of(b.id).map(|p| format!("PAGE {p}")).unwrap_or_else(|| "PAGE ?".into());
        rows.push((true, format!("{at} — {line}{more}")));
        for l in wrap(note, PAGE_COLS - 4) {
            rows.push((false, format!("    {l}")));
        }
        rows.push((false, String::new()));
    }
    rows
}

pub fn page_of_block(doc: &Document, id: u64) -> Option<usize> {
    paginate(&compose(doc))
        .iter()
        .position(|pg| pg.iter().any(|l| l.block == Some(id) || l.right.as_ref().and_then(|r| r.block) == Some(id)))
        .map(|k| k + 1)
}

pub fn describe_pages(pages: &[usize]) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut i = 0;
    while i < pages.len() {
        let start = pages[i];
        let mut end = start;
        while i + 1 < pages.len() && pages[i + 1] == end + 1 {
            i += 1;
            end = pages[i];
        }
        parts.push(if start == end { format!("{start}") } else { format!("{start}-{end}") });
        i += 1;
    }
    parts.join(", ")
}

#[cfg(not(target_arch = "wasm32"))]
pub fn to_pdf_opts(doc: &Document, path: &Path, opts: &PdfOptions) -> Result<(), String> {
    let bytes = pdf_bytes(doc, opts)?;
    std::fs::write(path, bytes).map_err(|e| e.to_string())
}

pub fn pdf_bytes(doc: &Document, opts: &PdfOptions) -> Result<Vec<u8>, String> {
    use printpdf::{BuiltinFont, Color, Mm, PdfDocument, Rgb};

    let plan = pdf_plan(doc, opts);
    if plan.is_empty() && !opts.title_page {
        return Err("Nothing to export: no pages chosen".into());
    }

    let title = if doc.meta.title.trim().is_empty() {
        "Untitled Script"
    } else {
        doc.meta.title.trim()
    };

    let (pdf, first_page, first_layer) =
        PdfDocument::new(title, Mm(PAGE_W_MM), Mm(PAGE_H_MM), "Script");
    let courier = pdf
        .add_builtin_font(BuiltinFont::Courier)
        .map_err(|e| e.to_string())?;
    let courier_bold = pdf
        .add_builtin_font(BuiltinFont::CourierBold)
        .map_err(|e| e.to_string())?;
    let colour = |ink: Option<Ink>| {
        let [r, g, b] = ink.unwrap_or([0, 0, 0]);
        Color::Rgb(Rgb::new(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, None))
    };

    let mut first = Some((first_page, first_layer));
    let mut next_sheet = |name: String| match first.take() {
        Some(s) => s,
        None => pdf.add_page(Mm(PAGE_W_MM), Mm(PAGE_H_MM), name),
    };

    // title page here---
    if opts.title_page {
        let (pg, ly) = next_sheet("Title".into());
        let layer = pdf.get_page(pg).get_layer(ly);
        let put = |text: &str, line: f32, bold: bool| {
            let cols = text.chars().count() as f32;
            let x = LEFT_MARGIN_MM + (PAGE_COLS as f32 - cols).max(0.0) / 2.0 * CHAR_W_MM;
            let y = PAGE_H_MM - TOP_MARGIN_MM - line * LINE_H_MM;
            layer.use_text(
                text,
                12.0,
                Mm(x),
                Mm(y),
                if bold { &courier_bold } else { &courier },
            );
        };
        put(&title.to_uppercase(), 20.0, true);
        if !doc.meta.author.trim().is_empty() {
            put("written by", 24.0, false);
            put(doc.meta.author.trim(), 26.0, false);
        }
        if !doc.meta.draft.trim().is_empty() {
            put(doc.meta.draft.trim(), 44.0, false);
        }
        if !doc.meta.contact.trim().is_empty() {
            let y = PAGE_H_MM - TOP_MARGIN_MM - 48.0 * LINE_H_MM;
            layer.use_text(doc.meta.contact.trim(), 12.0, Mm(LEFT_MARGIN_MM), Mm(y), &courier);
        }
    }

    // body here---
    for page in &plan {
        let i = page.number - 1;
        let (page_idx, layer_idx) = next_sheet(format!("Page {}", page.number));
        let layer = pdf.get_page(page_idx).get_layer(layer_idx);
        layer.set_fill_color(colour(None));

        if i > 0 {
            let num = format!("{}.", i + 1);
            let cols = num.chars().count() as f32;
            let x = LEFT_MARGIN_MM + (PAGE_COLS as f32 - cols) * CHAR_W_MM;
            let y = PAGE_H_MM - 12.7 - LINE_H_MM;
            layer.use_text(&num, 12.0, Mm(x), Mm(y), &courier);
        }

        let mut inked: Option<Ink> = None;
        for (row, (line, ink)) in page.lines.iter().enumerate() {
            let y = PAGE_H_MM - TOP_MARGIN_MM - (row as f32 + 1.0) * LINE_H_MM;
            if let Some(r) = line.right.as_ref().filter(|r| !r.text.trim().is_empty()) {
                let rink = page.right_inks.get(row).copied().flatten();
                if rink != inked {
                    layer.set_fill_color(colour(rink));
                    inked = rink;
                }
                let rx = LEFT_MARGIN_MM + r.indent as f32 * CHAR_W_MM;
                layer.use_text(&r.text, 12.0, Mm(rx), Mm(y), &courier);
            }
            if line.text.trim().is_empty() {
                continue;
            }
            let x = LEFT_MARGIN_MM + line.indent as f32 * CHAR_W_MM;
            let bold = matches!(line.element, Some(Element::SceneHeading) | Some(Element::Act));
            if opts.scene_numbers {
                if let Some(n) = line.scene {
                    if inked.is_some() {
                        layer.set_fill_color(colour(None));
                        inked = None;
                    }
                    let num = format!("{n}");
                    let w = num.chars().count() as f32 * CHAR_W_MM;
                    layer.use_text(&num, 12.0, Mm(LEFT_MARGIN_MM - 12.7 - w), Mm(y), &courier_bold);
                    let right = LEFT_MARGIN_MM + PAGE_COLS as f32 * CHAR_W_MM + 5.0;
                    layer.use_text(&num, 12.0, Mm(right), Mm(y), &courier_bold);
                }
            }
            if *ink != inked {
                layer.set_fill_color(colour(*ink));
                inked = *ink;
            }
            layer.use_text(
                &line.text,
                12.0,
                Mm(x),
                Mm(y),
                if bold { &courier_bold } else { &courier },
            );
            if line.element == Some(Element::Act) {
                let w = line.text.chars().count() as f32 * CHAR_W_MM;
                let under = y - 0.9;
                layer.set_outline_color(colour(*ink));
                layer.set_outline_thickness(0.6);
                layer.add_line(printpdf::Line {
                    points: vec![
                        (printpdf::Point::new(Mm(x), Mm(under)), false),
                        (printpdf::Point::new(Mm(x + w), Mm(under)), false),
                    ],
                    is_closed: false,
                });
            }
        }
    }

    // endnotes here---
    if opts.notes {
        let rows = endnotes(doc);
        for (k, sheet) in rows.chunks(LINES_PER_PAGE).enumerate() {
            let (page_idx, layer_idx) = next_sheet(format!("Notes {}", k + 1));
            let layer = pdf.get_page(page_idx).get_layer(layer_idx);
            layer.set_fill_color(colour(None));
            for (row, (bold, text)) in sheet.iter().enumerate() {
                if text.trim().is_empty() {
                    continue;
                }
                let y = PAGE_H_MM - TOP_MARGIN_MM - (row as f32 + 1.0) * LINE_H_MM;
                layer.use_text(text, 12.0, Mm(LEFT_MARGIN_MM), Mm(y), if *bold { &courier_bold } else { &courier });
            }
        }
    }

    let mut writer = io::BufWriter::new(Vec::new());
    pdf.save(&mut writer).map_err(|e| e.to_string())?;
    writer.into_inner().map_err(|e| e.to_string())
}

// final draft here---

fn xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn to_fdx(doc: &Document) -> String {
    let mut s = String::new();
    s.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"no\" ?>\n");
    s.push_str("<FinalDraft DocumentType=\"Script\" Template=\"No\" Version=\"5\">\n");
    s.push_str("  <Content>\n");
    let mut act: Option<String> = None;
    let mut act_no = 0usize;
    let end_act = |s: &mut String, title: &str| {
        s.push_str(&format!(
            "    <Paragraph Type=\"End of Act\">\n      <Text>{}</Text>\n    </Paragraph>\n",
            xml(&end_of(title))
        ));
    };
    let pairs = doc.dual_pairs();
    for (i, b) in doc.blocks.iter().enumerate() {
        if pairs.iter().any(|p| p.left == i) {
            s.push_str("    <Paragraph>\n      <DualDialogue>\n");
        }
        if pairs.iter().any(|p| p.end == i) {
            s.push_str("      </DualDialogue>\n    </Paragraph>\n");
        }
        let text = b.text.trim();
        if b.element == Element::Act {
            act_no += 1;
            if let Some(t) = act.take() {
                end_act(&mut s, &t);
            }
            let title = act_heading(text, act_no);
            s.push_str(&format!(
                "    <Paragraph Type=\"New Act\">\n      <Text>{}</Text>\n    </Paragraph>\n",
                xml(&title)
            ));
            act = Some(title);
            continue;
        }
        if text.is_empty() {
            continue;
        }
        let kind = match b.element {
            Element::SceneHeading => "Scene Heading",
            Element::Action => "Action",
            Element::Character => "Character",
            Element::Parenthetical => "Parenthetical",
            Element::Dialogue => "Dialogue",
            Element::Transition => "Transition",
            Element::Shot => "Shot",
            Element::Act => "New Act",
        };
        let body = if b.element == Element::Parenthetical {
            format!("({})", text.trim_matches(|c| c == '(' || c == ')').trim())
        } else if b.element.is_upper() {
            text.to_uppercase()
        } else {
            text.to_string()
        };
        s.push_str(&format!("    <Paragraph Type=\"{kind}\">\n"));
        if b.element == Element::SceneHeading && !b.note.trim().is_empty() {
            s.push_str(&format!(
                "      <SceneProperties><Summary><Paragraph><Text>{}</Text></Paragraph></Summary></SceneProperties>\n",
                xml(b.note.trim())
            ));
        }
        s.push_str(&format!("      <Text>{}</Text>\n", xml(&body)));
        s.push_str("    </Paragraph>\n");
    }
    if pairs.iter().any(|p| p.end == doc.blocks.len()) {
        s.push_str("      </DualDialogue>\n    </Paragraph>\n");
    }
    if let Some(t) = act {
        end_act(&mut s, &t);
    }
    s.push_str("  </Content>\n");
    s.push_str("  <TitlePage>\n    <Content>\n");
    let mut title_line = |text: &str, align: &str| {
        s.push_str(&format!(
            "      <Paragraph Alignment=\"{align}\"><Text>{}</Text></Paragraph>\n",
            xml(text)
        ));
    };
    title_line(&doc.meta.title.trim().to_uppercase(), "Center");
    if !doc.meta.author.trim().is_empty() {
        title_line("written by", "Center");
        title_line(doc.meta.author.trim(), "Center");
    }
    if !doc.meta.draft.trim().is_empty() {
        title_line(doc.meta.draft.trim(), "Center");
    }
    if !doc.meta.contact.trim().is_empty() {
        title_line(doc.meta.contact.trim(), "Left");
    }
    s.push_str("    </Content>\n  </TitlePage>\n");
    s.push_str("</FinalDraft>\n");
    s
}

// entry points ui here---

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Pdf,
    Text,
    Fountain,
    FinalDraft,
}

impl Format {
    fn ext(self) -> &'static str {
        match self {
            Format::Pdf => "pdf",
            Format::Text => "txt",
            Format::Fountain => "fountain",
            Format::FinalDraft => "fdx",
        }
    }
}

#[allow(dead_code)]
#[cfg(not(target_arch = "wasm32"))]
pub fn export(doc: &Document, format: Format) -> Result<PathBuf, String> {
    export_with(doc, format, false)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn export_with(doc: &Document, format: Format, scene_numbers: bool) -> Result<PathBuf, String> {
    export_opts(
        doc,
        format,
        &PdfOptions {
            scene_numbers,
            ..PdfOptions::default()
        },
    )
}

#[cfg(not(target_arch = "wasm32"))]
pub fn export_opts(doc: &Document, format: Format, opts: &PdfOptions) -> Result<PathBuf, String> {
    storage::ensure_dirs().map_err(|e| e.to_string())?;
    let (name, bytes) = render(doc, format, opts)?;
    let path = storage::exports_dir().join(name);
    std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
    Ok(path)
}

pub fn render(doc: &Document, format: Format, opts: &PdfOptions) -> Result<(String, Vec<u8>), String> {
    let part = match (&opts.pages, format) {
        (Some(p), Format::Pdf) => format!("-pages-{}", describe_pages(p).replace(", ", "_")),
        _ => String::new(),
    };
    let name = format!(
        "{}{part}.{}",
        storage::slugify(&doc.meta.title),
        format.ext()
    );
    let bytes = match format {
        Format::Pdf => pdf_bytes(doc, opts)?,
        Format::FinalDraft => to_fdx(doc).into_bytes(),
        Format::Text => to_plain_text(doc).into_bytes(),
        Format::Fountain => to_fountain(doc).into_bytes(),
    };
    Ok((name, bytes))
}
