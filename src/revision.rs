use std::collections::{HashMap, HashSet};

use crate::model::Document;

pub const COLOURS: [&str; 8] = ["Blue", "Pink", "Yellow", "Green", "Goldenrod", "Buff", "Salmon", "Cherry"];

pub const PAPER: [[u8; 3]; 8] = [
    [0xD6, 0xE6, 0xFA],
    [0xFA, 0xDD, 0xE6],
    [0xFA, 0xF4, 0xC8],
    [0xD9, 0xF2, 0xD6],
    [0xF2, 0xDC, 0xA0],
    [0xF1, 0xE6, 0xCF],
    [0xFA, 0xD3, 0xC0],
    [0xF2, 0xC4, 0xCC],
];

pub fn name(index: usize) -> String {
    let base = COLOURS[index % COLOURS.len()];
    match index / COLOURS.len() {
        0 => base.to_string(),
        1 => format!("Double {base}"),
        2 => format!("Triple {base}"),
        n => format!("{base} {}", n + 1),
    }
}

pub fn slug(index: usize) -> String {
    name(index).to_lowercase().replace(' ', "-")
}

pub fn from_slug(s: &str) -> Option<usize> {
    let s = s.trim().to_lowercase();
    (0..COLOURS.len() * 4).find(|i| slug(*i) == s)
}

pub fn paper(index: usize) -> [u8; 3] {
    PAPER[index % PAPER.len()]
}


fn key(b: &crate::model::Block) -> Key {
    let text = b.text.split_whitespace().collect::<Vec<_>>().join(" ");
    (b.element, text, b.dual)
}

pub fn header(index: usize, date: &str) -> String {
    let mut parts = date.trim().split('-');
    match (parts.next(), parts.next(), parts.next()) {
        (Some(y), Some(m), Some(d)) if y.len() == 4 => format!("{} Rev. {m}/{d}/{}", name(index), &y[2..]),
        _ => format!("{} Rev.", name(index)),
    }
}

enum Step {
    Same(usize, usize),
    Added(usize),
    Dropped,
}

fn align(a: &[Key], b: &[Key]) -> Vec<Step> {
    let mut head = 0;
    while head < a.len() && head < b.len() && a[head] == b[head] {
        head += 1;
    }
    let mut tail = 0;
    while tail < a.len() - head && tail < b.len() - head && a[a.len() - 1 - tail] == b[b.len() - 1 - tail] {
        tail += 1;
    }
    let (am, bm) = (&a[head..a.len() - tail], &b[head..b.len() - tail]);
    let mut out: Vec<Step> = (0..head).map(|k| Step::Same(k, k)).collect();
    let (n, m) = (am.len(), bm.len());
    let mut table = vec![0u32; (n + 1) * (m + 1)];
    let at = |i: usize, j: usize| i * (m + 1) + j;
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            table[at(i, j)] = if am[i] == bm[j] {
                table[at(i + 1, j + 1)] + 1
            } else {
                table[at(i + 1, j)].max(table[at(i, j + 1)])
            };
        }
    }
    let (mut i, mut j) = (0, 0);
    while i < n || j < m {
        if i < n && j < m && am[i] == bm[j] {
            out.push(Step::Same(head + i, head + j));
            i += 1;
            j += 1;
        } else if j < m && (i == n || table[at(i, j + 1)] >= table[at(i + 1, j)]) {
            out.push(Step::Added(head + j));
            j += 1;
        } else {
            out.push(Step::Dropped);
            i += 1;
        }
    }
    out.extend((0..tail).map(|k| Step::Same(a.len() - tail + k, b.len() - tail + k)));
    out
}

type Key = (crate::model::Element, String, bool);

fn live(doc: &Document) -> Vec<&crate::model::Block> {
    doc.blocks.iter().filter(|b| !b.text.trim().is_empty()).collect()
}

pub fn pairs(base: &Document, now: &Document) -> HashMap<u64, u64> {
    let (old, new) = (live(base), live(now));
    let a: Vec<Key> = old.iter().map(|b| key(b)).collect();
    let b: Vec<Key> = new.iter().map(|b| key(b)).collect();
    align(&a, &b)
        .into_iter()
        .filter_map(|s| match s {
            Step::Same(i, j) => Some((old[i].id, new[j].id)),
            _ => None,
        })
        .collect()
}

pub fn changed(base: &Document, now: &Document) -> HashSet<u64> {
    let (old, new) = (live(base), live(now));
    let a: Vec<Key> = old.iter().map(|b| key(b)).collect();
    let b: Vec<Key> = new.iter().map(|b| key(b)).collect();
    let mut out = HashSet::new();
    let (mut dropped, mut added) = (false, false);
    let mut after_last: usize = 0;
    for s in align(&a, &b) {
        match s {
            Step::Same(_, j) => {
                if dropped && !added {
                    out.insert(new[j].id);
                }
                dropped = false;
                added = false;
                after_last = j + 1;
            }
            Step::Added(j) => {
                out.insert(new[j].id);
                added = true;
                after_last = j + 1;
            }
            Step::Dropped => dropped = true,
        }
    }
    if dropped && !added {
        if let Some(next) = new.get(after_last) {
            out.insert(next.id);
        } else if let Some(prev) = after_last.checked_sub(1).and_then(|k| new.get(k)) {
            out.insert(prev.id);
        }
    }
    out
}
