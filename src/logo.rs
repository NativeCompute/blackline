use eframe::egui::{self, Color32, Pos2, Rect, Stroke, Vec2};

use crate::theme::{self, pal};

pub const SIGNAL: Color32 = Color32::from_rgb(0x3B, 0x4C, 0xFF);

type Stroke2 = ((f32, f32), (f32, f32), f32);
const STROKES: [Stroke2; 2] = [
    ((0.27, 0.83), (0.49, 0.17), 0.21),
    ((0.585, 0.83), (0.805, 0.17), 0.075),
];

fn capsule(a: Pos2, b: Pos2, w: f32) -> Vec<Pos2> {
    let along = (b - a).normalized();
    let side = Vec2::new(-along.y, along.x);
    let r = w * 0.5;
    let steps = 10;
    let mut pts = Vec::with_capacity(2 * steps + 2);
    let base = side.y.atan2(side.x);
    for (c, from) in [(b, base + std::f32::consts::PI), (a, base)] {
        for k in 0..=steps {
            let t = from + std::f32::consts::PI * k as f32 / steps as f32;
            pts.push(Pos2::new(c.x + t.cos() * r, c.y + t.sin() * r));
        }
    }
    pts
}

fn shapes(rect: Rect) -> [Vec<Pos2>; 2] {
    let side = rect.width().min(rect.height());
    let origin = rect.center() - Vec2::splat(side * 0.5);
    let at = |(x, y): (f32, f32)| origin + Vec2::new(x * side, y * side);
    STROKES.map(|(a, b, w)| capsule(at(a), at(b), w * side))
}

pub fn paint(painter: &egui::Painter, rect: Rect, _glow: f32) {
    let p = pal();
    if rect.width().min(rect.height()) < 4.0 {
        return;
    }
    let [slab, line] = shapes(rect);
    let (lit, deep) = if p.dark {
        (Color32::from_rgb(0xF2, 0xF3, 0xF7), Color32::from_rgb(0x8A, 0x8F, 0xA8))
    } else {
        (Color32::from_rgb(0x2A, 0x2D, 0x3A), Color32::from_rgb(0x05, 0x06, 0x0A))
    };
    theme::fill_grad_poly(painter, &slab, lit, deep, Vec2::new(0.35, 1.0));
    painter.add(egui::Shape::convex_polygon(line, SIGNAL, Stroke::NONE));
}

pub fn spinner(painter: &egui::Painter, centre: Pos2, radius: f32, t: f32, color: Color32) {
    let head = t * std::f32::consts::TAU * 1.35;
    let sweep = 0.55 + 0.85 * (0.5 - 0.5 * (t * std::f32::consts::TAU * 0.9).cos());
    let steps = 64;
    let w = radius * 0.14;
    for i in 0..steps {
        let f0 = i as f32 / steps as f32;
        let f1 = (i + 1) as f32 / steps as f32;
        let a0 = head - sweep * (1.0 - f0);
        let a1 = head - sweep * (1.0 - f1);
        let fade = f0.powf(0.55);
        let col = theme::wash(color, (235.0 * fade) as u8);
        painter.line_segment(
            [
                Pos2::new(centre.x + a0.cos() * radius, centre.y + a0.sin() * radius),
                Pos2::new(centre.x + a1.cos() * radius, centre.y + a1.sin() * radius),
            ],
            Stroke::new(w * (0.55 + 0.45 * fade), col),
        );
    }
    painter.circle_filled(
        Pos2::new(centre.x + head.cos() * radius, centre.y + head.sin() * radius),
        w * 0.5,
        color,
    );
}

pub fn svg() -> String {
    let poly = |pts: &[Pos2]| -> String {
        pts.iter()
            .map(|q| format!("{:.2},{:.2}", q.x, q.y))
            .collect::<Vec<_>>()
            .join(" ")
    };
    let [slab, line] = shapes(Rect::from_min_size(Pos2::new(56.0, 56.0), Vec2::splat(400.0)));
    let mut s = String::new();
    s.push_str(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 512 512\" \
         width=\"512\" height=\"512\">\n  <title>Blackline</title>\n",
    );
    s.push_str("  <rect width=\"512\" height=\"512\" rx=\"116\" fill=\"#000000\"/>\n");
    s.push_str(&format!("  <polygon points=\"{}\" fill=\"#F2F3F7\"/>\n", poly(&slab)));
    s.push_str(&format!("  <polygon points=\"{}\" fill=\"#3B4CFF\"/>\n", poly(&line)));
    s.push_str("</svg>\n");
    s
}

pub fn rgba(size: u32) -> Vec<u8> {
    let n = size as f32;
    let scale = n / 512.0;
    let [slab, line] = shapes(Rect::from_min_size(Pos2::new(56.0 * scale, 56.0 * scale), Vec2::splat(400.0 * scale)));
    let radius = 116.0 * scale;
    let inside_tile = |x: f32, y: f32| {
        let cx = x.clamp(radius, n - radius);
        let cy = y.clamp(radius, n - radius);
        (x - cx).powi(2) + (y - cy).powi(2) <= radius * radius
    };
    let inside = |pts: &[Pos2], x: f32, y: f32| {
        let mut odd = false;
        let mut j = pts.len() - 1;
        for i in 0..pts.len() {
            let (a, b) = (pts[i], pts[j]);
            if (a.y > y) != (b.y > y) && x < (b.x - a.x) * (y - a.y) / (b.y - a.y) + a.x {
                odd = !odd;
            }
            j = i;
        }
        odd
    };
    const SS: usize = 4;
    let mut out = vec![0u8; (size * size * 4) as usize];
    for py in 0..size {
        for px in 0..size {
            let mut acc = [0.0f32; 4];
            for sy in 0..SS {
                for sx in 0..SS {
                    let x = px as f32 + (sx as f32 + 0.5) / SS as f32;
                    let y = py as f32 + (sy as f32 + 0.5) / SS as f32;
                    if !inside_tile(x, y) {
                        continue;
                    }
                    let c = if inside(&line, x, y) {
                        [0x3B, 0x4C, 0xFF]
                    } else if inside(&slab, x, y) {
                        [0xF2, 0xF3, 0xF7]
                    } else {
                        [0, 0, 0]
                    };
                    acc[0] += c[0] as f32;
                    acc[1] += c[1] as f32;
                    acc[2] += c[2] as f32;
                    acc[3] += 255.0;
                }
            }
            let k = (SS * SS) as f32;
            let a = acc[3] / k;
            let at = ((py * size + px) * 4) as usize;
            if a > 0.0 {
                let cover = acc[3] / 255.0;
                out[at] = (acc[0] / cover).round() as u8;
                out[at + 1] = (acc[1] / cover).round() as u8;
                out[at + 2] = (acc[2] / cover).round() as u8;
                out[at + 3] = a.round() as u8;
            }
        }
    }
    out
}

pub fn ico() -> Vec<u8> {
    let sizes = [16u32, 24, 32, 48, 64, 256];
    let images: Vec<Vec<u8>> = sizes.iter().map(|&s| bmp_entry(s)).collect();
    let mut out = Vec::new();
    out.extend_from_slice(&[0, 0, 1, 0]);
    out.extend_from_slice(&(sizes.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * sizes.len() as u32;
    for (s, img) in sizes.iter().zip(&images) {
        let dim = if *s >= 256 { 0 } else { *s as u8 };
        out.extend_from_slice(&[dim, dim, 0, 0]);
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&32u16.to_le_bytes());
        out.extend_from_slice(&(img.len() as u32).to_le_bytes());
        out.extend_from_slice(&offset.to_le_bytes());
        offset += img.len() as u32;
    }
    for img in images {
        out.extend_from_slice(&img);
    }
    out
}

fn bmp_entry(size: u32) -> Vec<u8> {
    let px = rgba(size);
    let mask_row = size.div_ceil(32) * 4;
    let mut out = Vec::new();
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&(size as i32).to_le_bytes());
    out.extend_from_slice(&((size * 2) as i32).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&32u16.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&(size * size * 4 + mask_row * size).to_le_bytes());
    out.extend_from_slice(&[0u8; 16]);
    for y in (0..size).rev() {
        for x in 0..size {
            let at = ((y * size + x) * 4) as usize;
            out.extend_from_slice(&[px[at + 2], px[at + 1], px[at], px[at + 3]]);
        }
    }
    out.extend(std::iter::repeat(0u8).take((mask_row * size) as usize));
    out
}
