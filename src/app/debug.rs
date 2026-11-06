use eframe::egui::{self, Vec2};
use ncf::debug::Request;

use super::onboarding::Kind;
use super::{App, Ask, Popover, FULL_SIZE};
use crate::alerts::Tone;
use crate::theme::{self, pal};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Tour,
    WhatsNew,
    FailSave,
    Outside,
    Problems,
    Snapshot,
    Snapshots,
    Library,
    UpdateLog,
    ErrorLog,
}

const TOOLS: &[(&str, &[(&str, Tool)])] = &[
    ("Tour", &[("Tour", Tool::Tour), ("What's new", Tool::WhatsNew)]),
    (
        "Data safety",
        &[
            ("Fail next save", Tool::FailSave),
            ("Script from outside", Tool::Outside),
            ("Import with problems", Tool::Problems),
            ("Snapshot now", Tool::Snapshot),
            ("Snapshots", Tool::Snapshots),
        ],
    ),
    ("Files", &[("Library", Tool::Library), ("update.log", Tool::UpdateLog), ("error.log", Tool::ErrorLog)]),
];

// debug here---

impl App {
    pub(super) fn version_line(&mut self, ui: &mut egui::Ui) {
        let p = pal();
        let line = ui.label(
            egui::RichText::new(format!("Page font: {} · Blackline {}", theme::PAGE_FONT_NAME, env!("CARGO_PKG_VERSION")))
                .font(theme::font(theme::T_MICRO))
                .color(p.text_faint),
        );
        self.version_rect = line.rect;
        if self.debug.knock(ui, line.rect) {
            self.debug_switched();
        }
    }

    fn debug_switched(&mut self) {
        self.settings.debug = self.debug.on;
        self.save_settings();
        if self.debug.on {
            self.deck.say("Debug mode on", "Its panel opens from the DEBUG tag in the title bar.", Tone::Info);
        } else {
            self.deck.say("Debug mode off", "", Tone::Info);
        }
    }

    pub(super) fn debug_panel(&mut self, ctx: &egui::Context) {
        if !self.debug.on || !self.debug.open {
            return;
        }
        let mut debug = std::mem::take(&mut self.debug);
        let mut tools = Vec::new();
        let requests = debug.panel(ctx, FULL_SIZE, |ui, _| {
            for (title, row) in TOOLS {
                ncf::ui::section(ui, title);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
                    for (label, tool) in row.iter() {
                        if ncf::ui::quiet_button(ui, label, None) {
                            tools.push(*tool);
                        }
                    }
                });
            }
        });
        self.debug = debug;
        for request in requests {
            self.apply_debug(ctx, &request);
        }
        for tool in tools {
            self.use_tool(tool);
        }
    }

    pub fn apply_debug(&mut self, ctx: &egui::Context, request: &Request) {
        if ncf::debug::apply(ctx, &mut self.debug, request) {
            if *request == Request::Off {
                self.debug_switched();
            }
            return;
        }
        match request {
            Request::FakeUpdate => {
                let version = ncf::debug::next_version(crate::INFO.version);
                self.debug_offer_update(&version);
                self.offer = self.offer_for_update();
            }
            #[cfg(not(target_arch = "wasm32"))]
            Request::UpdaterDemo(demo) => match ncf::updater::start_demo(*demo) {
                Ok(()) => self.deck.say("Updater demo", "It opens in its own window. Nothing is installed.", Tone::Info),
                Err(e) => self.deck.say("Couldn't open the updater demo", &e, Tone::Warn),
            },
            #[cfg(not(target_arch = "wasm32"))]
            Request::UpdateFrom { dir, version } => {
                std::env::set_var("NCF_UPDATE_SOURCE", format!("file://{}", dir.trim_end_matches(['/', '\\'])));
                self.debug_offer_update(version);
                self.ask_update(version);
            }
            Request::Splash(status) => {
                self.splash = Some(crate::splash::Splash::replay(status.clone()));
                self.splash_replaying = true;
            }
            Request::Notice(tone) => {
                let (title, body) = ncf::debug::notice(*tone);
                self.deck.say(title, body, *tone);
            }
            Request::Confirm => self.deck.ask(
                "A confirm card",
                "This is how a question looks. Nothing happens either way.",
                "Fine",
                Tone::Info,
                Ask::Nothing,
            ),
            #[cfg(not(target_arch = "wasm32"))]
            Request::Crash => {
                let detail = "A test error from debug mode. Nothing is wrong.".to_string();
                let mut text = format!("Blackline stopped unexpectedly. Your scripts are saved as you type, so nothing should be lost.\n\n{detail}");
                match crate::storage::write_error_log(&detail) {
                    Some(log) => {
                        text.push_str(&format!("\n\nThis was also written to {}", log.display()));
                        self.deck.say("Crash report", &format!("Written to {}", log.display()), Tone::Info);
                    }
                    None => self.deck.say("Crash report", "error.log could not be written.", Tone::Warn),
                }
                std::thread::spawn(move || ncf::platform::alert("Blackline", &text));
            }
            Request::TextScale(step) => {
                self.settings.font_scale = (self.settings.font_scale + step).clamp(0.8, 1.4);
                self.theme_dirty = true;
                self.save_settings();
            }
            Request::Light(light) => {
                self.settings.light_mode = *light;
                self.theme_dirty = true;
                self.save_settings();
            }
            Request::Animations(on) => {
                self.settings.animations = *on;
                self.theme_dirty = true;
                self.save_settings();
            }
            _ => {}
        }
    }

    pub fn use_tool(&mut self, tool: Tool) {
        match tool {
            Tool::Tour => self.start_tour(Kind::Full),
            Tool::WhatsNew => self.start_tour(Kind::New),
            Tool::Snapshot => {
                if self.path.is_some() {
                    self.take_snapshot();
                } else {
                    self.deck.say("Open a script first", "", Tone::Info);
                }
            }
            Tool::Snapshots => {
                if self.home || self.path.is_none() {
                    self.deck.say("Open a script first", "", Tone::Info);
                } else {
                    self.popover = Some(Popover::Snapshots);
                    self.popover_frame = self.frame_no;
                }
            }
            Tool::Library => self.store.reveal(None),
            #[cfg(not(target_arch = "wasm32"))]
            Tool::FailSave => {
                crate::storage::fail_next_save();
                self.deck.say("The next save fails", "Type something and wait. The save after it works again.", Tone::Info);
            }
            #[cfg(not(target_arch = "wasm32"))]
            Tool::Outside => {
                let mut doc = crate::storage::import_text("outside.fountain", "INT. SOMEWHERE ELSE - DAY\n\nWritten without Blackline.\n");
                doc.meta.title = "Written Outside".into();
                let path = crate::storage::unique_path(&doc.meta.title, None);
                let wrote = crate::storage::ensure_dirs().and_then(|_| std::fs::write(&path, crate::storage::to_markdown(&doc)));
                match wrote {
                    Ok(()) => self.deck.say("Script added from outside", "The library should list it within a few seconds.", Tone::Info),
                    Err(e) => self.deck.say("Couldn't write the script", &e.to_string(), Tone::Warn),
                }
            }
            #[cfg(not(target_arch = "wasm32"))]
            Tool::Problems => self.import_bytes("odd.fountain", crate::smoke::ODD_FOUNTAIN.as_bytes()),
            #[cfg(not(target_arch = "wasm32"))]
            Tool::UpdateLog | Tool::ErrorLog => {
                let name = if tool == Tool::UpdateLog { "update.log" } else { "error.log" };
                let log = crate::storage::library_root().join(name);
                if log.exists() {
                    crate::storage::open_with_desktop(&log);
                } else {
                    self.deck.say(&format!("No {name} yet"), &log.display().to_string(), Tone::Info);
                }
            }
            #[cfg(target_arch = "wasm32")]
            _ => self.deck.say("Not on the web", "This one needs the desktop app.", Tone::Info),
        }
    }
}

#[allow(dead_code)]
impl App {
    pub fn debug_mode(&self) -> &ncf::debug::Debug {
        &self.debug
    }
    pub fn debug_mode_mut(&mut self) -> &mut ncf::debug::Debug {
        &mut self.debug
    }
    pub fn debug_splash_replaying(&self) -> bool {
        self.splash_replaying && self.splash.is_some()
    }
    pub fn debug_check_answers(&mut self, status: crate::update::Status) {
        self.update = status;
        self.offer = self.offer_for_update();
    }
    pub fn debug_update_offer(&self) -> Option<ncf::splash::Offer> {
        self.offer.clone()
    }
    pub fn debug_answer_offer(&mut self, choice: ncf::splash::Choice) {
        self.offer = None;
        self.answer_offer(choice);
    }
    pub fn update_status(&self) -> &crate::update::Status {
        &self.update
    }
    pub fn debug_version_rect(&self) -> egui::Rect {
        self.version_rect
    }
}
