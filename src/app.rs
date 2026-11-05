use std::collections::HashMap;
use std::path::{Path, PathBuf};
use web_time::{Duration, Instant, SystemTime};

use eframe::egui::{self, Align, Color32, Layout, Pos2, Rect, Sense, Stroke, Vec2};

use crate::alerts::{self, Tone};
use crate::anim;
use crate::backend::{Access, Event, Load, Store};
use crate::caret;
use crate::cards::{self, CardsState};
use crate::chrome;
use crate::editor::{self, Caret, EditorState};
use crate::export::{self, Format};
use crate::icons::{self, Icon};
use crate::keys;
use crate::logo;
use crate::model::{act_title, eighths_label, CastMember, Document, Element};
use crate::pages::{self, PagesState};
use crate::settings::{AfterExport, Settings, SortBy};
use crate::splash::Splash;
use crate::storage::{self, Entry};
use crate::theme::{self, pal};
use crate::ui;

mod home;
mod team;
mod keyboard;
mod notes;
mod reports;
mod imported;
pub mod onboarding;
mod voices;

const FULL_SIZE: Vec2 = Vec2::new(1320.0, 900.0);
const MIN_SIZE: Vec2 = Vec2::new(880.0, 580.0);
const SNAPSHOT_IDLE: Duration = Duration::from_millis(700);

const TEAM_TAB: usize = 7;
const KEYS_TAB: usize = 8;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Write,
    Cards,
    Read,
}

impl Mode {
    fn index(self) -> usize {
        match self {
            Mode::Write => 0,
            Mode::Cards => 1,
            Mode::Read => 2,
        }
    }
    fn from_index(i: usize) -> Mode {
        match i {
            1 => Mode::Cards,
            2 => Mode::Read,
            _ => Mode::Write,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Ask {
    DeleteScript(PathBuf),
    DeleteScene(usize),
    RenameScript,
    RestoreSnapshot(PathBuf),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Popover {
    Menu,
    Settings,
    Export,
    Snapshots,
    Elements,
    Account,
    History,
    Trash,
    Reports,
    Imported,
}

#[derive(Default)]
struct Layout2 {
    page_starts: HashMap<u64, usize>,
    lengths: HashMap<u64, (usize, usize)>,
    pages: usize,
    words: usize,
    scenes: usize,
    acts: HashMap<u64, String>,
    cast: Vec<CastMember>,
    speakers: Vec<String>,
    dialogue: f32,
    block_pages: HashMap<u64, usize>,
    changes: std::collections::HashSet<u64>,
    labels: Vec<String>,
}

#[derive(Default)]
struct Rail {
    menu: Option<(PathBuf, Pos2, u64)>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum PageChoice {
    #[default]
    All,
    This,
    Chosen,
}

#[derive(Default)]
struct Find {
    open: bool,
    needle: String,
    with: String,
    current: usize,
    focus: bool,
    rect: Option<Rect>,
}

pub struct App {
    store: Box<dyn Store>,
    doc: Document,
    path: Option<PathBuf>,
    access: Access,
    opening: Option<PathBuf>,
    restoring: Option<PathBuf>,
    lock_free: bool,
    read_only_said: Option<Instant>,
    home: bool,
    home_born: Instant,
    home_search: String,
    focus_home_search: bool,
    home_sel: usize,
    home_order: Vec<PathBuf>,
    home_keyed: bool,
    home_hits: Option<(String, Vec<storage::Found>)>,
    hit_rows: Vec<(PathBuf, usize, Rect)>,
    pending_jump: Option<(PathBuf, usize)>,
    account_button_rect: Rect,
    team_add_id: String,
    team_add_role: crate::backend::Role,
    entries: Vec<Entry>,
    ed: EditorState,
    cards: CardsState,
    pages: PagesState,
    mode: Mode,

    focus_title: bool,
    focus_mode: bool,

    settings: Settings,
    update: crate::update::Status,
    #[cfg(not(target_arch = "wasm32"))]
    update_rx: Option<std::sync::mpsc::Receiver<crate::update::Status>>,
    theme_dirty: bool,

    dirty: bool,
    last_change: Option<Instant>,
    saved_at: Option<Instant>,

    undo: Vec<Document>,
    redo: Vec<Document>,
    baseline: Document,
    snapshot_due: bool,

    layout: Layout2,
    layout_at: Option<Instant>,
    layout_stale: bool,
    session_words: i64,
    words_seen: Option<(PathBuf, usize)>,
    cast_cursor: HashMap<String, usize>,
    voice_pick: Option<voices::Pick>,
    key_capture: Option<keys::Command>,
    key_refusal: Option<(keys::Command, String)>,

    deck: alerts::Deck<Ask>,
    popover: Option<Popover>,
    popover_frame: u64,
    settings_tab: usize,
    export_choice: PageChoice,
    export_changed_only: bool,
    export_range: String,
    window_seen: Option<Popover>,
    menu_button_rect: Rect,
    settings_button_rect: Rect,
    element_button_rect: Rect,
    menu_item_rects: Vec<Rect>,
    page_rect: Rect,
    foot_rect: Rect,
    aside_rect: Rect,
    aside_islands: Vec<Rect>,

    rail: Rail,
    find: Find,
    note_edit: Option<notes::NoteEdit>,
    left_tab: usize,
    left_rect: Rect,
    left_rows: Vec<(u64, Rect)>,
    report: crate::report::Report,
    report_tab: usize,
    update_rect: Rect,
    tour: Option<onboarding::Tour>,
    imported: Option<imported::Imported>,
    report_rows: Vec<(u64, Rect)>,
    library_button: Rect,
    rev_base: Option<(String, Document)>,

    frame_no: u64,
    page_key: String,
    page_born: Instant,
    splash: Option<Splash>,
    settled: bool,
    pub arrivals: Vec<PathBuf>,
}

impl App {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut app = App::with_context(&cc.egui_ctx);
        if app.settings.check_updates {
            app.update_rx = Some(crate::update::spawn(crate::INFO.repo, crate::INFO.version));
            app.update = crate::update::Status::Checking;
        }
        app
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn with_context(ctx: &egui::Context) -> Self {
        App::with_store(ctx, Box::new(crate::backend::LocalBackend::new()))
    }

    pub fn with_store(ctx: &egui::Context, mut store: Box<dyn Store>) -> Self {
        let settings = store.read_settings();
        theme::set_palette(!settings.light_mode);
        anim::set_enabled(settings.animations);
        theme::apply(ctx);
        theme::install_fonts(ctx);
        ctx.set_zoom_factor(settings.font_scale);

        let entries = store.list();
        let first_run = settings.fresh && entries.is_empty() && store.team().is_none();
        let home = store.team().is_some() || !entries.is_empty();
        let (doc, path) = match entries.first() {
            Some(_) if home => (starter_document(), None),
            Some(e) => match store.load(&e.path) {
                Load::Ready(d, _) => (*d, Some(e.path.clone())),
                Load::Pending | Load::Failed(_) => (starter_document(), None),
            },
            None => (starter_document(), None),
        };

        let splash = if settings.splash {
            Some(Splash::default())
        } else {
            None
        };
        let mut ed = EditorState::default();
        ed.font_px = settings.page_px;

        let mut app = App {
            store,
            baseline: doc.clone(),
            doc,
            path,
            access: Access::Edit,
            opening: None,
            restoring: None,
            lock_free: false,
            read_only_said: None,
            home,
            home_born: Instant::now(),
            home_search: String::new(),
            focus_home_search: false,
            home_sel: 0,
            home_order: Vec::new(),
            home_keyed: false,
            home_hits: None,
            hit_rows: Vec::new(),
            pending_jump: None,
            account_button_rect: Rect::NOTHING,
            team_add_id: String::new(),
            team_add_role: crate::backend::Role::Editor,
            entries,
            ed,
            cards: CardsState::default(),
            pages: PagesState::default(),
            mode: Mode::Write,
            focus_title: false,
            focus_mode: false,
            settings,
            update: crate::update::Status::Off,
            #[cfg(not(target_arch = "wasm32"))]
            update_rx: None,
            theme_dirty: false,
            dirty: false,
            last_change: None,
            saved_at: None,
            undo: Vec::new(),
            redo: Vec::new(),
            snapshot_due: false,
            layout: Layout2::default(),
            layout_at: None,
            layout_stale: true,
            session_words: 0,
            words_seen: None,
            cast_cursor: HashMap::new(),
            voice_pick: None,
            key_capture: None,
            key_refusal: None,
            deck: alerts::Deck::default(),
            popover: None,
            popover_frame: 0,
            settings_tab: 0,
            export_choice: PageChoice::All,
            export_changed_only: false,
            export_range: String::new(),
            window_seen: None,
            menu_button_rect: Rect::NOTHING,
            settings_button_rect: Rect::NOTHING,
            element_button_rect: Rect::NOTHING,
            menu_item_rects: Vec::new(),
            page_rect: Rect::NOTHING,
            foot_rect: Rect::NOTHING,
            aside_rect: Rect::NOTHING,
            aside_islands: Vec::new(),
            rail: Rail::default(),
            find: Find::default(),
            note_edit: None,
            left_tab: 0,
            left_rect: Rect::NOTHING,
            left_rows: Vec::new(),
            report: crate::report::Report::default(),
            report_tab: 0,
            update_rect: Rect::NOTHING,
            tour: None,
            imported: None,
            report_rows: Vec::new(),
            library_button: Rect::NOTHING,
            rev_base: None,
            frame_no: 0,
            page_key: String::new(),
            page_born: Instant::now(),
            splash,
            settled: false,
            arrivals: Vec::new(),
        };

        if app.path.is_none() && !app.home && app.store.team().is_none() {
            app.create_current();
            app.refresh_entries();
        }
        app.ed.focus_block = app.doc.blocks.first().map(|b| b.id);
        app.ed.pending_focus = app.doc.blocks.first().map(|b| (b.id, Caret::End));
        app.refresh_layout(true);
        if let Some(kind) = onboarding::wanted(&app.settings.onboarded, first_run) {
            app.start_tour(kind);
        }
        app
    }

    // document lifecycle here---

    fn mark_changed(&mut self) {
        if !self.access.can_edit() {
            self.doc = self.baseline.clone();
            self.ed.fresh = true;
            self.layout_stale = true;
            self.guard_edit();
            return;
        }
        self.dirty = true;
        self.last_change = Some(Instant::now());
        self.snapshot_due = true;
        self.layout_stale = true;
    }

    fn snapshot_now(&mut self) {
        let current = self.doc.clone();
        self.undo.push(std::mem::replace(&mut self.baseline, current));
        if self.undo.len() > 200 {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.snapshot_due = false;
    }

    fn checkpoint(&mut self) {
        if self.snapshot_due {
            self.snapshot_now();
        }
        self.undo.push(self.doc.clone());
        if self.undo.len() > 200 {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    fn after_history(&mut self) {
        self.doc.ensure_not_empty();
        self.doc.reseed_ids();
        self.dirty = true;
        self.last_change = Some(Instant::now());
        self.layout_stale = true;
        self.ed.fresh = true;
    }

    fn undo(&mut self) {
        if self.snapshot_due {
            self.snapshot_now();
        }
        match self.undo.pop() {
            Some(prev) => {
                self.redo.push(self.doc.clone());
                self.doc = prev.clone();
                self.baseline = prev;
                self.after_history();
            }
            None => self.deck.say("Nothing to undo", "", Tone::Info),
        }
    }

    fn redo(&mut self) {
        match self.redo.pop() {
            Some(next) => {
                self.undo.push(self.doc.clone());
                self.doc = next.clone();
                self.baseline = next;
                self.after_history();
            }
            None => self.deck.say("Nothing to redo", "", Tone::Info),
        }
    }

    fn save(&mut self, allow_rename: bool) {
        if !self.access.can_edit() {
            self.dirty = false;
            return;
        }
        self.doc.normalize();
        let Some(mut path) = self.path.clone() else {
            self.create_current();
            return;
        };

        if allow_rename {
            path = self.store.follow_title(&path, &self.doc.meta.title);
        }

        match self.store.save(&path, &self.doc) {
            Ok(()) => {
                self.path = Some(path);
                self.dirty = false;
                self.saved_at = Some(Instant::now());
                self.refresh_entries();
            }
            Err(e) => self.deck.say("Could not save", &e, Tone::Danger),
        }
    }

    fn create_current(&mut self) {
        self.doc.normalize();
        match self.store.create(&self.doc) {
            Ok(path) => {
                self.path = Some(path);
                self.access = Access::Edit;
                self.dirty = false;
                self.saved_at = Some(Instant::now());
                self.refresh_entries();
            }
            Err(e) => self.deck.say("Could not save", &e, Tone::Danger),
        }
    }

    fn open(&mut self, path: PathBuf) {
        if self.dirty {
            self.save(false);
        }
        if let Some(old) = self.path.clone() {
            if old != path {
                self.store.close(&old);
            }
        }
        match self.store.load(&path) {
            Load::Ready(doc, access) => self.install(path, *doc, access),
            Load::Pending => self.opening = Some(path),
            Load::Failed(e) => self.deck.say("Could not open", &e, Tone::Danger),
        }
    }

    fn install(&mut self, path: PathBuf, doc: Document, access: Access) {
        self.opening = None;
        self.home = false;
        self.doc = doc;
        self.path = Some(path);
        self.access = access;
        self.lock_free = false;
        self.baseline = self.doc.clone();
        self.undo.clear();
        self.redo.clear();
        self.dirty = false;
        self.snapshot_due = false;
        self.layout_stale = true;
        self.words_seen = None;
        self.find.current = 0;
        self.ed.fresh = true;
        self.ed.focus_block = self.doc.blocks.first().map(|b| b.id);
        if !self.access.can_edit() {
            self.mode = Mode::Read;
        }
        if self.mode == Mode::Write {
            self.ed.pending_focus = self.doc.blocks.first().map(|b| (b.id, Caret::End));
        }
        if let Some((want, k)) = self.pending_jump.take() {
            if self.path.as_ref() == Some(&want) {
                if let Some(id) = self.doc.blocks.get(k).map(|b| b.id) {
                    self.ed.focus_block = Some(id);
                    self.ed.jump_to(id);
                }
            }
        }
        self.refresh_layout(true);
    }

    fn new_script(&mut self) {
        if !self.can_write() {
            return;
        }
        if self.dirty {
            self.save(false);
        }
        let mut doc = Document::default();
        doc.meta.title = "Untitled Script".to_string();
        doc.meta.draft = today();
        if let Some(team) = self.store.team() {
            doc.meta.author = team.me.name.clone();
        }
        if let Some(old) = self.path.take() {
            self.store.close(&old);
        }
        self.doc = doc;
        self.undo.clear();
        self.redo.clear();
        self.baseline = self.doc.clone();
        self.create_current();
        self.refresh_entries();
        self.ed.fresh = true;
        self.words_seen = None;
        self.mode = Mode::Write;
        self.home = false;
        self.focus_title = true;
        self.refresh_layout(true);
        self.deck.ok("Script created", "Name it, then press Enter to start writing.");
    }

    fn duplicate(&mut self, path: &Path) {
        if self.path.as_deref() == Some(path) && self.dirty {
            self.save(false);
        }
        match self.store.duplicate(path) {
            Ok(Some(title)) => {
                self.refresh_entries();
                self.deck.ok("Duplicated", &title);
            }
            Ok(None) => {}
            Err(e) => self.deck.say("Could not duplicate", &e, Tone::Danger),
        }
    }

    fn delete(&mut self, path: &Path) {
        let title = self
            .entries
            .iter()
            .find(|e| e.path == path)
            .map(|e| e.title.clone())
            .unwrap_or_default();
        match self.store.delete(path) {
            Ok(()) => {
                if self.path.as_deref() == Some(path) {
                    self.dirty = false;
                    self.refresh_entries();
                    match self.entries.first().map(|e| e.path.clone()) {
                        Some(next) => {
                            self.path = None;
                            self.open(next)
                        }
                        None => {
                            self.path = None;
                            self.doc = starter_document();
                            self.create_current();
                            self.ed.fresh = true;
                        }
                    }
                }
                self.refresh_entries();
                if self.store.team().is_some() {
                    self.deck.say("Moved to Recently deleted", &title, Tone::Warn);
                } else {
                    self.deck.say("Script deleted", &title, Tone::Warn);
                }
            }
            Err(e) => self.deck.say("Could not delete", &e, Tone::Danger),
        }
    }

    fn toggle_star(&mut self, path: &Path) {
        if self.store.team().is_none() && self.path.as_deref() == Some(path) {
            self.doc.meta.starred = !self.doc.meta.starred;
            self.save(false);
            return;
        }
        let on = !self
            .entries
            .iter()
            .find(|e| e.path == path)
            .map(|e| e.starred)
            .unwrap_or(false);
        match self.store.set_starred(path, on) {
            Ok(()) => self.refresh_entries(),
            Err(e) if self.store.team().is_some() => self.deck.say("Could not star", &e, Tone::Danger),
            Err(_) => {}
        }
    }

    fn refresh_entries(&mut self) {
        self.entries = self.store.list();
        self.home_hits = None;
    }

    fn watch_library(&mut self) {
        if self.store.changed() {
            self.refresh_entries();
        }
    }

    fn refresh_layout(&mut self, now: bool) {
        let stale_enough = self
            .layout_at
            .map(|t| t.elapsed() > Duration::from_millis(350))
            .unwrap_or(true);
        if !(self.layout_stale && (stale_enough || now)) {
            return;
        }
        self.ensure_rev_base();
        let changes = match &self.rev_base {
            Some((_, base)) if self.doc.meta.revising() => crate::revision::changed(base, &self.doc),
            _ => Default::default(),
        };
        let laid = export::layout(&self.doc, self.rev_lock());
        let lengths = export::scene_lengths(&self.doc);
        let pages = laid.len();
        let mut block_pages: HashMap<u64, usize> = HashMap::new();
        let mut starts: HashMap<u64, usize> = HashMap::new();
        for (k, (_, page)) in laid.iter().enumerate() {
            if k > 0 {
                if let Some(id) = page.iter().find_map(|l| l.block) {
                    starts.entry(id).or_insert(k + 1);
                }
            }
            for l in page {
                for id in [l.block, l.right.as_ref().and_then(|r| r.block)].into_iter().flatten() {
                    block_pages.entry(id).or_insert(k + 1);
                }
            }
        }
        let labels: Vec<String> = laid.into_iter().map(|(l, _)| l).collect();
        self.layout = Layout2 {
            block_pages,
            changes,
            labels,
            acts: editor::act_notes(&self.doc, &starts, pages),
            page_starts: starts,
            pages,
            lengths: lengths.into_iter().map(|(id, pg, e)| (id, (pg, e))).collect(),
            words: self.doc.word_count(),
            scenes: self.doc.scene_count(),
            cast: self.doc.cast(),
            speakers: self.doc.speakers(),
            dialogue: self.doc.dialogue_share(),
        };
        if let Some(path) = self.path.clone() {
            match &self.words_seen {
                Some((p, before)) if *p == path => {
                    self.session_words += self.layout.words as i64 - *before as i64;
                }
                _ => {}
            }
            self.words_seen = Some((path, self.layout.words));
        }
        self.layout_at = Some(Instant::now());
        self.layout_stale = false;
    }

    pub(super) fn rev_lock(&self) -> Option<&Document> {
        match &self.rev_base {
            Some((_, base)) if self.doc.meta.revising() && self.doc.meta.locked => Some(base),
            _ => None,
        }
    }

    fn changed_page_count(&self) -> usize {
        let mut pages: Vec<usize> = self.layout.changes.iter().filter_map(|id| self.layout.block_pages.get(id).copied()).collect();
        pages.sort_unstable();
        pages.dedup();
        pages.len()
    }

    pub(super) fn page_label(&self, n: usize) -> String {
        self.layout.labels.get(n.wrapping_sub(1)).cloned().unwrap_or_else(|| n.to_string())
    }

    fn pdf_options(&self, pages: Option<Vec<usize>>) -> export::PdfOptions {
        let revising = self.doc.meta.revising();
        let colour = self.doc.meta.revision.unwrap_or(0);
        export::PdfOptions {
            title_page: self.settings.pdf_title_page && !(revising && self.export_changed_only),
            scene_numbers: self.settings.pdf_scene_numbers,
            notes: self.settings.pdf_notes,
            changed: if revising { self.layout.changes.clone() } else { Default::default() },
            revision: revising.then(|| (crate::revision::header(colour, &self.doc.meta.revision_date), crate::revision::paper(colour))),
            lock: self.rev_lock().map(|d| Box::new(d.clone())),
            changed_only: revising && self.export_changed_only,
            tint: revising && self.settings.pdf_tint,
            pages,
            voices: if self.settings.pdf_character_colors {
                Some(theme::voice_inks(&self.layout.speakers, self.settings.colour_seed, self.chosen_voices()))
            } else {
                None
            },
        }
    }

    fn page_here(&self) -> Option<usize> {
        if self.mode == Mode::Read {
            return (self.pages.current > 0).then_some(self.pages.current);
        }
        self.ed
            .focus_block
            .and_then(|id| self.layout.block_pages.get(&id).copied().or_else(|| export::page_of_block(&self.doc, id)))
            .or(Some(1))
    }

    fn export_selection(&self) -> Result<Option<Vec<usize>>, String> {
        match self.export_choice {
            PageChoice::All => Ok(None),
            PageChoice::This => Ok(Some(self.page_here().into_iter().collect())),
            PageChoice::Chosen => export::parse_page_range(&self.export_range, self.layout.pages).map(Some),
        }
    }

    fn open_export_window(&mut self) {
        self.refresh_layout(true);
        self.popover = Some(Popover::Export);
        self.popover_frame = self.frame_no;
    }

    fn export_pdf_with(&mut self, pages: Option<Vec<usize>>) {
        self.doc.normalize();
        let opts = self.pdf_options(pages);
        let after = self.settings.after_export;
        let done = export::render(&self.doc, Format::Pdf, &opts)
            .and_then(|(name, bytes)| self.store.deliver(&name, bytes, Some(after)));
        match done {
            Ok(name) => self.deck.ok("Exported", &name),
            Err(e) => self.deck.say("Export failed", &e, Tone::Danger),
        }
    }

    fn export(&mut self, format: Format, quick: bool) {
        self.doc.normalize();
        let opts = self.pdf_options(None);
        let after = (format == Format::Pdf && quick).then_some(self.settings.after_export);
        let done = export::render(&self.doc, format, &opts)
            .and_then(|(name, bytes)| self.store.deliver(&name, bytes, after));
        match done {
            Ok(name) => self.deck.ok("Exported", &name),
            Err(e) => self.deck.say("Export failed", &e, Tone::Danger),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn import_path(&mut self, path: &Path) {
        match storage::import_file_checked(path) {
            Ok((doc, issues)) => self.import_doc(doc, issues),
            Err(e) => self.deck.say("Could not import", &e, Tone::Danger),
        }
    }

    fn import_bytes(&mut self, name: &str, bytes: &[u8]) {
        let text = String::from_utf8_lossy(bytes);
        let (doc, issues) = storage::import_text_checked(name, &text);
        self.import_doc(doc, issues);
    }

    fn import_doc(&mut self, doc: Document, issues: Vec<crate::fountain::Issue>) {
        if self.dirty {
            self.save(false);
        }
        match self.store.create(&doc) {
            Ok(new_path) => {
                self.refresh_entries();
                let title = doc.meta.title.clone();
                self.open(new_path);
                self.mode = Mode::Write;
                if issues.is_empty() {
                    self.deck.ok(
                        "Imported",
                        &format!("{title} — {} scenes", doc.scene_count()),
                    );
                } else {
                    self.show_import_report(title, issues);
                }
            }
            Err(e) => self.deck.say("Could not import", &e, Tone::Danger),
        }
    }

    fn start_import(&mut self) {
        self.store.start_import();
    }

    fn take_snapshot(&mut self) {
        let Some(path) = self.path.clone() else { return };
        self.doc.normalize();
        match self.store.take_snapshot(&path, &self.doc, None) {
            Ok(()) => self.deck.ok("Snapshot taken", "Find it under More → Snapshots."),
            Err(e) => self.deck.say("Could not take a snapshot", &e, Tone::Danger),
        }
    }

    fn restore_snapshot(&mut self, snap: &Path) {
        if self.path.is_none() {
            return;
        }
        match self.store.load_snapshot(snap) {
            Load::Ready(old, _) => self.apply_snapshot(*old),
            Load::Pending => self.restoring = Some(snap.to_path_buf()),
            Load::Failed(e) => self.deck.say("Could not restore", &e, Tone::Danger),
        }
    }

    fn apply_snapshot(&mut self, old: Document) {
        let Some(path) = self.path.clone() else { return };
        self.doc.normalize();
        let _ = self.store.take_snapshot(&path, &self.doc, Some("Before a restore"));
        self.checkpoint();
        let title = self.doc.meta.title.clone();
        self.doc = old;
        self.doc.meta.title = title;
        self.after_history();
        self.save(false);
        self.refresh_layout(true);
        self.deck.ok("Snapshot restored", "The version before it was kept too.");
    }

    fn new_scene(&mut self) {
        self.checkpoint();
        let at = self
            .ed
            .focus_block
            .and_then(|id| self.doc.scene_of(id))
            .and_then(|k| self.doc.scenes().get(k).map(|s| s.end))
            .unwrap_or(self.doc.blocks.len());
        let b = self.doc.new_block(Element::SceneHeading, "");
        let id = b.id;
        self.doc.blocks.insert(at, b);
        self.mode = Mode::Write;
        self.ed.focus(id, Caret::Start);
        self.ed.scroll_to_focus = true;
        self.mark_changed();
        self.snapshot_due = false;
        self.baseline = self.doc.clone();
    }

    fn new_act(&mut self) {
        let focus = self.ed.focus_block.and_then(|id| self.doc.index_of(id));
        let (at, replace) = match focus {
            None => (self.doc.blocks.len(), false),
            Some(i) => {
                let b = &self.doc.blocks[i];
                match b.element {
                    Element::SceneHeading => (i, false),
                    Element::Act => (i + 1, false),
                    _ if b.text.trim().is_empty() => (i, true),
                    _ if self.ed.caret == Some(0) => (i, false),
                    _ => (i + 1, false),
                }
            }
        };
        self.checkpoint();
        let before = self.doc.blocks[..at].iter().filter(|b| b.element == Element::Act).count();
        let after = if replace { at + 1 } else { at };
        let later = self.doc.blocks[after..].iter_mut().filter(|b| b.element == Element::Act);
        for (n, b) in (before + 1..).zip(later) {
            if b.text.trim() == act_title(n) {
                b.text = act_title(n + 1);
            }
        }
        let title = act_title(before + 1);
        let id = if replace {
            let b = &mut self.doc.blocks[at];
            b.element = Element::Act;
            b.text = title.clone();
            b.note.clear();
            b.tint = None;
            b.id
        } else {
            let b = self.doc.new_block(Element::Act, &title);
            let id = b.id;
            self.doc.blocks.insert(at, b);
            id
        };
        self.mode = Mode::Write;
        self.ed.focus(id, editor::Caret::Range(0, title.chars().count()));
        self.ed.scroll_to_focus = true;
        self.mark_changed();
        self.snapshot_due = false;
        self.baseline = self.doc.clone();
    }

    fn window_ground(&self) -> Color32 {
        pal().backdrop
    }

    pub fn save_before_quit(&mut self) {
        if self.dirty {
            self.save(false);
        }
    }

    fn save_settings(&mut self) {
        if let Err(e) = self.store.write_settings(&self.settings) {
            self.deck.say("Could not save settings", &e, Tone::Danger);
        }
    }

    fn answer(&mut self, a: alerts::Answer<Ask>) {
        match a.action {
            Ask::DeleteScript(path) => self.delete(&path),
            Ask::DeleteScene(n) => {
                let heading = self
                    .doc
                    .scenes()
                    .get(n)
                    .map(|s| s.heading.clone())
                    .unwrap_or_default();
                self.checkpoint();
                if self.doc.remove_scene(n) {
                    self.mark_changed();
                    self.snapshot_due = false;
                    self.baseline = self.doc.clone();
                    let undo = self.keys_for(keys::Command::Undo);
                    self.deck.say("Scene deleted", &format!("{heading} · {undo} brings it back"), Tone::Warn);
                }
            }
            Ask::RenameScript => {
                if let Some(t) = a.text {
                    if !t.trim().is_empty() {
                        self.doc.meta.title = t.trim().to_string();
                        self.mark_changed();
                        self.save(true);
                        self.deck.ok("Renamed", t.trim());
                    }
                }
            }
            Ask::RestoreSnapshot(p) => self.restore_snapshot(&p),
        }
    }

    fn ask_delete(&mut self, path: PathBuf) {
        let title = self
            .entries
            .iter()
            .find(|e| e.path == path)
            .map(|e| e.title.clone())
            .unwrap_or_else(|| "this script".into());
        let what = if self.store.team().is_some() {
            format!("“{title}” moves to Recently deleted, where anyone on the team can bring it back for 30 days.")
        } else {
            format!("“{title}” goes for good. Its snapshots stay in the library folder.")
        };
        self.deck.ask("Delete this script?", &what, "Delete", Tone::Danger, Ask::DeleteScript(path));
    }

    fn ask_delete_scene(&mut self, n: usize) {
        let heading = self.doc.scenes().get(n).map(|s| s.heading.clone()).unwrap_or_default();
        self.deck.ask(
            "Delete this scene?",
            &format!(
                "“{}” and everything in it. {} brings it back.",
                if heading.trim().is_empty() { "Untitled scene" } else { heading.trim() },
                self.keys_for(keys::Command::Undo)
            ),
            "Delete",
            Tone::Danger,
            Ask::DeleteScene(n),
        );
    }

    fn ask_restore(&mut self, s: &storage::Snapshot) {
        self.deck.ask(
            "Restore this snapshot?",
            &format!(
                "The script as it was at {}. What is there now is kept as a snapshot first, so nothing is lost.",
                s.when
            ),
            "Restore",
            Tone::Warn,
            Ask::RestoreSnapshot(s.path.clone()),
        );
    }

    fn toggle_popover(&mut self, which: Popover) {
        if self.popover == Some(which) {
            self.popover = None;
        } else {
            self.popover = Some(which);
            self.popover_frame = self.frame_no;
        }
    }

    fn close_popover_if_outside(&mut self, ctx: &egui::Context, areas: &[Rect]) {
        let d = ui::Dismisser {
            opened_on: self.popover_frame,
        };
        if d.should_close(ctx, self.frame_no, areas) {
            self.popover = None;
        }
    }

    fn voices(&self) -> Option<HashMap<String, Color32>> {
        if self.settings.character_colors {
            Some(theme::voice_colors(&self.layout.speakers, self.settings.colour_seed, self.chosen_voices()))
        } else {
            None
        }
    }

    fn title_or_untitled(&self) -> String {
        if self.doc.meta.title.trim().is_empty() {
            "Untitled Script".to_string()
        } else {
            self.doc.meta.title.clone()
        }
    }

    // ribbon here---

    fn top_panel(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("ns-ribbon")
            .exact_height(if self.home { chrome::TITLE_H + 10.0 } else { theme::TOPBAR_H })
            .frame(egui::Frame::none().inner_margin(egui::Margin {
                left: theme::GAP,
                right: theme::GAP,
                top: 5.0,
                bottom: 0.0,
            }))
            .show_separator_line(false)
            .show(ctx, |ui| {
                chrome::title_bar(ui, ctx, |ui| self.title_extras(ui));
                if !self.home {
                    ui.add_space(6.0);
                    self.ribbon_row(ui);
                }
            });
    }

    fn ribbon_row(&mut self, ui: &mut egui::Ui) {
        let p = pal();
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;

            let tip = format!("Back to your scripts · {}", self.keys_for(keys::Command::FindScripts));
            if ui::ribbon_button(ui, Icon::ChevronLeft, "Library", &tip, false) {
                self.go_home();
            }
            self.library_button = ui.min_rect();
            ui::rule(ui, 22.0);

            if let Some(picked) = ui::segmented(
                ui,
                &[
                    ("Write", Icon::Pencil),
                    ("Cards", Icon::Grid),
                    ("Read", Icon::Book),
                ],
                self.mode.index(),
            ) {
                let m = Mode::from_index(picked);
                if m == Mode::Read || self.guard_edit() {
                    self.mode = m;
                }
            }

            ui.add_space(6.0);
            ui::rule(ui, 22.0);
            ui.add_space(6.0);

            let right_w = 247.0 + if self.saved_label().is_some() { 96.0 } else { 0.0 };
            let room = (ui.available_width() - right_w).max(0.0);
            let tools = Rect::from_min_size(ui.cursor().min, Vec2::new(room, 30.0));
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(tools)
                    .layout(Layout::left_to_right(Align::Center)),
            );
            child.set_clip_rect(tools.expand2(Vec2::new(0.0, 20.0)).intersect(ui.clip_rect()));
            match self.mode {
                Mode::Write => self.write_tools(&mut child, room),
                Mode::Cards => self.cards_tools(&mut child, room),
                Mode::Read => self.read_tools(&mut child, room),
            }

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 3.0;

                let (rect, menu) = ui.allocate_exact_size(Vec2::new(30.0, 30.0), Sense::click());
                self.menu_button_rect = rect;
                let open = matches!(self.popover, Some(Popover::Menu) | Some(Popover::Snapshots));
                ui::ghost_slot(ui, rect, menu.hovered() || open, open);
                icons::draw(
                    ui.painter(),
                    rect.shrink(9.0),
                    Icon::More,
                    if open { p.primary_light } else { p.text_dim },
                );
                if menu.on_hover_text("More actions").clicked() {
                    self.toggle_popover(Popover::Menu);
                }

                let (srect, sresp) = ui.allocate_exact_size(Vec2::new(30.0, 30.0), Sense::click());
                self.settings_button_rect = srect;
                let sopen = self.popover == Some(Popover::Settings);
                ui::ghost_slot(ui, srect, sresp.hovered() || sopen, sopen);
                icons::draw(
                    ui.painter(),
                    srect.shrink(8.0),
                    Icon::Settings,
                    if sopen { p.primary_light } else { p.text_dim },
                );
                if sresp
                    .on_hover_text(format!("Settings · {}", self.keys_for(keys::Command::Settings)))
                    .clicked()
                {
                    self.toggle_popover(Popover::Settings);
                }

                ui.add_space(4.0);
                ui::rule(ui, 20.0);
                ui.add_space(4.0);

                let tip = format!("Reports — characters, scenes, locations · {}", self.keys_for(keys::Command::Reports));
                if ui::ribbon_button(ui, Icon::Graph, "", &tip, self.popover == Some(Popover::Reports)) {
                    self.open_reports();
                }
                let tip = format!("Focus — just the scene you are in · {}", self.keys_for(keys::Command::FocusMode));
                if ui::ribbon_button(ui, Icon::Eye, "", &tip, self.focus_mode) {
                    self.focus_mode = !self.focus_mode;
                }
                let tip = format!("Details — title page and figures · {}", self.keys_for(keys::Command::ToggleDetails));
                if ui::ribbon_button(ui, Icon::Info, "", &tip, self.settings.show_details) {
                    self.settings.show_details = !self.settings.show_details;
                    self.save_settings();
                }
                if ui::ribbon_button(ui, Icon::Users, "", "Cast", self.settings.show_cast) {
                    self.settings.show_cast = !self.settings.show_cast;
                    self.save_settings();
                }
                let tip = format!("Scenes · {}", self.keys_for(keys::Command::ToggleScenes));
                if ui::ribbon_button(ui, Icon::List, "", &tip, self.settings.show_scenes) {
                    self.settings.show_scenes = !self.settings.show_scenes;
                    self.save_settings();
                }
                let tip = format!("Notes and changes · {}", self.keys_for(keys::Command::ToggleLibrary));
                if ui::ribbon_button(ui, Icon::Sidebar, "", &tip, self.settings.show_library) {
                    self.settings.show_library = !self.settings.show_library;
                    self.save_settings();
                }

                if let Some((text, col)) = self.saved_label() {
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new(text)
                            .font(theme::font_mono(theme::T_MICRO))
                            .color(col),
                    );
                }
            });
        });
    }

    fn saved_label(&self) -> Option<(String, Color32)> {
        let p = pal();
        if self.dirty {
            Some(("unsaved".to_string(), p.sec_light))
        } else {
            self.saved_at
                .map(|t| (format!("saved {}", ago_instant(t)), p.text_faint))
        }
    }

    fn stats_line(&self, ui: &mut egui::Ui, room: f32, with_scenes: bool) {
        let p = pal();
        let l = &self.layout;
        let s = |n: usize, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
        let mut long = format!(
            "{} · ~{} min · {} · {} words",
            s(l.pages, "page", "pages"),
            l.pages,
            s(l.scenes, "scene", "scenes"),
            l.words
        );
        if !with_scenes {
            long = format!("{} · ~{} min", s(l.pages, "page", "pages"), l.pages);
        }
        let session = if self.session_words > 0 {
            format!(" · +{} this session", self.session_words)
        } else {
            String::new()
        };
        let longer = format!("{long}{session}");
        let mid = format!("{} pg · {} sc · {} w", l.pages, l.scenes, l.words);
        let short = format!("{} pg", l.pages);
        let goal = self.settings.session_goal;
        let bar_w = if goal > 0 { 52.0 } else { 0.0 };
        let text = ui::pick_that_fits(ui, &[&longer, &long, &mid, &short], room - bar_w - 8.0);
        if !text.is_empty() {
            ui.label(
                egui::RichText::new(text.to_uppercase())
                    .font(theme::font_hud(theme::T_CAP))
                    .color(p.text_faint),
            );
        }
        if goal > 0 && room > bar_w + 20.0 {
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(46.0, 14.0), Sense::hover());
            let t = (self.session_words.max(0) as f32 / goal as f32).clamp(0.0, 1.0);
            let track = Rect::from_center_size(rect.center(), Vec2::new(46.0, 2.0));
            ui.painter().rect_filled(track, egui::Rounding::ZERO, p.line_strong);
            if t > 0.0 {
                let fill = Rect::from_min_max(track.min, Pos2::new(track.left() + track.width() * t, track.bottom()));
                ui.painter().rect_filled(fill, egui::Rounding::ZERO, p.primary_light);
            }
            resp.on_hover_text(format!(
                "Session goal: {} of {goal} words",
                self.session_words.max(0)
            ));
        }
    }

    fn write_tools(&mut self, ui: &mut egui::Ui, room: f32) {
        let p = pal();
        ui.spacing_mut().item_spacing.x = 3.0;
        let current = self.ed.current_element(&self.doc);
        let acts = self.can_write();
        let wide = room > 400.0 + 110.0 + 120.0 + if acts { 104.0 } else { 0.0 };
        let mut wanted: Option<Element> = None;
        if wide {
            for (k, e) in Element::ALL.into_iter().enumerate() {
                let chord = self.keys_for(keys::Command::Element(k as u8 + 1));
                if element_chip(ui, e, current == Some(e), &chord) {
                    wanted = Some(e);
                }
            }
        } else {
            let open = self.popover == Some(Popover::Elements);
            let label = current.map(|e| e.label()).unwrap_or("Element");
            let resp = ui::dropdown(ui, Icon::Pencil, label, open, 150.0);
            self.element_button_rect = resp.rect;
            if resp.on_hover_text("The element the caret is in · Tab cycles").clicked() {
                self.toggle_popover(Popover::Elements);
            }
        }
        if let Some(e) = wanted {
            if editor::set_element(&mut self.doc, &mut self.ed, e) {
                self.mark_changed();
            }
        }
        ui.add_space(6.0);
        ui.spacing_mut().item_spacing.x = 6.0;
        let act_tip = format!("A new act, starting where the cursor is  ·  {}", self.keys_for(keys::Command::NewAct));
        let export_tip = format!("Export this script as a PDF  ·  {}", self.keys_for(keys::Command::QuickExport));
        if acts
            && ui::ribbon_button(
                ui,
                Icon::Star4,
                "New act",
                &act_tip,
                false,
            )
            && self.guard_edit()
        {
            self.new_act();
        }
        if ui::ribbon_button(
            ui,
            Icon::Print,
            "Quick Export",
            &export_tip,
            false,
        ) {
            self.export(Format::Pdf, true);
        }
        let left = (room - (ui.min_rect().width())).max(0.0);
        let _ = p;
        self.stats_line(ui, left, true);
    }

    fn cards_tools(&mut self, ui: &mut egui::Ui, room: f32) {
        ui.spacing_mut().item_spacing.x = 6.0;
        let tip = format!("A new scene after the one you are in  ·  {}", self.keys_for(keys::Command::NewScene));
        if ui::ribbon_button(ui, Icon::Plus, "New scene", &tip, false) {
            self.new_scene();
            self.mode = Mode::Cards;
        }
        let tip = "Beat guides on the timeline: inciting incident, the act breaks, midpoint and climax";
        if ui::ribbon_button(ui, Icon::Timeline, "Beats", tip, self.settings.beat_guides) {
            self.settings.beat_guides = !self.settings.beat_guides;
            self.save_settings();
        }
        let left = (room - ui.min_rect().width()).max(0.0);
        let text = ui::pick_that_fits(
            ui,
            &[
                "·  drag a card to move its scene · right click for colour",
                "·  drag to move · right click for more",
                "·  right click for more",
            ],
            left - 170.0,
        );
        self.stats_line(ui, 170.0_f32.min(left), true);
        if !text.is_empty() {
            ui.label(
                egui::RichText::new(text)
                    .font(theme::font_mono(theme::T_CAP))
                    .color(pal().text_faint),
            );
        }
    }

    fn read_tools(&mut self, ui: &mut egui::Ui, room: f32) {
        ui.spacing_mut().item_spacing.x = 6.0;
        if !self.access.can_edit() {
            self.lock_chip(ui);
            ui.add_space(4.0);
        }
        let tip = format!("Export this script as a PDF  ·  {}", self.keys_for(keys::Command::QuickExport));
        if ui::ribbon_button(ui, Icon::Print, "Quick Export", &tip, false) {
            self.export(Format::Pdf, true);
        }
        let tip = format!("Choose pages and colours  ·  {}", self.keys_for(keys::Command::ExportWindow));
        if ui::ribbon_button(ui, Icon::Download, "Export…", &tip, self.popover == Some(Popover::Export)) {
            self.open_export_window();
        }
        let left = (room - ui.min_rect().width()).max(0.0);
        self.stats_line(ui, left.min(200.0), false);
        let hints: &[&str] = if self.access.can_edit() {
            &["·  click a line to go and write it", "·  click a line to edit"]
        } else {
            &[]
        };
        let hint = ui::pick_that_fits(ui, hints, left - 200.0);
        if !hint.is_empty() {
            ui.label(
                egui::RichText::new(hint)
                    .font(theme::font_mono(theme::T_CAP))
                    .color(pal().text_faint),
            );
        }
    }

    // library rail here---

    fn browser(&mut self, ctx: &egui::Context) {
        self.left_panel(ctx);
    }

    fn row_menu(&mut self, ctx: &egui::Context) {
        let Some((path, at, opened)) = self.rail.menu.clone() else {
            return;
        };
        let starred = self.entries.iter().find(|e| e.path == path).map(|e| e.starred).unwrap_or(false);
        let team = self.store.team().is_some();
        let write = self.can_write();
        let mut close = false;
        let area = egui::Area::new(egui::Id::new("ns-row-menu"))
            .order(egui::Order::Foreground)
            .fixed_pos(at)
            .show(ctx, |ui| {
                ui::popover_frame().show(ui, |ui| {
                    ui.set_width(ui::menu_width(ui, &["Open", "Duplicate", "Show the file", "Remove the star", "Delete script"]));
                    let mut rects = Vec::new();
                    if ui::menu_item(ui, &mut rects, "Open", Icon::Pencil, false) {
                        if self.home {
                            self.open_from_home(path.clone());
                        } else if self.path.as_deref() != Some(path.as_path()) {
                            self.open(path.clone());
                        }
                        close = true;
                    }
                    if write && ui::menu_item(ui, &mut rects, "Duplicate", Icon::Copy, false) {
                        self.duplicate(&path);
                        close = true;
                    }
                    if !team && ui::menu_item(ui, &mut rects, "Show the file", Icon::FolderOpen, false) {
                        self.store.reveal(Some(&path));
                        close = true;
                    }
                    if ui::menu_item(
                        ui,
                        &mut rects,
                        if starred { "Remove the star" } else { "Star this script" },
                        if starred { Icon::Star } else { Icon::StarFilled },
                        false,
                    ) {
                        self.toggle_star(&path);
                        close = true;
                    }
                    if write {
                        ui::separator(ui);
                        if ui::menu_item(ui, &mut rects, "Delete script", Icon::Trash, true) {
                            self.ask_delete(path.clone());
                            close = true;
                        }
                    }
                });
            })
            .response;
        let d = ui::Dismisser { opened_on: opened };
        if close
            || d.should_close(ctx, self.frame_no, &[area.rect])
            || ctx.input(|i| i.key_pressed(egui::Key::Escape))
        {
            self.rail.menu = None;
        }
    }

    // right island here---

    fn aside(&mut self, ctx: &egui::Context) {
        let details = self.settings.show_details;
        let nav = self.settings.show_scenes || self.settings.show_cast;
        if !(details || nav) || self.focus_mode {
            self.aside_rect = Rect::NOTHING;
            return;
        }
        egui::SidePanel::right("ns-aside")
            .exact_width(theme::OUTLINE_W + theme::GAP)
            .resizable(false)
            .show_separator_line(false)
            .frame(egui::Frame::none().inner_margin(egui::Margin {
                left: 0.0,
                right: theme::GAP,
                top: theme::GAP,
                bottom: theme::GAP,
            }))
            .show(ctx, |ui| {
                self.aside_rect = ui.max_rect();
                self.aside_islands.clear();
                ui.spacing_mut().item_spacing.y = theme::GAP;
                if details {
                    let island = theme::island_frame(theme::R_ISLAND)
                        .inner_margin(egui::Margin::symmetric(12.0, 14.0));
                    if nav {
                        let cap = (ui.available_height() * 0.46 - 28.0).max(120.0);
                        let r = island.show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            egui::ScrollArea::vertical()
                                .id_salt("ns-details")
                                .max_height(cap)
                                .auto_shrink([false, true])
                                .show(ui, |ui| {
                                    ui.set_max_width(ui.available_width() - 6.0);
                                    self.details_panel(ui);
                                });
                        });
                        self.aside_islands.push(r.response.rect);
                    } else {
                        let r = island.show(ui, |ui| {
                            ui.set_min_size(ui.available_size());
                            let room = ui.available_height();
                            egui::ScrollArea::vertical()
                                .id_salt("ns-details")
                                .max_height(room)
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    ui.set_max_width(ui.available_width() - 6.0);
                                    self.details_panel(ui);
                                });
                        });
                        self.aside_islands.push(r.response.rect);
                    }
                }
                if nav {
                    let r = theme::island_frame(theme::R_ISLAND)
                        .inner_margin(egui::Margin::symmetric(12.0, 14.0))
                        .show(ui, |ui| {
                            ui.set_min_size(ui.available_size());
                            let both = self.settings.show_scenes && self.settings.show_cast;
                            if self.settings.show_scenes {
                                let h = if both {
                                    (ui.available_height() * 0.58)
                                        .min(ui.available_height() - 120.0)
                                        .max(60.0)
                                } else {
                                    ui.available_height()
                                };
                                let (rect, _) = ui.allocate_exact_size(
                                    Vec2::new(ui.available_width(), h),
                                    Sense::hover(),
                                );
                                let mut child = ui.new_child(
                                    egui::UiBuilder::new()
                                        .max_rect(rect)
                                        .layout(Layout::top_down(Align::Min)),
                                );
                                self.scene_outliner(&mut child);
                            }
                            if both {
                                ui::separator(ui);
                            }
                            if self.settings.show_cast {
                                let room = Rect::from_min_size(
                                    ui.cursor().min,
                                    Vec2::new(ui.available_width(), ui.available_height().max(0.0)),
                                );
                                let mut child = ui.new_child(
                                    egui::UiBuilder::new()
                                        .max_rect(room)
                                        .layout(Layout::top_down(Align::Min)),
                                );
                                child.set_clip_rect(room.intersect(ui.clip_rect()));
                                self.cast_panel(&mut child);
                                ui.allocate_rect(room, Sense::hover());
                            }
                        });
                    self.aside_islands.push(r.response.rect);
                }
            });
    }

    fn details_panel(&mut self, ui: &mut egui::Ui) {
        let p = pal();
        let mut edited = false;
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            icons::show(ui, Icon::Info, 12.0, p.text_faint);
            ui.label(
                egui::RichText::new("DETAILS")
                    .font(theme::font_semi(theme::T_MICRO))
                    .color(p.text_faint),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui::icon_button(ui, Icon::Close, &format!("Hide · {}", self.keys_for(keys::Command::ToggleDetails)), 20.0) {
                    self.settings.show_details = false;
                    self.save_settings();
                }
            });
        });
        ui.add_space(4.0);
        ui::section(ui, "Title page");
        ui.spacing_mut().item_spacing.y = 4.0;
        let can_edit = self.access.can_edit();
        for (label, value, hint) in [
            ("Title", &mut self.doc.meta.title, "The Long Way Down"),
            ("Written by", &mut self.doc.meta.author, "A. Writer"),
            ("Draft", &mut self.doc.meta.draft, "First Draft"),
            ("Contact", &mut self.doc.meta.contact, "agent@example.com"),
        ] {
            ui.label(
                egui::RichText::new(label)
                    .font(theme::font_med(theme::T_CAP))
                    .color(p.text_faint),
            );
            if ui
                .add_enabled(
                    can_edit,
                    egui::TextEdit::singleline(value)
                        .desired_width(f32::INFINITY)
                        .margin(egui::Margin::symmetric(10.0, 6.0))
                        .hint_text(hint),
                )
                .changed()
            {
                edited = true;
            }
            ui.add_space(4.0);
        }
        if can_edit {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(5.0, 5.0);
                for (label, value) in [
                    ("today", today()),
                    ("first draft", "First Draft".to_string()),
                    ("revised", format!("Revised {}", today())),
                ] {
                    if ui::chip_button(ui, label) {
                        self.doc.meta.draft = value;
                        edited = true;
                    }
                }
            });
        }
        if edited {
            self.mark_changed();
        }

        ui.add_space(6.0);
        ui::separator(ui);
        ui::section(ui, "The script");
        let l = &self.layout;
        let mut rows = vec![
            ("Pages", format!("{}", l.pages)),
            ("Running time", format!("about {} min", l.pages)),
            ("Scenes", format!("{}", l.scenes)),
            ("Speaking parts", format!("{}", l.speakers.len())),
            ("Words", format!("{}", l.words)),
            ("Dialogue", format!("{:.0}%", l.dialogue * 100.0)),
        ];
        if !l.acts.is_empty() {
            rows.insert(2, ("Acts", format!("{}", l.acts.len())));
        }
        for (k, v) in rows {
            ui.horizontal(|ui| {
                ui.add_space(2.0);
                ui.label(
                    egui::RichText::new(k)
                        .font(theme::font(theme::T_SM))
                        .color(p.text_dim),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.add_space(2.0);
                    ui.label(
                        egui::RichText::new(v)
                            .font(theme::font_mono(theme::T_CAP))
                            .color(p.text),
                    );
                });
            });
        }
        self.details_people(ui);
    }

    fn scene_outliner(&mut self, ui: &mut egui::Ui) {
        let p = pal();
        let scenes = self.doc.scenes();
        let acts = self.doc.acts();
        let live = self.ed.focus_block.and_then(|id| self.doc.scene_of(id));

        ui.horizontal(|ui| {
            ui.add_space(4.0);
            icons::show(ui, Icon::List, 12.0, p.text_faint);
            ui::hud(ui, "SCENES", Some(p.text_dim));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui::hud(ui, &format!("{:02}", scenes.len()), None);
            });
        });
        ui.add_space(8.0);

        if scenes.is_empty() {
            ui.label(
                egui::RichText::new(format!("No scene headings yet. {} turns a line into one.", self.keys_for(keys::Command::Element(1))))
                    .font(theme::font(theme::T_CAP))
                    .color(p.text_faint),
            );
            return;
        }

        let mut jump = None;
        egui::ScrollArea::vertical()
            .id_salt("ns-outline")
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                let rail_shape = ui.painter().add(egui::Shape::Noop);
                let left = ui.min_rect().left() + 16.0;
                let text_left = left + 18.0;
                let mut dots: Vec<f32> = Vec::new();

                ui.spacing_mut().item_spacing.y = 2.0;
                let mut next_act = 0usize;
                for (i, sc) in scenes.iter().enumerate() {
                    while let Some(a) = acts.get(next_act).filter(|a| a.start < sc.start) {
                        let (cy, clicked) = self.outline_act(ui, a, left, text_left);
                        if clicked {
                            jump = Some(a.id);
                        }
                        dots.push(cy);
                        next_act += 1;
                    }
                    let w = ui.available_width();
                    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 38.0), Sense::click());
                    let is_live = live == Some(i);
                    let hot = anim::ease(ui.ctx(), resp.id, resp.hovered(), anim::HOVER);

                    let slot = rect.shrink2(Vec2::new(0.0, 2.0));
                    let fill = if p.dark { p.raised } else { p.sunken };
                    if is_live {
                        ui.painter().rect_filled(slot, egui::Rounding::ZERO, fill);
                        ui.painter().rect_filled(
                            Rect::from_min_size(slot.left_top(), Vec2::new(2.0, slot.height())),
                            egui::Rounding::ZERO,
                            p.primary_light,
                        );
                    } else if hot > 0.02 {
                        ui.painter().rect_filled(slot, egui::Rounding::ZERO, theme::wash(fill, (255.0 * hot) as u8));
                    }

                    let cy = rect.center().y;
                    dots.push(cy);
                    let r = if is_live { 5.6 } else { 4.4 };
                    let tint = sc.tint.map(|t| p.group(t));
                    let lit = anim::ease(ui.ctx(), resp.id.with("lit"), is_live, 0.26);
                    if lit > 0.01 {
                        theme::glow_star(
                            ui.painter(),
                            Pos2::new(left, cy),
                            r,
                            tint.unwrap_or(p.sec_grad.1),
                            10.0,
                            0.14 * lit,
                        );
                    }
                    match tint {
                        Some(t) => theme::grad_star(ui.painter(), Pos2::new(left, cy), r, theme::darken(t, 0.2), theme::lighten(t, 0.2)),
                        None => theme::grad_star(ui.painter(), Pos2::new(left, cy), r, p.sec_grad.0, p.sec_grad.1),
                    }

                    let name = if sc.heading.trim().is_empty() {
                        format!("Scene {}", i + 1)
                    } else {
                        sc.heading.clone()
                    };
                    let label = if self.settings.scene_numbers {
                        format!("{}. {name}", sc.number)
                    } else {
                        name
                    };
                    let label_font = if is_live { theme::font_med(theme::T_SM) } else { theme::font(theme::T_SM) };
                    ui.painter().with_clip_rect(rect).text(
                        Pos2::new(text_left, cy - 6.5),
                        egui::Align2::LEFT_CENTER,
                        ui::elide_to_width(ui.ctx(), &label, label_font.clone(), 0.0, rect.right() - text_left - 10.0),
                        label_font,
                        if is_live { p.text } else { theme::mix(p.text_dim, p.text, hot) },
                    );
                    let (page, eighths) = self.layout.lengths.get(&sc.id).copied().unwrap_or((0, 0));
                    let meta = if page > 0 {
                        format!("p.{page} \u{2014} {} pg \u{2014} {} words", eighths_label(eighths), sc.words)
                    } else {
                        "empty".to_string()
                    };
                    theme::tracked_text(
                        ui.painter(),
                        Pos2::new(text_left, cy + 8.5),
                        &meta.to_uppercase(),
                        theme::font_hud(theme::T_MICRO - 0.5),
                        p.text_faint,
                        0.5,
                    );
                    if !sc.synopsis.trim().is_empty() {
                        let _ = resp.clone().on_hover_text(sc.synopsis.clone());
                    }
                    if resp.clicked() {
                        jump = Some(sc.id);
                    }
                }

                for a in &acts[next_act.min(acts.len())..] {
                    let (cy, clicked) = self.outline_act(ui, a, left, text_left);
                    if clicked {
                        jump = Some(a.id);
                    }
                    dots.push(cy);
                }

                let mut shapes = Vec::new();
                let gap = 9.0;
                let mut cursor = ui.min_rect().top() + 4.0;
                let mut edges = Vec::new();
                for cy in &dots {
                    if cy - gap > cursor {
                        edges.push((cursor, cy - gap));
                    }
                    cursor = cy + gap;
                }
                for (a, b) in edges {
                    shapes.push(egui::Shape::line_segment(
                        [Pos2::new(left, a), Pos2::new(left, b)],
                        Stroke::new(1.0_f32, p.line_strong),
                    ));
                }
                ui.painter().set(rail_shape, egui::Shape::Vec(shapes));
            });

        if let Some(id) = jump {
            self.mode = Mode::Write;
            self.ed.jump_to(id);
        }
    }

    fn outline_act(&self, ui: &mut egui::Ui, a: &crate::model::ActSpan, left: f32, text_left: f32) -> (f32, bool) {
        let p = pal();
        let w = ui.available_width();
        if a.number > 1 || ui.min_rect().height() > 1.0 {
            ui.add_space(6.0);
        }
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 30.0), Sense::click());
        let hot = anim::ease(ui.ctx(), resp.id, resp.hovered(), anim::HOVER);
        let cy = rect.center().y;
        if hot > 0.01 {
            ui.painter().rect_filled(
                rect.shrink2(Vec2::new(0.0, 2.0)),
                egui::Rounding::ZERO,
                theme::wash(if p.dark { p.raised } else { p.sunken }, (255.0 * hot) as u8),
            );
        }
        ui.painter().rect_filled(
            Rect::from_center_size(Pos2::new(left, cy), Vec2::splat(7.0)),
            egui::Rounding::ZERO,
            p.primary_light,
        );
        let title = if a.title.trim().is_empty() { act_title(a.number) } else { a.title.trim().to_string() };
        let room = rect.right() - text_left - 8.0;
        let pages = self
            .layout
            .acts
            .get(&a.id)
            .and_then(|n| n.rsplit(" \u{2014} ").next())
            .unwrap_or("")
            .to_uppercase();
        let font = theme::font_hud(theme::T_MICRO + 0.5);
        let pages_w = theme::tracked_width(ui.ctx(), &pages, font.clone(), 0.8);
        let show_pages = !pages.is_empty() && room > pages_w + 70.0;
        let title_room = if show_pages { room - pages_w - 12.0 } else { room };
        let shown = ui::elide_to_width(ui.ctx(), &title.to_uppercase(), font.clone(), 0.8, title_room);
        theme::tracked_text(
            ui.painter(),
            Pos2::new(text_left, cy),
            &shown,
            font.clone(),
            theme::mix(p.text, p.primary_light, 0.4 * hot),
            0.8,
        );
        if show_pages {
            theme::tracked_text(
                ui.painter(),
                Pos2::new(rect.right() - 6.0 - pages_w, cy),
                &pages,
                font,
                p.text_faint,
                0.8,
            );
        }
        let resp = resp.on_hover_text("Go to this act");
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        (cy, resp.clicked())
    }

    fn cast_panel(&mut self, ui: &mut egui::Ui) {
        let p = pal();
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            icons::show(ui, Icon::Users, 13.0, p.text_faint);
            ui::hud(ui, "CAST", Some(p.text_dim));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui::hud(ui, &format!("{:02}", self.layout.cast.len()), None);
            });
        });
        ui.add_space(6.0);

        if self.layout.cast.is_empty() {
            ui.label(
                egui::RichText::new(format!("Nobody speaks yet. A character cue is {}.", self.keys_for(keys::Command::Element(3))))
                    .font(theme::font(theme::T_CAP))
                    .color(p.text_faint),
            );
            return;
        }

        let mut jump: Option<(String, u64)> = None;
        let mut pick: Option<(String, Pos2)> = None;
        let choose = self.can_choose_voices();
        let cast = self.layout.cast.clone();
        let voices = self.voices();
        let d = self.layout.dialogue;
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(format!("dialogue {:.0}%  ·  the rest {:.0}%", d * 100.0, (1.0 - d) * 100.0))
                    .font(theme::font_mono(theme::T_MICRO))
                    .color(p.text_faint),
            );
        });
        ui.add_space(-2.0);
        let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 8.0), Sense::hover());
        let track = Rect::from_center_size(rect.center(), Vec2::new(rect.width() - 8.0, 4.0));
        ui.painter()
            .rect_filled(track, egui::Rounding::same(2.0), theme::wash(p.text_faint, 45));
        if d > 0.0 {
            let fill = Rect::from_min_max(track.min, Pos2::new(track.left() + track.width() * d, track.bottom()));
            theme::grad_rect(ui.painter(), fill, 2.0, p.prim_grad.0, p.prim_grad.1, Vec2::new(1.0, 0.0));
        }
        ui.add_space(2.0);
        let list_h = ui.available_height().max(0.0);
        egui::ScrollArea::vertical()
            .id_salt("ns-cast")
            .auto_shrink([false; 2])
            .max_height(list_h)
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                for (k, c) in cast.iter().enumerate() {
                    let w = ui.available_width();
                    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 28.0), Sense::click());
                    let hot = anim::ease(ui.ctx(), resp.id, resp.hovered(), anim::HOVER);
                    if hot > 0.02 {
                        ui.painter().rect_filled(
                            rect,
                            egui::Rounding::same(theme::R_SM),
                            theme::wash(p.primary, (30.0 * hot) as u8),
                        );
                    }
                    let dot = Pos2::new(rect.left() + 12.0, rect.center().y);
                    let colour = voices
                        .as_ref()
                        .and_then(|v| v.get(&c.name).copied())
                        .unwrap_or_else(|| p.group(k));
                    if choose {
                        let hit = Rect::from_center_size(dot, Vec2::splat(18.0));
                        let sr = ui.interact(hit, resp.id.with("swatch"), Sense::click());
                        let on = anim::ease(ui.ctx(), sr.id, sr.hovered(), anim::HOVER);
                        ui.painter().circle_filled(dot, 4.2 + 1.2 * on, colour);
                        if on > 0.02 {
                            ui.painter().circle_stroke(dot, 7.5, Stroke::new(1.2_f32, theme::wash(colour, (200.0 * on) as u8)));
                        }
                        if sr.on_hover_text("Choose their colour").clicked() {
                            pick = Some((c.name.clone(), hit.left_bottom()));
                        }
                    } else {
                        ui.painter().circle_filled(dot, 3.0, colour);
                    }
                    ui.painter().text(
                        Pos2::new(rect.left() + 24.0, rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        ui::elide_to_width(ui.ctx(), &c.name, theme::font(theme::T_SM), 0.0, rect.width() - 76.0),
                        theme::font(theme::T_SM),
                        theme::mix(p.text_dim, p.text, hot),
                    );
                    ui.painter().text(
                        Pos2::new(rect.right() - 8.0, rect.center().y),
                        egui::Align2::RIGHT_CENTER,
                        format!("{}", c.cues),
                        theme::font_mono(theme::T_MICRO),
                        p.text_faint,
                    );
                    let resp = resp.on_hover_text(format!(
                        "{} cue{} · {} words · {} scene{}\nClick to step through their lines",
                        c.cues,
                        if c.cues == 1 { "" } else { "s" },
                        c.words,
                        c.scenes,
                        if c.scenes == 1 { "" } else { "s" }
                    ));
                    if resp.clicked() {
                        let n = self.cast_cursor.get(&c.name).copied().unwrap_or(0);
                        if let Some(id) = c.cue_ids.get(n % c.cue_ids.len().max(1)) {
                            jump = Some((c.name.clone(), *id));
                        }
                    }
                }
            });
        if let Some((name, at)) = pick {
            self.open_voice_picker(&name, at);
        } else if let Some((name, id)) = jump {
            *self.cast_cursor.entry(name).or_insert(0) += 1;
            self.mode = Mode::Write;
            self.ed.jump_to(id);
        }
    }

    // pages here---

    fn page_arrival(&self, ctx: &egui::Context) -> f32 {
        if !anim::enabled() {
            return 1.0;
        }
        let t = (self.page_born.elapsed().as_secs_f32() / 0.55).clamp(0.0, 1.0);
        if t < 1.0 {
            ctx.request_repaint();
        }
        anim::bounce(t)
    }

    fn page(&mut self, ctx: &egui::Context) {
        let arrive = self.page_arrival(ctx);
        let mode = self.mode;
        egui::CentralPanel::default()
            .frame(egui::Frame::none().inner_margin(egui::Margin::same(theme::GAP)))
            .show(ctx, |ui| {
                theme::island_frame(theme::R_ISLAND)
                    .inner_margin(egui::Margin::ZERO)
                    .show(ui, |ui| {
                        ui.set_min_size(ui.available_size());
                        let island = ui.max_rect();
                        self.page_rect = island;
                        let mut ui = slide_in(ui, island, arrive, if mode == Mode::Write { 40.0 } else { 24.0 });
                        let ui = &mut ui;
                        match mode {
                            Mode::Write => self.write_page(ui),
                            Mode::Cards => self.cards_page(ui),
                            Mode::Read => self.read_page(ui),
                        }
                    });
            });
    }

    fn write_page(&mut self, ui: &mut egui::Ui) {
        let mut changed = false;
        let starts = std::mem::take(&mut self.layout.page_starts);
        let labels = std::mem::take(&mut self.layout.labels);
        let changes = std::mem::take(&mut self.layout.changes);
        let voices = self.voices();
        egui::ScrollArea::vertical()
            .id_salt("ns-write")
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                let avail = ui.available_width();
                let col = editor::column_width(ui, self.ed.font_px).min(avail - 16.0);
                let pad = ((avail - col) * 0.5).max(8.0);

                changed |= self.banner(ui, avail, pad);
                ui.add_space(18.0);

                let needle = if self.find.open { Some(self.find.needle.clone()) } else { None };
                let current = if self.find.open {
                    self.doc.find(&self.find.needle).get(self.find.current).copied()
                } else {
                    None
                };
                let view = editor::View {
                    page_starts: &starts,
                    page_breaks: self.settings.page_breaks,
                    scene_numbers: self.settings.scene_numbers,
                    smart_type: self.settings.smart_type,
                    typewriter: self.settings.typewriter,
                    focus_mode: self.focus_mode,
                    find: needle.as_deref(),
                    current,
                    element_colors: self.settings.element_colors,
                    char_colors: voices.as_ref(),
                    pages: self.layout.pages,
                    margin: pad,
                    labels: &labels,
                    changed: &changes,
                };
                ui.horizontal(|ui| {
                    ui.add_space(pad);
                    ui.vertical(|ui| {
                        ui.set_width(col);
                        ui.set_max_width(col);
                        changed |= editor::show(ui, &mut self.doc, &mut self.ed, &view);
                        ui.add_space(90.0);
                    });
                });
            });
        self.layout.page_starts = starts;
        self.layout.labels = labels;
        self.layout.changes = changes;
        if changed {
            self.mark_changed();
        }
        if let Some(id) = self.ed.note_request.take() {
            self.open_note(id);
        }
    }

    fn banner(&mut self, ui: &mut egui::Ui, avail: f32, pad: f32) -> bool {
        let p = pal();
        let mut changed = false;
        let banner_bg = ui.painter().add(egui::Shape::Noop);
        let (band, _) = ui.allocate_exact_size(Vec2::new(avail, BANNER_H), Sense::hover());
        let mut head = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(Rect::from_min_max(
                    Pos2::new(band.left() + pad, band.top() + 16.0),
                    Pos2::new(band.right() - pad, band.bottom()),
                ))
                .layout(Layout::top_down(Align::Min)),
        );
        let title_id = egui::Id::new("ns-script-title");
        let title_px = {
            let room = head.available_width() - editor::GUTTER - 20.0;
            let full = theme::tracked_width(head.ctx(), &self.doc.meta.title, theme::font_bold(theme::T_TITLE), 0.0);
            if full <= room {
                theme::T_TITLE
            } else {
                (theme::T_TITLE * room / full * 0.97).max(TITLE_MIN_PX)
            }
        };
        let r = head.add(
            egui::TextEdit::singleline(&mut self.doc.meta.title)
                .id(title_id)
                .frame(false)
                .desired_width(f32::INFINITY)
                .font(theme::font_bold(title_px))
                .text_color(p.text)
                .margin(egui::Margin {
                    left: editor::GUTTER,
                    right: 16.0,
                    top: 2.0,
                    bottom: 2.0,
                })
                .hint_text("Name this script"),
        );
        if self.focus_title {
            r.request_focus();
            let n = caret::char_len(&self.doc.meta.title);
            caret::select(ui.ctx(), title_id, 0, n);
            self.focus_title = false;
        }
        if r.changed() {
            changed = true;
        }
        if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            if let Some(b) = self.doc.blocks.first() {
                self.ed.focus(b.id, Caret::End);
            }
        }

        let chip_font = theme::font_hud(theme::T_MICRO + 0.5);
        let pg = self.layout.pages;
        let mut chips: Vec<(String, Option<Color32>)> = Vec::new();
        if !self.doc.meta.author.trim().is_empty() {
            chips.push((format!("by {}", self.doc.meta.author.trim()), Some(p.primary_light)));
        }
        if !self.doc.meta.draft.trim().is_empty() {
            chips.push((self.doc.meta.draft.trim().to_string(), Some(p.text_dim)));
        }
        chips.push((
            format!("{pg} pg \u{2014} {pg} min"),
            None,
        ));
        let details_label = if self.settings.show_details { "details" } else { "+ title page" };
        let gap = 14.0;
        let mut chips_w = 0.0;
        for (t, _) in &chips {
            chips_w += theme::tracked_width(head.ctx(), &t.to_uppercase(), chip_font.clone(), 0.8) + gap;
        }
        chips_w += theme::tracked_width(head.ctx(), &details_label.to_uppercase(), chip_font.clone(), 0.8) + 16.0;
        let file = match (self.store.team(), self.path.as_ref()) {
            (Some(team), Some(path)) => {
                let me = team.me.clone();
                self.entries
                    .iter()
                    .find(|e| &e.path == path)
                    .and_then(|e| {
                        e.edited_by
                            .as_ref()
                            .map(|by| format!("saved by {}", crate::people::by_line(by, Some(&me), &relative_time(e.modified))))
                    })
                    .unwrap_or_else(|| team.name.clone())
            }
            _ => self
                .path
                .as_ref()
                .and_then(|x| x.file_name())
                .and_then(|s| s.to_str())
                .unwrap_or("not saved yet")
                .to_string(),
        };
        let room = head.available_width() - editor::GUTTER - chips_w - 16.0;
        let char_w = head
            .painter()
            .layout_no_wrap("m".to_string(), theme::font_mono(theme::T_CAP), p.text)
            .rect
            .width()
            .max(1.0);
        let file = if room < char_w * 6.0 {
            String::new()
        } else {
            ui::elide(&file, (room / char_w) as usize)
        };
        let mut toggle_details = false;
        head.horizontal(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(gap, 0.0);
            ui.set_height(20.0);
            ui.add_space(editor::GUTTER);
            if !file.is_empty() {
                ui.label(
                    egui::RichText::new(file)
                        .font(theme::font_mono(theme::T_CAP))
                        .color(p.text_faint),
                );
                ui.add_space(4.0);
            }
            for (t, tint) in &chips {
                ui::chip(ui, t, *tint);
            }
            if ui::chip_button(ui, details_label) {
                toggle_details = true;
            }
        });
        if toggle_details {
            self.settings.show_details = !self.settings.show_details;
            self.save_settings();
        }
        ui.painter().set(banner_bg, banner_shape(band));
        changed
    }

    fn cards_page(&mut self, ui: &mut egui::Ui) {
        let live = self
            .ed
            .focus_block
            .and_then(|id| self.doc.scene_of(id))
            .and_then(|k| self.doc.scenes().get(k).map(|s| s.id));
        let lengths = self.layout.lengths.clone();
        let act_notes = self.layout.acts.clone();
        let mut out = cards::Out { changed: false, act: None };
        egui::ScrollArea::vertical()
            .id_salt("ns-cards")
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                out = cards::show(ui, &mut self.doc, &mut self.cards, &lengths, &act_notes, live, self.frame_no, self.settings.beat_guides);
            });
        if out.changed {
            self.mark_changed();
        }
        match out.act {
            Some(cards::Act::Open(id)) => {
                self.mode = Mode::Write;
                self.ed.jump_to(id);
            }
            Some(cards::Act::Move(from, to)) => {
                self.checkpoint();
                if self.doc.move_scene(from, to) {
                    self.mark_changed();
                    self.snapshot_due = false;
                    self.baseline = self.doc.clone();
                    let undo = self.keys_for(keys::Command::Undo);
                    self.deck.ok("Scene moved", &format!("{undo} puts it back"));
                }
            }
            Some(cards::Act::Tint(id, t)) => {
                if let Some(i) = self.doc.index_of(id) {
                    self.checkpoint();
                    self.doc.blocks[i].tint = t;
                    self.mark_changed();
                    self.snapshot_due = false;
                    self.baseline = self.doc.clone();
                }
            }
            Some(cards::Act::Delete(n)) => self.ask_delete_scene(n),
            Some(cards::Act::OpenAct(id)) => {
                self.mode = Mode::Write;
                self.ed.jump_to(id);
            }
            Some(cards::Act::NewScene) => {
                self.checkpoint();
                let b = self.doc.new_block(Element::SceneHeading, "");
                let id = b.id;
                self.doc.blocks.push(b);
                self.mark_changed();
                self.snapshot_due = false;
                self.baseline = self.doc.clone();
                self.mode = Mode::Write;
                self.ed.focus(id, Caret::Start);
                self.ed.scroll_to_focus = true;
            }
            None => {}
        }
    }

    fn read_page(&mut self, ui: &mut egui::Ui) {
        let voices = if self.settings.character_colors_reading { self.voices() } else { None };
        let revising = self.doc.meta.revising();
        let locked = revising && self.doc.meta.locked;
        let rev = pages::Revised {
            lock: self.rev_base.as_ref().filter(|_| locked).map(|(_, d)| d),
            changed: revising.then_some(&self.layout.changes),
            header: revising.then(|| crate::revision::header(self.doc.meta.revision.unwrap_or(0), &self.doc.meta.revision_date)),
        };
        let jump = pages::show(ui, &self.doc, &mut self.pages, self.settings.scene_numbers, voices.as_ref(), &rev);
        if let Some(id) = jump {
            self.mode = Mode::Write;
            self.ed.jump_to(id);
        }
    }

    // find replace here---

    fn find_bar(&mut self, ctx: &egui::Context) {
        if !self.find.open || self.mode != Mode::Write {
            return;
        }
        let p = pal();
        let matches = self.doc.find(&self.find.needle);
        if self.find.current >= matches.len() {
            self.find.current = 0;
        }
        let n_len = self.find.needle.chars().count();
        let field_id = egui::Id::new("ns-find-field");
        let replace_id = egui::Id::new("ns-find-replace");

        let in_field = ctx.memory(|m| m.has_focus(field_id));
        let mut step: i32 = 0;
        if in_field {
            ctx.input_mut(|i| {
                if i.consume_key(egui::Modifiers::SHIFT, egui::Key::Enter) {
                    step = -1;
                } else if i.consume_key(egui::Modifiers::NONE, egui::Key::Enter) {
                    step = 1;
                }
            });
        }

        let w = 404.0;
        let h = 78.0;
        let at = Pos2::new(self.page_rect.right() - w - 16.0, self.page_rect.top() + 16.0);
        let mut replace_one = false;
        let mut replace_all = false;
        let mut close = false;
        let area = egui::Area::new(egui::Id::new("ns-find"))
            .order(egui::Order::Foreground)
            .fixed_pos(at)
            .show(ctx, |ui| {
                let (rect, _) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
                theme::lift_shadow(ui.painter(), rect, theme::R_MD, 1.0);
                theme::glass_surface(ui.painter(), rect, theme::R_MD, 0.96);
                let inner = rect.shrink2(Vec2::new(14.0, 8.0));
                let mut child = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(inner)
                        .layout(Layout::top_down(Align::Min)),
                );
                child.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
                child.horizontal(|ui| {
                    icons::show(ui, Icon::Search, 13.0, p.text_faint);
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut self.find.needle)
                            .id(field_id)
                            .desired_width(186.0)
                            .margin(egui::Margin::symmetric(8.0, 5.0))
                            .hint_text("Find in the script"),
                    );
                    if self.find.focus {
                        resp.request_focus();
                        self.find.focus = false;
                    }
                    if resp.changed() {
                        self.find.current = 0;
                        step = 0;
                    }
                    let count = if self.find.needle.is_empty() {
                        String::new()
                    } else if matches.is_empty() {
                        "none".to_string()
                    } else {
                        format!("{} of {}", self.find.current + 1, matches.len())
                    };
                    ui.add_sized(
                        Vec2::new(52.0, 24.0),
                        egui::Label::new(
                            egui::RichText::new(count)
                                .font(theme::font_mono(theme::T_MICRO))
                                .color(p.text_faint),
                        ),
                    );
                    if ui::icon_button(ui, Icon::ChevronUp, "Previous · Shift+Enter", 24.0) {
                        step = -1;
                    }
                    if ui::icon_button(ui, Icon::ChevronDown, "Next · Enter", 24.0) {
                        step = 1;
                    }
                    if ui::icon_button_tinted(ui, Icon::Close, "Close · Esc", 24.0, None) {
                        close = true;
                    }
                });
                child.horizontal(|ui| {
                    icons::show(ui, Icon::Refresh, 13.0, p.text_faint);
                    ui.add(
                        egui::TextEdit::singleline(&mut self.find.with)
                            .id(replace_id)
                            .desired_width(186.0)
                            .margin(egui::Margin::symmetric(8.0, 5.0))
                            .hint_text("Replace with"),
                    );
                    if ui::button_sized(ui, "Replace", None, false, Some(72.0)) {
                        replace_one = true;
                    }
                    if ui::button_sized(ui, "All", None, true, Some(48.0)) {
                        replace_all = true;
                    }
                });
            })
            .response;
        self.find.rect = Some(area.rect);

        if !matches.is_empty() && step != 0 {
            let n = matches.len() as i32;
            self.find.current = ((self.find.current as i32 + step).rem_euclid(n)) as usize;
        }
        if let Some(&(id, start)) = matches.get(self.find.current) {
            if step != 0 {
                self.ed.select(id, start, start + n_len);
                self.find.focus = true;
            }
        }
        if replace_one {
            if let Some(&(id, start)) = matches.get(self.find.current) {
                if let Some(i) = self.doc.index_of(id) {
                    self.checkpoint();
                    let chars: Vec<char> = self.doc.blocks[i].text.chars().collect();
                    let end = (start + n_len).min(chars.len());
                    let mut t: String = chars[..start].iter().collect();
                    t.push_str(&self.find.with);
                    t.extend(chars[end..].iter());
                    if self.doc.blocks[i].element.is_upper() {
                        t = t.to_uppercase();
                    }
                    self.doc.blocks[i].text = t;
                    self.mark_changed();
                    self.snapshot_due = false;
                    self.baseline = self.doc.clone();
                }
            }
        }
        if replace_all && !self.find.needle.is_empty() {
            self.checkpoint();
            let n = self.doc.replace_all(&self.find.needle.clone(), &self.find.with.clone());
            if n > 0 {
                self.mark_changed();
                self.snapshot_due = false;
                self.baseline = self.doc.clone();
                self.ed.fresh = true;
                let undo = self.keys_for(keys::Command::Undo);
                self.deck.ok(
                    &format!("Replaced {n}"),
                    &format!("{undo} puts them back"),
                );
            } else {
                self.undo.pop();
                self.deck.say("Nothing to replace", "", Tone::Info);
            }
        }
        if close {
            self.find.open = false;
        }
    }

    // popovers here---

    fn popovers(&mut self, ctx: &egui::Context) {
        let window = self.popover.filter(|p| matches!(p, Popover::Settings | Popover::Export | Popover::Reports | Popover::Imported));
        if window.is_some() && window != self.window_seen {
            ctx.memory_mut(|m| {
                if let Some(id) = m.focused() {
                    m.surrender_focus(id);
                }
            });
        }
        self.window_seen = window;
        if self.popover != Some(Popover::Settings) {
            ctx.animate_bool_with_time(egui::Id::new("ns-settings-veil"), false, 0.0);
            ctx.animate_bool_with_time(egui::Id::new("ns-settings-rise"), false, 0.0);
        }
        if self.popover != Some(Popover::Export) {
            ctx.animate_bool_with_time(egui::Id::new("ns-export-veil"), false, 0.0);
            ctx.animate_bool_with_time(egui::Id::new("ns-export-rise"), false, 0.0);
        }
        if self.popover != Some(Popover::Imported) {
            ctx.animate_bool_with_time(egui::Id::new("ns-imported-veil"), false, 0.0);
            ctx.animate_bool_with_time(egui::Id::new("ns-imported-rise"), false, 0.0);
        }
        if self.popover != Some(Popover::Reports) {
            ctx.animate_bool_with_time(egui::Id::new("ns-reports-veil"), false, 0.0);
            ctx.animate_bool_with_time(egui::Id::new("ns-reports-rise"), false, 0.0);
        }
        self.voice_picker(ctx);
        match self.popover {
            Some(Popover::Menu) => self.more_menu(ctx),
            Some(Popover::Settings) => self.settings_panel(ctx),
            Some(Popover::Export) => self.export_window(ctx),
            Some(Popover::Snapshots) => self.snapshots_panel(ctx),
            Some(Popover::Elements) => self.elements_menu(ctx),
            Some(Popover::Account) => self.account_menu(ctx),
            Some(Popover::History) => self.history_panel(ctx),
            Some(Popover::Trash) => self.trash_panel(ctx),
            Some(Popover::Reports) => self.reports_window(ctx),
            Some(Popover::Imported) => self.imported_window(ctx),
            None => {}
        }
        self.row_menu(ctx);
    }

    fn more_menu(&mut self, ctx: &egui::Context) {
        let screen = ctx.screen_rect();
        let anchor = self.menu_button_rect;
        let area = egui::Area::new(egui::Id::new("ns-menu"))
            .order(egui::Order::Foreground)
            .fixed_pos(Pos2::new(
                (anchor.right() - 236.0).max(screen.left() + 8.0),
                theme::TOPBAR_H + 4.0,
            ))
            .show(ctx, |ui| {
                let team = self.store.team().is_some();
                let write = self.can_write();
                ui::popover_frame().show(ui, |ui| {
                    ui.set_width(ui::menu_width(
                        ui,
                        &[
                            "Export Final Draft (.fdx)",
                            "Import a script…",
                            "Open the exports folder",
                            "Snapshots…",
                        ],
                    ));
                    let mut rects = Vec::new();
                    ui::section(ui, "Export");
                    if ui::menu_item(ui, &mut rects, "Export PDF…", Icon::Print, false) {
                        self.open_export_window();
                    }
                    if ui::menu_item(ui, &mut rects, "Export Final Draft (.fdx)", Icon::Download, false) {
                        self.export(Format::FinalDraft, false);
                        self.popover = None;
                    }
                    if ui::menu_item(ui, &mut rects, "Export Fountain", Icon::Download, false) {
                        self.export(Format::Fountain, false);
                        self.popover = None;
                    }
                    if ui::menu_item(ui, &mut rects, "Export plain text", Icon::File, false) {
                        self.export(Format::Text, false);
                        self.popover = None;
                    }
                    if !team && ui::menu_item(ui, &mut rects, "Open the exports folder", Icon::Folder, false) {
                        self.store.open_exports();
                        self.popover = None;
                    }
                    ui::separator(ui);
                    if write && ui::menu_item(ui, &mut rects, "Import a script…", Icon::Upload, false) {
                        self.start_import();
                        self.popover = None;
                    }
                    if write && ui::menu_item(ui, &mut rects, "Take a snapshot", Icon::Copy, false) {
                        self.take_snapshot();
                        self.popover = None;
                    }
                    if ui::menu_item(ui, &mut rects, "Snapshots…", Icon::History, false) {
                        self.popover = Some(Popover::Snapshots);
                        self.popover_frame = self.frame_no;
                    }
                    if team {
                        if ui::menu_item(ui, &mut rects, "History…", Icon::Timeline, false) {
                            self.popover = Some(Popover::History);
                            self.popover_frame = self.frame_no;
                        }
                        if ui::menu_item(ui, &mut rects, "Recently deleted…", Icon::Trash, false) {
                            self.popover = Some(Popover::Trash);
                            self.popover_frame = self.frame_no;
                        }
                    }
                    ui::separator(ui);
                    if write && ui::menu_item(ui, &mut rects, "New act", Icon::Star4, false) {
                        if self.guard_edit() {
                            self.new_act();
                        }
                        self.popover = None;
                    }
                    if ui::menu_item(ui, &mut rects, "Find and replace", Icon::Search, false) {
                        self.mode = Mode::Write;
                        self.find.open = true;
                        self.find.focus = true;
                        self.popover = None;
                    }
                    if write && ui::menu_item(ui, &mut rects, "Rename script", Icon::Pencil, false) && self.guard_edit() {
                        let seed = self.doc.meta.title.clone();
                        self.deck.prompt(
                            "Rename script",
                            "The file on disk follows the title.",
                            "Script title",
                            &seed,
                            "Rename",
                            Ask::RenameScript,
                        );
                        self.popover = None;
                    }
                    if write && ui::menu_item(ui, &mut rects, "Duplicate", Icon::Copy, false) {
                        if let Some(path) = self.path.clone() {
                            self.duplicate(&path);
                        }
                        self.popover = None;
                    }
                    let starred = if team {
                        self.path
                            .as_ref()
                            .and_then(|x| self.entries.iter().find(|e| &e.path == x))
                            .map(|e| e.starred)
                            .unwrap_or(false)
                    } else {
                        self.doc.meta.starred
                    };
                    if ui::menu_item(
                        ui,
                        &mut rects,
                        if starred { "Remove the star" } else { "Star this script" },
                        if starred { Icon::Star } else { Icon::StarFilled },
                        false,
                    ) {
                        if let Some(path) = self.path.clone() {
                            self.toggle_star(&path);
                        }
                        self.popover = None;
                    }
                    if !team && ui::menu_item(ui, &mut rects, "Show the file", Icon::FolderOpen, false) {
                        let here = self.path.clone();
                        self.store.reveal(here.as_deref());
                        self.popover = None;
                    }
                    ui::separator(ui);
                    if ui::menu_item(ui, &mut rects, "NativeCompute website", Icon::External, false) {
                        self.store.open_url(crate::NATIVECOMPUTE_URL);
                        self.popover = None;
                    }
                    if write {
                        ui::separator(ui);
                        if ui::menu_item(ui, &mut rects, "Delete script", Icon::Trash, true) {
                            if let Some(path) = self.path.clone() {
                                self.ask_delete(path);
                            }
                            self.popover = None;
                        }
                    }
                    self.menu_item_rects = rects;
                });
            })
            .response;
        self.close_popover_if_outside(ctx, &[area.rect, self.menu_button_rect]);
    }

    fn snapshots_panel(&mut self, ctx: &egui::Context) {
        let p = pal();
        let screen = ctx.screen_rect();
        let anchor = self.menu_button_rect;
        let path = self.path.clone();
        let snaps = path.as_ref().and_then(|x| self.store.snapshots(x));
        let loading = snaps.is_none() && path.is_some();
        let snaps = snaps.unwrap_or_default();
        let area = egui::Area::new(egui::Id::new("ns-snapshots"))
            .order(egui::Order::Foreground)
            .fixed_pos(Pos2::new(
                (anchor.right() - 280.0).max(screen.left() + 8.0),
                theme::TOPBAR_H + 4.0,
            ))
            .show(ctx, |ui| {
                ui::popover_frame()
                    .inner_margin(egui::Margin::symmetric(10.0, 10.0))
                    .show(ui, |ui| {
                        ui.set_width(260.0);
                        ui.horizontal(|ui| {
                            icons::show(ui, Icon::History, 14.0, p.primary);
                            ui.label(
                                egui::RichText::new("Snapshots")
                                    .font(theme::font_semi(theme::T_H - 1.0))
                                    .color(p.text),
                            );
                        });
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new("Dated copies of this script. Restoring one keeps what is there now as a snapshot too.")
                                .font(theme::font(theme::T_CAP))
                                .color(p.text_faint),
                        );
                        ui.add_space(6.0);
                        let mut rects = Vec::new();
                        if snaps.is_empty() {
                            ui.label(
                                egui::RichText::new(if loading { "Fetching…" } else { "None yet." })
                                    .font(theme::font(theme::T_SM))
                                    .color(p.text_dim),
                            );
                        }
                        egui::ScrollArea::vertical()
                            .max_height(320.0)
                            .auto_shrink([false, true])
                            .show(ui, |ui| {
                                for s in &snaps {
                                    if ui::menu_item(
                                        ui,
                                        &mut rects,
                                        &format!("{}  ·  {} pg", s.when, s.pages),
                                        Icon::History,
                                        false,
                                    ) {
                                        self.ask_restore(s);
                                        self.popover = None;
                                    }
                                }
                            });
                        ui::separator(ui);
                        if ui::button_sized(ui, "Take a snapshot now", Some(Icon::Plus), true, Some(ui.available_width())) {
                            self.take_snapshot();
                        }
                    });
            })
            .response;
        self.close_popover_if_outside(ctx, &[area.rect, self.menu_button_rect]);
    }

    fn elements_menu(&mut self, ctx: &egui::Context) {
        let anchor = self.element_button_rect;
        let current = self.ed.current_element(&self.doc);
        let area = egui::Area::new(egui::Id::new("ns-elements"))
            .order(egui::Order::Foreground)
            .fixed_pos(Pos2::new(anchor.left(), anchor.bottom() + 6.0))
            .show(ctx, |ui| {
                ui::popover_frame().show(ui, |ui| {
                    let labels: Vec<String> = Element::ALL
                        .iter()
                        .enumerate()
                        .map(|(k, e)| format!("{}  ·  {}", e.label(), self.keys_for(keys::Command::Element(k as u8 + 1))))
                        .collect();
                    let dual = format!("Dual dialogue  ·  {}", self.keys_for(keys::Command::DualDialogue));
                    let mut refs: Vec<&str> = labels.iter().map(String::as_str).collect();
                    refs.push(&dual);
                    ui.set_width(ui::menu_width(ui, &refs));
                    let mut rects = Vec::new();
                    for (k, e) in Element::ALL.iter().enumerate() {
                        let icon = if current == Some(*e) { Icon::Check } else { Icon::Minus };
                        if ui::menu_item(ui, &mut rects, &labels[k], icon, false) {
                            if editor::set_element(&mut self.doc, &mut self.ed, *e) {
                                self.mark_changed();
                            }
                            self.popover = None;
                        }
                    }
                    ui::separator(ui);
                    if ui::menu_item(ui, &mut rects, &dual, Icon::Group, false) {
                        self.popover = None;
                        self.run(keys::Command::DualDialogue);
                    }
                });
            })
            .response;
        self.close_popover_if_outside(ctx, &[area.rect, self.element_button_rect]);
    }

    fn export_window(&mut self, ctx: &egui::Context) {
        let p = pal();
        let screen = ctx.screen_rect();
        let before = self.settings.clone();
        let mut close = false;
        let mut go = false;

        let veil = anim::ease(ctx, "ns-export-veil", true, 0.24);
        egui::Area::new(egui::Id::new("ns-export-scrim"))
            .order(egui::Order::Middle)
            .fixed_pos(screen.min)
            .interactable(true)
            .show(ctx, |ui| {
                let (rect, resp) = ui.allocate_exact_size(screen.size(), Sense::click());
                ui.painter().rect_filled(
                    rect,
                    egui::Rounding::same(chrome::window_radius(ctx, theme::R_WINDOW)),
                    Color32::from_black_alpha((120.0 * veil) as u8),
                );
                if resp.clicked() && self.frame_no > self.popover_frame + 1 {
                    close = true;
                }
            });

        let total = self.layout.pages;
        let selection = self.export_selection();
        let here = self.page_here();
        let t = anim::ease(ctx, "ns-export-rise", true, 0.30);
        let revising = self.doc.meta.revising();
        let size = Vec2::new(452.0, 476.0 + if revising { 128.0 } else { 0.0 });
        let rect = Rect::from_center_size(screen.center(), size * (0.97 + 0.03 * t));

        egui::Area::new(egui::Id::new("ns-export"))
            .order(egui::Order::Foreground)
            .fixed_pos(rect.min)
            .show(ctx, |ui| {
                ui.set_opacity(0.2 + 0.8 * t);
                let (win, _) = ui.allocate_exact_size(rect.size(), Sense::click());
                theme::lift_shadow(ui.painter(), win, theme::R_ISLAND, 1.0);
                ui.painter().rect_filled(win, egui::Rounding::same(theme::R_ISLAND), p.solid);
                ui.painter().rect_stroke(
                    win.shrink(0.5),
                    egui::Rounding::same(theme::R_ISLAND),
                    Stroke::new(1.0_f32, theme::wash(Color32::WHITE, if p.dark { 18 } else { 120 })),
                );
                let mut ui = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(win.shrink2(Vec2::new(22.0, 18.0)))
                        .layout(Layout::top_down(Align::Min)),
                );
                let ui = &mut ui;

                // head here---
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 10.0;
                    icons::show(ui, Icon::Print, 16.0, p.primary_light);
                    ui.label(
                        egui::RichText::new("Export PDF")
                            .font(theme::font_semi(theme::T_H))
                            .color(p.text),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui::icon_button(ui, Icon::Close, "Cancel · Esc", 26.0) {
                            close = true;
                        }
                    });
                });
                ui.label(
                    egui::RichText::new(ui::elide(&self.title_or_untitled(), 56))
                        .font(theme::font(theme::T_CAP))
                        .color(p.text_faint),
                );
                ui.add_space(8.0);

                // pages here---
                ui::section(ui, "Pages");
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    let this_label = match here {
                        Some(n) => format!("This page · {}", self.page_label(n)),
                        None => "This page · title".to_string(),
                    };
                    for (choice, label) in [
                        (PageChoice::All, format!("All {total}")),
                        (PageChoice::This, this_label),
                        (PageChoice::Chosen, "Choose…".to_string()),
                    ] {
                        let on = self.export_choice == choice;
                        if ui::button(ui, &label, None, on) && !on {
                            self.export_choice = choice;
                        }
                    }
                });
                ui.add_space(6.0);
                if self.export_choice == PageChoice::Chosen {
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut self.export_range)
                            .id(egui::Id::new("ns-export-range"))
                            .desired_width(f32::INFINITY)
                            .font(theme::font_mono(theme::T_SM))
                            .margin(egui::Margin::symmetric(10.0, 7.0))
                            .hint_text(format!("1-3, 7, 10-  (of {total})")),
                    );
                    if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        go = true;
                    }
                }
                let (what, bad) = match &selection {
                    _ if revising && self.export_changed_only => {
                        let mut pages: Vec<usize> = self.layout.changes.iter().filter_map(|id| self.layout.block_pages.get(id).copied()).collect();
                        pages.sort_unstable();
                        pages.dedup();
                        let names: Vec<String> = pages.iter().map(|n| self.page_label(*n)).collect();
                        if names.is_empty() {
                            ("No page has changed yet".to_string(), true)
                        } else {
                            (format!("Only the changed pages: {}", names.join(", ")), false)
                        }
                    }
                    Ok(None) => (
                        format!("Every page, 1 to {total}{}", if self.settings.pdf_title_page { ", after the title page" } else { "" }),
                        false,
                    ),
                    Ok(Some(v)) if v.is_empty() => (
                        if self.settings.pdf_title_page { "Just the title page".to_string() } else { "No page to export".to_string() },
                        !self.settings.pdf_title_page,
                    ),
                    Ok(Some(v)) => (
                        format!(
                            "{} page{}: {}  ·  printed with their own numbers",
                            v.len(),
                            if v.len() == 1 { "" } else { "s" },
                            export::describe_pages(v)
                        ),
                        false,
                    ),
                    Err(e) => (e.clone(), true),
                };
                ui.label(
                    egui::RichText::new(what)
                        .font(theme::font(theme::T_CAP))
                        .color(if bad { p.danger_light } else { p.text_faint }),
                );

                if revising {
                    ui.add_space(4.0);
                    ui::separator(ui);
                    let colour = self.doc.meta.revision.unwrap_or(0);
                    ui::section(ui, &crate::revision::header(colour, &self.doc.meta.revision_date));
                    let mut v = self.export_changed_only;
                    let changed_pages = self.layout.labels.len().min(self.changed_page_count());
                    let hint = match changed_pages {
                        0 => "Nothing has changed yet.".to_string(),
                        1 => "Just the page with a change, to hand out in the new colour.".to_string(),
                        n => format!("Just the {n} pages with a change, to hand out in the new colour."),
                    };
                    if ui::toggle_row(ui, "Changed pages only", &hint, &mut v) {
                        self.export_changed_only = v;
                    }
                    let mut v = self.settings.pdf_tint;
                    if ui::toggle_row(ui, "Coloured paper", "Tint the changed pages in the revision colour, for printing on white.", &mut v) {
                        self.settings.pdf_tint = v;
                    }
                }

                ui.add_space(4.0);
                ui::separator(ui);
                ui::section(ui, "On the page");
                let mut v = self.settings.pdf_title_page;
                if ui::toggle_row(ui, "Title page", "Title, author, draft and contact, on a page of their own.", &mut v) {
                    self.settings.pdf_title_page = v;
                }
                let mut v = self.settings.pdf_scene_numbers;
                if ui::toggle_row(ui, "Scene numbers", "In both margins, the way a shooting script carries them.", &mut v) {
                    self.settings.pdf_scene_numbers = v;
                }
                let mut v = self.settings.pdf_notes;
                if ui::toggle_row(ui, "Notes", "Every note you left, listed after the script with the page it belongs to.", &mut v) {
                    self.settings.pdf_notes = v;
                }
                let mut v = self.settings.pdf_character_colors;
                if ui::toggle_row(
                    ui,
                    "Character colours",
                    "Each speaker's cue and lines in their colour — the app's hues, deepened to read as ink on paper.",
                    &mut v,
                ) {
                    self.settings.pdf_character_colors = v;
                }
                if self.settings.pdf_character_colors {
                    let inks = theme::voice_inks(&self.layout.speakers, self.settings.colour_seed, self.chosen_voices());
                    let names: Vec<String> = self.layout.speakers.iter().take(6).cloned().collect();
                    let (strip, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 26.0), Sense::hover());
                    ui.painter().rect_filled(strip, egui::Rounding::same(theme::R_SM), Color32::WHITE);
                    let mut x = strip.left() + 10.0;
                    for n in &names {
                        if let Some([r, g, b]) = inks.get(n) {
                            let g2 = ui.painter().layout_no_wrap(
                                n.clone(),
                                theme::font_page_bold(11.0),
                                Color32::from_rgb(*r, *g, *b),
                            );
                            let w = g2.rect.width();
                            if x + w > strip.right() - 8.0 {
                                break;
                            }
                            ui.painter().galley(
                                Pos2::new(x, strip.center().y - g2.rect.height() * 0.5),
                                g2,
                                Color32::from_rgb(*r, *g, *b),
                            );
                            x += w + 14.0;
                        }
                    }
                    if names.is_empty() {
                        ui.painter().text(
                            strip.center(),
                            egui::Align2::CENTER_CENTER,
                            "nobody speaks yet",
                            theme::font(theme::T_CAP),
                            Color32::from_gray(120),
                        );
                    }
                }

                // foot here---
                let foot = Rect::from_min_max(
                    Pos2::new(win.left() + 22.0, win.bottom() - 18.0 - 30.0),
                    Pos2::new(win.right() - 22.0, win.bottom() - 18.0),
                );
                let mut f = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(foot)
                        .layout(Layout::right_to_left(Align::Center)),
                );
                f.spacing_mut().item_spacing.x = 8.0;
                let can = !bad;
                if ui::button_sized(&mut f, "Export", Some(Icon::Print), can, Some(104.0)) && can {
                    go = true;
                }
                if ui::button(&mut f, "Cancel", None, false) {
                    close = true;
                }
                f.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(match self.settings.after_export {
                            AfterExport::Open => "then opens it",
                            AfterExport::Reveal => "then shows the file",
                            AfterExport::Nothing => "to the exports folder",
                        })
                        .font(theme::font(theme::T_CAP))
                        .color(p.text_faint),
                    );
                });
            });

        let typing = ctx.memory(|m| m.focused().is_some());
        if !typing && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter)) {
            go = true;
        }
        if self.settings != before {
            self.save_settings();
        }
        if go {
            match self.export_selection() {
                Ok(pages) => {
                    let nothing = matches!(&pages, Some(v) if v.is_empty()) && !self.settings.pdf_title_page;
                    if nothing {
                        self.deck.warn("Nothing to export", "Choose a page, or keep the title page.");
                    } else {
                        self.popover = None;
                        self.export_pdf_with(pages);
                    }
                }
                Err(e) => self.deck.warn("Those pages will not do", &e),
            }
        } else if close {
            self.popover = None;
        }
    }

    fn settings_panel(&mut self, ctx: &egui::Context) {
        let p = pal();
        let screen = ctx.screen_rect();
        let before = self.settings.clone();
        let mut tabs: Vec<(&str, Icon, usize)> = vec![
            ("Appearance", Icon::Palette, 0),
            ("The page", Icon::File, 1),
            ("Colour", Icon::Eye, 2),
            ("Panels", Icon::Sidebar, 3),
            ("Writing & saving", Icon::Pencil, 4),
            ("YouTrack", Icon::External, 5),
            ("About", Icon::Info, 6),
        ];
        tabs.insert(5, ("Keyboard", Icon::Keyboard, KEYS_TAB));
        if self.store.team().is_some() {
            tabs.push(("Team", Icon::Users, TEAM_TAB));
        }
        let tabs = tabs;

        let veil = anim::ease(ctx, "ns-settings-veil", true, 0.24);
        let mut close = false;
        egui::Area::new(egui::Id::new("ns-settings-scrim"))
            .order(egui::Order::Middle)
            .fixed_pos(screen.min)
            .interactable(true)
            .show(ctx, |ui| {
                let (rect, resp) = ui.allocate_exact_size(screen.size(), Sense::click());
                ui.painter().rect_filled(
                    rect,
                    egui::Rounding::same(chrome::window_radius(ctx, theme::R_WINDOW)),
                    Color32::from_black_alpha((120.0 * veil) as u8),
                );
                if resp.clicked() && self.frame_no > self.popover_frame + 1 {
                    close = true;
                }
            });

        let size = Vec2::new(
            (screen.width() - 80.0).clamp(560.0, 780.0),
            (screen.height() - 80.0).clamp(420.0, 620.0),
        );
        let t = anim::ease(ctx, "ns-settings-rise", true, 0.30);
        let scale = 0.97 + 0.03 * t;
        let rect = Rect::from_center_size(screen.center(), size * scale);
        let tab = if tabs.iter().any(|t| t.2 == self.settings_tab) {
            self.settings_tab
        } else {
            tabs[tabs.len() - 1].2
        };
        if tab != KEYS_TAB {
            self.key_capture = None;
        }

        egui::Area::new(egui::Id::new("ns-settings"))
            .order(egui::Order::Foreground)
            .fixed_pos(rect.min)
            .show(ctx, |ui| {
                ui.set_opacity(0.2 + 0.8 * t);
                let (win, _) = ui.allocate_exact_size(rect.size(), Sense::click());
                ui.painter().rect_filled(win, egui::Rounding::same(theme::R_ISLAND), p.solid);
                ui.painter().rect_stroke(
                    win.shrink(0.5),
                    egui::Rounding::same(theme::R_ISLAND),
                    Stroke::new(1.0_f32, p.line_strong),
                );

                // head here---
                let head = Rect::from_min_size(win.min, Vec2::new(win.width(), 56.0));
                let mut h = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(head.shrink2(Vec2::new(20.0, 0.0)))
                        .layout(Layout::left_to_right(Align::Center)),
                );
                h.spacing_mut().item_spacing.x = 10.0;
                h.label(
                    egui::RichText::new("Settings")
                        .font(theme::font_bold(theme::T_H + 1.0))
                        .color(p.text),
                );
                ui::hud(&mut h, if p.dark { "Blackline / dark" } else { "Blackline / light" }, None);
                h.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui::icon_button(ui, Icon::Close, "Close · Esc", 28.0) {
                        close = true;
                    }
                });

                // categories here---
                let nav = Rect::from_min_max(
                    Pos2::new(win.left() + 14.0, head.bottom()),
                    Pos2::new(win.left() + 14.0 + 184.0, win.bottom() - 14.0),
                );
                let mut n = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(nav)
                        .layout(Layout::top_down(Align::Min)),
                );
                n.spacing_mut().item_spacing.y = 0.0;
                let mut picked = None;
                let mut rows = Vec::new();
                for (label, icon, id) in tabs.iter() {
                    let (clicked, r) = settings_tab_row(&mut n, label, *icon, *id == tab);
                    if clicked {
                        picked = Some(*id);
                    }
                    rows.push(r);
                }
                if let (Some(first), Some(last)) = (rows.first(), rows.last()) {
                    for r in &rows[1..] {
                        let y = r.top().round() + 0.5;
                        ui.painter().line_segment(
                            [Pos2::new(r.left(), y), Pos2::new(r.right(), y)],
                            Stroke::new(1.0_f32, p.line),
                        );
                    }
                    ui.painter().rect_stroke(
                        Rect::from_min_max(first.min, last.max).shrink(0.5),
                        egui::Rounding::ZERO,
                        Stroke::new(1.0_f32, p.line_strong),
                    );
                }
                if let Some(k) = picked {
                    self.settings_tab = k;
                }
                let foot = Pos2::new(nav.left() + 10.0, nav.bottom() - 10.0);
                theme::tracked_text(
                    ui.painter(),
                    foot - Vec2::new(0.0, 6.0),
                    concat!("BLACKLINE V", env!("CARGO_PKG_VERSION")),
                    theme::font_hud(theme::T_MICRO),
                    p.text_faint,
                    0.8,
                );

                // pane here---
                let pane = Rect::from_min_max(
                    Pos2::new(nav.right() + 14.0, head.bottom()),
                    Pos2::new(win.right() - 10.0, win.bottom() - 10.0),
                );
                ui.painter().rect_filled(pane, egui::Rounding::same(theme::R_MD), p.solid_hi);
                ui.painter().rect_stroke(pane.shrink(0.5), egui::Rounding::same(theme::R_MD), Stroke::new(1.0_f32, p.line));
                let mut pane_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(pane.shrink2(Vec2::new(18.0, 14.0)))
                        .layout(Layout::top_down(Align::Min)),
                );
                let name = tabs.iter().find(|t| t.2 == tab).map(|t| t.0).unwrap_or("");
                pane_ui.label(
                    egui::RichText::new(name.to_uppercase())
                        .font(theme::font_display(theme::T_H + 1.0))
                        .color(p.text),
                );
                pane_ui.add_space(4.0);
                egui::ScrollArea::vertical()
                    .id_salt(("ns-settings-pane", tab))
                    .auto_shrink([false, false])
                    .show(&mut pane_ui, |ui| {
                        ui.set_max_width(ui.available_width() - 14.0);
                        self.settings_body(ui, tab);
                        ui.add_space(8.0);
                    });
            });

        if close {
            self.popover = None;
        }
        if self.settings != before {
            self.ed.font_px = self.settings.page_px;
            self.theme_dirty = true;
            self.save_settings();
        }
    }

    fn settings_body(&mut self, ui: &mut egui::Ui, tab: usize) {
        let p = pal();
        match tab {
            0 => {
            ui::section(ui, "Mode");
            let dark = !self.settings.light_mode;
            if let Some(k) = ui::segmented(ui, &[("Dark", Icon::Moon), ("Light", Icon::Sun)], if dark { 0 } else { 1 }) {
                self.settings.light_mode = k == 1;
            }
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new("Dark is NativeCompute's own: void black and one signal blue. Light is the same language on paper white.")
                    .font(theme::font(theme::T_CAP))
                    .color(p.text_faint),
            );

            ui::separator(ui);
            ui::section(ui, "Motion");
            let mut v = self.settings.animations;
            if ui::toggle_row(ui, "Animations", "Every transition in the app, at once.", &mut v) {
                self.settings.animations = v;
            }
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new(format!("Interface size · {:.0}%", self.settings.font_scale * 100.0))
                    .font(theme::font_med(theme::T_SM))
                    .color(p.text),
            );
            let mut fs = self.settings.font_scale;
            if ui::slider(ui, &mut fs, 0.8..=1.4) {
                self.settings.font_scale = (fs * 20.0).round() / 20.0;
            }

                        }
            1 => {
            ui.label(
                egui::RichText::new(format!("Page text · {:.0} pt · {}+plus / minus", self.settings.page_px, keys::command_key()))
                    .font(theme::font_med(theme::T_SM))
                    .color(p.text),
            );
            let mut px = self.settings.page_px;
            if ui::slider(ui, &mut px, 11.0..=26.0) {
                self.settings.page_px = px.round();
            }
            ui.add_space(4.0);
            let mut v = self.settings.page_breaks;
            if ui::toggle_row(ui, "Page breaks", "Where each printed page begins, as you write.", &mut v) {
                self.settings.page_breaks = v;
            }
            let mut v = self.settings.scene_numbers;
            if ui::toggle_row(ui, "Scene numbers", "In the gutter, the navigator and the PDF.", &mut v) {
                self.settings.scene_numbers = v;
            }
            let mut v = self.settings.smart_type;
            if ui::toggle_row(ui, "SmartType", "Finish names, places and transitions. Tab takes it.", &mut v) {
                self.settings.smart_type = v;
            }
            let mut v = self.settings.typewriter;
            if ui::toggle_row(ui, "Typewriter scrolling", "Keep the line you are on mid-page.", &mut v) {
                self.settings.typewriter = v;
            }

                        }
            2 => {
            let mut v = self.settings.element_colors;
            if ui::toggle_row(
                ui,
                "Element colours",
                "A faint band behind each block, one colour per element. Never in Reading mode or on paper.",
                &mut v,
            ) {
                self.settings.element_colors = v;
            }
            let mut v = self.settings.character_colors;
            if ui::toggle_row(
                ui,
                "Character colours",
                "Each speaker's name and lines in a colour of their own.",
                &mut v,
            ) {
                self.settings.character_colors = v;
            }
            if self.settings.character_colors {
                let mut v = self.settings.character_colors_reading;
                if ui::toggle_row(ui, "…in Reading mode too", "", &mut v) {
                    self.settings.character_colors_reading = v;
                }
                self.colour_mode_settings(ui);
                if !self.settings.custom_colors {
                ui.horizontal(|ui| {
                    let voices = theme::character_colors(
                        &self.layout.speakers,
                        self.settings.colour_seed,
                    );
                    for name in self.layout.speakers.iter().take(8) {
                        if let Some(c) = voices.get(name) {
                            let (r, resp) = ui.allocate_exact_size(Vec2::splat(14.0), Sense::hover());
                            ui.painter().circle_filled(r.center(), 5.0, *c);
                            resp.on_hover_text(name.clone());
                        }
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui::button(ui, "Shuffle", Some(Icon::Refresh), false) {
                            self.settings.colour_seed = self.settings.colour_seed.wrapping_add(1);
                        }
                    });
                });
                }
            }

                        }
            3 => {
            ui::section(ui, "Panels");
            let panel_hints = [
                ("Details", format!("Title page and figures · {}", self.keys_for(keys::Command::ToggleDetails)), 0),
                ("Scenes", format!("The navigator · {}", self.keys_for(keys::Command::ToggleScenes)), 1),
                ("Cast", "Who speaks, and how much".to_string(), 2),
                ("Notes and changes", format!("Beside the page · {}", self.keys_for(keys::Command::ToggleLibrary)), 3),
            ];
            for (label, hint, key) in panel_hints {
                let slot = match key {
                    0 => &mut self.settings.show_details,
                    1 => &mut self.settings.show_scenes,
                    2 => &mut self.settings.show_cast,
                    _ => &mut self.settings.show_library,
                };
                let mut v = *slot;
                if ui::toggle_row(ui, label, &hint, &mut v) {
                    *slot = v;
                }
            }

                        ui::separator(ui);
            ui::section(ui, "Library");
            let mut v = self.settings.starred_first;
            if ui::toggle_row(ui, "Starred at the top", "", &mut v) {
                self.settings.starred_first = v;
            }
            ui.label(
                egui::RichText::new("Sort by")
                    .font(theme::font_med(theme::T_SM))
                    .color(p.text),
            );
            ui.horizontal_wrapped(|ui| {
                for s in SortBy::ALL {
                    let on = self.settings.sort_by == s;
                    if ui::button(ui, s.label(), None, on) && !on {
                        self.settings.sort_by = s;
                    }
                }
            });

                        }
            4 => {
            ui::section(ui, "Writing");
            ui.label(
                egui::RichText::new(if self.settings.session_goal == 0 {
                    "Session goal · off".to_string()
                } else {
                    format!("Session goal · {} words", self.settings.session_goal)
                })
                .font(theme::font_med(theme::T_SM))
                .color(p.text),
            );
            let mut g = self.settings.session_goal as f32;
            if ui::slider(ui, &mut g, 0.0..=3000.0) {
                self.settings.session_goal = ((g / 50.0).round() * 50.0) as u32;
            }

                        ui::separator(ui);
            ui::section(ui, "Starting up & saving");
            if !crate::WEB {
                let mut v = self.settings.splash;
                if ui::toggle_row(ui, "Splash screen", "", &mut v) {
                    self.settings.splash = v;
                }
            }
            if crate::WEB {
                ui.label(
                    egui::RichText::new("Exports download through your browser. Your work saves to the team library a few seconds after you stop typing, and is kept in this browser until it has.")
                        .font(theme::font(theme::T_CAP))
                        .color(p.text_faint),
                );
            } else {
            ui.label(
                egui::RichText::new("After Quick Export")
                    .font(theme::font_med(theme::T_SM))
                    .color(p.text),
            );
            ui.horizontal(|ui| {
                for a in AfterExport::ALL {
                    let on = self.settings.after_export == a;
                    if ui::button(ui, a.label(), None, on) && !on {
                        self.settings.after_export = a;
                    }
                }
            });
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(format!(
                    "Autosave after {} ms of quiet",
                    self.settings.autosave_ms
                ))
                .font(theme::font_med(theme::T_SM))
                .color(p.text),
            );
            let mut ms = self.settings.autosave_ms as f32;
            if ui::slider(ui, &mut ms, 200.0..=5000.0) {
                self.settings.autosave_ms = (ms / 50.0).round() as u64 * 50;
            }
            }

                        }
            5 => {
            ui::section(ui, "Your workspace");
            ui.add(
                egui::TextEdit::singleline(&mut self.settings.youtrack_url)
                    .id(egui::Id::new("ns-youtrack-url"))
                    .desired_width(f32::INFINITY)
                    .font(theme::font_mono(theme::T_CAP))
                    .margin(egui::Margin::symmetric(10.0, 6.0))
                    .hint_text("YouTrack workspace link here"),
            );
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new(
                    "YouTrack is JetBrains' issue tracker and project management tool. Paste your workspace's link and a View on YouTrack button appears at the foot of the library.",
                )
                .font(theme::font(theme::T_CAP))
                .color(p.text_dim),
            );
            ui.add_space(6.0);
            if ui::link(ui, "Learn about YouTrack at jetbrains.com") {
                self.store.open_url(crate::YOUTRACK_INFO_URL);
            }

                        }
            6 if self.store.team().is_some() => {
            ui::section(ui, "Where things live");
            ui.label(
                egui::RichText::new(format!(
                    "Scripts live in the team library, {}. Nothing is kept on this computer except what has not reached it yet.",
                    self.store.location()
                ))
                .font(theme::font(theme::T_CAP))
                .color(p.text_faint),
            );
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new(format!(
                    "Page font: {} · Blackline {}",
                    theme::PAGE_FONT_NAME,
                    env!("CARGO_PKG_VERSION")
                ))
                .font(theme::font(theme::T_MICRO))
                .color(p.text_faint),
            );
            self.about_updates(ui);
            self.about_maker(ui);
            }
            TEAM_TAB => self.team_settings(ui),
            KEYS_TAB => self.keyboard_settings(ui),
            6 => {
            ui::section(ui, "Where things live");
            ui.label(
                egui::RichText::new(short_home(&PathBuf::from(self.store.location())))
                    .font(theme::font_mono(theme::T_MICRO))
                    .color(p.text_faint),
            );
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui::button(ui, "Open library", Some(Icon::Folder), false) {
                    self.store.reveal(None);
                }
                if ui::button(ui, "Exports", Some(Icon::Download), false) {
                    self.store.open_exports();
                }
            });
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new(format!(
                    "Page font: {} · Blackline {}",
                    theme::PAGE_FONT_NAME,
                    env!("CARGO_PKG_VERSION")
                ))
                .font(theme::font(theme::T_MICRO))
                .color(p.text_faint),
            );
            self.about_updates(ui);
            self.about_maker(ui);
            }
            _ => {}
        }
    }

    fn take_update_answer(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(rx) = &self.update_rx {
            if let Ok(status) = rx.try_recv() {
                self.update_rx = None;
                if let crate::update::Status::Newer(r) = &status {
                    self.deck.say(
                        &format!("Blackline {} is out", r.version),
                        "Download it from Settings → About.",
                        Tone::Info,
                    );
                }
                self.update = status;
            }
        }
    }

    fn about_updates(&mut self, ui: &mut egui::Ui) {
        if crate::WEB {
            return;
        }
        let p = pal();
        ui::separator(ui);
        ui::section(ui, "Updates");
        let mut v = self.settings.check_updates;
        if ui::toggle_row(
            ui,
            "Check for updates on launch",
            "Asks GitHub once whether a newer Blackline is out. Nothing is downloaded or installed by itself.",
            &mut v,
        ) {
            self.settings.check_updates = v;
        }
        let line = match &self.update {
            crate::update::Status::Off => None,
            crate::update::Status::Checking => Some("Checking…".to_string()),
            crate::update::Status::Latest => Some(format!("Blackline {} is the latest version.", env!("CARGO_PKG_VERSION"))),
            crate::update::Status::Failed => Some("Could not reach GitHub to check.".to_string()),
            crate::update::Status::Newer(_) => None,
        };
        if let Some(line) = line {
            ui.label(egui::RichText::new(line).font(theme::font(theme::T_CAP)).color(p.text_faint));
        }
        if let crate::update::Status::Newer(r) = self.update.clone() {
            if ui::link(ui, &format!("Blackline {} is out — download it", r.version)) {
                self.store.open_url(&r.url);
            }
        }
        if ui::link(ui, "All releases") {
            self.store.open_url(&crate::update::releases_page(crate::INFO.repo));
        }
    }

    fn about_maker(&mut self, ui: &mut egui::Ui) {
        let p = pal();
        ui::separator(ui);
        ui::section(ui, "Made by");
        ui.label(
            egui::RichText::new("NativeCompute")
                .font(theme::font_bold(theme::T_H))
                .color(p.text),
        );
        ui.label(
            egui::RichText::new("Blackline is free and open source, made by NativeCompute.")
                .font(theme::font(theme::T_CAP))
                .color(p.text_dim),
        );
        ui.add_space(4.0);
        if ui::link(ui, "nativecompute.net") {
            self.store.open_url(crate::NATIVECOMPUTE_URL);
        }
    }

    // keys here---

    fn shortcuts(&mut self, ctx: &egui::Context) {
        use egui::Key;
        if self.note_edit.is_some() {
            return;
        }
        let commands = self.commands();
        for (cmd, chord) in self.settings.keys.listen(&commands, crate::WEB) {
            if ctx.input_mut(|i| i.consume_shortcut(&chord)) {
                self.run(cmd);
            }
        }

        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            if self.popover.is_some() {
                self.popover = None;
            } else if self.find.open {
                self.find.open = false;
            } else if self.focus_mode {
                self.focus_mode = false;
            }
        }
    }

    fn commands(&self) -> Vec<keys::Command> {
        keys::Command::ALL.to_vec()
    }

    fn keys_for(&self, cmd: keys::Command) -> String {
        self.settings.keys.label(cmd, crate::WEB)
    }

    fn run(&mut self, cmd: keys::Command) {
        use keys::Command as C;
        match cmd {
            C::Save => {
                if self.guard_edit() {
                    self.save(true);
                    self.deck.ok("Saved", "");
                }
            }
            C::NewScript => {
                if self.can_write() {
                    self.new_script();
                }
            }
            C::Redo => {
                if self.guard_edit() {
                    self.redo();
                }
            }
            C::Undo => {
                if self.guard_edit() {
                    self.undo();
                }
            }
            C::FindScripts => {
                if !self.home {
                    self.go_home();
                }
                self.focus_home_search = true;
            }
            C::Find => {
                if self.guard_edit() {
                    self.mode = Mode::Write;
                    self.find.open = true;
                    self.find.focus = true;
                }
            }
            C::ToggleLibrary => {
                self.settings.show_library = !self.settings.show_library;
                self.save_settings();
            }
            C::ToggleScenes => {
                self.settings.show_scenes = !self.settings.show_scenes;
                self.save_settings();
            }
            C::ToggleDetails => {
                self.settings.show_details = !self.settings.show_details;
                self.save_settings();
            }
            C::ExportWindow => self.open_export_window(),
            C::QuickExport => self.export(Format::Pdf, true),
            C::Settings => self.toggle_popover(Popover::Settings),
            C::FocusMode => self.focus_mode = !self.focus_mode,
            C::NextView => {
                if self.guard_edit() {
                    self.mode = Mode::from_index((self.mode.index() + 1) % 3);
                }
            }
            C::NewAct => {
                if self.guard_edit() {
                    self.new_act();
                }
            }
            C::NewScene => {
                if self.guard_edit() {
                    self.new_scene();
                }
            }
            C::Bigger | C::Smaller => {
                let px = if cmd == C::Bigger {
                    (self.ed.font_px + 1.0).min(26.0)
                } else {
                    (self.ed.font_px - 1.0).max(11.0)
                };
                self.ed.font_px = px;
                self.settings.page_px = px;
                self.save_settings();
            }
            C::DualDialogue => {
                if !self.guard_edit() {
                    return;
                }
                let Some(id) = self.ed.focus_block else {
                    return;
                };
                let mut next = self.doc.clone();
                match next.toggle_dual(id) {
                    Ok(on) => {
                        self.checkpoint();
                        self.doc = next;
                        self.mark_changed();
                        if on {
                            self.deck.ok("Dual dialogue", "The two speeches now sit side by side.");
                        } else {
                            self.deck.ok("One speech after the other", "");
                        }
                    }
                    Err(why) => self.deck.say("Dual dialogue", why, Tone::Info),
                }
            }
            C::AddNote => {
                self.mode = Mode::Write;
                if let Some(id) = self.ed.focus_block {
                    self.open_note(id);
                }
            }
            C::Reports => self.open_reports(),
            C::Element(n) => {
                if !self.guard_edit() {
                    return;
                }
                if let Some(e) = Element::from_digit(n as usize) {
                    if editor::set_element(&mut self.doc, &mut self.ed, e) {
                        self.mark_changed();
                    }
                }
            }
        }
    }

    fn home_shortcuts(&mut self, ctx: &egui::Context) {
        use egui::Key;
        use keys::Command as C;
        for (cmd, chord) in self.settings.keys.listen(&[C::NewScript, C::Settings], crate::WEB) {
            if ctx.input_mut(|i| i.consume_shortcut(&chord)) {
                match cmd {
                    C::NewScript if self.can_write() => self.new_script(),
                    C::Settings => self.toggle_popover(Popover::Settings),
                    _ => {}
                }
            }
        }
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.popover = None;
        }
        if self.popover.is_none() {
            self.home_keys(ctx);
        }
    }

    // frame here---

    fn take_arrivals(&mut self, ctx: &egui::Context) {
        let dropped: Vec<egui::DroppedFile> = ctx.input(|i| i.raw.dropped_files.clone());
        #[cfg(not(target_arch = "wasm32"))]
        {
            let mut paths: Vec<PathBuf> = dropped.iter().filter_map(|f| f.path.clone()).collect();
            paths.append(&mut self.arrivals);
            for path in paths {
                let in_library = path
                    .canonicalize()
                    .ok()
                    .zip(storage::scripts_dir().canonicalize().ok())
                    .map(|(f, lib)| f.starts_with(lib))
                    .unwrap_or(false);
                if in_library {
                    self.open(path);
                } else {
                    self.import_path(&path);
                }
            }
        }
        for f in dropped.iter().filter(|f| f.path.is_none()) {
            if let Some(bytes) = &f.bytes {
                if self.can_write() {
                    self.import_bytes(&f.name, bytes);
                }
            }
        }
    }

    fn can_write(&self) -> bool {
        self.store.team().map(|t| t.role.can_write()).unwrap_or(true)
    }

    fn on_event(&mut self, event: Event) {
        let here = |key: &Path, app: &App| app.path.as_deref() == Some(key);
        match event {
            Event::Opened { key, doc, access } => {
                if self.opening.as_deref() == Some(key.as_path()) {
                    self.install(key, doc, access);
                }
            }
            Event::LibraryChanged => self.refresh_entries(),
            Event::Conflict { key, theirs, by } => {
                if here(&key, self) {
                    self.install(key, theirs, Access::Edit);
                    self.deck.say(
                        &format!("{} saved a newer version", by.first_name()),
                        "You are looking at theirs now. Yours was kept as a snapshot, under More → Snapshots.",
                        Tone::Warn,
                    );
                }
                self.refresh_entries();
            }
            Event::Locked { key, by } => {
                if here(&key, self) {
                    if self.dirty {
                        let doc = self.doc.clone();
                        let _ = self.store.take_snapshot(&key, &doc, Some("Your version (kept)"));
                    }
                    self.dirty = false;
                    self.access = Access::ReadOnly { by: Some(by.clone()) };
                    self.lock_free = false;
                    self.mode = Mode::Read;
                    self.deck.say(
                        &format!("{} is editing this script", by.first_name()),
                        "It is read only for you until they finish. Anything you had not saved was kept as a snapshot.",
                        Tone::Warn,
                    );
                }
            }
            Event::LockFree { key } => {
                if here(&key, self) && !self.access.can_edit() {
                    self.lock_free = true;
                }
            }
            Event::Refreshed { key, doc } => {
                if here(&key, self) && !self.access.can_edit() {
                    self.doc = doc;
                    self.baseline = self.doc.clone();
                    self.layout_stale = true;
                    self.ed.fresh = true;
                    self.refresh_layout(true);
                }
            }
            Event::SnapshotDoc { snap, doc } => {
                if self.restoring.as_deref() == Some(snap.as_path()) {
                    self.restoring = None;
                    self.apply_snapshot(doc);
                }
            }
            #[cfg(not(target_arch = "wasm32"))]
            Event::ImportPath(path) => self.import_path(&path),
            #[cfg(target_arch = "wasm32")]
            Event::ImportPath(_) => {}
            Event::Imported { name, bytes } => self.import_bytes(&name, &bytes),
            Event::Notice { title, detail, tone } => self.deck.say(&title, &detail, tone),
            Event::SignedOut => self.deck.say("Signed out", "Sign in again to keep writing.", Tone::Info),
        }
    }

    pub fn frame(&mut self, ctx: &egui::Context) {
        self.frame_no = self.frame_no.wrapping_add(1);

        self.watch_library();
        self.take_update_answer();
        if self.theme_dirty {
            theme::set_palette(!self.settings.light_mode);
            anim::set_enabled(self.settings.animations);
            theme::apply(ctx);
            ctx.set_zoom_factor(self.settings.font_scale);
            self.theme_dirty = false;
        }

        chrome::backdrop(ctx, self.window_ground());

        if self.tour.is_some() {
            self.tour_keys(ctx);
        } else if self.key_capture.is_some() {
            self.capture_chord(ctx);
        } else if !self.deck.asking() {
            if self.home {
                self.home_shortcuts(ctx);
            } else {
                self.shortcuts(ctx);
            }
        }

        self.take_arrivals(ctx);
        for event in self.store.events() {
            self.on_event(event);
        }
        if self.store.busy() {
            ctx.request_repaint_after(Duration::from_millis(200));
        }
        self.keep_read_only();

        let key = format!(
            "{:?}|{}",
            self.mode,
            self.path.as_ref().map(|x| x.display().to_string()).unwrap_or_default()
        );
        if key != self.page_key {
            self.page_key = key;
            self.page_born = Instant::now();
            if self.mode == Mode::Read {
                self.pages.show_block = self.ed.focus_block;
            }
        }

        self.refresh_layout(false);

        self.top_panel(ctx);
        if self.home {
            self.home_screen(ctx);
        } else {
            self.browser(ctx);
            self.aside(ctx);
            self.page(ctx);
            self.find_bar(ctx);
            self.note_window(ctx);
        }
        self.popovers(ctx);
        self.tour_overlay(ctx);
        chrome::resize_handles(ctx);

        if let Some(answer) = self.deck.show(ctx) {
            self.answer(answer);
        }

        let own_window = self.splash.as_ref().map(|s| !s.is_inline()).unwrap_or(false);
        if !own_window && !self.settled {
            self.settled = crate::splash::settle(ctx, FULL_SIZE);
        }
        if let Some(splash) = self.splash.as_mut() {
            splash.set_status(self.update.clone());
            if splash.show(ctx) {
                if own_window && self.frame_no < 3 {
                    crate::splash::centre(ctx, crate::splash::CARD);
                }
            } else {
                self.splash = None;
                if own_window {
                    let full = chrome::fit(FULL_SIZE, ctx.input(|i| i.viewport().monitor_size), ncf::OS);
                    ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(MIN_SIZE));
                    ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(full));
                    crate::splash::centre(ctx, full);
                }
            }
        }

        if let Some(last) = self.last_change {
            if self.snapshot_due && last.elapsed() > SNAPSHOT_IDLE {
                self.snapshot_now();
            }
            if self.dirty && last.elapsed() > Duration::from_millis(self.settings.autosave_ms) {
                self.save(false);
            }
        }
        if ctx.input(|i| i.viewport().close_requested()) {
            self.save(true);
        }
        ctx.request_repaint_after(Duration::from_millis(500));
    }
}

#[allow(dead_code)]
impl App {
    pub fn debug_run(&mut self, cmd: keys::Command) {
        self.run(cmd);
    }
    pub fn debug_append(&mut self, text: &str) {
        if let Some(b) = self.doc.blocks.iter_mut().rev().find(|b| b.element == Element::Action) {
            b.text.push_str(text);
            self.mark_changed();
        }
    }
    pub fn is_dirty_now(&self) -> bool {
        self.dirty
    }
    pub fn mode(&self) -> Mode {
        self.mode
    }
    pub fn set_mode(&mut self, m: Mode) {
        self.mode = m;
    }
    pub fn doc(&self) -> &Document {
        &self.doc
    }
    pub fn doc_mut(&mut self) -> &mut Document {
        self.mark_changed();
        &mut self.doc
    }
    pub fn path(&self) -> Option<PathBuf> {
        self.path.clone()
    }
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }
    pub fn settings(&self) -> &Settings {
        &self.settings
    }
    pub fn focus_block(&self) -> Option<u64> {
        self.ed.focus_block
    }
    pub fn focus_on(&mut self, id: u64) {
        self.ed.focus(id, Caret::End);
    }
    pub fn focus_start(&mut self, id: u64) {
        self.ed.focus(id, Caret::Start);
    }
    pub fn debug_pending_ask(&self) -> bool {
        self.deck.asking()
    }
    pub fn debug_answer_yes(&mut self, text: Option<&str>) {
        if let Some(a) = self.deck.take_top(text.map(|s| s.to_string())) {
            self.answer(a);
        }
    }
    pub fn debug_popover_open(&self) -> bool {
        self.popover.is_some()
    }
    pub fn debug_export_open(&self) -> bool {
        self.popover == Some(Popover::Export)
    }
    pub fn debug_export_choose(&mut self, choice: PageChoice, range: &str) {
        self.export_choice = choice;
        self.export_range = range.to_string();
    }
    pub fn debug_settings_tab(&self) -> usize {
        self.settings_tab
    }
    pub fn debug_settings_open(&self) -> bool {
        self.popover == Some(Popover::Settings)
    }
    pub fn debug_menu_button(&self) -> Rect {
        self.menu_button_rect
    }
    pub fn debug_settings_button(&self) -> Rect {
        self.settings_button_rect
    }
    pub fn debug_menu_item(&self, n: usize) -> Option<Rect> {
        self.menu_item_rects.get(n).copied()
    }
    pub fn debug_cards(&self) -> &cards::CardsState {
        &self.cards
    }
    pub fn debug_card_rect(&self, n: usize) -> Option<Rect> {
        self.cards.last_rects.get(n).copied()
    }
    pub fn debug_sheet_count(&self) -> usize {
        self.pages.sheets
    }
    pub fn debug_sheet_on_show(&self) -> usize {
        self.pages.current
    }
    pub fn debug_sheet_rect(&self) -> Rect {
        self.pages.sheet_rect
    }
    pub fn debug_page_rect(&self) -> Rect {
        self.page_rect
    }
    pub fn debug_find_open(&self) -> bool {
        self.find.open
    }
    pub fn debug_set_find(&mut self, needle: &str, with: &str) {
        self.find.open = true;
        self.find.needle = needle.to_string();
        self.find.with = with.to_string();
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub fn debug_import(&mut self, path: &Path) {
        self.import_path(path);
    }
    pub fn debug_open(&mut self, path: PathBuf) {
        self.open(path);
    }
    pub fn debug_save(&mut self) {
        self.save(true);
    }
    pub fn debug_undo(&mut self) {
        self.undo();
    }
    pub fn debug_snapshot(&mut self) {
        self.take_snapshot();
    }
    pub fn debug_ask_delete_scene(&mut self, n: usize) {
        self.ask_delete_scene(n);
    }
    pub fn debug_ask_restore(&mut self, s: &storage::Snapshot) {
        self.ask_restore(s);
    }
    pub fn debug_page_starts(&mut self) -> Vec<(u64, usize)> {
        self.layout_stale = true;
        self.refresh_layout(true);
        self.layout.page_starts.iter().map(|(a, b)| (*a, *b)).collect()
    }
    pub fn debug_set_light(&mut self, light: bool) {
        self.settings.light_mode = light;
        self.theme_dirty = true;
    }
    pub fn debug_focus_mode(&mut self, on: bool) {
        self.focus_mode = on;
    }
    pub fn debug_set_update(&mut self, status: crate::update::Status) {
        self.update = status;
    }
    pub fn debug_update_rect(&self) -> Rect {
        self.update_rect
    }
    pub fn debug_library_button(&self) -> Rect {
        self.library_button
    }
    pub fn debug_left_rows(&self) -> Vec<(u64, Rect)> {
        self.left_rows.clone()
    }
    pub fn debug_left_tab(&mut self, tab: usize) {
        self.left_tab = tab;
    }
    pub fn debug_start_revision(&mut self) {
        self.start_revision();
    }
    pub fn debug_changes(&mut self) -> Vec<u64> {
        self.refresh_layout(true);
        let mut v: Vec<u64> = self.layout.changes.iter().copied().collect();
        v.sort_unstable();
        v
    }
    pub fn debug_foot_rect(&self) -> Rect {
        self.foot_rect
    }
    pub fn debug_aside_rect(&self) -> Rect {
        self.aside_rect
    }
    pub fn debug_aside_islands(&self) -> Vec<Rect> {
        self.aside_islands.clone()
    }
    pub fn settings_mut(&mut self) -> &mut Settings {
        &mut self.settings
    }
    pub fn debug_session_words(&self) -> i64 {
        self.session_words
    }
    pub fn is_home(&self) -> bool {
        self.home
    }
    pub fn debug_note_open(&self) -> Option<u64> {
        self.note_edit.as_ref().map(|n| n.block)
    }
    pub fn access(&self) -> &Access {
        &self.access
    }
    pub fn debug_open_from_home(&mut self, path: PathBuf) {
        self.open_from_home(path);
    }
    pub fn debug_go_home(&mut self) {
        self.go_home();
    }
    pub fn debug_lock_free(&self) -> bool {
        self.lock_free
    }
    pub fn debug_edit_now(&mut self) {
        if let Some(path) = self.path.clone() {
            self.opening = Some(path.clone());
            self.store.request_edit(&path);
        }
    }
    pub fn debug_toggle_star(&mut self, path: &Path) {
        self.toggle_star(path);
    }
    pub fn debug_new_script(&mut self) {
        self.new_script();
    }
    pub fn debug_open_settings(&mut self, tab: usize) {
        self.settings_tab = tab;
        self.popover = Some(Popover::Settings);
        self.popover_frame = self.frame_no;
    }
    pub fn debug_open_popover_history(&mut self) {
        self.popover = Some(Popover::History);
        self.popover_frame = self.frame_no;
    }
    pub fn debug_open_popover_trash(&mut self) {
        self.popover = Some(Popover::Trash);
        self.popover_frame = self.frame_no;
    }
    pub fn debug_hit_rows(&self) -> Vec<(PathBuf, usize, Rect)> {
        self.hit_rows.clone()
    }
    pub fn debug_home_search(&mut self, needle: &str) {
        self.home_search = needle.to_string();
    }
    pub fn debug_home_order(&self) -> Vec<PathBuf> {
        self.home_order.clone()
    }
    pub fn debug_capture_keys(&mut self, cmd: keys::Command) {
        self.key_capture = Some(cmd);
    }
    pub fn debug_key_capture(&self) -> Option<keys::Command> {
        self.key_capture
    }
    pub fn debug_key_refusal(&self) -> Option<String> {
        self.key_refusal.as_ref().map(|(_, w)| w.clone())
    }
    pub fn debug_close_popover(&mut self) {
        self.popover = None;
    }
    pub fn debug_open_voice_picker(&mut self, name: &str) {
        self.open_voice_picker(name, Pos2::new(400.0, 300.0));
    }
    pub fn debug_voice_picker_open(&self) -> bool {
        self.voice_pick.is_some()
    }
    pub fn debug_set_voice(&mut self, name: &str, hue: Option<u16>) {
        self.set_voice(name, hue);
    }
    pub fn debug_voices(&self) -> Option<HashMap<String, Color32>> {
        self.voices()
    }
    pub fn debug_pdf_inks(&self) -> Option<HashMap<String, export::Ink>> {
        self.pdf_options(None).voices
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.frame(ctx);
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        chrome::clear_color(ncf::OS, self.window_ground())
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.save_before_quit();
    }
}

// small pieces here---

fn new_script_button(ui: &mut egui::Ui, width: f32, chord: &str) -> bool {
    ui::button_response(ui, "New script", Some(Icon::Plus), true, Some(width))
        .on_hover_text(format!("New script  ·  {chord}"))
        .clicked()
}

fn element_chip(ui: &mut egui::Ui, e: Element, on: bool, chord: &str) -> bool {
    let p = pal();
    let font = theme::font_hud(theme::T_MICRO + 0.5);
    let label = e.short().to_uppercase();
    let tw = theme::tracked_width(ui.ctx(), &label, font.clone(), 0.6);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(tw + 26.0, 30.0), Sense::click());
    let hot = anim::ease(ui.ctx(), resp.id, resp.hovered(), anim::HOVER);
    theme::hover_surface(ui.painter(), rect, theme::R_SM, p.primary, hot, on);
    ui.painter().rect_filled(
        Rect::from_center_size(Pos2::new(rect.left() + 10.0, rect.center().y), Vec2::splat(5.0)),
        egui::Rounding::ZERO,
        theme::wash(theme::element_color(e), if on { 255 } else { (150.0 + 100.0 * hot) as u8 }),
    );
    let ink = if on { p.primary_light } else { theme::mix(p.text_dim, p.text, hot) };
    theme::tracked_text(ui.painter(), Pos2::new(rect.left() + 17.0, rect.center().y), &label, font, ink, 0.6);
    resp.on_hover_text(format!("{}  ·  {chord}", e.label())).clicked()
}

const TITLE_MIN_PX: f32 = 16.0;

fn slide_in(ui: &mut egui::Ui, island: Rect, arrive: f32, distance: f32) -> egui::Ui {
    let dy = (1.0 - arrive) * distance;
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(island.translate(Vec2::new(0.0, dy)))
            .layout(Layout::top_down(Align::Min)),
    );
    child.set_clip_rect(island.intersect(ui.clip_rect()));
    child.set_opacity(0.08 + 0.92 * arrive);
    child
}

const BANNER_H: f32 = 86.0;

fn banner_shape(rect: Rect) -> egui::Shape {
    let p = pal();
    egui::Shape::Vec(vec![
        egui::Shape::rect_filled(rect, egui::Rounding::ZERO, p.solid),
        egui::Shape::line_segment(
            [rect.left_bottom(), rect.right_bottom()],
            Stroke::new(1.0_f32, p.line),
        ),
    ])
}

#[allow(dead_code)]
fn empty_state(ui: &mut egui::Ui) {
    let p = pal();
    ui.vertical_centered(|ui| {
        ui.add_space(150.0);
        let (tile, _) = ui.allocate_exact_size(Vec2::splat(78.0), Sense::hover());
        logo::paint(ui.painter(), tile, 0.3 * anim::breathe(ui.ctx(), 4.0));
        ui.add_space(18.0);
        ui.label(
            egui::RichText::new("No script open")
                .font(theme::font_semi(theme::T_H))
                .color(p.text),
        );
    });
}

fn short_home(q: &Path) -> String {
    let s = q.display().to_string();
    match dirs::home_dir() {
        Some(h) => {
            let h = h.display().to_string();
            if s.starts_with(&h) {
                format!("~{}", &s[h.len()..])
            } else {
                s
            }
        }
        None => s,
    }
}

fn relative_time(t: SystemTime) -> String {
    let Ok(d) = SystemTime::now().duration_since(t) else {
        return "now".to_string();
    };
    ago_secs(d.as_secs())
}

fn ago_instant(t: Instant) -> String {
    ago_secs(t.elapsed().as_secs())
}

fn ago_secs(s: u64) -> String {
    if s < 90 {
        "just now".to_string()
    } else if s < 3600 {
        format!("{}m ago", s / 60)
    } else if s < 86_400 {
        format!("{}h ago", s / 3600)
    } else {
        format!("{}d ago", s / 86_400)
    }
}

pub fn today() -> String {
    chrono::Local::now().format("%B %e, %Y").to_string().replace("  ", " ")
}

fn starter_document() -> Document {
    let mut doc = Document::default();
    doc.blocks.clear();
    doc.meta.title = "Untitled Script".to_string();
    doc.meta.draft = today();
    doc.push(Element::SceneHeading, "INT. EDIT BAY - NIGHT");
    doc.push(
        Element::Action,
        "Monitors glow. Coffee gone cold. A cursor blinks in an empty document.",
    );
    doc.push(Element::Character, "THE WRITER");
    doc.push(Element::Parenthetical, "(to no one)");
    doc.push(Element::Dialogue, "Alright. Page one.");
    doc.push(Element::Transition, "SMASH CUT TO:");
    doc.reseed_ids();
    if let Some(b) = doc.blocks.first_mut() {
        b.note = "The writer faces the empty page.".to_string();
    }
    doc
}

fn youtrack_button(ui: &mut egui::Ui, url: &str) -> bool {
    let p = pal();
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 34.0), Sense::click());
    let hot = anim::ease(ui.ctx(), resp.id, resp.hovered(), anim::HOVER);

    ui.painter().rect_stroke(
        rect.shrink(0.5),
        egui::Rounding::same(theme::R_SM),
        Stroke::new(1.0_f32, theme::mix(p.line_strong, p.text, hot)),
    );
    ui::wipe(ui.painter(), rect.shrink(1.0), hot, if p.dark { p.raised } else { p.sunken });

    let mark = Rect::from_center_size(Pos2::new(rect.left() + 20.0, rect.center().y), Vec2::splat(20.0));
    youtrack_logo(ui.painter(), mark);
    ui.painter().text(
        Pos2::new(rect.left() + 38.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        "View on YouTrack",
        theme::font_semi(theme::T_SM),
        theme::mix(p.text_dim, p.text, hot),
    );
    icons::draw(
        ui.painter(),
        Rect::from_center_size(Pos2::new(rect.right() - 16.0, rect.center().y), Vec2::splat(12.0)),
        Icon::External,
        theme::mix(p.text_faint, p.text, hot),
    );
    resp.on_hover_text(url.to_string()).clicked()
}

pub fn youtrack_logo(painter: &egui::Painter, rect: Rect) {
    let s = rect.width().min(rect.height());
    let r = Rect::from_center_size(rect.center(), Vec2::splat(s));
    let pink = Color32::from_rgb(0xFF, 0x31, 0x8C);
    let violet = Color32::from_rgb(0x6B, 0x57, 0xFF);
    let cyan = Color32::from_rgb(0x07, 0xC3, 0xF2);
    theme::grad_rect(painter, r, s * 0.18, pink, violet, Vec2::new(1.0, 0.6));
    let corner = Rect::from_min_max(
        Pos2::new(r.left() + s * 0.45, r.top() + s * 0.45),
        r.max,
    );
    theme::fill_grad_poly(
        painter,
        &theme::rounded_poly(corner, s * 0.18),
        theme::wash(cyan, 0),
        theme::wash(cyan, 230),
        Vec2::new(0.7, 1.0),
    );
    let inner = Rect::from_min_max(
        Pos2::new(r.left() + s * 0.17, r.top() + s * 0.17),
        Pos2::new(r.right() - s * 0.17, r.bottom() - s * 0.17),
    );
    painter.rect_filled(inner, egui::Rounding::same(s * 0.04), Color32::BLACK);
    painter.text(
        Pos2::new(inner.left() + s * 0.08, inner.top() + s * 0.02),
        egui::Align2::LEFT_TOP,
        "YT",
        theme::font_bold(s * 0.33),
        Color32::WHITE,
    );
    let bar = Rect::from_min_size(
        Pos2::new(inner.left() + s * 0.09, inner.bottom() - s * 0.15),
        Vec2::new(s * 0.30, s * 0.055),
    );
    painter.rect_filled(bar, egui::Rounding::ZERO, Color32::WHITE);
}

fn settings_tab_row(ui: &mut egui::Ui, label: &str, icon: Icon, on: bool) -> (bool, Rect) {
    let p = pal();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 36.0), Sense::click());
    let hot = anim::ease(ui.ctx(), resp.id, resp.hovered(), anim::HOVER);
    let lit = anim::ease(ui.ctx(), resp.id.with("on"), on, 0.15);
    if lit > 0.005 {
        ui.painter()
            .rect_filled(rect, egui::Rounding::ZERO, theme::wash(p.text, (255.0 * lit) as u8));
    } else if hot > 0.01 {
        ui.painter().rect_filled(
            rect,
            egui::Rounding::ZERO,
            theme::wash(if p.dark { p.raised } else { p.sunken }, (255.0 * hot) as u8),
        );
    }
    let ink = theme::mix(theme::mix(p.text_dim, p.text, hot), p.backdrop, lit);
    icons::draw(
        ui.painter(),
        Rect::from_center_size(Pos2::new(rect.left() + 18.0, rect.center().y), Vec2::splat(13.0)),
        icon,
        ink,
    );
    theme::tracked_text(
        ui.painter(),
        Pos2::new(rect.left() + 34.0, rect.center().y),
        &label.to_uppercase(),
        theme::font_hud(theme::T_CAP),
        ink,
        0.9,
    );
    (resp.clicked(), rect)
}
