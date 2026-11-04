#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum SortBy {
    #[default]
    Recent,
    Title,
    Length,
}

impl SortBy {
    pub const ALL: [SortBy; 3] = [SortBy::Recent, SortBy::Title, SortBy::Length];
    pub fn label(self) -> &'static str {
        match self {
            SortBy::Recent => "Last touched",
            SortBy::Title => "Title",
            SortBy::Length => "Length",
        }
    }
    fn slug(self) -> &'static str {
        match self {
            SortBy::Recent => "recent",
            SortBy::Title => "title",
            SortBy::Length => "length",
        }
    }
    fn from_slug(s: &str) -> SortBy {
        match s {
            "title" => SortBy::Title,
            "length" => SortBy::Length,
            _ => SortBy::Recent,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum AfterExport {
    #[default]
    Open,
    Reveal,
    Nothing,
}

impl AfterExport {
    pub const ALL: [AfterExport; 3] = [AfterExport::Open, AfterExport::Reveal, AfterExport::Nothing];
    pub fn label(self) -> &'static str {
        match self {
            AfterExport::Open => "Open it",
            AfterExport::Reveal => "Show file",
            AfterExport::Nothing => "Nothing",
        }
    }
    fn slug(self) -> &'static str {
        match self {
            AfterExport::Open => "open",
            AfterExport::Reveal => "reveal",
            AfterExport::Nothing => "nothing",
        }
    }
    fn from_slug(s: &str) -> AfterExport {
        match s {
            "reveal" => AfterExport::Reveal,
            "nothing" => AfterExport::Nothing,
            _ => AfterExport::Open,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    // look here---
    pub light_mode: bool,
    pub animations: bool,
    pub font_scale: f32,
    pub splash: bool,
    pub starred_first: bool,
    pub sort_by: SortBy,
    pub autosave_ms: u64,

    // app here---
    pub show_library: bool,
    pub show_scenes: bool,
    pub show_cast: bool,
    pub beat_guides: bool,
    pub pdf_tint: bool,
    pub dismissed_update: String,
    pub onboarded: String,
    pub fresh: bool,
    pub page_px: f32,
    pub page_breaks: bool,
    pub scene_numbers: bool,
    pub smart_type: bool,
    pub typewriter: bool,
    pub after_export: AfterExport,
    pub session_goal: u32,
    pub show_details: bool,
    pub element_colors: bool,
    pub character_colors: bool,
    pub character_colors_reading: bool,
    pub colour_seed: u32,
    pub custom_colors: bool,
    pub youtrack_url: String,
    pub check_updates: bool,
    // pdf carries remembered here---
    pub pdf_title_page: bool,
    pub pdf_scene_numbers: bool,
    pub pdf_character_colors: bool,
    pub pdf_notes: bool,
    pub keys: crate::keys::Keymap,
}

pub const DEFAULT_YOUTRACK: &str = "";

impl Default for Settings {
    fn default() -> Self {
        Settings {
            light_mode: false,
            animations: true,
            font_scale: 1.0,
            splash: true,
            starred_first: true,
            sort_by: SortBy::Recent,
            autosave_ms: 1200,
            show_library: true,
            show_scenes: true,
            show_cast: false,
            beat_guides: true,
            pdf_tint: false,
            dismissed_update: String::new(),
            onboarded: String::new(),
            fresh: true,
            page_px: 16.0,
            page_breaks: true,
            scene_numbers: false,
            smart_type: true,
            typewriter: false,
            after_export: AfterExport::Open,
            session_goal: 0,
            show_details: false,
            element_colors: true,
            character_colors: false,
            character_colors_reading: false,
            colour_seed: 0,
            custom_colors: false,
            youtrack_url: DEFAULT_YOUTRACK.to_string(),
            check_updates: true,
            pdf_title_page: true,
            pdf_scene_numbers: false,
            pdf_character_colors: false,
            pdf_notes: false,
            keys: crate::keys::Keymap::default(),
        }
    }
}

impl Settings {
    pub fn serialize(&self) -> String {
        let b = |v: bool| if v { "yes" } else { "no" };
        format!(
            "# Blackline settings.\n\
             light_mode = {}\n\
             animations = {}\n\
             font_scale = {:.2}\n\
             splash = {}\n\
             starred_first = {}\n\
             sort_by = {}\n\
             autosave_ms = {}\n\
             show_library = {}\n\
             show_scenes = {}\n\
             show_cast = {}\n\
             page_px = {:.0}\n\
             page_breaks = {}\n\
             scene_numbers = {}\n\
             smart_type = {}\n\
             typewriter = {}\n\
             after_export = {}\n\
             session_goal = {}\n\
             show_details = {}\n\
             element_colors = {}\n\
             character_colors = {}\n\
             character_colors_reading = {}\n\
             colour_seed = {}\n\
             youtrack_url = {}\n\
             check_updates = {}\n\
             pdf_title_page = {}\n\
             pdf_scene_numbers = {}\n\
             pdf_character_colors = {}\n\
             pdf_notes = {}\n\
             beat_guides = {}\n\
             pdf_tint = {}\n\
             dismissed_update = {}\n\
             onboarded = {}\n",
            b(self.light_mode),
            b(self.animations),
            self.font_scale,
            b(self.splash),
            b(self.starred_first),
            self.sort_by.slug(),
            self.autosave_ms,
            b(self.show_library),
            b(self.show_scenes),
            b(self.show_cast),
            self.page_px,
            b(self.page_breaks),
            b(self.scene_numbers),
            b(self.smart_type),
            b(self.typewriter),
            self.after_export.slug(),
            self.session_goal,
            b(self.show_details),
            b(self.element_colors),
            b(self.character_colors),
            b(self.character_colors_reading),
            self.colour_seed,
            self.youtrack_url.trim(),
            b(self.check_updates),
            b(self.pdf_title_page),
            b(self.pdf_scene_numbers),
            b(self.pdf_character_colors),
            b(self.pdf_notes),
            b(self.beat_guides),
            b(self.pdf_tint),
            self.dismissed_update.trim(),
            self.onboarded.trim(),
        ) + if self.custom_colors {
            "character_colors_mode = custom\n"
        } else {
            ""
        } + &self.keys.serialize()
    }

    pub fn parse(text: &str) -> Settings {
        let mut s = Settings { fresh: false, ..Default::default() };
        let yes = |v: &str| matches!(v, "yes" | "true" | "1" | "on");
        let mut pdf_numbers_said = false;
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            let (k, v) = (k.trim(), v.trim());
            match k {
                "light_mode" => s.light_mode = yes(v),
                "animations" => s.animations = yes(v),
                "font_scale" => s.font_scale = v.parse::<f32>().unwrap_or(1.0).clamp(0.8, 1.4),
                "splash" => s.splash = yes(v),
                "starred_first" => s.starred_first = yes(v),
                "sort_by" => s.sort_by = SortBy::from_slug(v),
                "autosave_ms" => {
                    s.autosave_ms = v.parse::<u64>().unwrap_or(1200).clamp(150, 10_000)
                }
                "show_library" => s.show_library = yes(v),
                "show_scenes" => s.show_scenes = yes(v),
                "show_cast" => s.show_cast = yes(v),
                "beat_guides" => s.beat_guides = yes(v),
                "pdf_tint" => s.pdf_tint = yes(v),
                "dismissed_update" => s.dismissed_update = v.trim().to_string(),
                "onboarded" => s.onboarded = v.trim().to_string(),
                "page_px" => s.page_px = v.parse::<f32>().unwrap_or(16.0).clamp(11.0, 26.0),
                "page_breaks" => s.page_breaks = yes(v),
                "scene_numbers" => s.scene_numbers = yes(v),
                "smart_type" => s.smart_type = yes(v),
                "typewriter" => s.typewriter = yes(v),
                "after_export" => s.after_export = AfterExport::from_slug(v),
                "session_goal" => s.session_goal = v.parse::<u32>().unwrap_or(0).min(20_000),
                "show_details" => s.show_details = yes(v),
                "element_colors" => s.element_colors = yes(v),
                "character_colors" => s.character_colors = yes(v),
                "character_colors_reading" => s.character_colors_reading = yes(v),
                "colour_seed" => s.colour_seed = v.parse::<u32>().unwrap_or(0),
                "character_colors_mode" => s.custom_colors = v == "custom",
                k if k.starts_with("key.") => {
                    s.keys.parse_line(k, v);
                }
                "pdf_title_page" => s.pdf_title_page = yes(v),
                "pdf_scene_numbers" => {
                    s.pdf_scene_numbers = yes(v);
                    pdf_numbers_said = true;
                }
                "pdf_character_colors" => s.pdf_character_colors = yes(v),
                "pdf_notes" => s.pdf_notes = yes(v),
                "check_updates" => s.check_updates = yes(v),
                "youtrack_url" => {
                    s.youtrack_url = v.to_string()
                }
                _ => {}
            }
        }
        if !pdf_numbers_said {
            s.pdf_scene_numbers = s.scene_numbers;
        }
        s
    }
}
