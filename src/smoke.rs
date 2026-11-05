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
        Smoke { app, dir, renderer, step: 0, entered: Instant::now(), frames: 0, asked: None, done: false }
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
            let empty = blank(&image);
            note(format!(
                "{} shot {:?} {}x{}{}{}",
                if saved.is_ok() && !empty { "ok" } else { "FAIL" },
                step,
                image.size[0],
                image.size[1],
                if empty { " looks blank" } else { "" },
                saved.err().map(|e| format!(" could not save: {e}")).unwrap_or_default()
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
    fn the_smoke_library_is_a_throwaway_folder() {
        let dir = std::env::temp_dir().join(format!("blackline-smoke-{}", std::process::id()));
        assert!(library(&dir).starts_with(dir.join("data")));
        assert!(crate::storage::from_markdown(OLD_SCRIPT).blocks.len() > 5);
        assert_eq!(crate::storage::to_markdown(&crate::storage::from_markdown(OLD_SCRIPT)), OLD_SCRIPT, "the old fixture round-trips byte for byte");
    }
}
