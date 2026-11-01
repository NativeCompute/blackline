use eframe::egui::{Key, KeyboardShortcut, Modifiers};

pub use ncf::keys::{chord, keycap, parse, refused, text, DIGITS};

use crate::model::Element;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Command {
    Save,
    NewScript,
    NewScene,
    NewAct,
    Undo,
    Redo,
    Find,
    FindScripts,
    QuickExport,
    ExportWindow,
    NextView,
    FocusMode,
    ToggleLibrary,
    ToggleDetails,
    ToggleScenes,
    Settings,
    Bigger,
    Smaller,
    Element(u8),
}

use Command::*;

impl Command {
    pub const ALL: [Command; 25] = [
        Save,
        NewScript,
        NewScene,
        NewAct,
        Undo,
        Redo,
        Find,
        FindScripts,
        QuickExport,
        ExportWindow,
        NextView,
        FocusMode,
        ToggleLibrary,
        ToggleDetails,
        ToggleScenes,
        Settings,
        Bigger,
        Smaller,
        Element(1),
        Element(2),
        Element(3),
        Element(4),
        Element(5),
        Element(6),
        Element(7),
    ];

    pub fn slug(self) -> String {
        match self {
            Save => "save".into(),
            NewScript => "new_script".into(),
            NewScene => "new_scene".into(),
            NewAct => "new_act".into(),
            Undo => "undo".into(),
            Redo => "redo".into(),
            Find => "find".into(),
            FindScripts => "find_scripts".into(),
            QuickExport => "quick_export".into(),
            ExportWindow => "export".into(),
            NextView => "next_view".into(),
            FocusMode => "focus_mode".into(),
            ToggleLibrary => "library".into(),
            ToggleDetails => "details".into(),
            ToggleScenes => "scenes".into(),
            Settings => "settings".into(),
            Bigger => "text_bigger".into(),
            Smaller => "text_smaller".into(),
            Element(n) => format!("element_{n}"),
        }
    }

    pub fn from_slug(s: &str) -> Option<Command> {
        Command::ALL.into_iter().find(|c| c.slug() == s)
    }

    pub fn label(self) -> String {
        match self {
            Save => "Save".into(),
            NewScript => "New script".into(),
            NewScene => "New scene".into(),
            NewAct => "New act".into(),
            Undo => "Undo".into(),
            Redo => "Redo".into(),
            Find => "Find and replace".into(),
            FindScripts => "Search your scripts".into(),
            QuickExport => "Quick Export (PDF)".into(),
            ExportWindow => "Export…".into(),
            NextView => "Next view (Write, Cards, Read)".into(),
            FocusMode => "Focus mode".into(),
            ToggleLibrary => "Show or hide the library".into(),
            ToggleDetails => "Show or hide Details".into(),
            ToggleScenes => "Show or hide Scenes".into(),
            Settings => "Settings".into(),
            Bigger => "Larger page text".into(),
            Smaller => "Smaller page text".into(),
            Element(n) => Element::from_digit(n as usize).map(|e| e.label()).unwrap_or("Element").to_string(),
        }
    }

    pub fn group(self) -> &'static str {
        match self {
            Save | NewScript | NewScene | NewAct | Undo | Redo | Find | FindScripts => "Writing",
            QuickExport | ExportWindow => "Export",
            NextView | FocusMode | ToggleLibrary | ToggleDetails | ToggleScenes | Settings | Bigger | Smaller => {
                "Views and panels"
            }
            Element(_) => "Elements",
        }
    }

    pub fn defaults(self, web: bool) -> Vec<KeyboardShortcut> {
        let c = |m: Modifiers, k: Key| KeyboardShortcut::new(m, k);
        let cmd = Modifiers::COMMAND;
        let cmd_shift = Modifiers::COMMAND | Modifiers::SHIFT;
        match self {
            Save => vec![c(cmd, Key::S)],
            NewScript if web => vec![c(cmd | Modifiers::ALT, Key::N), c(cmd, Key::N)],
            NewScript => vec![c(cmd, Key::N), c(cmd | Modifiers::ALT, Key::N)],
            NewScene => vec![c(cmd, Key::Enter)],
            NewAct => vec![c(cmd_shift, Key::Enter)],
            Undo => vec![c(cmd, Key::Z)],
            Redo => vec![c(cmd_shift, Key::Z), c(cmd, Key::Y)],
            Find => vec![c(cmd, Key::F), c(cmd, Key::H)],
            FindScripts => vec![c(cmd_shift, Key::F)],
            QuickExport => vec![c(cmd, Key::E)],
            ExportWindow => vec![c(cmd_shift, Key::E)],
            NextView => vec![c(cmd, Key::G)],
            FocusMode => vec![c(cmd, Key::Period)],
            ToggleLibrary => vec![c(cmd, Key::B)],
            ToggleDetails => vec![c(cmd, Key::I)],
            ToggleScenes => vec![c(cmd_shift, Key::I)],
            Settings => vec![c(cmd, Key::Comma)],
            Bigger => vec![c(cmd, Key::Plus), c(cmd, Key::Equals)],
            Smaller => vec![c(cmd, Key::Minus)],
            Element(n) => {
                let k = DIGITS[(n as usize).clamp(1, 7) - 1];
                if web {
                    vec![c(Modifiers::ALT, k), c(cmd, k)]
                } else {
                    vec![c(cmd, k), c(Modifiers::ALT, k)]
                }
            }
        }
    }
}

impl ncf::keys::Command for Command {
    fn all() -> Vec<Self> {
        Command::ALL.to_vec()
    }
    fn slug(self) -> String {
        Command::slug(self)
    }
    fn label(self) -> String {
        Command::label(self)
    }
    fn group(self) -> &'static str {
        Command::group(self)
    }
    fn defaults(self, web: bool) -> Vec<KeyboardShortcut> {
        Command::defaults(self, web)
    }
    fn from_slug(s: &str) -> Option<Self> {
        Command::from_slug(s)
    }
}

pub type Keymap = ncf::keys::Keymap<Command>;
