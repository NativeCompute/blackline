use std::collections::HashMap;

use crate::export;
use crate::model::{base_character, Document, Element};

#[derive(Clone, Debug, PartialEq)]
pub struct Speaker {
    pub name: String,
    pub cues: usize,
    pub lines: usize,
    pub words: usize,
    pub scenes: usize,
    pub first_scene: usize,
    pub first_page: usize,
    pub share: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SceneRow {
    pub id: u64,
    pub number: usize,
    pub heading: String,
    pub side: String,
    pub place: String,
    pub time: String,
    pub page: usize,
    pub eighths: usize,
    pub cast: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Place {
    pub name: String,
    pub sides: Vec<String>,
    pub scenes: usize,
    pub eighths: usize,
    pub first_page: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Time {
    pub name: String,
    pub scenes: usize,
    pub eighths: usize,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Report {
    pub speakers: Vec<Speaker>,
    pub scenes: Vec<SceneRow>,
    pub places: Vec<Place>,
    pub times: Vec<Time>,
    pub pages: usize,
    pub eighths: usize,
    pub words: usize,
}

const SIDES: [(&str, &str); 7] = [
    ("INT./EXT.", "INT./EXT."),
    ("EXT./INT.", "INT./EXT."),
    ("INT/EXT", "INT./EXT."),
    ("I/E.", "INT./EXT."),
    ("INT.", "INT."),
    ("EXT.", "EXT."),
    ("EST.", "EXT."),
];

pub fn split_heading(heading: &str) -> (String, String, String) {
    let h = heading.split_whitespace().collect::<Vec<_>>().join(" ").to_uppercase();
    let mut side = String::new();
    let mut rest = h.as_str();
    for (k, v) in SIDES {
        if let Some(r) = rest.strip_prefix(k) {
            side = v.to_string();
            rest = r.trim_start();
            break;
        }
    }
    let (place, time) = match rest.rfind(" - ") {
        Some(at) => (rest[..at].trim(), rest[at + 3..].trim()),
        None => (rest.trim(), ""),
    };
    (side, place.trim_end_matches(['-', ' ']).to_string(), time.to_string())
}

pub fn build(doc: &Document) -> Report {
    let pages = export::paginate(&export::compose(doc));
    let mut page_of: HashMap<u64, usize> = HashMap::new();
    for (k, page) in pages.iter().enumerate() {
        for l in page {
            for id in [l.block, l.right.as_ref().and_then(|r| r.block)].into_iter().flatten() {
                page_of.entry(id).or_insert(k + 1);
            }
        }
    }
    let lengths: HashMap<u64, (usize, usize)> = export::scene_lengths(doc).into_iter().map(|(id, p, e)| (id, (p, e))).collect();

    let mut scenes = Vec::new();
    for s in doc.scenes() {
        let (side, place, time) = split_heading(&s.heading);
        let (page, eighths) = lengths.get(&s.id).copied().unwrap_or((0, 0));
        scenes.push(SceneRow { id: s.id, number: s.number, heading: s.heading.trim().to_string(), side, place, time, page, eighths, cast: s.cast.clone() });
    }

    let mut speakers: Vec<Speaker> = Vec::new();
    let mut current: Option<usize> = None;
    let mut scene_no = 0usize;
    let mut seen: Vec<(usize, usize)> = Vec::new();
    for b in &doc.blocks {
        match b.element {
            Element::SceneHeading => {
                scene_no += 1;
                current = None;
            }
            Element::Character => {
                let name = base_character(&b.text);
                if name.is_empty() {
                    current = None;
                    continue;
                }
                let k = match speakers.iter().position(|s| s.name == name) {
                    Some(k) => k,
                    None => {
                        speakers.push(Speaker {
                            name,
                            cues: 0,
                            lines: 0,
                            words: 0,
                            scenes: 0,
                            first_scene: scene_no,
                            first_page: page_of.get(&b.id).copied().unwrap_or(0),
                            share: 0.0,
                        });
                        speakers.len() - 1
                    }
                };
                speakers[k].cues += 1;
                if !seen.contains(&(k, scene_no)) {
                    seen.push((k, scene_no));
                    speakers[k].scenes += 1;
                }
                current = Some(k);
            }
            Element::Dialogue => {
                if let Some(k) = current {
                    speakers[k].lines += 1;
                    speakers[k].words += b.text.split_whitespace().count();
                }
            }
            Element::Parenthetical => {}
            _ => current = None,
        }
    }
    let spoken: usize = speakers.iter().map(|s| s.words).sum();
    for s in &mut speakers {
        s.share = if spoken == 0 { 0.0 } else { s.words as f32 / spoken as f32 };
    }
    speakers.sort_by(|a, b| b.words.cmp(&a.words).then(b.cues.cmp(&a.cues)).then(a.name.cmp(&b.name)));

    let mut places: Vec<Place> = Vec::new();
    let mut times: Vec<Time> = Vec::new();
    for s in &scenes {
        let name = if s.place.is_empty() { "(no location)".to_string() } else { s.place.clone() };
        let k = match places.iter().position(|p| p.name == name) {
            Some(k) => k,
            None => {
                places.push(Place { name, sides: Vec::new(), scenes: 0, eighths: 0, first_page: s.page });
                places.len() - 1
            }
        };
        places[k].scenes += 1;
        places[k].eighths += s.eighths;
        if !s.side.is_empty() && !places[k].sides.contains(&s.side) {
            places[k].sides.push(s.side.clone());
        }
        let t = if s.time.is_empty() { "(not given)".to_string() } else { s.time.clone() };
        let k = match times.iter().position(|x| x.name == t) {
            Some(k) => k,
            None => {
                times.push(Time { name: t, scenes: 0, eighths: 0 });
                times.len() - 1
            }
        };
        times[k].scenes += 1;
        times[k].eighths += s.eighths;
    }
    places.sort_by(|a, b| b.eighths.cmp(&a.eighths).then(b.scenes.cmp(&a.scenes)).then(a.name.cmp(&b.name)));
    times.sort_by(|a, b| b.eighths.cmp(&a.eighths).then(a.name.cmp(&b.name)));

    Report {
        eighths: scenes.iter().map(|s| s.eighths).sum(),
        words: doc.blocks.iter().map(|b| b.text.split_whitespace().count()).sum(),
        pages: pages.len().max(1),
        speakers,
        scenes,
        places,
        times,
    }
}

fn table(head: &[&str], rows: Vec<Vec<String>>) -> String {
    let mut widths: Vec<usize> = head.iter().map(|h| h.chars().count()).collect();
    for r in &rows {
        for (k, c) in r.iter().enumerate() {
            widths[k] = widths[k].max(c.chars().count());
        }
    }
    let line = |cells: Vec<String>| {
        let last = cells.len() - 1;
        cells
            .into_iter()
            .enumerate()
            .map(|(k, c)| if k == last { c } else { format!("{c:<w$}", w = widths[k]) })
            .collect::<Vec<_>>()
            .join("  ")
            .trim_end()
            .to_string()
    };
    let mut out = vec![line(head.iter().map(|h| h.to_string()).collect())];
    out.extend(rows.into_iter().map(line));
    out.join("\n")
}

impl Report {
    pub fn text(&self, tab: usize, title: &str) -> String {
        let e = crate::model::eighths_label;
        let (name, body) = match tab {
            0 => (
                "Characters",
                table(
                    &["CHARACTER", "CUES", "LINES", "WORDS", "SHARE", "SCENES", "FIRST"],
                    self.speakers
                        .iter()
                        .map(|s| {
                            vec![
                                s.name.clone(),
                                s.cues.to_string(),
                                s.lines.to_string(),
                                s.words.to_string(),
                                format!("{:.0}%", s.share * 100.0),
                                s.scenes.to_string(),
                                format!("sc. {} · p. {}", s.first_scene, s.first_page),
                            ]
                        })
                        .collect(),
                ),
            ),
            1 => (
                "Scenes",
                table(
                    &["#", "HEADING", "PAGE", "LENGTH", "CAST"],
                    self.scenes
                        .iter()
                        .map(|s| vec![s.number.to_string(), s.heading.clone(), s.page.to_string(), e(s.eighths), s.cast.join(", ")])
                        .collect(),
                ),
            ),
            2 => (
                "Locations",
                table(
                    &["LOCATION", "INT/EXT", "SCENES", "LENGTH", "FIRST PAGE"],
                    self.places
                        .iter()
                        .map(|p| vec![p.name.clone(), p.sides.join(" "), p.scenes.to_string(), e(p.eighths), p.first_page.to_string()])
                        .collect(),
                ),
            ),
            _ => (
                "Day and night",
                table(
                    &["TIME", "SCENES", "LENGTH"],
                    self.times.iter().map(|t| vec![t.name.clone(), t.scenes.to_string(), e(t.eighths)]).collect(),
                ),
            ),
        };
        format!("{title} · {name}\n\n{body}\n")
    }
}
