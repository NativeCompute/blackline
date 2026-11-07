use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use ncf::cloud::{self as nc, Client, CloudError, Device, Engine, Heartbeat, Outbox, Pending, Worker};
use serde_json::{json, Value};
use web_time::{Instant, SystemTime};

use crate::alerts::Tone;
use crate::backend::{Access, EditSession, Event, Load, LocalBackend, Member, Person, Role, Source, Space, Store, Sync, Team};
use crate::model::Document;
use crate::settings::{AfterExport, Settings};
use crate::storage::{self, Entry, Found, Snapshot};

pub const APP: &str = "blackline";

pub struct All {
    me: nc::Me,
    projects: Vec<nc::Project>,
    teams: Vec<nc::Team>,
    leases: Vec<(String, nc::Lease)>,
    stars: Vec<String>,
}

pub enum Reply {
    All(Result<Box<All>, CloudError>),
    Fetched(String, Result<nc::Project, CloudError>),
    Opened(PathBuf, bool, Result<nc::Opened, CloudError>),
    Leases(Result<Vec<(String, nc::Lease)>, CloudError>),
    Snaps(PathBuf, Result<Vec<nc::Snapshot>, CloudError>),
    SnapBody(PathBuf, Result<String, CloudError>),
    Sessions(PathBuf, Result<Vec<nc::EditSession>, CloudError>),
    Done(&'static str, Result<(), CloudError>),
}

type SpaceRow = (nc::Workspace, nc::Role, Vec<(nc::Member, nc::Person)>);

#[derive(Clone, Debug, PartialEq)]
struct Known {
    workspace: String,
    version: i64,
}

pub struct CloudStore {
    local: LocalBackend,
    client: Client,
    engine: Engine<Reply>,
    root: PathBuf,
    index: HashMap<String, Known>,
    spaces: Vec<SpaceRow>,
    projects: Vec<nc::Project>,
    leases: HashMap<String, nc::Lease>,
    stars: HashSet<String>,
    me: Option<nc::Me>,
    source: Source,
    team: Option<Team>,
    events: Vec<Event>,
    open: Option<(PathBuf, String)>,
    editing: bool,
    snaps: HashMap<PathBuf, (Vec<Snapshot>, Instant)>,
    sessions: HashMap<PathBuf, (Vec<EditSession>, Instant)>,
    kept: HashSet<String>,
    fetching: HashSet<String>,
    changed: bool,
    state: nc::SyncState,
    loaded: bool,
}

fn person(p: &nc::Person) -> Person {
    Person::new(p.id.clone(), p.shown())
}

fn role(r: nc::Role) -> Role {
    match r {
        nc::Role::Owner | nc::Role::Admin => Role::Owner,
        nc::Role::Editor => Role::Editor,
        nc::Role::Viewer => Role::Viewer,
    }
}

fn system_time(text: &str) -> SystemTime {
    nc::parse_time(text).map(|t| SystemTime::UNIX_EPOCH + Duration::from_secs(t.max(0) as u64)).unwrap_or(SystemTime::UNIX_EPOCH)
}

fn local_when(text: &str) -> String {
    match nc::parse_time(text).and_then(|t| chrono::DateTime::from_timestamp(t, 0)) {
        Some(t) => t.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M").to_string(),
        None => text.to_string(),
    }
}

pub fn meta_of(doc: &Document) -> Value {
    let body = storage::to_markdown(doc);
    let preview = storage::entry_from_text(PathBuf::new(), &body, SystemTime::UNIX_EPOCH).preview;
    json!({"pages": crate::export::page_count(doc), "scenes": doc.scene_count(), "preview": preview, "author": doc.meta.author})
}

impl CloudStore {
    pub fn new(client: Client, device: Device, ctx: Option<&eframe::egui::Context>) -> CloudStore {
        let root = storage::library_root().join("cloud");
        let outbox = Outbox::load(root.join("outbox.json"));
        let engine = Engine::spawn(Worker::new(client.clone(), device, outbox, Heartbeat::sync()), ctx.cloned());
        CloudStore::with_engine(client, engine, root)
    }

    pub fn with_engine(client: Client, engine: Engine<Reply>, root: PathBuf) -> CloudStore {
        let _ = std::fs::create_dir_all(root.join("snapshots"));
        let mut s = CloudStore {
            local: LocalBackend::new(),
            client,
            engine,
            root,
            index: HashMap::new(),
            spaces: Vec::new(),
            projects: Vec::new(),
            leases: HashMap::new(),
            stars: HashSet::new(),
            me: None,
            source: Source::Local,
            team: None,
            events: Vec::new(),
            open: None,
            editing: false,
            snaps: HashMap::new(),
            sessions: HashMap::new(),
            kept: HashSet::new(),
            fetching: HashSet::new(),
            changed: false,
            state: nc::SyncState::default(),
            loaded: false,
        };
        s.read_index();
        if s.client.signed_in() {
            s.refresh();
        }
        s
    }

    pub fn engine(&mut self) -> &mut Engine<Reply> {
        &mut self.engine
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn key_for(&self, id: &str) -> PathBuf {
        self.root.join(format!("{id}.md"))
    }

    pub fn id_of(&self, key: &Path) -> Option<String> {
        if key.parent()? != self.root {
            return None;
        }
        let stem = key.file_stem()?.to_str()?;
        nc::is_id(stem).then(|| stem.to_string())
    }

    fn snap_key(&self, id: &str) -> PathBuf {
        self.root.join("snapshots").join(format!("{id}.md"))
    }

    fn snap_id(&self, key: &Path) -> Option<String> {
        if key.parent()? != self.root.join("snapshots") {
            return None;
        }
        key.file_stem()?.to_str().map(str::to_string)
    }

    // index here---

    fn read_index(&mut self) {
        let text = std::fs::read_to_string(self.root.join("index.json")).unwrap_or_default();
        let v: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
        if let Some(map) = v.get("projects").and_then(Value::as_object) {
            for (id, k) in map {
                let workspace = k.get("workspace").and_then(Value::as_str).unwrap_or("").to_string();
                let version = k.get("version").and_then(Value::as_i64).unwrap_or(0);
                self.index.insert(id.clone(), Known { workspace, version });
            }
        }
        if let Some(list) = v.get("spaces").and_then(Value::as_array) {
            for w in list {
                let ws: nc::Workspace = serde_json::from_value(w.get("workspace").cloned().unwrap_or(Value::Null)).unwrap_or_default();
                let r = nc::Role::parse(w.get("role").and_then(Value::as_str).unwrap_or("")).unwrap_or_default();
                if !ws.id.is_empty() {
                    self.spaces.push((ws, r, Vec::new()));
                }
            }
        }
        if let Some(src) = v.get("source").and_then(Value::as_str) {
            if self.spaces.iter().any(|(w, _, _)| w.id == src) {
                self.source = Source::Space(src.to_string());
            }
        }
        self.rebuild_team();
    }

    fn write_index(&self) {
        let projects: serde_json::Map<String, Value> =
            self.index.iter().map(|(id, k)| (id.clone(), json!({"workspace": k.workspace, "version": k.version}))).collect();
        let spaces: Vec<Value> = self
            .spaces
            .iter()
            .map(|(w, r, _)| json!({"workspace": {"id": w.id, "name": w.name, "kind": w.kind, "owner_id": w.owner_id}, "role": r.as_str()}))
            .collect();
        let source = match &self.source {
            Source::Space(id) => Value::String(id.clone()),
            Source::Local => Value::Null,
        };
        let text = json!({"projects": projects, "spaces": spaces, "source": source}).to_string();
        let _ = std::fs::create_dir_all(&self.root);
        let _ = std::fs::write(self.root.join("index.json"), text);
    }

    fn write_cache(&mut self, p: &nc::Project) {
        if let Some(body) = &p.body {
            let _ = std::fs::write(self.key_for(&p.id), body);
        }
        self.index.insert(p.id.clone(), Known { workspace: p.workspace_id.clone(), version: p.version });
        self.write_index();
    }

    fn waiting(&self, id: &str) -> bool {
        let text = std::fs::read_to_string(self.root.join("outbox.json")).unwrap_or_default();
        text.contains(id)
    }

    // spaces here---

    fn space_name(&self, id: &str) -> String {
        self.spaces.iter().find(|(w, _, _)| w.id == id).map(|(w, _, _)| w.shown()).unwrap_or_else(|| "NativeCloud".into())
    }

    fn space_role(&self, id: &str) -> Option<nc::Role> {
        self.spaces.iter().find(|(w, _, _)| w.id == id).map(|(_, r, _)| *r)
    }

    fn rebuild_team(&mut self) {
        self.team = None;
        let Source::Space(id) = &self.source else {
            return;
        };
        let Some((w, r, members)) = self.spaces.iter().find(|(w, _, _)| &w.id == id) else {
            return;
        };
        if w.is_personal() {
            return;
        }
        let me_id = self.client.user_id();
        let me = members.iter().find(|(m, _)| m.user_id == me_id).map(|(_, p)| person(p)).unwrap_or_else(|| {
            let name = self.me.as_ref().map(|m| m.shown()).unwrap_or_default();
            Person::new(me_id.clone(), name)
        });
        let members = members
            .iter()
            .map(|(m, p)| Member { person: person(p), role: role(m.role), pending: false, fixed: m.role == nc::Role::Owner })
            .collect();
        self.team = Some(Team { name: w.name.clone(), me, role: role(*r), members });
    }

    pub fn refresh(&mut self) {
        self.engine.run(|c| {
            let all = || -> Result<Box<All>, CloudError> {
                Ok(Box::new(All { me: c.me()?, projects: c.projects(APP)?, teams: c.teams()?, leases: c.leases()?, stars: c.stars()? }))
            };
            Reply::All(all())
        });
    }

    fn fetch(&mut self, id: &str) {
        if !self.fetching.insert(id.to_string()) {
            return;
        }
        let id = id.to_string();
        self.engine.run(move |c| Reply::Fetched(id.clone(), c.project(&id)));
    }

    fn cloud_entries(&self, space: &str) -> Vec<Entry> {
        let me = self.client.user_id();
        let mut out = Vec::new();
        let mut seen = HashSet::new();
        for p in self.projects.iter().filter(|p| p.workspace_id == space && !p.in_trash()) {
            seen.insert(p.id.clone());
            out.push(self.entry_for(p.id.as_str(), Some(p), &me));
        }
        for id in self.index.iter().filter(|(_, k)| k.workspace == space).map(|(id, _)| id) {
            if !seen.contains(id) && (!self.loaded || self.waiting(id)) {
                out.push(self.entry_for(id, None, &me));
            }
        }
        out.sort_by_key(|a| std::cmp::Reverse(a.modified));
        out
    }

    fn entry_for(&self, id: &str, p: Option<&nc::Project>, me: &str) -> Entry {
        let key = self.key_for(id);
        let text = std::fs::read_to_string(&key).ok();
        let modified = p.map(|p| system_time(&p.updated_at)).unwrap_or_else(|| std::fs::metadata(&key).and_then(|m| m.modified()).unwrap_or(SystemTime::UNIX_EPOCH));
        let mut e = match &text {
            Some(t) => storage::entry_from_text(key.clone(), t, modified),
            None => Entry {
                path: key.clone(),
                title: p.map(|p| p.title.clone()).unwrap_or_default(),
                modified,
                preview: p.and_then(|p| p.meta.get("preview")).and_then(Value::as_str).unwrap_or("").to_string(),
                pages: p.and_then(|p| p.meta.get("pages")).and_then(Value::as_u64).unwrap_or(0) as usize,
                scenes: p.and_then(|p| p.meta.get("scenes")).and_then(Value::as_u64).unwrap_or(0) as usize,
                ..Default::default()
            },
        };
        let personal = p.map(|p| self.spaces.iter().any(|(w, _, _)| w.id == p.workspace_id && w.is_personal())).unwrap_or(true);
        if let Some(p) = p {
            if !p.title.trim().is_empty() {
                e.title = p.title.clone();
            }
            e.edited_by = p.updated_by.as_ref().filter(|b| !b.id.is_empty() && !personal).map(|b| {
                let named = self.spaces.iter().flat_map(|(_, _, m)| m.iter()).find(|(_, x)| x.id == b.id).map(|(_, x)| x.clone()).unwrap_or_else(|| b.clone());
                person(&named)
            });
        }
        e.starred = self.stars.contains(id);
        e.editing = self.leases.get(id).filter(|l| l.user.as_ref().map(|u| u.id != me).unwrap_or(true)).and_then(|l| l.user.as_ref()).map(person);
        e.cloud = Some(self.space_name(&self.index.get(id).map(|k| k.workspace.clone()).or(p.map(|p| p.workspace_id.clone())).unwrap_or_default()));
        e.waiting = self.waiting(id);
        e
    }

    // replies here---

    fn handle(&mut self, r: Reply) {
        match r {
            Reply::All(Ok(all)) => {
                let All { me, projects, teams, leases, stars } = *all;
                self.me = Some(me);
                self.spaces = teams.into_iter().map(|t| (t.workspace, t.my_role, t.members)).collect();
                self.spaces.sort_by_key(|(w, _, _)| (!w.is_personal(), w.name.to_lowercase()));
                if let Source::Space(id) = &self.source {
                    if !self.spaces.iter().any(|(w, _, _)| &w.id == id) {
                        self.source = Source::Local;
                    }
                }
                self.leases = leases.into_iter().collect();
                self.stars = stars.into_iter().collect();
                let live: HashSet<String> = projects.iter().map(|p| p.id.clone()).collect();
                let gone: Vec<String> = self.index.keys().filter(|id| !live.contains(*id) && !self.waiting(id)).cloned().collect();
                for id in gone {
                    self.index.remove(&id);
                    let _ = std::fs::remove_file(self.key_for(&id));
                }
                for p in &projects {
                    let stale = self.index.get(&p.id).map(|k| k.version != p.version || k.workspace != p.workspace_id).unwrap_or(true);
                    let missing = !self.key_for(&p.id).exists();
                    if p.in_trash() {
                        continue;
                    }
                    if (stale || missing) && !self.waiting(&p.id) && !self.is_editing(&p.id) {
                        self.fetch(&p.id);
                    } else if let Some(k) = self.index.get_mut(&p.id) {
                        k.workspace = p.workspace_id.clone();
                    }
                }
                self.projects = projects;
                self.loaded = true;
                self.rebuild_team();
                self.write_index();
                self.changed = true;
                self.events.push(Event::LibraryChanged);
                self.lease_events();
            }
            Reply::Fetched(id, Ok(p)) => {
                self.fetching.remove(&id);
                if self.waiting(&id) || self.is_editing(&id) {
                    return;
                }
                self.write_cache(&p);
                if let Some(old) = self.projects.iter_mut().find(|x| x.id == id) {
                    *old = nc::Project { body: None, ..p.clone() };
                } else {
                    self.projects.push(nc::Project { body: None, ..p.clone() });
                }
                let key = self.key_for(&id);
                if self.open.as_ref().map(|(_, o)| o == &id).unwrap_or(false) {
                    self.events.push(Event::Refreshed { key, doc: storage::from_markdown(p.body_text()) });
                }
                self.changed = true;
                self.events.push(Event::LibraryChanged);
            }
            Reply::Opened(key, asked, Ok(o)) => {
                let id = o.project.id.clone();
                if self.open.as_ref().map(|(k, _)| k != &key).unwrap_or(true) {
                    return;
                }
                let doc = if self.waiting(&id) {
                    storage::load(&key).unwrap_or_else(|_| storage::from_markdown(o.project.body_text()))
                } else {
                    self.write_cache(&o.project);
                    storage::from_markdown(o.project.body_text())
                };
                let access = if o.editing() {
                    self.editing = true;
                    self.engine.hold(&id);
                    self.engine.watch(None);
                    Access::Edit
                } else {
                    self.editing = false;
                    self.engine.watch(Some(&id));
                    Access::ReadOnly { by: if o.can_write { o.lease.as_ref().and_then(|l| l.user.as_ref()).map(person) } else { None } }
                };
                let holder = o.lease_holder();
                if let Some(l) = o.lease {
                    self.leases.insert(id.clone(), l);
                }
                if asked && !access.can_edit() {
                    if let Some(by) = holder {
                        self.events.push(Event::Locked { key: key.clone(), by });
                    }
                    return;
                }
                self.events.push(Event::Opened { key, doc, access });
            }
            Reply::Opened(key, _, Err(CloudError::Offline)) => {
                if self.open.as_ref().map(|(k, _)| k == &key).unwrap_or(false) {
                    match storage::load(&key) {
                        Ok(doc) => {
                            self.editing = true;
                            self.events.push(Event::Opened { key, doc, access: Access::Edit });
                            self.events.push(Event::notice(
                                "You're offline",
                                "You're writing in this computer's copy. It syncs when you're back; if someone else changed it meanwhile, yours is kept as a snapshot.",
                                Tone::Warn,
                            ));
                        }
                        Err(_) => self.events.push(Event::notice("You're offline", "This script hasn't been downloaded to this computer yet.", Tone::Warn)),
                    }
                }
            }
            Reply::Opened(key, _, Err(e)) => {
                if self.open.as_ref().map(|(k, _)| k == &key).unwrap_or(false) {
                    self.open = None;
                }
                self.say(&e);
                self.refresh();
            }
            Reply::Leases(Ok(list)) => {
                self.leases = list.into_iter().collect();
                self.lease_events();
                self.changed = true;
            }
            Reply::Snaps(key, Ok(list)) => {
                let shown = list
                    .into_iter()
                    .map(|s| Snapshot {
                        path: self.snap_key(&s.id),
                        when: local_when(&s.created_at),
                        pages: 0,
                        by: self.spaces.iter().flat_map(|(_, _, m)| m.iter()).find(|(_, p)| p.id == s.created_by).map(|(_, p)| person(p)),
                        label: Some(if s.label.trim().is_empty() { kind_label(&s.kind).to_string() } else { s.label.clone() }),
                    })
                    .collect();
                self.snaps.insert(key, (shown, Instant::now()));
            }
            Reply::SnapBody(snap, Ok(body)) => {
                let _ = std::fs::write(&snap, &body);
                self.events.push(Event::SnapshotDoc { snap, doc: storage::from_markdown(&body) });
            }
            Reply::Sessions(key, Ok(list)) => {
                let people: Vec<nc::Person> = self.spaces.iter().flat_map(|(_, _, m)| m.iter().map(|(_, p)| p.clone())).collect();
                let shown = list
                    .into_iter()
                    .map(|s| EditSession {
                        by: people.iter().find(|p| p.id == s.user_id).map(person).unwrap_or_else(|| Person::new(s.user_id.clone(), "Someone")),
                        from: system_time(&s.started_at),
                        to: system_time(&s.ended_at),
                        words: s.words_delta,
                        saves: s.saves.max(0) as u32,
                    })
                    .collect();
                self.sessions.insert(key, (shown, Instant::now()));
            }
            Reply::Done(what, Ok(())) => {
                if matches!(what, "star" | "snapshot") {
                    return;
                }
                self.refresh();
            }
            Reply::Fetched(id, Err(e)) => {
                self.fetching.remove(&id);
                if !e.is_offline() && e != CloudError::NotFound {
                    self.say(&e);
                }
            }
            Reply::All(Err(e)) | Reply::Leases(Err(e)) | Reply::Snaps(_, Err(e)) | Reply::SnapBody(_, Err(e)) | Reply::Sessions(_, Err(e)) | Reply::Done(_, Err(e)) => {
                if !e.is_offline() {
                    self.say(&e);
                }
            }
        }
    }

    fn say(&mut self, e: &CloudError) {
        if *e == CloudError::SignedOut {
            self.events.push(Event::SignedOut);
            return;
        }
        self.events.push(Event::notice(e.title(), &e.message(), if e.is_offline() { Tone::Warn } else { Tone::Danger }));
    }

    fn is_editing(&self, id: &str) -> bool {
        self.editing && self.open.as_ref().map(|(_, o)| o == id).unwrap_or(false)
    }

    fn lease_events(&mut self) {
        let Some((key, id)) = self.open.clone() else {
            return;
        };
        if self.editing {
            return;
        }
        let me = self.client.user_id();
        match self.leases.get(&id) {
            Some(l) if l.user.as_ref().map(|u| u.id != me).unwrap_or(true) || !l.mine => {}
            _ => self.events.push(Event::LockFree { key }),
        }
    }

    fn engine_event(&mut self, e: nc::Event<Reply>) {
        match e {
            nc::Event::Reply(r) => self.handle(r),
            nc::Event::Synced { project, version } => {
                if let Some(k) = self.index.get_mut(&project) {
                    k.version = version;
                }
                if let Some(p) = self.projects.iter_mut().find(|p| p.id == project) {
                    p.version = version;
                    p.updated_at = nc::time_now_text();
                }
                self.write_index();
                self.changed = true;
            }
            nc::Event::Kept { project, theirs, lease, .. } => {
                let key = self.key_for(&project);
                self.kept.insert(project.clone());
                match theirs {
                    Some(t) => {
                        self.write_cache(&t);
                        let by = t.updated_by.as_ref().map(person).unwrap_or_else(|| Person::new("", "Someone"));
                        self.events.push(Event::Conflict { key, theirs: storage::from_markdown(t.body_text()), by });
                    }
                    None => {
                        self.editing = false;
                        let by = lease.as_ref().and_then(|l| l.user.as_ref()).map(person).unwrap_or_else(|| Person::new("", "Someone"));
                        self.events.push(Event::Locked { key, by });
                        self.fetch(&project);
                    }
                }
                self.changed = true;
            }
            nc::Event::Dropped { project, error } => {
                let key = self.key_for(&project);
                if let Ok(doc) = storage::load(&key) {
                    if let Ok(path) = self.local.create(&doc) {
                        self.events.push(Event::notice(
                            "Kept on this computer",
                            &format!("“{}” couldn't be saved to NativeCloud ({}). It's in On this computer now.", doc.meta.title, error.message()),
                            Tone::Warn,
                        ));
                        let _ = path;
                    }
                }
                self.index.remove(&project);
                let _ = std::fs::remove_file(&key);
                self.write_index();
                self.changed = true;
            }
            nc::Event::LeaseLost { project, lease } => {
                if self.open.as_ref().map(|(_, o)| o == &project).unwrap_or(false) {
                    self.editing = false;
                    self.engine.watch(Some(&project));
                    let by = lease.as_ref().and_then(|l| l.user.as_ref()).map(person).unwrap_or_else(|| Person::new("", "Someone"));
                    self.events.push(Event::Locked { key: self.key_for(&project), by });
                }
            }
            nc::Event::Feed(feed) => {
                let mut all = false;
                let mut leases = false;
                let me = self.client.user_id();
                for c in &feed.changes {
                    match (c.kind.as_str(), &c.project_id) {
                        ("project", Some(id)) if self.projects.iter().any(|p| &p.id == id) => {
                            let ours = c.by.as_deref() == Some(me.as_str()) && self.index.get(id).map(|k| self.projects.iter().any(|p| &p.id == id && p.version == k.version)).unwrap_or(false);
                            if !ours {
                                all = true;
                            }
                        }
                        ("lease", _) => leases = true,
                        ("versions", Some(id)) => {
                            let key = self.key_for(id);
                            self.snaps.remove(&key);
                        }
                        _ => all = true,
                    }
                }
                if all {
                    self.refresh();
                } else if leases {
                    self.engine.run(|c| Reply::Leases(c.leases()));
                }
            }
            nc::Event::Resync => self.refresh(),
            nc::Event::State(s) => {
                self.state = s;
            }
            nc::Event::SignedOut => self.events.push(Event::SignedOut),
        }
    }
}

fn kind_label(kind: &str) -> &'static str {
    match kind {
        "auto" => "Automatic",
        "offline" => "Your offline version",
        "conflict" => "Your version (kept)",
        "restore" => "Before a restore",
        "revision" => "Revision",
        _ => "Snapshot",
    }
}

trait Holder {
    fn lease_holder(&self) -> Option<Person>;
}

impl Holder for nc::Opened {
    fn lease_holder(&self) -> Option<Person> {
        self.lease.as_ref().and_then(|l| l.user.as_ref()).map(person)
    }
}

impl Store for CloudStore {
    fn team(&self) -> Option<&Team> {
        self.team.as_ref()
    }

    fn location(&self) -> String {
        match &self.source {
            Source::Local => self.local.location(),
            Source::Space(id) => format!("NativeCloud / {}", self.space_name(id)),
        }
    }

    fn list(&mut self) -> Vec<Entry> {
        self.changed = false;
        match self.source.clone() {
            Source::Local => self.local.list(),
            Source::Space(id) => self.cloud_entries(&id),
        }
    }

    fn changed(&mut self) -> bool {
        match self.source {
            Source::Local => self.local.changed() || std::mem::take(&mut self.changed),
            Source::Space(_) => std::mem::take(&mut self.changed),
        }
    }

    fn load(&mut self, key: &Path) -> Load {
        let Some(id) = self.id_of(key) else {
            return self.local.load(key);
        };
        if let Some((old, old_id)) = self.open.clone() {
            if old != key {
                self.engine.release(&old_id);
            }
        }
        self.open = Some((key.to_path_buf(), id.clone()));
        self.editing = false;
        if let Some(ws) = self.index.get(&id).map(|k| k.workspace.clone()) {
            if self.source != Source::Space(ws.clone()) {
                self.source = Source::Space(ws);
                self.rebuild_team();
            }
        }
        if !self.client.signed_in() {
            return match storage::load(key) {
                Ok(doc) => Load::Ready(Box::new(doc), Access::ReadOnly { by: None }),
                Err(e) => Load::Failed(e.to_string()),
            };
        }
        let device = self.engine.device().clone();
        let want = self.index.get(&id).and_then(|k| self.space_role(&k.workspace)).map(|r| r.can_write()).unwrap_or(true);
        let k = key.to_path_buf();
        self.engine.run(move |c| Reply::Opened(k, false, c.open_project(&id, &device, want)));
        Load::Pending
    }

    fn close(&mut self, key: &Path) {
        if let Some(id) = self.id_of(key) {
            if self.open.as_ref().map(|(k, _)| k == key).unwrap_or(false) {
                self.open = None;
                self.editing = false;
                self.engine.watch(None);
                if self.client.signed_in() {
                    self.engine.release(&id);
                }
            }
        }
    }

    fn create(&mut self, doc: &Document) -> Result<PathBuf, String> {
        let Source::Space(ws) = self.source.clone() else {
            return self.local.create(doc);
        };
        let body = storage::to_markdown(doc);
        match self.client.create_project(&ws, APP, &doc.meta.title, &body, &meta_of(doc)) {
            Ok(p) => {
                let p = nc::Project { body: Some(body), ..p };
                self.write_cache(&p);
                self.projects.push(nc::Project { body: None, ..p.clone() });
                self.changed = true;
                Ok(self.key_for(&p.id))
            }
            Err(CloudError::Offline) => {
                let path = self.local.create(doc)?;
                self.source = Source::Local;
                self.rebuild_team();
                self.events.push(Event::notice(
                    "Saved on this computer",
                    "NativeCloud can't be reached, so the new script is in On this computer. Move it to NativeCloud from its ⋯ menu later.",
                    Tone::Warn,
                ));
                Ok(path)
            }
            Err(e) => Err(e.message()),
        }
    }

    fn save(&mut self, key: &Path, doc: &Document) -> Result<(), String> {
        let Some(id) = self.id_of(key) else {
            return self.local.save(key, doc);
        };
        storage::save(key, doc).map_err(|e| e.to_string())?;
        let base = self.index.get(&id).map(|k| k.version).unwrap_or(0);
        let body = storage::to_markdown(doc);
        self.engine.save(Pending::new(&id, base, &body).titled(&doc.meta.title).with_meta(meta_of(doc)));
        self.changed = true;
        Ok(())
    }

    fn follow_title(&mut self, key: &Path, title: &str) -> PathBuf {
        if self.id_of(key).is_some() {
            return key.to_path_buf();
        }
        self.local.follow_title(key, title)
    }

    fn duplicate(&mut self, key: &Path) -> Result<Option<String>, String> {
        let Some(id) = self.id_of(key) else {
            return self.local.duplicate(key);
        };
        let title = storage::load(key).map(|d| format!("{} (copy)", d.meta.title)).unwrap_or_else(|_| "Copy".into());
        self.client.duplicate_project(&id, Some(&title)).map_err(|e| e.message())?;
        self.refresh();
        Ok(Some(title))
    }

    fn delete(&mut self, key: &Path) -> Result<(), String> {
        let Some(id) = self.id_of(key) else {
            return self.local.delete(key);
        };
        self.client.delete_project(&id).map_err(|e| e.message())?;
        if let Some(p) = self.projects.iter_mut().find(|p| p.id == id) {
            p.deleted_at = Some(nc::time_now_text());
        }
        self.index.remove(&id);
        let _ = std::fs::remove_file(key);
        self.write_index();
        self.changed = true;
        Ok(())
    }

    fn set_starred(&mut self, key: &Path, on: bool) -> Result<(), String> {
        let Some(id) = self.id_of(key) else {
            return self.local.set_starred(key, on);
        };
        if on {
            self.stars.insert(id.clone());
        } else {
            self.stars.remove(&id);
        }
        self.changed = true;
        self.engine.run(move |c| Reply::Done("star", c.set_star(&id, on)));
        Ok(())
    }

    fn request_edit(&mut self, key: &Path) {
        let Some(id) = self.id_of(key) else {
            return;
        };
        let device = self.engine.device().clone();
        let k = key.to_path_buf();
        self.engine.run(move |c| Reply::Opened(k, true, c.request_edit(&id, &device)));
    }

    fn search(&mut self, needle: &str) -> Option<Vec<Found>> {
        match self.source.clone() {
            Source::Local => self.local.search(needle),
            Source::Space(id) => Some(storage::search_entries(self.cloud_entries(&id), needle, 3)),
        }
    }

    fn take_snapshot(&mut self, key: &Path, doc: &Document, label: Option<&str>) -> Result<(), String> {
        let Some(id) = self.id_of(key) else {
            return self.local.take_snapshot(key, doc, label);
        };
        let kind = match label {
            Some("Your version (kept)") => {
                if self.kept.remove(&id) {
                    return Ok(());
                }
                "conflict"
            }
            Some("Before a restore") => "restore",
            _ => "manual",
        };
        let (label, body) = (label.unwrap_or("").to_string(), storage::to_markdown(doc));
        self.snaps.remove(key);
        if !self.client.signed_in() {
            return Err(CloudError::SignedOut.message());
        }
        self.engine.run(move |c| Reply::Done("snapshot", c.take_snapshot(&id, &label, kind, Some(&body)).map(|_| ())));
        Ok(())
    }

    fn snapshots(&mut self, key: &Path) -> Option<Vec<Snapshot>> {
        let Some(id) = self.id_of(key) else {
            return self.local.snapshots(key);
        };
        let fresh = self.snaps.get(key).map(|(_, at)| at.elapsed() < Duration::from_secs(20)).unwrap_or(false);
        if !fresh && self.client.signed_in() {
            self.snaps.entry(key.to_path_buf()).or_insert_with(|| (Vec::new(), Instant::now())).1 = Instant::now();
            let k = key.to_path_buf();
            self.engine.run(move |c| Reply::Snaps(k, c.snapshots(&id)));
        }
        Some(self.snaps.get(key).map(|(s, _)| s.clone()).unwrap_or_default())
    }

    fn load_snapshot(&mut self, snap: &Path) -> Load {
        let Some(id) = self.snap_id(snap) else {
            return self.local.load_snapshot(snap);
        };
        if let Ok(doc) = storage::load(snap) {
            return Load::Ready(Box::new(doc), Access::Edit);
        }
        let s = snap.to_path_buf();
        self.engine.run(move |c| Reply::SnapBody(s, c.snapshot_body(&id)));
        Load::Pending
    }

    fn history(&mut self, key: &Path) -> Option<Vec<EditSession>> {
        let id = self.id_of(key)?;
        let fresh = self.sessions.get(key).map(|(_, at)| at.elapsed() < Duration::from_secs(30)).unwrap_or(false);
        if !fresh && self.client.signed_in() {
            self.sessions.entry(key.to_path_buf()).or_insert_with(|| (Vec::new(), Instant::now())).1 = Instant::now();
            let k = key.to_path_buf();
            self.engine.run(move |c| Reply::Sessions(k, c.edit_sessions(&id)));
        }
        Some(self.sessions.get(key).map(|(s, _)| s.clone()).unwrap_or_default())
    }

    fn trash(&mut self) -> Option<Vec<Entry>> {
        let Source::Space(ws) = self.source.clone() else {
            return None;
        };
        let me = self.client.user_id();
        Some(self.projects.iter().filter(|p| p.workspace_id == ws && p.in_trash()).map(|p| self.entry_for(&p.id, Some(p), &me)).collect())
    }

    fn restore(&mut self, key: &Path) {
        if let Some(id) = self.id_of(key) {
            self.engine.run(move |c| Reply::Done("restored", c.restore_project(&id)));
        }
    }

    fn add_member(&mut self, email: &str, r: Role) {
        let Source::Space(ws) = self.source.clone() else {
            return;
        };
        let (email, r) = (email.to_string(), if r == Role::Viewer { nc::Role::Viewer } else { nc::Role::Editor });
        self.engine.run(move |c| Reply::Done("invited", c.invite_member(&ws, &email, r).map(|id| {
            let _ = c.send_invite(&id, APP);
        })));
    }

    fn set_role(&mut self, member: &str, r: Role) {
        let Source::Space(ws) = self.source.clone() else {
            return;
        };
        let (member, r) = (member.to_string(), match r {
            Role::Owner => nc::Role::Admin,
            Role::Editor => nc::Role::Editor,
            Role::Viewer => nc::Role::Viewer,
        });
        self.engine.run(move |c| Reply::Done("role", c.set_role(&ws, &member, r)));
    }

    fn remove_member(&mut self, member: &str) {
        let Source::Space(ws) = self.source.clone() else {
            return;
        };
        let member = member.to_string();
        self.engine.run(move |c| Reply::Done("removed", c.remove_member(&ws, &member)));
    }

    fn sign_out(&mut self) {
        let _ = self.client.sign_out(false);
        self.account_changed(false);
        self.events.push(Event::SignedOut);
    }

    fn read_settings(&mut self) -> Settings {
        self.local.read_settings()
    }

    fn write_settings(&mut self, s: &Settings) -> Result<(), String> {
        self.local.write_settings(s)
    }

    fn deliver(&mut self, name: &str, bytes: Vec<u8>, after: Option<AfterExport>) -> Result<String, String> {
        self.local.deliver(name, bytes, after)
    }

    fn start_import(&mut self) {
        self.local.start_import();
    }

    fn open_url(&mut self, url: &str) {
        self.local.open_url(url);
    }

    fn reveal(&mut self, key: Option<&Path>) {
        match key {
            Some(k) if self.id_of(k).is_some() => {}
            _ => self.local.reveal(key),
        }
    }

    fn open_exports(&mut self) {
        self.local.open_exports();
    }

    fn events(&mut self) -> Vec<Event> {
        for e in self.engine.poll() {
            self.engine_event(e);
        }
        let mut out = self.local.events();
        out.append(&mut self.events);
        out
    }

    fn busy(&self) -> bool {
        self.local.busy() || (self.open.is_some() && self.client.signed_in())
    }

    fn spaces(&self) -> Vec<Space> {
        if !self.client.signed_in() && self.index.is_empty() {
            return Vec::new();
        }
        self.spaces
            .iter()
            .map(|(w, r, _)| Space {
                id: w.id.clone(),
                name: w.shown(),
                personal: w.is_personal(),
                can_write: r.can_write(),
                scripts: if self.loaded {
                    self.projects.iter().filter(|p| p.workspace_id == w.id && !p.in_trash()).count()
                } else {
                    self.index.values().filter(|k| k.workspace == w.id).count()
                },
            })
            .collect()
    }

    fn source(&self) -> Source {
        self.source.clone()
    }

    fn set_source(&mut self, source: Source) {
        if self.source != source {
            self.source = source;
            self.rebuild_team();
            self.write_index();
            self.changed = true;
        }
    }

    fn place(&self, key: &Path) -> Option<String> {
        let id = self.id_of(key)?;
        self.index.get(&id).map(|k| k.workspace.clone())
    }

    fn move_to(&mut self, key: &Path, space: Option<&str>) -> Result<PathBuf, String> {
        match (self.id_of(key), space) {
            (None, Some(ws)) => {
                let text = std::fs::read_to_string(key).map_err(|e| e.to_string())?;
                let doc = storage::from_markdown(&text);
                let p = self.client.create_project(ws, APP, &doc.meta.title, &text, &meta_of(&doc)).map_err(|e| e.message())?;
                let p = nc::Project { body: Some(text), ..p };
                self.write_cache(&p);
                self.projects.push(nc::Project { body: None, ..p.clone() });
                let kept = self.root.join("uploaded");
                let _ = std::fs::create_dir_all(&kept);
                let name = key.file_name().map(|n| n.to_os_string()).unwrap_or_default();
                let mut backup = kept.join(&name);
                let mut n = 2;
                while backup.exists() {
                    backup = kept.join(format!("{}-{n}.md", key.file_stem().and_then(|s| s.to_str()).unwrap_or("script")));
                    n += 1;
                }
                std::fs::copy(key, &backup).map_err(|e| e.to_string())?;
                self.local.delete(key)?;
                self.changed = true;
                Ok(self.key_for(&p.id))
            }
            (Some(id), Some(ws)) => {
                self.client.move_project(&id, ws).map_err(|e| e.message())?;
                if let Some(k) = self.index.get_mut(&id) {
                    k.workspace = ws.to_string();
                }
                if let Some(p) = self.projects.iter_mut().find(|p| p.id == id) {
                    p.workspace_id = ws.to_string();
                }
                self.write_index();
                self.refresh();
                self.changed = true;
                Ok(key.to_path_buf())
            }
            (Some(id), None) => {
                let doc = storage::load(key).map_err(|e| e.to_string())?;
                let path = self.local.create(&doc)?;
                if let Err(e) = self.client.delete_project(&id) {
                    let _ = self.local.delete(&path);
                    return Err(e.message());
                }
                self.close(key);
                self.index.remove(&id);
                if let Some(p) = self.projects.iter_mut().find(|p| p.id == id) {
                    p.deleted_at = Some(nc::time_now_text());
                }
                let _ = std::fs::remove_file(key);
                self.write_index();
                self.changed = true;
                Ok(path)
            }
            (None, None) => Ok(key.to_path_buf()),
        }
    }

    fn sync(&self) -> Option<Sync> {
        if !self.client.signed_in() && self.index.is_empty() {
            return None;
        }
        let (text, mood) = self.state.readout(self.client.server_now());
        Some(Sync { text, warn: mood == nc::Mood::Warn, busy: mood == nc::Mood::Busy })
    }

    fn account_changed(&mut self, signed_in: bool) {
        if signed_in {
            self.engine.poke();
            self.refresh();
        } else {
            self.projects.clear();
            self.leases.clear();
            self.stars.clear();
            self.me = None;
            self.loaded = false;
            self.state.signed_in = false;
            if let Source::Space(_) = self.source {
                self.source = Source::Local;
            }
            self.rebuild_team();
        }
        self.changed = true;
        self.events.push(Event::LibraryChanged);
    }
}
