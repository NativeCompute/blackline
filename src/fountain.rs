use crate::model::{Document, Element, Meta};

#[derive(Clone, Debug, PartialEq)]
pub struct Issue {
    pub what: String,
    pub count: usize,
    pub at: Option<usize>,
}

#[derive(Default)]
struct Issues(Vec<(String, Issue)>);

impl Issues {
    fn add(&mut self, key: &str, what: &str, at: Option<usize>) {
        self.add_many(key, what, at, 1);
    }

    fn add_many(&mut self, key: &str, what: &str, at: Option<usize>, n: usize) {
        if n == 0 {
            return;
        }
        match self.0.iter_mut().find(|(k, _)| k == key) {
            Some((_, i)) => {
                i.count += n;
                if i.at.is_none() {
                    i.at = at;
                }
            }
            None => self.0.push((key.to_string(), Issue { what: what.to_string(), count: n, at })),
        }
    }

    fn done(self) -> Vec<Issue> {
        self.0.into_iter().map(|(_, i)| i).collect()
    }
}

const NOTE_OPEN: char = '\u{E000}';
const NOTE_CLOSE: char = '\u{E001}';

fn mark_notes(line: &str) -> String {
    let mut out = String::new();
    let mut rest = line;
    while let Some(a) = rest.find("[[") {
        match rest[a + 2..].find("]]") {
            Some(b) => {
                out.push_str(&rest[..a]);
                out.push(NOTE_OPEN);
                out.push_str(&rest[a + 2..a + 2 + b]);
                out.push(NOTE_CLOSE);
                rest = &rest[a + 2 + b + 2..];
            }
            None => break,
        }
    }
    out.push_str(rest);
    out
}

fn take_notes(text: &str) -> (String, Vec<String>) {
    let mut clean = String::new();
    let mut notes = Vec::new();
    let mut rest = text;
    while let Some(a) = rest.find(NOTE_OPEN) {
        clean.push_str(&rest[..a]);
        let after = &rest[a + NOTE_OPEN.len_utf8()..];
        let b = after.find(NOTE_CLOSE).unwrap_or(after.len());
        let note = after[..b].trim();
        if !note.is_empty() {
            notes.push(note.to_string());
        }
        rest = after.get(b + NOTE_CLOSE.len_utf8()..).unwrap_or("");
    }
    clean.push_str(rest);
    let clean = clean.split_whitespace().collect::<Vec<_>>().join(" ");
    (clean, notes)
}

fn settle_notes(doc: &mut Document) {
    let mut k = 0;
    while k < doc.blocks.len() {
        let (text, notes) = take_notes(&doc.blocks[k].text);
        let (synopsis, more) = take_notes(&doc.blocks[k].note);
        let notes: Vec<String> = notes.into_iter().chain(more).collect();
        let had_markers = text != doc.blocks[k].text;
        doc.blocks[k].note = synopsis;
        if notes.is_empty() {
            if had_markers {
                doc.blocks[k].text = text;
            }
            k += 1;
            continue;
        }
        let joined = notes.join(" / ");
        if text.is_empty() && k > 0 {
            let prev = &mut doc.blocks[k - 1];
            prev.remark = if prev.remark.is_empty() { joined } else { format!("{} / {joined}", prev.remark) };
            doc.blocks.remove(k);
            continue;
        }
        doc.blocks[k].text = text;
        doc.blocks[k].remark = joined;
        k += 1;
    }
    for field in [&mut doc.meta.title, &mut doc.meta.author, &mut doc.meta.draft, &mut doc.meta.contact] {
        *field = take_notes(field).0;
    }
}

fn attach_notes(doc: &mut Document, pending: &mut Vec<String>) {
    if pending.is_empty() {
        return;
    }
    let joined = std::mem::take(pending).join(" / ");
    if let Some(last) = doc.blocks.last_mut() {
        last.remark = if last.remark.is_empty() { joined } else { format!("{} / {joined}", last.remark) };
    }
}

pub fn parse(text: &str) -> Document {
    parse_checked(text).0
}

fn strip_boneyard(s: &str, issues: &mut Issues) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(a) = rest.find("/*") {
        out.push_str(&rest[..a]);
        match rest[a + 2..].find("*/") {
            Some(b) => {
                issues.add("boneyard", "Boneyard text (/* … */) left out, as Fountain intends", None);
                rest = &rest[a + 2 + b + 2..];
            }
            None => {
                issues.add("unclosed", "An unclosed boneyard (/*) would have hidden the rest of the file, so it was ignored", None);
                out.push_str(&rest[a + 2..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

pub fn parse_checked(text: &str) -> (Document, Vec<Issue>) {
    let mut issues = Issues::default();
    let marked: Vec<String> = text.lines().map(mark_notes).collect();
    let text = strip_between(&strip_boneyard(&marked.join("\n"), &mut issues), "[[", "]]");
    let lines: Vec<&str> = text.lines().collect();

    let (meta, body_from) = title_page(&lines, &mut issues);
    let mut doc = Document::from_parts(meta, Vec::new());
    doc.blocks.clear();

    let mut i = body_from;
    let mut prev_blank = true;
    let mut speaking = false;
    let mut pending: Vec<String> = Vec::new();

    while i < lines.len() {
        attach_notes(&mut doc, &mut pending);
        let (bare, notes) = take_notes(lines[i].trim());
        let line = bare.as_str();
        let next_blank = lines.get(i + 1).map(|l| take_notes(l.trim()).0.is_empty()).unwrap_or(true);
        i += 1;
        if line.is_empty() && !notes.is_empty() {
            pending = notes;
            continue;
        }
        pending = notes;

        if line.is_empty() {
            prev_blank = true;
            speaking = false;
            continue;
        }

        if speaking {
            if line.starts_with('(') && line.ends_with(')') {
                doc.push(Element::Parenthetical, line);
            } else {
                match doc.blocks.last_mut() {
                    Some(b) if b.element == Element::Dialogue => {
                        b.text.push(' ');
                        b.text.push_str(line);
                    }
                    _ => doc.push(Element::Dialogue, line),
                }
            }
            prev_blank = false;
            continue;
        }

        if let Some(rest) = line.strip_prefix('=') {
            if rest.starts_with("==") {
                issues.add("break", "Page breaks (===) left out; Blackline sets its own pages", Some(doc.blocks.len()));
            } else if !doc.blocks.iter().any(|b| b.element == Element::SceneHeading) {
                issues.add("synopsis", "A synopsis (=) before the first scene had no card to go on", None);
            }
            if !rest.starts_with("==") {
                let note = rest.trim().to_string();
                if let Some(h) = doc
                    .blocks
                    .iter_mut()
                    .rev()
                    .find(|b| b.element == Element::SceneHeading)
                {
                    if h.note.is_empty() {
                        h.note = note;
                    } else {
                        h.note.push(' ');
                        h.note.push_str(&note);
                    }
                }
            }
            prev_blank = false;
            continue;
        }
        if let Some(rest) = line.strip_prefix('#') {
            if !rest.starts_with('#') {
                push_act(&mut doc, rest);
            } else {
                issues.add("section", "Section headings below the first level (##) left out; Blackline has acts only", Some(doc.blocks.len()));
            }
            prev_blank = false;
            continue;
        }
        if line.chars().all(|c| c == '=') {
            issues.add("break", "Page breaks (===) left out; Blackline sets its own pages", Some(doc.blocks.len()));
            prev_blank = false;
            continue;
        }

        if let Some(rest) = line.strip_prefix('!') {
            doc.push(Element::Action, rest.trim());
            prev_blank = false;
            continue;
        }
        if line.starts_with('.') && !line.starts_with("..") && line.len() > 1 {
            note_scene_number(&line[1..], &mut issues, doc.blocks.len());
            doc.push(Element::SceneHeading, &clean_heading(&line[1..]));
            prev_blank = false;
            continue;
        }
        if let Some(rest) = line.strip_prefix('>') {
            let rest = rest.trim();
            if let Some(inner) = rest.strip_suffix('<') {
                let bare = inner.trim().trim_matches(|c| c == '*' || c == '_').trim().to_uppercase();
                let in_act = doc.blocks.iter().any(|b| b.element == Element::Act);
                let same = doc.blocks.last().map(|b| b.element == Element::Act && b.text == bare).unwrap_or(false);
                if is_act_title(&bare) || same {
                    push_act(&mut doc, &bare);
                } else if !(in_act && bare.starts_with("END OF ")) {
                    issues.add("centred", "Centred text (> <) became action", Some(doc.blocks.len()));
                    doc.push(Element::Action, inner.trim());
                }
            } else {
                doc.push(Element::Transition, &rest.to_uppercase());
            }
            prev_blank = false;
            continue;
        }
        if let Some(rest) = line.strip_prefix('@') {
            push_cue(&mut doc, &rest.trim().to_uppercase());
            speaking = true;
            prev_blank = false;
            continue;
        }

        let upper = line.to_uppercase();
        if prev_blank && is_heading(&upper) {
            note_scene_number(line, &mut issues, doc.blocks.len());
            doc.push(Element::SceneHeading, &clean_heading(line));
        } else if prev_blank && next_blank && upper == line && line.ends_with("TO:") {
            doc.push(Element::Transition, line);
        } else if prev_blank
            && !next_blank
            && upper == line
            && line.chars().any(|c| c.is_alphabetic())
        {
            push_cue(&mut doc, line);
            speaking = true;
        } else {
            if line.starts_with('~') {
                issues.add("lyric", "Lyrics (~) became action", Some(doc.blocks.len()));
            }
            let stripped = line.trim_start_matches('~').trim();
            doc.push(Element::Action, stripped);
        }
        prev_blank = false;
    }

    attach_notes(&mut doc, &mut pending);
    settle_notes(&mut doc);
    if doc.blocks.is_empty() && !text.trim().is_empty() {
        issues.add("empty", "Nothing in the file read as a script", None);
    }
    doc.ensure_not_empty();
    doc.reseed_ids();
    (doc, issues.done())
}

fn note_scene_number(line: &str, issues: &mut Issues, at: usize) {
    let t = line.trim();
    if t.strip_suffix('#').and_then(|head| head.rfind('#')).is_some() {
        issues.add("number", "Scene numbers (#12#) left out; Blackline numbers scenes for you", Some(at));
    }
}

fn push_cue(doc: &mut Document, cue: &str) {
    let dual = cue.trim_end().ends_with('^');
    doc.push(Element::Character, cue.trim().trim_end_matches('^').trim());
    if let Some(last) = doc.blocks.last_mut() {
        last.dual = dual;
    }
}

fn is_act_title(upper: &str) -> bool {
    let after_act = upper.strip_prefix("ACT ").map(str::trim).unwrap_or("");
    (!after_act.is_empty() && !after_act.contains(' '))
        || matches!(upper, "TEASER" | "COLD OPEN" | "PROLOGUE" | "EPILOGUE")
}

fn push_act(doc: &mut Document, title: &str) {
    let title = title.trim().trim_matches(|c| c == '*' || c == '_').trim().to_uppercase();
    if let Some(last) = doc.blocks.last_mut() {
        if last.element == Element::Act && (last.text == title || last.text.is_empty()) {
            last.text = title;
            return;
        }
    }
    doc.push(Element::Act, &title);
}

fn is_heading(upper: &str) -> bool {
    const PREFIXES: [&str; 10] = [
        "INT.", "EXT.", "EST.", "INT./EXT.", "INT/EXT", "I/E.", "I/E ", "INT ", "EXT ", "EST ",
    ];
    PREFIXES.iter().any(|p| upper.starts_with(p))
}

fn clean_heading(s: &str) -> String {
    let t = s.trim();
    let t = match t.strip_suffix('#').and_then(|head| head.rfind('#')) {
        Some(k) => t[..k].trim(),
        None => t,
    };
    t.to_uppercase()
}

fn strip_between(s: &str, open: &str, close: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(a) = rest.find(open) {
        out.push_str(&rest[..a]);
        match rest[a + open.len()..].find(close) {
            Some(b) => rest = &rest[a + open.len() + b + close.len()..],
            None => {
                rest = "";
                break;
            }
        }
    }
    out.push_str(rest);
    out
}

fn title_page(lines: &[&str], issues: &mut Issues) -> (Meta, usize) {
    let mut meta = Meta::default();
    let first = lines.iter().position(|l| !l.trim().is_empty());
    let Some(first) = first else {
        return (meta, 0);
    };
    let looks_like_key = |l: &str| {
        l.split_once(':')
            .map(|(k, _)| {
                let k = k.trim();
                !k.is_empty()
                    && !l.starts_with(' ')
                    && !l.starts_with('\t')
                    && k.chars().all(|c| c.is_alphabetic() || c == ' ')
                    && k.len() < 24
            })
            .unwrap_or(false)
    };
    if !looks_like_key(lines[first]) {
        return (meta, 0);
    }
    let mut key = String::new();
    let mut i = first;
    while i < lines.len() && !lines[i].trim().is_empty() {
        let l = lines[i];
        let (k, v) = if looks_like_key(l) {
            let (k, v) = l.split_once(':').unwrap_or((l, ""));
            key = k.trim().to_lowercase();
            (key.clone(), v.trim().to_string())
        } else {
            (key.clone(), l.trim().to_string())
        };
        let target = match k.as_str() {
            "title" => Some(&mut meta.title),
            "author" | "authors" => Some(&mut meta.author),
            "draft date" | "draft" | "date" => Some(&mut meta.draft),
            "contact" => Some(&mut meta.contact),
            "credit" | "source" => None,
            other => {
                if !v.is_empty() {
                    let name = title_case(&other.to_uppercase());
                    issues.add(&format!("title:{other}"), &format!("The title page's \u{201c}{name}\u{201d} has no place in Blackline"), None);
                }
                None
            }
        };
        if let Some(t) = target {
            if !v.is_empty() {
                if !t.is_empty() {
                    t.push_str(", ");
                }
                t.push_str(v.trim_matches(|c| c == '*' || c == '_'));
            }
        }
        i += 1;
    }
    (meta, i)
}

// final draft here---

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(a) = rest.find('&') {
        out.push_str(&rest[..a]);
        let after = &rest[a..];
        let end = after.find(';').filter(|e| *e <= 10);
        let decoded = end.and_then(|e| {
            let name = &after[1..e];
            let code = match name {
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                "amp" => Some('&'),
                "nbsp" => Some(' '),
                _ => match name.strip_prefix("#x").or_else(|| name.strip_prefix("#X")) {
                    Some(hex) => u32::from_str_radix(hex, 16).ok().and_then(char::from_u32),
                    None => name.strip_prefix('#').and_then(|d| d.parse::<u32>().ok()).and_then(char::from_u32),
                },
            };
            code.map(|c| (c, e))
        });
        match decoded {
            Some((c, e)) => {
                out.push(c);
                rest = &after[e + 1..];
            }
            None => {
                out.push('&');
                rest = &after[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn texts(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(a) = rest.find("<Text") {
        let after = &rest[a..];
        let Some(open_end) = after.find('>') else { break };
        if after[..open_end].ends_with('/') {
            rest = &after[open_end + 1..];
            continue;
        }
        let body = &after[open_end + 1..];
        let Some(close) = body.find("</Text>") else { break };
        out.push_str(&unescape(&body[..close]));
        rest = &body[close + 7..];
    }
    out
}

fn attr(tag: &str, name: &str) -> Option<String> {
    let key = format!("{name}=\"");
    let a = tag.find(&key)? + key.len();
    let b = tag[a..].find('"')? + a;
    Some(tag[a..b].to_string())
}

pub fn parse_fdx(text: &str) -> Document {
    parse_fdx_checked(text).0
}

pub fn parse_fdx_checked(text: &str) -> (Document, Vec<Issue>) {
    let mut issues = Issues::default();
    let mut doc = Document::from_parts(Meta::default(), Vec::new());
    doc.blocks.clear();

    let content = section(text, "<Content>", "</Content>").unwrap_or("");
    if content.trim().is_empty() {
        issues.add("empty", "No script body (<Content>) was found in this file", None);
    }
    issues.add_many("notes", "Final Draft script notes were not brought over", None, text.matches("<ScriptNote ").count() + text.matches("<ScriptNote>").count());
    let mut rest = content;
    let mut in_dual = false;
    let mut cues_in_dual = 0usize;
    while let Some(a) = rest.find("<Paragraph") {
        if in_dual {
            if let Some(d) = rest.find("</DualDialogue>").filter(|d| *d < a) {
                in_dual = false;
                cues_in_dual = 0;
                rest = &rest[d + "</DualDialogue>".len()..];
                continue;
            }
        }
        let after = &rest[a..];
        let Some(open_end) = after.find('>') else { break };
        let tag = &after[..open_end];
        let body_start = &after[open_end + 1..];
        if let Some(inner) = body_start.trim_start().strip_prefix("<DualDialogue") {
            in_dual = true;
            cues_in_dual = 0;
            rest = inner.find('>').map(|k| &inner[k + 1..]).unwrap_or("");
            continue;
        }
        let (summary, body_from) = match (body_start.find("<SceneProperties"), body_start.find("</Paragraph>")) {
            (Some(sp), Some(end)) if sp < end => match body_start.find("</SceneProperties>") {
                Some(spe) => (
                    Some(texts(&body_start[sp..spe])),
                    &body_start[spe + "</SceneProperties>".len()..],
                ),
                None => (None, body_start),
            },
            _ => (None, body_start),
        };
        let Some(end) = body_from.find("</Paragraph>") else { break };
        let words = texts(&body_from[..end]);
        rest = &body_from[end + "</Paragraph>".len()..];

        let kind = attr(tag, "Type").unwrap_or_default();
        if kind == "End of Act" {
            continue;
        }
        let element = match kind.as_str() {
            "Scene Heading" => Element::SceneHeading,
            "Character" => Element::Character,
            "Parenthetical" => Element::Parenthetical,
            "Dialogue" => Element::Dialogue,
            "Transition" => Element::Transition,
            "Shot" => Element::Shot,
            "New Act" => Element::Act,
            _ => Element::Action,
        };
        let words = words.trim();
        if words.is_empty() && summary.is_none() {
            continue;
        }
        if element == Element::Action && kind != "Action" {
            let name = if kind.is_empty() { "untyped".to_string() } else { kind.clone() };
            issues.add(&format!("kind:{name}"), &format!("\u{201c}{name}\u{201d} paragraphs became action"), Some(doc.blocks.len()));
        }
        if in_dual && element == Element::Character && cues_in_dual >= 2 {
            issues.add("dual3", "Dual dialogue with more than two speakers was kept as two", Some(doc.blocks.len()));
        }
        let text = if element.is_upper() {
            words.to_uppercase()
        } else {
            words.to_string()
        };
        doc.push(element, &text);
        if in_dual && element == Element::Character {
            cues_in_dual += 1;
            if let Some(last) = doc.blocks.last_mut() {
                last.dual = cues_in_dual >= 2;
            }
        }
        if let (Some(sum), Some(last)) = (summary, doc.blocks.last_mut()) {
            if last.element == Element::SceneHeading {
                last.note = sum.trim().to_string();
            }
        }
    }

    if let Some(tp) = section(text, "<TitlePage>", "</TitlePage>") {
        let lines: Vec<String> = tp
            .split("</Paragraph>")
            .map(texts)
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .collect();
        if let Some(t) = lines.first() {
            doc.meta.title = title_case(t);
        }
        if let Some(k) = lines
            .iter()
            .position(|l| matches!(l.to_lowercase().as_str(), "written by" | "by"))
        {
            if let Some(a) = lines.get(k + 1) {
                doc.meta.author = a.clone();
            }
        }
    }

    doc.ensure_not_empty();
    doc.reseed_ids();
    (doc, issues.done())
}

fn section<'a>(s: &'a str, open: &str, close: &str) -> Option<&'a str> {
    let a = s.find(open)? + open.len();
    let b = s[a..].find(close)? + a;
    Some(&s[a..b])
}

fn title_case(s: &str) -> String {
    if s.to_uppercase() != s {
        return s.to_string();
    }
    s.split_whitespace()
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + &c.as_str().to_lowercase(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
