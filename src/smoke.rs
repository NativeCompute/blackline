use std::path::{Path, PathBuf};
use std::sync::Mutex;

use eframe::egui;
use web_time::{Duration, Instant};

use crate::app::{App, Mode};
use crate::keys::Command;

pub const OLD_SCRIPT: &str = "---\ntitle: The Long Way Down\nauthor: A. Writer\ncontact: a.writer@example.com · 555-0100\ndraft: Second Draft — 3 March\n---\n\n## INT. WAREHOUSE - NIGHT\n\nRain hammers the corrugated roof. MARIA moves between the crates, counting doors under her breath — “four, five” — until one isn’t locked.\n\n**MARIA (V.O.)**\n\n*(barely audible)*\n\n> Three, four... there you are.\n\n### ANGLE ON THE DOOR\n\nIt swings inward on its own. Café light spills across the floor.\n\n`SMASH CUT TO:`\n\n## EXT. ROOFTOP - CONTINUOUS\n\n**COLE**\n\n> You came back.\n\n**MARIA (CONT'D)**\n\n> I never left.\n\n`FADE OUT.`\n\n";

pub const NEW_SCRIPT: &str = "---\ntitle: Crossfire\nauthor: Sam\ncontact: \ndraft: Blue\n---\n\n## INT. KITCHEN - NIGHT\n\nThe kettle screams. Neither of them moves to take it off the heat. [[Is the kettle too on the nose?]]\n\n**MARIA**\n\n> I said stop. I said it twice and you kept going anyway.\n\n**JONAS ^**\n\n*(over her)*\n\n> I'm not stopping! Not tonight.\n\nSilence. The kettle dies down.\n\n**MARIA**\n\n> Fine. [[softer]]\n\n";

pub const ODD_FOUNTAIN: &str = "Title: Odd Bits\nCopyright: 2026 Somebody\n\nINT. ROOM - DAY #12#\n\nShe waits.\n\n~La la la\n\n===\n\nShe leaves.\n";

pub const UNSAVED: &str = " SMOKE-UNSAVED-EDIT";

const SETTINGS: &str = "splash = yes\nanimations = yes\ncheck_updates = no\nautosave_ms = 10000\nafter_export = nothing\n";

static NOTES: Mutex<Vec<String>> = Mutex::new(Vec::new());

fn note(line: String) {
    eprintln!("smoke: {line}");
    if let Ok(mut n) = NOTES.lock() {
        n.push(line);
    }
}

struct Catch;

static CAUGHT: Mutex<Vec<String>> = Mutex::new(Vec::new());

impl log::Log for Catch {
    fn enabled(&self, m: &log::Metadata) -> bool {
        m.level() <= log::Level::Warn
    }

    fn log(&self, r: &log::Record) {
        if !self.enabled(r.metadata()) {
            return;
        }
        let line = format!("{} {}: {}", r.level(), r.target(), r.args());
        let fresh = CAUGHT.lock().map(|mut c| {
            if c.len() >= 40 || c.contains(&line) {
                false
            } else {
                c.push(line.clone());
                true
            }
        });
        if fresh.unwrap_or(false) {
            note(format!("log {line}"));
        }
    }

    fn flush(&self) {}
}

static CATCH: Catch = Catch;

pub fn catch_logs() {
    if log::set_logger(&CATCH).is_ok() {
        log::set_max_level(log::LevelFilter::Warn);
    }
}

pub fn describe(image: &egui::ColorImage) -> String {
    let mut seen: Vec<[u8; 3]> = Vec::new();
    let (mut lo, mut hi) = (255u8, 0u8);
    for c in image.pixels.iter().step_by(97) {
        let rgb = [c.r(), c.g(), c.b()];
        let l = ((rgb[0] as u32 + rgb[1] as u32 + rgb[2] as u32) / 3) as u8;
        lo = lo.min(l);
        hi = hi.max(l);
        if seen.len() < 64 && !seen.contains(&rgb) {
            seen.push(rgb);
        }
    }
    let middle = image.pixels.get(image.size[1] / 2 * image.size[0] + image.size[0] / 2).map(|c| [c.r(), c.g(), c.b(), c.a()]).unwrap_or_default();
    format!("colours {} light {lo}..{hi} middle {middle:?}", if seen.len() >= 64 { "64+".to_string() } else { seen.len().to_string() })
}

fn absolute(dir: &Path) -> PathBuf {
    if dir.is_absolute() {
        dir.to_path_buf()
    } else {
        std::env::current_dir().map(|c| c.join(dir)).unwrap_or_else(|_| dir.to_path_buf())
    }
}

pub fn library(dir: &Path) -> PathBuf {
    crate::storage::library_in(&absolute(dir).join("data"))
}

pub fn prepare(dir: &Path) -> Result<(), String> {
    let dir = absolute(dir);
    let _ = std::fs::remove_dir_all(dir.join("data"));
    std::fs::create_dir_all(dir.join("shots")).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(dir.join("report.txt"));
    std::env::set_var("XDG_DATA_HOME", dir.join("data"));
    let lib = library(&dir);
    std::fs::create_dir_all(lib.join("scripts")).map_err(|e| e.to_string())?;
    std::fs::write(lib.join("settings.conf"), SETTINGS).map_err(|e| e.to_string())?;
    std::fs::write(lib.join("scripts/the-long-way-down.md"), OLD_SCRIPT).map_err(|e| e.to_string())?;
    std::fs::write(lib.join("scripts/crossfire.md"), NEW_SCRIPT).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("odd.fountain"), ODD_FOUNTAIN).map_err(|e| e.to_string())?;
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Step {
    Splash,
    Tour,
    Home,
    Write,
    Cards,
    Read,
    Reports,
    Export,
    Settings,
    Import,
    Revision,
    Quit,
}

const STEPS: [Step; 12] = [
    Step::Splash,
    Step::Tour,
    Step::Home,
    Step::Write,
    Step::Cards,
    Step::Read,
    Step::Reports,
    Step::Export,
    Step::Settings,
    Step::Import,
    Step::Revision,
    Step::Quit,
];

pub struct Smoke {
    app: App,
    dir: PathBuf,
    renderer: String,
    step: usize,
    entered: Instant,
    frames: u32,
    asked: Option<Instant>,
    done: bool,
    told: bool,
}

pub fn blank(image: &egui::ColorImage) -> bool {
    let mut seen: Vec<[u8; 3]> = Vec::new();
    let (mut lo, mut hi) = (255u8, 0u8);
    for c in image.pixels.iter().step_by(97) {
        let rgb = [c.r(), c.g(), c.b()];
        let l = ((rgb[0] as u32 + rgb[1] as u32 + rgb[2] as u32) / 3) as u8;
        lo = lo.min(l);
        hi = hi.max(l);
        if seen.len() < 64 && !seen.contains(&rgb) {
            seen.push(rgb);
        }
    }
    seen.len() < 6 || hi.saturating_sub(lo) < 40
}

pub fn read_back_empty(image: &egui::ColorImage) -> bool {
    !image.pixels.is_empty() && image.pixels.iter().all(|c| *c == egui::Color32::TRANSPARENT)
}

pub fn read_bmp(bytes: &[u8]) -> Result<egui::ColorImage, String> {
    let int = |at: usize| -> Result<i64, String> {
        bytes.get(at..at + 4).map(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]) as i64).ok_or_else(|| "the BMP is cut short".to_string())
    };
    if bytes.get(0..2) != Some(b"BM") {
        return Err("not a BMP".into());
    }
    let offset = int(10)? as usize;
    let width = int(18)?;
    let height = int(22)?;
    let bits = bytes.get(28..30).map(|b| u16::from_le_bytes([b[0], b[1]])).unwrap_or(0) as usize;
    if width <= 0 || height == 0 || !(bits == 24 || bits == 32) {
        return Err(format!("unexpected BMP: {width}x{height}, {bits} bits"));
    }
    let (w, h) = (width as usize, height.unsigned_abs() as usize);
    let row = (w * bits / 8).div_ceil(4) * 4;
    let mut pixels = Vec::with_capacity(w * h);
    for y in 0..h {
        let start = offset + y * row;
        let line = bytes.get(start..start + w * bits / 8).ok_or_else(|| "the BMP is cut short".to_string())?;
        for px in line.chunks_exact(bits / 8) {
            pixels.push(egui::Color32::from_rgb(px[2], px[1], px[0]));
        }
    }
    Ok(egui::ColorImage { size: [w, h], pixels })
}

#[cfg(target_os = "macos")]
fn screen_shows(ctx: &egui::Context, path: &Path) -> Result<egui::ColorImage, String> {
    let rect = ctx.input(|i| i.viewport().inner_rect).ok_or("the window has no position")?;
    let area = format!("-R{:.0},{:.0},{:.0},{:.0}", rect.min.x, rect.min.y, rect.width(), rect.height());
    let status = std::process::Command::new("/usr/sbin/screencapture")
        .args(["-x", "-t", "bmp", &area])
        .arg(path)
        .status()
        .map_err(|e| format!("screencapture: {e}"))?;
    if !status.success() {
        return Err(format!("screencapture exited with {status}"));
    }
    read_bmp(&std::fs::read(path).map_err(|e| e.to_string())?)
}

#[cfg(not(target_os = "macos"))]
fn screen_shows(_ctx: &egui::Context, _path: &Path) -> Result<egui::ColorImage, String> {
    Err("only checked this way on a Mac".into())
}

fn save_png(path: &Path, image: &egui::ColorImage) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), image.size[0] as u32, image.size[1] as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
    let bytes: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_srgba_unmultiplied()).collect();
    writer.write_image_data(&bytes).map_err(|e| e.to_string())
}

impl Smoke {
    pub fn new(app: App, dir: PathBuf, renderer: String) -> Smoke {
        let dir = absolute(&dir);
        let guard = dir.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(120));
            let _ = std::fs::write(guard.join("report.txt"), "FAIL\nThe smoke run did not finish within 120 seconds.\n");
            eprintln!("smoke: timed out");
            std::process::exit(3);
        });
        note(format!("renderer {renderer}"));
        Smoke { app, dir, renderer, step: 0, entered: Instant::now(), frames: 0, asked: None, done: false, told: false }
    }

    fn enter(&mut self, step: Step) {
        let app = &mut self.app;
        match step {
            Step::Splash | Step::Tour => {}
            Step::Home => app.debug_skip_tour(),
            Step::Write => {
                let lib = library(&self.dir);
                app.debug_open_from_home(lib.join("scripts/the-long-way-down.md"));
            }
            Step::Cards => app.set_mode(Mode::Cards),
            Step::Read => app.set_mode(Mode::Read),
            Step::Reports => app.debug_run(Command::Reports),
            Step::Export => {
                app.debug_close_popover();
                app.set_mode(Mode::Write);
                app.debug_run(Command::QuickExport);
                app.debug_run(Command::ExportWindow);
            }
            Step::Settings => {
                app.debug_close_popover();
                app.debug_run(Command::Settings);
            }
            Step::Import => {
                app.debug_close_popover();
                app.debug_import(&self.dir.join("odd.fountain"));
            }
            Step::Revision => {
                app.debug_close_popover();
                let lib = library(&self.dir);
                app.debug_open_from_home(lib.join("scripts/crossfire.md"));
                app.debug_left_tab(1);
                app.debug_start_revision();
            }
            Step::Quit => {
                app.debug_append(UNSAVED);
                if !app.is_dirty_now() {
                    note("FAIL the unsaved edit was not taken".into());
                }
            }
        }
    }

    fn check_window(&self, ctx: &egui::Context) {
        let (inner, monitor) = ctx.input(|i| (i.viewport().inner_rect, i.viewport().monitor_size));
        if let (Some(inner), Some(monitor)) = (inner, monitor) {
            let fits = inner.width() <= monitor.x + 1.0 && inner.height() <= monitor.y + 1.0;
            note(format!(
                "{} window {:.0}x{:.0} on a {:.0}x{:.0} screen",
                if fits { "ok" } else { "FAIL" },
                inner.width(),
                inner.height(),
                monitor.x,
                monitor.y
            ));
        }
    }
}

impl eframe::App for Smoke {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        if !self.told {
            self.told = true;
            if let Some(gl) = frame.gl() {
                use eframe::glow::HasContext as _;
                let (vendor, renderer, version, side) = unsafe {
                    (
                        gl.get_parameter_string(eframe::glow::VENDOR),
                        gl.get_parameter_string(eframe::glow::RENDERER),
                        gl.get_parameter_string(eframe::glow::VERSION),
                        gl.get_parameter_i32(eframe::glow::MAX_TEXTURE_SIZE),
                    )
                };
                note(format!("gl {vendor} / {renderer} / {version} / max texture {side}"));
            }
        }
        self.app.update(ctx, frame);
        ctx.request_repaint();
        if self.done {
            return;
        }
        self.frames += 1;
        let step = STEPS[self.step];
        if let Some(image) = ctx.input(|i| {
            i.raw.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        }) {
            let name = format!("{:02}-{:?}.png", self.step + 1, step).to_lowercase();
            let path = self.dir.join("shots").join(&name);
            let saved = save_png(&path, &image);
            let mut empty = blank(&image);
            let fonts = ctx.fonts(|f| f.font_image_size());
            let mut how = describe(&image);
            if empty && read_back_empty(&image) {
                let screen = self.dir.join("shots").join(name.replace(".png", "-screen.bmp"));
                match screen_shows(ctx, &screen) {
                    Ok(seen) => {
                        empty = blank(&seen);
                        how = format!("the {} renderer read back an empty frame; macOS's own screenshot of the window: {}", self.renderer, describe(&seen));
                    }
                    Err(e) => how = format!("the {} renderer read back an empty frame and the screen could not be checked: {e}", self.renderer),
                }
            }
            note(format!(
                "{} shot {:?} {}x{}{}{} ({how}, font texture {}x{}, {:.2} px per point)",
                if saved.is_ok() && !empty { "ok" } else { "FAIL" },
                step,
                image.size[0],
                image.size[1],
                if empty { " looks blank" } else { "" },
                saved.err().map(|e| format!(" could not save: {e}")).unwrap_or_default(),
                fonts[0],
                fonts[1],
                ctx.pixels_per_point()
            ));
            self.asked = None;
            self.step += 1;
            self.entered = Instant::now();
            self.frames = 0;
            if self.step == 3 {
                self.check_window(ctx);
            }
            if self.step >= STEPS.len() {
                self.done = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                return;
            }
            self.enter(STEPS[self.step]);
            return;
        }
        if let Some(at) = self.asked {
            if at.elapsed() > Duration::from_secs(8) {
                note(format!("FAIL {step:?}: the {} renderer sent no screenshot", self.renderer));
                self.asked = None;
                self.step += 1;
                self.entered = Instant::now();
                if self.step >= STEPS.len() {
                    self.done = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                } else {
                    self.enter(STEPS[self.step]);
                }
            }
            return;
        }
        let wait = match step {
            Step::Splash => Duration::from_millis(500),
            Step::Tour => Duration::from_millis(2800),
            _ => Duration::from_millis(900),
        };
        if self.entered.elapsed() >= wait && self.frames >= 8 {
            self.asked = Some(Instant::now());
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot);
        }
    }

    fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4] {
        self.app.clear_color(visuals)
    }

    fn on_exit(&mut self, gl: Option<&eframe::glow::Context>) {
        self.app.on_exit(gl);
    }
}

pub fn finish(dir: &Path) -> i32 {
    let dir = absolute(dir);
    let lib = library(&dir);
    let mut lines: Vec<String> = NOTES.lock().map(|n| n.clone()).unwrap_or_default();
    let shots = lines.iter().filter(|l| l.starts_with("ok shot")).count();
    let old = std::fs::read_to_string(lib.join("scripts/the-long-way-down.md")).unwrap_or_default();
    lines.push(format!("{} an old script opened in every view is byte-identical", if old == OLD_SCRIPT { "ok" } else { "FAIL" }));
    let saved = std::fs::read_dir(lib.join("scripts"))
        .map(|rd| rd.flatten().any(|e| std::fs::read_to_string(e.path()).map(|t| t.contains(UNSAVED)).unwrap_or(false)))
        .unwrap_or(false);
    lines.push(format!("{} an edit made just before quitting was saved", if saved { "ok" } else { "FAIL" }));
    let pdf = std::fs::read_dir(lib.join("exports"))
        .map(|rd| {
            rd.flatten().any(|e| {
                e.path().extension().and_then(|x| x.to_str()) == Some("pdf")
                    && std::fs::read(e.path()).map(|b| b.starts_with(b"%PDF") && b.len() > 1000).unwrap_or(false)
            })
        })
        .unwrap_or(false);
    lines.push(format!("{} Quick Export wrote a PDF", if pdf { "ok" } else { "FAIL" }));
    let imported = std::fs::read_dir(lib.join("scripts")).map(|rd| rd.flatten().count()).unwrap_or(0) >= 3;
    lines.push(format!("{} the import became a script", if imported { "ok" } else { "FAIL" }));
    let all_steps = shots >= STEPS.len();
    lines.push(format!("{} {} of {} screens captured", if all_steps { "ok" } else { "FAIL" }, shots.min(STEPS.len()), STEPS.len()));
    let pass = !lines.iter().any(|l| l.starts_with("FAIL"));
    let report = format!("{}\n{}\n", if pass { "PASS" } else { "FAIL" }, lines.join("\n"));
    let _ = std::fs::write(dir.join("report.txt"), &report);
    print!("{report}");
    if pass {
        0
    } else {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_blank_frame_is_caught_and_a_real_one_is_not() {
        let flat = egui::ColorImage::new([200, 100], egui::Color32::from_rgb(10, 10, 12));
        assert!(blank(&flat));
        let mut busy = flat.clone();
        for (k, p) in busy.pixels.iter_mut().enumerate() {
            let v = ((k * 37) % 255) as u8;
            *p = egui::Color32::from_rgb(v, 255 - v, (v / 2).wrapping_add(40));
        }
        assert!(!blank(&busy));
    }

    #[test]
    fn only_a_frame_with_nothing_in_it_counts_as_not_read_back() {
        assert!(read_back_empty(&egui::ColorImage::new([8, 8], egui::Color32::TRANSPARENT)));
        assert!(!read_back_empty(&egui::ColorImage::new([8, 8], egui::Color32::BLACK)));
        assert!(!read_back_empty(&egui::ColorImage::new([8, 8], egui::Color32::from_rgb(7, 8, 13))));
        assert!(!read_back_empty(&egui::ColorImage { size: [0, 0], pixels: Vec::new() }));
    }

    fn bmp(w: usize, h: i32, bits: usize, px: impl Fn(usize, usize) -> [u8; 3]) -> Vec<u8> {
        let row = (w * bits / 8).div_ceil(4) * 4;
        let rows = h.unsigned_abs() as usize;
        let mut out = b"BM".to_vec();
        out.extend(((54 + row * rows) as u32).to_le_bytes());
        out.extend([0u8; 4]);
        out.extend(54u32.to_le_bytes());
        out.extend(40u32.to_le_bytes());
        out.extend((w as i32).to_le_bytes());
        out.extend(h.to_le_bytes());
        out.extend(1u16.to_le_bytes());
        out.extend((bits as u16).to_le_bytes());
        out.extend([0u8; 24]);
        for y in 0..rows {
            let mut line = Vec::new();
            for x in 0..w {
                let [r, g, b] = px(x, y);
                line.extend([b, g, r]);
                if bits == 32 {
                    line.push(255);
                }
            }
            line.resize(row, 0);
            out.extend(line);
        }
        out
    }

    #[test]
    fn a_screen_capture_bmp_reads_back_with_its_colours() {
        for (bits, h) in [(24, 3), (32, -3), (24, -3), (32, 3)] {
            let bytes = bmp(5, h, bits, |x, y| [(x * 40) as u8, (y * 60) as u8, 200]);
            let image = read_bmp(&bytes).unwrap();
            assert_eq!(image.size, [5, 3], "{bits} bits, height {h}");
            assert_eq!(image.pixels[1], egui::Color32::from_rgb(40, 0, 200), "{bits} bits, height {h}");
            assert_eq!(image.pixels[5 + 2], egui::Color32::from_rgb(80, 60, 200), "{bits} bits, height {h}");
        }
        assert!(read_bmp(b"PNG").is_err());
        assert!(read_bmp(&bmp(5, 3, 24, |_, _| [1, 2, 3])[..60]).is_err());
        let flat = read_bmp(&bmp(64, 32, 32, |_, _| [7, 8, 13])).unwrap();
        assert!(blank(&flat));
    }

    #[test]
    fn the_smoke_library_is_a_throwaway_folder() {
        let dir = std::env::temp_dir().join(format!("blackline-smoke-{}", std::process::id()));
        assert!(library(&dir).starts_with(dir.join("data")));
        assert!(crate::storage::from_markdown(OLD_SCRIPT).blocks.len() > 5);
        assert_eq!(crate::storage::to_markdown(&crate::storage::from_markdown(OLD_SCRIPT)), OLD_SCRIPT, "the old fixture round-trips byte for byte");
    }
}
