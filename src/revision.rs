use std::collections::HashSet;

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


fn key(b: &crate::model::Block) -> (crate::model::Element, String, bool) {
    let text = b.text.split_whitespace().collect::<Vec<_>>().join(" ");
    (b.element, text, b.dual)
}

pub fn changed(base: &Document, now: &Document) -> HashSet<u64> {
    let a: Vec<_> = base.blocks.iter().filter(|b| !b.text.trim().is_empty()).map(key).collect();
    let live: Vec<&crate::model::Block> = now.blocks.iter().filter(|b| !b.text.trim().is_empty()).collect();
    let b: Vec<_> = live.iter().map(|x| key(x)).collect();

    let mut head = 0;
    while head < a.len() && head < b.len() && a[head] == b[head] {
        head += 1;
    }
    let mut tail = 0;
    while tail < a.len() - head && tail < b.len() - head && a[a.len() - 1 - tail] == b[b.len() - 1 - tail] {
        tail += 1;
    }
    let (am, bm) = (&a[head..a.len() - tail], &b[head..b.len() - tail]);
    let mut out = HashSet::new();
    if am.is_empty() && bm.is_empty() {
        return out;
    }
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
    let (mut dropped, mut added) = (false, false);
    while i < n || j < m {
        if i < n && j < m && am[i] == bm[j] {
            if dropped && !added {
                out.insert(live[head + j].id);
            }
            dropped = false;
            added = false;
            i += 1;
            j += 1;
        } else if j < m && (i == n || table[at(i, j + 1)] >= table[at(i + 1, j)]) {
            out.insert(live[head + j].id);
            added = true;
            j += 1;
        } else {
            dropped = true;
            i += 1;
        }
    }
    if dropped && !added {
        if let Some(next) = live.get(head + m) {
            out.insert(next.id);
        } else if let Some(prev) = (head + m).checked_sub(1).and_then(|k| live.get(k)) {
            out.insert(prev.id);
        }
    }
    out
}
