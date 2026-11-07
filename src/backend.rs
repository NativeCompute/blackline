use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use eframe::egui;
use web_time::SystemTime;

use crate::alerts::Tone;
use crate::model::Document;
use crate::settings::{AfterExport, Settings};
use crate::storage::{Entry, Snapshot};

#[derive(Clone)]
pub struct Person {
    pub id: String,
    pub name: String,
    pub avatar: Option<Arc<egui::ColorImage>>,
}

impl Person {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Person {
        Person {
            id: id.into(),
            name: name.into(),
            avatar: None,
        }
    }

    pub fn initials(&self) -> String {
        let mut out = String::new();
        for word in self.name.split_whitespace().take(2) {
            if let Some(c) = word.chars().next() {
                out.extend(c.to_uppercase());
            }
        }
        if out.is_empty() {
            out.push('?');
        }
        out
    }

    pub fn first_name(&self) -> &str {
        self.name.split_whitespace().next().unwrap_or(&self.name)
    }
}

impl PartialEq for Person {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl fmt::Debug for Person {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Person({} {:?})", self.id, self.name)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role {
    Owner,
    Editor,
    Viewer,
}

impl Role {
    pub const ALL: [Role; 3] = [Role::Owner, Role::Editor, Role::Viewer];
    pub fn label(self) -> &'static str {
        match self {
            Role::Owner => "Owner",
            Role::Editor => "Editor",
            Role::Viewer => "Viewer",
        }
    }
    pub fn slug(self) -> &'static str {
        match self {
            Role::Owner => "owner",
            Role::Editor => "editor",
            Role::Viewer => "viewer",
        }
    }
    pub fn from_slug(s: &str) -> Option<Role> {
        match s {
            "owner" => Some(Role::Owner),
            "editor" => Some(Role::Editor),
            "viewer" => Some(Role::Viewer),
            _ => None,
        }
    }
    pub fn can_write(self) -> bool {
        self != Role::Viewer
    }
}

#[derive(Clone, Debug)]
pub struct Member {
    pub person: Person,
    pub role: Role,
    pub pending: bool,
    pub fixed: bool,
}

#[derive(Clone, Debug)]
pub struct Team {
    pub name: String,
    pub me: Person,
    pub role: Role,
    pub members: Vec<Member>,
}

impl Team {
    pub fn person(&self, id: &str) -> Option<&Person> {
        self.members
            .iter()
            .map(|m| &m.person)
            .chain(std::iter::once(&self.me))
            .find(|p| p.id == id)
    }
}

#[derive(Clone, Debug)]
pub struct EditSession {
    pub by: Person,
    pub from: SystemTime,
    pub to: SystemTime,
    pub words: i64,
    pub saves: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Access {
    Edit,
    ReadOnly { by: Option<Person> },
}

impl Access {
    pub fn can_edit(&self) -> bool {
        matches!(self, Access::Edit)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
    Local,
    Space(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Space {
    pub id: String,
    pub name: String,
    pub personal: bool,
    pub can_write: bool,
    pub scripts: usize,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Sync {
    pub text: String,
    pub warn: bool,
    pub busy: bool,
}

#[derive(Debug)]
pub enum Load {
    Ready(Box<Document>, Access),
    Pending,
    Failed(String),
}

#[derive(Debug)]
pub enum Event {
    Opened {
        key: PathBuf,
        doc: Document,
        access: Access,
    },
    LibraryChanged,
    Conflict {
        key: PathBuf,
        theirs: Document,
        by: Person,
    },
    Locked { key: PathBuf, by: Person },
    LockFree { key: PathBuf },
    Refreshed { key: PathBuf, doc: Document },
    SnapshotDoc { snap: PathBuf, doc: Document },
    ImportPath(PathBuf),
    Imported { name: String, bytes: Vec<u8> },
    Notice {
        title: String,
        detail: String,
        tone: Tone,
    },
    SignedOut,
}

impl Event {
    pub fn notice(title: &str, detail: &str, tone: Tone) -> Event {
        Event::Notice {
            title: title.to_string(),
            detail: detail.to_string(),
            tone,
        }
    }
}

pub trait Store {
    // this library here---

    fn team(&self) -> Option<&Team> {
        None
    }
    fn location(&self) -> String;

    // scripts here---

    fn list(&mut self) -> Vec<Entry>;
    fn changed(&mut self) -> bool;
    fn load(&mut self, key: &Path) -> Load;
    fn close(&mut self, _key: &Path) {}
    fn create(&mut self, doc: &Document) -> Result<PathBuf, String>;
    fn save(&mut self, key: &Path, doc: &Document) -> Result<(), String>;
    fn follow_title(&mut self, key: &Path, _title: &str) -> PathBuf {
        key.to_path_buf()
    }
    fn duplicate(&mut self, key: &Path) -> Result<Option<String>, String>;
    fn delete(&mut self, key: &Path) -> Result<(), String>;
    fn set_starred(&mut self, key: &Path, on: bool) -> Result<(), String>;
    fn request_edit(&mut self, _key: &Path) {}
    fn search(&mut self, _needle: &str) -> Option<Vec<crate::storage::Found>> {
        None
    }

    // snapshots history here---

    fn take_snapshot(&mut self, key: &Path, doc: &Document, label: Option<&str>) -> Result<(), String>;
    fn snapshots(&mut self, key: &Path) -> Option<Vec<Snapshot>>;
    fn load_snapshot(&mut self, snap: &Path) -> Load;
    fn history(&mut self, _key: &Path) -> Option<Vec<EditSession>> {
        None
    }
    fn trash(&mut self) -> Option<Vec<Entry>> {
        None
    }
    fn restore(&mut self, _key: &Path) {}

    // team here---

    fn add_member(&mut self, _discord_id: &str, _role: Role) {}
    fn set_role(&mut self, _discord_id: &str, _role: Role) {}
    fn remove_member(&mut self, _discord_id: &str) {}
    fn sign_out(&mut self) {}

    // settings here---

    fn read_settings(&mut self) -> Settings;
    fn write_settings(&mut self, s: &Settings) -> Result<(), String>;

    // world outside here---

    fn deliver(&mut self, name: &str, bytes: Vec<u8>, after: Option<AfterExport>) -> Result<String, String>;
    fn start_import(&mut self);
    fn open_url(&mut self, url: &str);
    fn reveal(&mut self, _key: Option<&Path>) {}
    fn open_exports(&mut self) {}

    fn events(&mut self) -> Vec<Event>;
    fn busy(&self) -> bool {
        false
    }

    // cloud here---

    fn spaces(&self) -> Vec<Space> {
        Vec::new()
    }
    fn source(&self) -> Source {
        Source::Local
    }
    fn set_source(&mut self, _source: Source) {}
    fn place(&self, _key: &Path) -> Option<String> {
        None
    }
    fn move_to(&mut self, _key: &Path, _space: Option<&str>) -> Result<PathBuf, String> {
        Err("NativeCloud isn't on in this build.".into())
    }
    fn sync(&self) -> Option<Sync> {
        None
    }
    fn account_changed(&mut self, _signed_in: bool) {}
}

// desktop here---

#[cfg(not(target_arch = "wasm32"))]
pub use local::LocalBackend;

#[cfg(not(target_arch = "wasm32"))]
mod local {
    use std::path::{Path, PathBuf};
    use std::sync::mpsc;

    use web_time::{Duration, Instant, SystemTime};

    use super::{Access, Event, Load, Store};
    use crate::alerts::Tone;
    use crate::model::Document;
    use crate::settings::{AfterExport, Settings};
    use crate::storage::{self, Entry, Snapshot};

    #[derive(Default)]
    pub struct LocalBackend {
        library_seen: Option<((SystemTime, usize), Instant)>,
        importing: Option<mpsc::Receiver<Result<Option<PathBuf>, String>>>,
        events: Vec<Event>,
    }

    impl LocalBackend {
        pub fn new() -> LocalBackend {
            let _ = storage::ensure_dirs();
            LocalBackend::default()
        }
    }

    fn library_signature() -> Option<(SystemTime, usize)> {
        let dir = storage::scripts_dir();
        let modified = std::fs::metadata(&dir).and_then(|m| m.modified()).ok()?;
        let count = std::fs::read_dir(&dir).map(|d| d.count()).unwrap_or(0);
        Some((modified, count))
    }

    impl Store for LocalBackend {
        fn location(&self) -> String {
            storage::scripts_dir().display().to_string()
        }

        fn list(&mut self) -> Vec<Entry> {
            let entries = storage::list_scripts();
            if let Some(sig) = library_signature() {
                self.library_seen = Some((sig, Instant::now()));
            }
            entries
        }

        fn changed(&mut self) -> bool {
            if let Some((_, at)) = self.library_seen {
                if at.elapsed() < Duration::from_millis(1500) {
                    return false;
                }
            }
            match (library_signature(), self.library_seen) {
                (Some(now), Some((seen, _))) if now == seen => {
                    self.library_seen = Some((now, Instant::now()));
                    false
                }
                _ => true,
            }
        }

        fn search(&mut self, needle: &str) -> Option<Vec<storage::Found>> {
            Some(storage::search_scripts(needle, 3))
        }

        fn load(&mut self, key: &Path) -> Load {
            match storage::load(key) {
                Ok(doc) => Load::Ready(Box::new(doc), Access::Edit),
                Err(e) => Load::Failed(e.to_string()),
            }
        }

        fn create(&mut self, doc: &Document) -> Result<PathBuf, String> {
            let path = storage::unique_path(&doc.meta.title, None);
            storage::save(&path, doc).map_err(|e| e.to_string())?;
            Ok(path)
        }

        fn save(&mut self, key: &Path, doc: &Document) -> Result<(), String> {
            storage::save(key, doc).map_err(|e| e.to_string())
        }

        fn follow_title(&mut self, key: &Path, title: &str) -> PathBuf {
            let mut path = key.to_path_buf();
            let wanted = storage::slugify(title);
            let current_stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            let stem_matches = current_stem == wanted
                || current_stem
                    .rsplit_once('-')
                    .map(|(head, tail)| head == wanted && tail.parse::<u32>().is_ok())
                    .unwrap_or(false);
            if !stem_matches {
                let new_path = storage::unique_path(title, Some(&path));
                if path.exists() {
                    if std::fs::rename(&path, &new_path).is_ok() {
                        storage::follow_rename(&path, &new_path);
                        path = new_path;
                    }
                } else {
                    path = new_path;
                }
            }
            path
        }

        fn duplicate(&mut self, key: &Path) -> Result<Option<String>, String> {
            let mut doc = storage::load(key).map_err(|e| e.to_string())?;
            doc.meta.title = format!("{} (copy)", doc.meta.title);
            doc.meta.starred = false;
            let new_path = storage::unique_path(&doc.meta.title, None);
            storage::save(&new_path, &doc).map_err(|e| e.to_string())?;
            Ok(Some(doc.meta.title))
        }

        fn delete(&mut self, key: &Path) -> Result<(), String> {
            storage::delete(key).map_err(|e| e.to_string())
        }

        fn set_starred(&mut self, key: &Path, on: bool) -> Result<(), String> {
            let mut doc = storage::load(key).map_err(|e| e.to_string())?;
            doc.meta.starred = on;
            storage::save(key, &doc).map_err(|e| e.to_string())
        }

        fn take_snapshot(&mut self, key: &Path, doc: &Document, _label: Option<&str>) -> Result<(), String> {
            storage::take_snapshot(key, doc).map(|_| ()).map_err(|e| e.to_string())
        }

        fn snapshots(&mut self, key: &Path) -> Option<Vec<Snapshot>> {
            Some(storage::list_snapshots(key))
        }

        fn load_snapshot(&mut self, snap: &Path) -> Load {
            self.load(snap)
        }

        fn read_settings(&mut self) -> Settings {
            storage::read_settings()
        }

        fn write_settings(&mut self, s: &Settings) -> Result<(), String> {
            storage::write_settings(s).map_err(|e| e.to_string())
        }

        fn deliver(&mut self, name: &str, bytes: Vec<u8>, after: Option<AfterExport>) -> Result<String, String> {
            storage::ensure_dirs().map_err(|e| e.to_string())?;
            let path = storage::exports_dir().join(name);
            std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
            match after {
                Some(AfterExport::Open) => storage::open_with_desktop(&path),
                Some(AfterExport::Reveal) => storage::reveal_in_file_manager(&path),
                Some(AfterExport::Nothing) | None => {}
            }
            Ok(name.to_string())
        }

        fn start_import(&mut self) {
            let (tx, rx) = mpsc::channel();
            if cfg!(target_os = "macos") {
                let _ = tx.send(storage::pick_file_to_import());
            } else {
                std::thread::spawn(move || {
                    let _ = tx.send(storage::pick_file_to_import());
                });
            }
            self.importing = Some(rx);
        }

        fn open_url(&mut self, url: &str) {
            storage::open_target(std::ffi::OsStr::new(url));
        }

        fn reveal(&mut self, key: Option<&Path>) {
            match key {
                Some(p) => storage::reveal_in_file_manager(p),
                None => storage::open_with_desktop(&storage::scripts_dir()),
            }
        }

        fn open_exports(&mut self) {
            storage::open_with_desktop(&storage::exports_dir());
        }

        fn events(&mut self) -> Vec<Event> {
            if let Some(rx) = &self.importing {
                match rx.try_recv() {
                    Ok(Ok(Some(path))) => {
                        self.importing = None;
                        self.events.push(Event::ImportPath(path));
                    }
                    Ok(Ok(None)) => self.importing = None,
                    Ok(Err(e)) => {
                        self.importing = None;
                        self.events.push(Event::notice("No file picker", &e, Tone::Warn));
                    }
                    Err(mpsc::TryRecvError::Empty) => {}
                    Err(mpsc::TryRecvError::Disconnected) => self.importing = None,
                }
            }
            std::mem::take(&mut self.events)
        }

        fn busy(&self) -> bool {
            self.importing.is_some()
        }
    }
}
