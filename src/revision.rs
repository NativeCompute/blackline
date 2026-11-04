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
