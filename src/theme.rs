#![allow(dead_code)]

use std::sync::RwLock;

use crate::model::Element;

use eframe::egui::{
    self, Color32, FontData, FontDefinitions, FontFamily, Margin, Mesh, Pos2, Rect, Rounding,
    Shape, Stroke, Vec2,
};

// palette here---

#[derive(Clone, Copy)]
pub struct Palette {
    pub dark: bool,

    pub backdrop: Color32,

    pub glass: Color32,
    pub glass_hi: Color32,
    pub glass_line: Color32,
    pub glass_edge: Color32,

    pub solid: Color32,
    pub solid_hi: Color32,
    pub sunken: Color32,
    pub raised: Color32,

    pub line: Color32,
    pub line_strong: Color32,

    pub text: Color32,
    pub text_dim: Color32,
    pub text_faint: Color32,
    pub ink: Color32,

    pub primary: Color32,
    pub primary_light: Color32,
    pub primary_deep: Color32,
    pub primary_quiet: Color32,
    pub primary_quiet_hi: Color32,
    pub prim_grad: (Color32, Color32),

    pub sec: Color32,
    pub sec_light: Color32,
    pub sec_deep: Color32,
    pub sec_grad: (Color32, Color32),

    pub danger: Color32,
    pub danger_light: Color32,
    pub warn: Color32,
    pub ok: Color32,

    pub groups: [Color32; 6],
}

const fn c(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

impl Palette {
    pub fn group(&self, hue: usize) -> Color32 {
        self.groups[hue % self.groups.len()]
    }
}

pub fn palette(dark: bool) -> Palette {
    let a = |base: u8, alpha: u8| Color32::from_rgba_unmultiplied(base, base, base, alpha);
    if dark {
        Palette {
            dark,
            backdrop: c(0x00, 0x00, 0x00),
            glass: c(0x07, 0x08, 0x0D),
            glass_hi: c(0x13, 0x16, 0x24),
            glass_line: a(255, 31),
            glass_edge: a(255, 66),
            solid: c(0x07, 0x08, 0x0D),
            solid_hi: c(0x0C, 0x0E, 0x16),
            sunken: c(0x00, 0x00, 0x00),
            raised: c(0x13, 0x16, 0x24),
            line: a(255, 31),
            line_strong: a(255, 66),
            text: c(0xF2, 0xF3, 0xF7),
            text_dim: c(0xA6, 0xAB, 0xC2),
            text_faint: c(0x8A, 0x8F, 0xA8),
            ink: c(0xFF, 0xFF, 0xFF),
            primary: c(0x3B, 0x4C, 0xFF),
            primary_light: c(0x6E, 0x7B, 0xFF),
            primary_deep: c(0x2A, 0x36, 0xC8),
            primary_quiet: Color32::from_rgba_unmultiplied(0x3B, 0x4C, 0xFF, 36),
            primary_quiet_hi: Color32::from_rgba_unmultiplied(0x3B, 0x4C, 0xFF, 56),
            prim_grad: (c(0x3B, 0x4C, 0xFF), c(0x3B, 0x4C, 0xFF)),
            sec: c(0xC9, 0xCD, 0xDB),
            sec_light: c(0xF2, 0xF3, 0xF7),
            sec_deep: c(0x8A, 0x8F, 0xA8),
            sec_grad: (c(0x8A, 0x8F, 0xA8), c(0xF2, 0xF3, 0xF7)),
            danger: c(0xFF, 0x6B, 0x7A),
            danger_light: c(0xFF, 0x9A, 0xA5),
            warn: c(0xF5, 0xC4, 0x51),
            ok: c(0x5E, 0xE0, 0xA0),
            groups: [
                c(0x6E, 0x7B, 0xFF),
                c(0x5C, 0xC8, 0xFF),
                c(0xA7, 0x8B, 0xFA),
                c(0x5E, 0xE0, 0xA0),
                c(0xF5, 0xC4, 0x51),
                c(0xFF, 0x6B, 0x7A),
            ],
        }
    } else {
        Palette {
            dark,
            backdrop: c(0xF4, 0xF5, 0xF8),
            glass: c(0xFF, 0xFF, 0xFF),
            glass_hi: c(0xEC, 0xEE, 0xF4),
            glass_line: a(0, 31),
            glass_edge: a(0, 66),
            solid: c(0xFF, 0xFF, 0xFF),
            solid_hi: c(0xF6, 0xF7, 0xFA),
            sunken: c(0xEC, 0xEE, 0xF4),
            raised: c(0xE6, 0xE9, 0xF1),
            line: a(0, 31),
            line_strong: a(0, 66),
            text: c(0x0B, 0x0C, 0x12),
            text_dim: c(0x45, 0x4A, 0x5E),
            text_faint: c(0x5F, 0x64, 0x7A),
            ink: c(0xFF, 0xFF, 0xFF),
            primary: c(0x3B, 0x4C, 0xFF),
            primary_light: c(0x2B, 0x3A, 0xE0),
            primary_deep: c(0x1F, 0x2A, 0xB0),
            primary_quiet: Color32::from_rgba_unmultiplied(0x3B, 0x4C, 0xFF, 26),
            primary_quiet_hi: Color32::from_rgba_unmultiplied(0x3B, 0x4C, 0xFF, 44),
            prim_grad: (c(0x3B, 0x4C, 0xFF), c(0x3B, 0x4C, 0xFF)),
            sec: c(0x2A, 0x2D, 0x3A),
            sec_light: c(0x0B, 0x0C, 0x12),
            sec_deep: c(0x5F, 0x64, 0x7A),
            sec_grad: (c(0x05, 0x06, 0x0A), c(0x3A, 0x3E, 0x4E)),
            danger: c(0xD1, 0x24, 0x3B),
            danger_light: c(0xE5, 0x4B, 0x5E),
            warn: c(0xB7, 0x7B, 0x00),
            ok: c(0x14, 0x8A, 0x4F),
            groups: [
                c(0x3B, 0x4C, 0xFF),
                c(0x00, 0x86, 0xC2),
                c(0x7C, 0x4D, 0xE8),
                c(0x14, 0x8A, 0x4F),
                c(0xB7, 0x7B, 0x00),
                c(0xD1, 0x24, 0x3B),
            ],
        }
    }
}

static CURRENT: RwLock<Option<Palette>> = RwLock::new(None);

pub fn pal() -> Palette {
    if let Ok(g) = CURRENT.read() {
        if let Some(p) = *g {
            return p;
        }
    }
    palette(true)
}

pub fn set_palette(dark: bool) {
    if let Ok(mut g) = CURRENT.write() {
        *g = Some(palette(dark));
    }
}

// metrics here---

pub const R_WINDOW: f32 = 2.0;
pub const R_SM: f32 = 2.0;
pub const R_MD: f32 = 2.0;
pub const R_CTRL: f32 = 2.0;
pub const R_CARD: f32 = 2.0;
pub const R_ISLAND: f32 = 2.0;
pub const R_PANEL: f32 = 2.0;
pub const R_PILL: f32 = 2.0;

pub const RAIL_W: f32 = 268.0;
pub const OUTLINE_W: f32 = 246.0;
pub const INSPECT_W: f32 = 272.0;
pub const TOPBAR_H: f32 = 92.0;
pub const DOC_MAX: f32 = 820.0;
pub const GAP: f32 = 14.0;

pub const T_TITLE: f32 = 26.0;
pub const T_H: f32 = 17.0;
pub const T_BODY: f32 = 14.5;
pub const T_SM: f32 = 13.0;
pub const T_LABEL: f32 = 12.0;
pub const T_CAP: f32 = 11.0;
pub const T_MICRO: f32 = 10.0;

// colour math here---

pub fn wash(col: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(col.r(), col.g(), col.b(), alpha)
}

pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let f = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgba_unmultiplied(
        f(a.r(), b.r()),
        f(a.g(), b.g()),
        f(a.b(), b.b()),
        f(a.a(), b.a()),
    )
}

pub fn lighten(col: Color32, t: f32) -> Color32 {
    mix(col, Color32::WHITE, t)
}

pub fn darken(col: Color32, t: f32) -> Color32 {
    mix(col, Color32::BLACK, t)
}

pub fn on(bg: Color32) -> Color32 {
    let l = 0.299 * bg.r() as f32 + 0.587 * bg.g() as f32 + 0.114 * bg.b() as f32;
    if l > 150.0 {
        c(0x10, 0x12, 0x11)
    } else {
        c(0xF4, 0xF7, 0xF5)
    }
}

// shapes here---

pub fn rounded_poly(rect: Rect, radius: f32) -> Vec<Pos2> {
    let r = radius.min(rect.width() * 0.5).min(rect.height() * 0.5).max(0.0);
    if r <= 0.5 {
        return vec![
            rect.left_top(),
            rect.right_top(),
            rect.right_bottom(),
            rect.left_bottom(),
        ];
    }
    let steps = ((r * 0.9) as usize).clamp(5, 18);
    let mut pts = Vec::with_capacity(steps * 4 + 4);
    let corners = [
        (rect.right() - r, rect.top() + r, -std::f32::consts::FRAC_PI_2),
        (rect.right() - r, rect.bottom() - r, 0.0),
        (rect.left() + r, rect.bottom() - r, std::f32::consts::FRAC_PI_2),
        (rect.left() + r, rect.top() + r, std::f32::consts::PI),
    ];
    for (cx, cy, a0) in corners {
        for i in 0..=steps {
            let a = a0 + std::f32::consts::FRAC_PI_2 * (i as f32 / steps as f32);
            pts.push(Pos2::new(cx + a.cos() * r, cy + a.sin() * r));
        }
    }
    pts
}

pub fn fill_grad_poly(painter: &egui::Painter, pts: &[Pos2], a: Color32, b: Color32, dir: Vec2) {
    painter.add(grad_poly_shape(pts, a, b, dir));
}

pub fn grad_poly_shape(pts: &[Pos2], a: Color32, b: Color32, dir: Vec2) -> Shape {
    if pts.len() < 3 {
        return Shape::Noop;
    }
    let dir = if dir.length_sq() < 1e-6 {
        Vec2::new(0.0, 1.0)
    } else {
        dir.normalized()
    };
    let (mut lo, mut hi) = (f32::MAX, f32::MIN);
    for p in pts {
        let t = p.to_vec2().dot(dir);
        lo = lo.min(t);
        hi = hi.max(t);
    }
    let span = (hi - lo).max(0.0001);
    let at = |p: Pos2| -> Color32 { mix(a, b, (p.to_vec2().dot(dir) - lo) / span) };

    let mut centre = Vec2::ZERO;
    for p in pts {
        centre += p.to_vec2();
    }
    let centre = (centre / pts.len() as f32).to_pos2();

    let mut mesh = Mesh::default();
    mesh.colored_vertex(centre, at(centre));
    for p in pts {
        mesh.colored_vertex(*p, at(*p));
    }
    let n = pts.len() as u32;
    for i in 0..n {
        mesh.add_triangle(0, 1 + i, 1 + (i + 1) % n);
    }

    const FEATHER: f32 = 0.9;
    let base = mesh.vertices.len() as u32;
    for p in pts {
        let out = (*p - centre).normalized() * FEATHER;
        let q = *p + out;
        mesh.colored_vertex(q, wash(at(*p), 0));
    }
    for i in 0..n {
        let j = (i + 1) % n;
        mesh.add_triangle(1 + i, base + i, base + j);
        mesh.add_triangle(1 + i, base + j, 1 + j);
    }
    Shape::mesh(mesh)
}

pub fn grad_rect(painter: &egui::Painter, rect: Rect, radius: f32, a: Color32, b: Color32, dir: Vec2) {
    let pts = rounded_poly(rect, radius);
    fill_grad_poly(painter, &pts, a, b, dir);
}

pub fn grad_diamond(painter: &egui::Painter, centre: Pos2, r: f32, a: Color32, b: Color32) {
    let pts = vec![
        Pos2::new(centre.x, centre.y - r),
        Pos2::new(centre.x + r, centre.y),
        Pos2::new(centre.x, centre.y + r),
        Pos2::new(centre.x - r, centre.y),
    ];
    fill_grad_poly(painter, &pts, a, b, Vec2::new(0.6, 1.0));
}

pub fn fading_arc(
    painter: &egui::Painter,
    centre: Pos2,
    radius: f32,
    from: f32,
    to: f32,
    width: f32,
    color: Color32,
) {
    let steps = 22;
    for i in 0..steps {
        let t0 = i as f32 / steps as f32;
        let t1 = (i + 1) as f32 / steps as f32;
        let a0 = from + (to - from) * t0;
        let a1 = from + (to - from) * t1;
        let mid = (t0 + t1) * 0.5;
        let fade = (mid * std::f32::consts::PI).sin().powf(0.7);
        let col = wash(color, (color.a() as f32 * fade) as u8);
        painter.line_segment(
            [
                Pos2::new(centre.x + a0.cos() * radius, centre.y + a0.sin() * radius),
                Pos2::new(centre.x + a1.cos() * radius, centre.y + a1.sin() * radius),
            ],
            Stroke::new(width, col),
        );
    }
}

// bloom here---

fn bloom_rings(spread: f32) -> usize {
    ((spread * 0.40) as usize).clamp(10, 30)
}

fn falloff(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    let near = (-3.2 * t).exp();
    let far = (1.0 - t).powf(2.6);
    (0.22 * near + 0.78 * far) * 0.82
}

const GLOWS: bool = false;

fn bloom_mesh<F>(
    painter: &egui::Painter,
    centre: Pos2,
    spread: f32,
    color: Color32,
    strength: f32,
    ring: F,
) where
    F: Fn(f32) -> Vec<Pos2>,
{
    if !GLOWS || strength <= 0.002 || spread <= 0.1 {
        return;
    }
    let peak = (strength.clamp(0.0, 2.0) * 62.0).min(255.0) as u8;
    let rings = bloom_rings(spread);

    let mut mesh = Mesh::default();
    mesh.colored_vertex(centre, wash(color, peak));

    let mut prev: Option<(u32, usize)> = None;
    for k in 0..=rings {
        let t = k as f32 / rings as f32;
        let pts = ring(spread * t);
        if pts.len() < 3 {
            return;
        }
        let col = wash(color, (peak as f32 * falloff(t)) as u8);
        let base = mesh.vertices.len() as u32;
        for q in &pts {
            mesh.colored_vertex(*q, col);
        }
        let n = pts.len() as u32;
        match prev {
            None => {
                for i in 0..n {
                    mesh.add_triangle(0, base + i, base + (i + 1) % n);
                }
            }
            Some((pbase, pcount)) => {
                if pcount != pts.len() {
                    return;
                }
                for i in 0..n {
                    let j = (i + 1) % n;
                    mesh.add_triangle(pbase + i, base + i, base + j);
                    mesh.add_triangle(pbase + i, base + j, pbase + j);
                }
            }
        }
        prev = Some((base, pts.len()));
    }
    painter.add(Shape::mesh(mesh));
}

const BLOOM_SEGMENTS: usize = 44;

pub fn glow_circle(
    painter: &egui::Painter,
    centre: Pos2,
    r: f32,
    color: Color32,
    spread: f32,
    strength: f32,
) {
    bloom_mesh(painter, centre, spread, color, strength, |grow| {
        let rr = r + grow;
        (0..BLOOM_SEGMENTS)
            .map(|i| {
                let a = std::f32::consts::TAU * i as f32 / BLOOM_SEGMENTS as f32;
                Pos2::new(centre.x + a.cos() * rr, centre.y + a.sin() * rr)
            })
            .collect()
    });
}

pub fn glow_rect(
    painter: &egui::Painter,
    rect: Rect,
    radius: f32,
    color: Color32,
    spread: f32,
    strength: f32,
) {
    let steps = 9usize;
    bloom_mesh(painter, rect.center(), spread, color, strength, move |grow| {
        let r = rect.expand(grow);
        let rad = (radius + grow).min(r.width() * 0.5).min(r.height() * 0.5);
        let quarter = std::f32::consts::FRAC_PI_2;
        let corners = [
            (Pos2::new(r.right() - rad, r.top() + rad), -quarter),
            (Pos2::new(r.right() - rad, r.bottom() - rad), 0.0),
            (Pos2::new(r.left() + rad, r.bottom() - rad), quarter),
            (Pos2::new(r.left() + rad, r.top() + rad), std::f32::consts::PI),
        ];
        let mut pts = Vec::with_capacity(steps * 4 + 4);
        for (c, a0) in corners {
            for i in 0..=steps {
                let a = a0 + quarter * (i as f32 / steps as f32);
                pts.push(Pos2::new(c.x + a.cos() * rad, c.y + a.sin() * rad));
            }
        }
        pts
    });
}

pub fn glow_diamond(
    painter: &egui::Painter,
    centre: Pos2,
    r: f32,
    color: Color32,
    spread: f32,
    strength: f32,
) {
    bloom_mesh(painter, centre, spread, color, strength, move |grow| {
        let rr = r + grow;
        let n = 1.0 + 1.15 * (grow / spread.max(0.001)).clamp(0.0, 1.0);
        (0..BLOOM_SEGMENTS)
            .map(|i| {
                let a = std::f32::consts::TAU * i as f32 / BLOOM_SEGMENTS as f32;
                let (c, s) = (a.cos(), a.sin());
                let d = (c.abs().powf(n) + s.abs().powf(n)).powf(1.0 / n);
                let k = rr / d.max(0.001);
                Pos2::new(centre.x + c * k, centre.y + s * k)
            })
            .collect()
    });
}
pub fn star_points(centre: Pos2, r: f32, waist: f32) -> [Pos2; 8] {
    let w = r * waist * std::f32::consts::FRAC_1_SQRT_2;
    [
        Pos2::new(centre.x, centre.y - r),
        Pos2::new(centre.x + w, centre.y - w),
        Pos2::new(centre.x + r, centre.y),
        Pos2::new(centre.x + w, centre.y + w),
        Pos2::new(centre.x, centre.y + r),
        Pos2::new(centre.x - w, centre.y + w),
        Pos2::new(centre.x - r, centre.y),
        Pos2::new(centre.x - w, centre.y - w),
    ]
}

pub fn grad_star(painter: &egui::Painter, centre: Pos2, r: f32, _a: Color32, b: Color32) {
    painter.add(marker(centre, r, b));
}

pub fn marker(centre: Pos2, r: f32, color: Color32) -> Shape {
    let side = (r * 1.1).round().max(4.0);
    Shape::rect_filled(Rect::from_center_size(centre, Vec2::splat(side)), Rounding::ZERO, color)
}

pub fn star_shapes(centre: Pos2, r: f32, _waist: f32, _a: Color32, b: Color32) -> Vec<Shape> {
    vec![marker(centre, r, b)]
}

pub fn star_kites(centre: Pos2, r: f32, waist: f32, a: Color32, b: Color32) -> Vec<Shape> {
    let pts = star_points(centre, r, waist);
    let dir = Vec2::new(0.6, 1.0).normalized();
    let (lo, hi) = (
        (centre.to_vec2() - Vec2::splat(r)).dot(dir),
        (centre.to_vec2() + Vec2::splat(r)).dot(dir),
    );
    let at = |p: Pos2| mix(a, b, (p.to_vec2().dot(dir) - lo) / (hi - lo).max(0.0001));
    (0..4)
        .map(|k| {
            let tip = pts[k * 2];
            let right = pts[k * 2 + 1];
            let left = pts[(k * 2 + 7) % 8];
            let kite = [tip, right, centre, left];
            let (ca, cb) = (at(tip), at(centre));
            let kd = centre - tip;
            let kd = if kd.length_sq() < 1e-6 { dir } else { kd };
            grad_poly_shape(&kite, ca, cb, kd)
        })
        .collect()
}

pub fn glow_star(
    painter: &egui::Painter,
    centre: Pos2,
    r: f32,
    color: Color32,
    spread: f32,
    strength: f32,
) {
    bloom_mesh(painter, centre, spread, color, strength, move |grow| {
        let rr = r + grow;
        let n = 0.72 + 1.43 * (grow / spread.max(0.001)).clamp(0.0, 1.0);
        (0..BLOOM_SEGMENTS)
            .map(|i| {
                let a = std::f32::consts::TAU * i as f32 / BLOOM_SEGMENTS as f32;
                let (c, s) = (a.cos(), a.sin());
                let d = (c.abs().powf(n) + s.abs().powf(n)).powf(1.0 / n);
                let k = rr / d.max(0.001);
                Pos2::new(centre.x + c * k, centre.y + s * k)
            })
            .collect()
    });
}

pub fn glass_body(opacity: f32) -> Color32 {
    let _ = opacity;
    pal().glass
}

pub fn hover_surface(
    painter: &egui::Painter,
    rect: Rect,
    radius: f32,
    accent: Color32,
    t: f32,
    on: bool,
) {
    let p = pal();
    let t = t.clamp(0.0, 1.0);
    if t <= 0.005 && !on {
        return;
    }
    let lit = t.max(if on { 1.0 } else { 0.0 });
    let fill = if p.dark { p.raised } else { p.sunken };
    painter.rect_filled(rect, Rounding::same(radius), wash(fill, (255.0 * lit) as u8));
    let edge = if on { accent } else { p.line_strong };
    painter.rect_stroke(
        rect.shrink(0.5),
        Rounding::same(radius),
        Stroke::new(1.0_f32, wash(edge, ((if on { 255.0 } else { 140.0 }) * lit) as u8)),
    );
}

pub fn glass_surface(painter: &egui::Painter, rect: Rect, radius: f32, opacity: f32) {
    let _ = opacity;
    let p = pal();
    painter.rect_filled(rect, Rounding::same(radius), p.solid_hi);
    painter.rect_stroke(rect.shrink(0.5), Rounding::same(radius), Stroke::new(1.0_f32, p.line_strong));
    top_edge(painter, rect);
}

pub fn top_edge(painter: &egui::Painter, rect: Rect) {
    let p = pal();
    let a = if p.dark { 15 } else { 0 };
    if a > 0 {
        painter.line_segment(
            [rect.left_top() + Vec2::new(1.0, 1.5), rect.right_top() + Vec2::new(-1.0, 1.5)],
            Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, a)),
        );
    }
}

pub fn glass_plate(painter: &egui::Painter, rect: Rect, radius: f32, opacity: f32) {
    let _ = opacity;
    let p = pal();
    painter.rect_filled(rect, Rounding::same(radius), p.glass);
    painter.rect_stroke(rect.shrink(0.5), Rounding::same(radius), Stroke::new(1.0_f32, p.line));
}

pub fn island_frame(radius: f32) -> egui::Frame {
    let p = pal();
    egui::Frame::none()
        .fill(p.solid)
        .rounding(Rounding::same(radius))
        .stroke(Stroke::new(1.0_f32, p.line))
}

pub fn tracked_text(
    painter: &egui::Painter,
    left_centre: Pos2,
    text: &str,
    font: egui::FontId,
    color: Color32,
    tracking: f32,
) -> f32 {
    let mut job = egui::text::LayoutJob::default();
    job.append(
        text,
        0.0,
        egui::TextFormat {
            font_id: font,
            color,
            extra_letter_spacing: tracking,
            ..Default::default()
        },
    );
    let galley = painter.ctx().fonts(|f| f.layout_job(job));
    let size = galley.rect.size();
    painter.galley(
        Pos2::new(left_centre.x, left_centre.y - size.y * 0.5),
        galley,
        color,
    );
    (size.x - tracking).max(0.0)
}

pub fn tracked_width(ctx: &egui::Context, text: &str, font: egui::FontId, tracking: f32) -> f32 {
    let mut job = egui::text::LayoutJob::default();
    job.append(
        text,
        0.0,
        egui::TextFormat {
            font_id: font,
            extra_letter_spacing: tracking,
            ..Default::default()
        },
    );
    let w = ctx.fonts(|f| f.layout_job(job)).rect.width();
    (w - tracking).max(0.0)
}

pub fn lift_shadow(painter: &egui::Painter, rect: Rect, radius: f32, lift: f32) {
    if !GLOWS {
        let _ = (painter, rect, radius, lift);
        return;
    }
    let p = pal();
    let lift = lift.clamp(0.0, 1.0);
    if lift <= 0.01 {
        return;
    }
    let rounding = Rounding::same(radius.min(rect.width() * 0.5).min(rect.height() * 0.5));
    let base = if p.dark { 1.0 } else { 0.32 };
    for (dy, blur, alpha) in [(1.0_f32, 2.0_f32, 60.0_f32), (4.0, 10.0, 38.0), (12.0, 28.0, 26.0)] {
        painter.add(
            egui::epaint::Shadow {
                offset: Vec2::new(0.0, dy * (0.5 + 0.5 * lift)),
                blur: blur * (0.6 + 0.4 * lift),
                spread: 0.0,
                color: Color32::from_black_alpha((alpha * base * lift) as u8),
            }
            .as_shape(rect, rounding),
        );
    }
}

// visuals here---

pub fn apply(ctx: &egui::Context) {
    let p = pal();
    let mut v = if p.dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };

    v.override_text_color = Some(p.text);
    v.panel_fill = Color32::TRANSPARENT;
    v.window_fill = p.solid;
    v.extreme_bg_color = if p.dark { Color32::BLACK } else { Color32::WHITE };
    v.faint_bg_color = p.raised;
    v.window_stroke = Stroke::new(1.0_f32, p.line);
    v.window_rounding = Rounding::same(R_PANEL);
    v.menu_rounding = Rounding::same(R_MD);
    v.selection.bg_fill = wash(p.primary, if p.dark { 150 } else { 90 });
    v.selection.stroke = Stroke::new(1.0_f32, p.primary);
    v.hyperlink_color = p.primary_light;
    v.popup_shadow = egui::epaint::Shadow::NONE;
    v.window_shadow = v.popup_shadow;
    v.text_cursor.stroke = Stroke::new(2.0_f32, p.primary_light);
    v.window_stroke = Stroke::new(1.0_f32, p.line_strong);

    let w = &mut v.widgets;
    w.noninteractive.bg_fill = p.solid;
    w.noninteractive.weak_bg_fill = p.solid;
    w.noninteractive.bg_stroke = Stroke::new(1.0_f32, p.line);
    w.noninteractive.fg_stroke = Stroke::new(1.0_f32, p.text_dim);
    w.noninteractive.rounding = Rounding::same(R_CTRL);

    w.inactive.bg_fill = p.sunken;
    w.inactive.weak_bg_fill = p.sunken;
    w.inactive.bg_stroke = Stroke::new(1.0_f32, p.line_strong);
    w.inactive.fg_stroke = Stroke::new(1.0_f32, p.text);
    w.inactive.rounding = Rounding::same(R_CTRL);

    w.hovered.bg_fill = p.solid_hi;
    w.hovered.weak_bg_fill = p.solid_hi;
    w.hovered.bg_stroke = Stroke::new(1.0_f32, p.line_strong);
    w.hovered.fg_stroke = Stroke::new(1.0_f32, p.text);
    w.hovered.rounding = Rounding::same(R_CTRL);
    w.hovered.expansion = 0.0;

    w.active.bg_fill = p.primary_quiet_hi;
    w.active.weak_bg_fill = p.primary_quiet_hi;
    w.active.bg_stroke = Stroke::new(1.0_f32, p.primary);
    w.active.fg_stroke = Stroke::new(1.0_f32, p.primary_light);
    w.active.rounding = Rounding::same(R_CTRL);

    w.open.bg_fill = p.raised;
    w.open.weak_bg_fill = p.raised;
    w.open.bg_stroke = Stroke::new(1.0_f32, p.line);
    w.open.fg_stroke = Stroke::new(1.0_f32, p.text);
    w.open.rounding = Rounding::same(R_CTRL);

    ctx.all_styles_mut(|s| {
        s.visuals = v.clone();
        s.spacing.item_spacing = Vec2::new(8.0, 8.0);
        s.spacing.button_padding = Vec2::new(11.0, 6.0);
        s.spacing.window_margin = Margin::same(18.0);
        s.spacing.menu_margin = Margin::same(6.0);
        s.spacing.scroll.bar_width = 8.0;
        s.spacing.scroll.floating = true;
        s.visuals.clip_rect_margin = 2.0;
    });
}

// script colours here---

pub fn element_color(e: Element) -> Color32 {
    let p = pal();
    match e {
        Element::SceneHeading => p.sec_light,
        Element::Action => p.text_faint,
        Element::Character => p.primary_light,
        Element::Parenthetical => p.group(3),
        Element::Dialogue => p.group(2),
        Element::Transition => p.group(4),
        Element::Shot => p.group(5),
        Element::Act => p.primary_light,
    }
}

pub fn element_band(e: Element) -> Color32 {
    let p = pal();
    match e {
        Element::Action | Element::Act => Color32::TRANSPARENT,
        _ => tint(element_color(e), if p.dark { 0.035 } else { 0.05 }),
    }
}

pub fn tint(col: Color32, k: f32) -> Color32 {
    let k = k.clamp(0.0, 1.0);
    let f = |v: u8| (v as f32 * k).round() as u8;
    Color32::from_rgba_premultiplied(f(col.r()), f(col.g()), f(col.b()), (255.0 * k).round() as u8)
}

pub fn character_colors(names: &[String], seed: u32) -> std::collections::HashMap<String, Color32> {
    voice_colors(names, seed, &[])
}

fn voice_sl() -> (f32, f32) {
    if pal().dark {
        (0.62, 0.72)
    } else {
        (0.66, 0.36)
    }
}

pub fn voice_colors(names: &[String], seed: u32, chosen: &[(String, u16)]) -> std::collections::HashMap<String, Color32> {
    let (s, l) = voice_sl();
    with_chosen(voices_at(names, seed, s, l), chosen, s, l)
}

pub fn voice_inks(names: &[String], seed: u32, chosen: &[(String, u16)]) -> std::collections::HashMap<String, [u8; 3]> {
    with_chosen(voices_at(names, seed, 0.70, 0.34), chosen, 0.70, 0.34)
        .into_iter()
        .map(|(n, c)| (n, [c.r(), c.g(), c.b()]))
        .collect()
}

pub fn voice_of_hue(hue: u16) -> Color32 {
    let (s, l) = voice_sl();
    hsl(hue as f32 / 360.0, s, l)
}

fn with_chosen(
    mut dealt: std::collections::HashMap<String, Color32>,
    chosen: &[(String, u16)],
    s: f32,
    l: f32,
) -> std::collections::HashMap<String, Color32> {
    for (name, hue) in chosen {
        if let Some(c) = dealt.get_mut(name) {
            *c = hsl(*hue as f32 / 360.0, s, l);
        }
    }
    dealt
}

pub fn character_inks(names: &[String], seed: u32) -> std::collections::HashMap<String, [u8; 3]> {
    voice_inks(names, seed, &[])
}

fn voices_at(names: &[String], seed: u32, s: f32, l: f32) -> std::collections::HashMap<String, Color32> {
    let start = (seed as f32 * 0.137_5).fract();
    names
        .iter()
        .enumerate()
        .map(|(k, n)| {
            let h = (start + k as f32 * 0.618_034).fract();
            (n.clone(), hsl(h, s, l))
        })
        .collect()
}

pub fn hsl(h: f32, s: f32, l: f32) -> Color32 {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = (h.fract() + 1.0).fract() * 6.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r, g, b) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c * 0.5;
    let to = |v: f32| ((v + m).clamp(0.0, 1.0) * 255.0).round() as u8;
    c3(to(r), to(g), to(b))
}

fn c3(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

pub fn element_ink(e: Element) -> Color32 {
    let p = pal();
    match e {
        Element::SceneHeading | Element::Shot | Element::Character | Element::Act => p.text,
        Element::Dialogue => mix(p.text, p.text_dim, 0.15),
        Element::Action => mix(p.text, p.text_dim, 0.35),
        Element::Parenthetical | Element::Transition => p.text_dim,
    }
}

// fonts here---

const ANYBODY_REGULAR: &[u8] = include_bytes!("../assets/fonts/Anybody-Regular.ttf");
const ANYBODY_MEDIUM: &[u8] = include_bytes!("../assets/fonts/Anybody-Medium.ttf");
const ANYBODY_SEMIBOLD: &[u8] = include_bytes!("../assets/fonts/Anybody-SemiBold.ttf");
const ANYBODY_BOLD: &[u8] = include_bytes!("../assets/fonts/Anybody-Bold.ttf");
const ANYBODY_DISPLAY: &[u8] = include_bytes!("../assets/fonts/Anybody-Display.ttf");
const MONO_REGULAR: &[u8] = include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf");
const MONO_MEDIUM: &[u8] = include_bytes!("../assets/fonts/JetBrainsMono-Medium.ttf");

const PAGE_REGULAR: &[u8] = include_bytes!("../assets/fonts/CourierPrime-Regular.ttf");
const PAGE_BOLD: &[u8] = include_bytes!("../assets/fonts/CourierPrime-Bold.ttf");

pub const PAGE_FONT_NAME: &str = "Courier Prime";

pub fn medium() -> FontFamily {
    FontFamily::Name("anybody-medium".into())
}
pub fn semibold() -> FontFamily {
    FontFamily::Name("anybody-semibold".into())
}
pub fn bold() -> FontFamily {
    FontFamily::Name("anybody-bold".into())
}
pub fn display() -> FontFamily {
    FontFamily::Name("anybody-display".into())
}
pub fn hud() -> FontFamily {
    FontFamily::Name("mono-medium".into())
}
pub fn page() -> FontFamily {
    FontFamily::Name("page".into())
}
pub fn page_bold() -> FontFamily {
    FontFamily::Name("page-bold".into())
}

pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();

    for (key, bytes) in [
        ("Anybody", ANYBODY_REGULAR),
        ("AnybodyMedium", ANYBODY_MEDIUM),
        ("AnybodySemiBold", ANYBODY_SEMIBOLD),
        ("AnybodyBold", ANYBODY_BOLD),
        ("AnybodyDisplay", ANYBODY_DISPLAY),
        ("JetBrainsMono", MONO_REGULAR),
        ("JetBrainsMonoMedium", MONO_MEDIUM),
        ("CourierPrime", PAGE_REGULAR),
        ("CourierPrimeBold", PAGE_BOLD),
    ] {
        fonts
            .font_data
            .insert(key.to_string(), FontData::from_static(bytes));
    }

    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, "Anybody".to_string());
    fonts
        .families
        .entry(FontFamily::Monospace)
        .or_default()
        .insert(0, "JetBrainsMono".to_string());

    let proportional = fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    let mono = fonts.families.get(&FontFamily::Monospace).cloned().unwrap_or_default();
    for (family, key, rest) in [
        (medium(), "AnybodyMedium", &proportional),
        (semibold(), "AnybodySemiBold", &proportional),
        (bold(), "AnybodyBold", &proportional),
        (display(), "AnybodyDisplay", &proportional),
        (hud(), "JetBrainsMonoMedium", &mono),
    ] {
        let mut stack = vec![key.to_string()];
        stack.extend(rest.iter().cloned());
        fonts.families.insert(family, stack);
    }

    let mut page_stack = vec!["CourierPrime".to_string()];
    page_stack.extend(mono.iter().cloned());
    fonts.families.insert(page(), page_stack.clone());
    let mut bold_stack = vec!["CourierPrimeBold".to_string()];
    bold_stack.extend(page_stack);
    fonts.families.insert(page_bold(), bold_stack);

    ctx.set_fonts(fonts);
}

pub fn font(size: f32) -> egui::FontId {
    egui::FontId::proportional(size)
}
pub fn font_med(size: f32) -> egui::FontId {
    egui::FontId::new(size, medium())
}
pub fn font_semi(size: f32) -> egui::FontId {
    egui::FontId::new(size, semibold())
}
pub fn font_bold(size: f32) -> egui::FontId {
    egui::FontId::new(size, bold())
}
pub fn font_mono(size: f32) -> egui::FontId {
    egui::FontId::monospace(size)
}
pub fn font_display(size: f32) -> egui::FontId {
    egui::FontId::new(size, display())
}
pub fn font_hud(size: f32) -> egui::FontId {
    egui::FontId::new(size, hud())
}
pub fn font_page(size: f32) -> egui::FontId {
    egui::FontId::new(size, page())
}
pub fn font_page_bold(size: f32) -> egui::FontId {
    egui::FontId::new(size, page_bold())
}
