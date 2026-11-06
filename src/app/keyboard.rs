use eframe::egui::{self, Align, Key, Layout};

use super::App;
use crate::icons::Icon;
use crate::keys::{self, Command};
use crate::theme::{self, pal};
use crate::ui;

const GROUPS: [&str; 4] = ["Writing", "Export", "Views and panels", "Elements"];

impl App {
    pub(super) fn capture_chord(&mut self, ctx: &egui::Context) {
        let Some(cmd) = self.key_capture else {
            return;
        };
        if self.popover != Some(super::Popover::Settings) {
            self.key_capture = None;
            self.key_refusal = None;
            return;
        }
        let pressed = ctx.input_mut(|i| {
            let mut found = None;
            i.events.retain(|e| match e {
                egui::Event::Key { key, pressed: true, modifiers, .. } => {
                    if found.is_none() {
                        found = Some((*key, *modifiers));
                    }
                    false
                }
                egui::Event::Key { .. } | egui::Event::Text(_) => false,
                _ => true,
            });
            found
        });
        let Some((key, mods)) = pressed else {
            return;
        };
        if key == Key::Escape && !(mods.ctrl || mods.command || mods.alt || mods.shift) {
            self.key_capture = None;
            self.key_refusal = None;
            return;
        }
        let chord = keys::chord(mods, key);
        if let Some(why) = keys::refused(&chord, crate::WEB) {
            self.key_refusal = Some((cmd, format!("{}: {why}", keys::label(&chord))));
            return;
        }
        let commands = self.commands();
        if let Some(other) = self.settings.keys.taken_by(&chord, cmd, &commands, crate::WEB) {
            self.key_refusal = Some((
                cmd,
                format!("{} is already {}. Give that one another shortcut first.", keys::label(&chord), other.label()),
            ));
            return;
        }
        self.settings.keys.set(cmd, chord, crate::WEB);
        self.key_capture = None;
        self.key_refusal = None;
        self.save_settings();
    }

    pub(super) fn keyboard_settings(&mut self, ui: &mut egui::Ui) {
        let p = pal();
        ui.label(
            egui::RichText::new(
                "Click a shortcut, then press the keys you want it to be. Esc cancels. \
                 Your shortcuts are yours alone: they follow you to any browser you sign in on.",
            )
            .font(theme::font(theme::T_CAP))
            .color(p.text_dim),
        );
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let changed = !self.settings.keys.is_default();
            if changed && ui::button(ui, "Reset all to default", Some(Icon::Refresh), false) {
                self.settings.keys.reset_all();
                self.key_capture = None;
                self.key_refusal = None;
            }
            if !changed {
                ui.label(
                    egui::RichText::new("All shortcuts are Blackline's own.")
                        .font(theme::font(theme::T_CAP))
                        .color(p.text_faint),
                );
            }
        });
        ui.add_space(4.0);

        let commands = self.commands();
        for group in GROUPS {
            ui::section(ui, group);
            for cmd in commands.iter().copied().filter(|c| c.group() == group) {
                self.key_row(ui, cmd);
            }
            ui.add_space(4.0);
        }
        ui.label(
            egui::RichText::new(
                format!("Writing keys stay as they are: Enter, Tab and Shift+Tab, Backspace, and {}+Up and Down to move a block.", keys::alt_key()),
            )
            .font(theme::font(theme::T_CAP))
            .color(p.text_faint),
        );
    }

    fn key_row(&mut self, ui: &mut egui::Ui, cmd: Command) {
        let p = pal();
        let web = crate::WEB;
        let listening = self.key_capture == Some(cmd);
        let custom = self.settings.keys.is_custom(cmd);
        ui.horizontal(|ui| {
            ui.set_min_height(28.0);
            ui.label(egui::RichText::new(cmd.label()).font(theme::font(theme::T_SM)).color(p.text));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                if custom {
                    let back = cmd.defaults(web).first().map(keys::label).unwrap_or_default();
                    if ui::icon_button(ui, Icon::Refresh, &format!("Back to {back}"), 24.0) {
                        self.settings.keys.reset(cmd);
                        self.key_refusal = None;
                    }
                }
                let chords: Vec<String> = self.settings.keys.chords(cmd, web).iter().map(keys::label).collect();
                let text = if listening { "Press keys…".to_string() } else { chords.join("  or  ") };
                if keys::keycap(ui, &text, listening, custom) {
                    self.key_capture = if listening { None } else { Some(cmd) };
                    self.key_refusal = None;
                }
            });
        });
        if let Some((c, why)) = &self.key_refusal {
            if *c == cmd {
                ui.label(egui::RichText::new(why).font(theme::font(theme::T_CAP)).color(p.warn));
            }
        }
    }
}
