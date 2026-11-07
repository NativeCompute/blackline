use std::path::PathBuf;

use eframe::egui::{self, pos2, vec2, RawInput, Rect};
use ncf::cloud::fake::{self, FakeCloud};
use ncf::cloud::{self as nc, Client, Device, Engine, Heartbeat, Outbox, Worker};

use crate::app::App;
use crate::backend::{Access, Event, Load, Role, Source, Store};
use crate::cloud::CloudStore;
use crate::model::{Document, Element};
use crate::storage;
use crate::tests::VAULT;

fn sandbox(name: &str) -> std::sync::MutexGuard<'static, ()> {
    let guard = VAULT.lock().unwrap_or_else(|e| e.into_inner());
    let dir = std::env::temp_dir().join(format!("blackline-cloud-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(dir.join("blackline"));
    std::env::set_var("XDG_DATA_HOME", &dir);
    let _ = std::fs::write(dir.join("blackline/settings.conf"), "splash = no\nanimations = no\nautosave_ms = 150\nonboarded = blue-pages\n");
    guard
}

fn store_for(client: &Client, device: &str) -> CloudStore {
    let root = storage::library_root().join("cloud").join(device.replace(' ', "-"));
    let outbox = Outbox::load(root.join("outbox.json"));
    let d = Device { name: device.into(), ..Device::load_or_create(&root) };
    let engine = Engine::inline(Worker::new(client.clone(), d, outbox, Heartbeat::sync()));
    CloudStore::with_engine(client.clone(), engine, root)
}

fn drain(s: &mut CloudStore) -> Vec<Event> {
    let mut out = Vec::new();
    for _ in 0..4 {
        out.extend(s.events());
    }
    out
}

fn doc(title: &str, line: &str) -> Document {
    let mut d = Document::default();
    d.blocks.clear();
    d.meta.title = title.into();
    d.push(Element::SceneHeading, "INT. ROOM - DAY");
    d.push(Element::Action, line);
    d
}

struct Room {
    cloud: FakeCloud,
    ann: Client,
    ben: Client,
    team: String,
    personal: String,
    script: String,
}

fn room() -> Room {
    let cloud = FakeCloud::new();
    let ann = cloud.signed_in("ann@example.com", "Ann Lee");
    let ben = cloud.signed_in("ben@example.com", "Ben Ito");
    let team = ann.create_workspace("Writers room").unwrap();
    let invite = ann.invite_member(&team, "ben@example.com", nc::Role::Editor).unwrap();
    ben.accept_invite(&invite).unwrap();
    let personal = ann.me().unwrap().personal_workspace;
    let body = storage::to_markdown(&doc("Harbour", "Waves."));
    let script = ann.create_project(&team, "blackline", "Harbour", &body, &serde_json::json!({"pages": 1})).unwrap().id;
    Room { cloud, ann, ben, team, personal, script }
}

fn opened(events: &[Event]) -> Option<(PathBuf, Document, Access)> {
    events.iter().find_map(|e| match e {
        Event::Opened { key, doc, access } => Some((key.clone(), doc.clone(), access.clone())),
        _ => None,
    })
}

#[test]
fn signed_out_it_is_the_local_library() {
    let _g = sandbox("signed-out");
    let cloud = FakeCloud::new();
    let mut s = store_for(&cloud.client(), "Mac");
    assert!(s.spaces().is_empty());
    assert_eq!(s.source(), Source::Local);
    let key = s.create(&doc("Local one", "Hello.")).unwrap();
    assert!(key.starts_with(storage::scripts_dir()));
    assert_eq!(s.list().len(), 1);
    assert!(s.sync().is_none());
    assert!(s.team().is_none());
}

#[test]
fn spaces_and_team_scripts_come_down() {
    let _g = sandbox("spaces");
    let r = room();
    let mut s = store_for(&r.ann, "Ann's Mac");
    drain(&mut s);
    let spaces = s.spaces();
    assert_eq!(spaces.len(), 2);
    assert!(spaces[0].personal && spaces[0].name == "My cloud");
    assert_eq!((spaces[1].name.as_str(), spaces[1].scripts), ("Writers room", 1));
    assert!(s.list().iter().all(|e| e.cloud.is_none()));
    s.set_source(Source::Space(r.team.clone()));
    let list = s.list();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].title, "Harbour");
    assert_eq!(list[0].cloud.as_deref(), Some("Writers room"));
    assert!(list[0].path.exists(), "the cache copy is on disk");
    let team = s.team().unwrap();
    assert_eq!(team.name, "Writers room");
    assert_eq!(team.role, Role::Owner);
    assert_eq!(team.members.len(), 2);
    assert!(team.members.iter().any(|m| m.person.name == "Ben Ito" && m.role == Role::Editor));
    assert!(team.members.iter().any(|m| m.person.id == team.me.id && m.fixed));
    assert_eq!(s.location(), "NativeCloud / Writers room");
    let found = s.search("waves").unwrap();
    assert_eq!(found.len(), 1);
    s.set_source(Source::Space(r.personal.clone()));
    assert!(s.team().is_none());
    assert!(s.list().is_empty());
}

#[test]
fn open_edit_and_save_round_trip() {
    let _g = sandbox("round-trip");
    let r = room();
    let mut s = store_for(&r.ann, "Ann's Mac");
    drain(&mut s);
    let key = s.key_for(&r.script);
    assert!(matches!(s.load(&key), Load::Pending));
    let (k, d, access) = opened(&drain(&mut s)).unwrap();
    assert_eq!(k, key);
    assert_eq!(d.meta.title, "Harbour");
    assert_eq!(access, Access::Edit);
    assert_eq!(r.cloud.lease_holder(&r.script).as_deref(), Some("ann@example.com"));
    s.save(&key, &doc("Harbour", "Waves crash.")).unwrap();
    drain(&mut s);
    assert!(r.cloud.body(&r.script).contains("Waves crash."));
    s.save(&key, &doc("Harbour", "Waves crash twice.")).unwrap();
    s.save(&key, &doc("Harbour", "Waves crash three times.")).unwrap();
    drain(&mut s);
    assert_eq!(r.cloud.version(&r.script), 4);
    assert!(r.cloud.body(&r.script).contains("three times"));
    assert!(r.cloud.snapshots(&r.script).iter().all(|(k, _, _)| k != "offline"));
    s.close(&key);
    drain(&mut s);
    assert_eq!(r.cloud.lease_holder(&r.script), None);
}

#[test]
fn a_teammate_reads_while_someone_writes_then_takes_over() {
    let _g = sandbox("lock");
    let r = room();
    let mut a = store_for(&r.ann, "Ann's Mac");
    let mut b = store_for(&r.ben, "Ben's PC");
    drain(&mut a);
    drain(&mut b);
    let key_a = a.key_for(&r.script);
    a.load(&key_a);
    drain(&mut a);
    let key_b = b.key_for(&r.script);
    b.load(&key_b);
    let (_, _, access) = opened(&drain(&mut b)).unwrap();
    match access {
        Access::ReadOnly { by: Some(p) } => assert_eq!(p.name, "Ann Lee"),
        other => panic!("{other:?}"),
    }
    b.set_source(Source::Space(r.team.clone()));
    let e = b.list().into_iter().find(|e| e.path == key_b).unwrap();
    assert_eq!(e.editing.unwrap().name, "Ann Lee");
    a.save(&key_a, &doc("Harbour", "Ann was here.")).unwrap();
    drain(&mut a);
    b.engine().poke();
    let ev = drain(&mut b);
    assert!(ev.iter().any(|e| matches!(e, Event::Refreshed { doc, .. } if storage::to_markdown(doc).contains("Ann was here."))));
    a.close(&key_a);
    drain(&mut a);
    b.engine().poke();
    assert!(drain(&mut b).iter().any(|e| matches!(e, Event::LockFree { .. })));
    b.request_edit(&key_b);
    let (_, _, access) = opened(&drain(&mut b)).unwrap();
    assert_eq!(access, Access::Edit);
}

#[test]
fn losing_the_lock_reads_only() {
    let _g = sandbox("lost");
    let r = room();
    let mut a = store_for(&r.ann, "Ann's Mac");
    drain(&mut a);
    let key = a.key_for(&r.script);
    a.load(&key);
    drain(&mut a);
    r.cloud.advance(200);
    r.cloud.lock_as("ben@example.com", &r.script, "Ben's PC");
    a.save(&key, &doc("Harbour", "Ann, too late.")).unwrap();
    let ev = drain(&mut a);
    let by = ev.iter().find_map(|e| if let Event::Locked { by, .. } = e { Some(by.clone()) } else { None }).unwrap();
    assert_eq!(by.name, "Ben Ito");
    assert!(r.cloud.snapshots(&r.script).iter().any(|(_, _, b)| b.contains("Ann, too late.")));
    assert!(a.take_snapshot(&key, &doc("Harbour", "x"), Some("Your version (kept)")).is_ok());
    assert_eq!(r.cloud.snapshots(&r.script).len(), 1, "the kept copy isn't stored twice");
}

#[test]
fn offline_edits_wait_and_lose_to_a_newer_version_as_a_snapshot() {
    let _g = sandbox("offline");
    let r = room();
    let mut a = store_for(&r.ann, "Ann's Mac");
    drain(&mut a);
    let key = a.key_for(&r.script);
    a.load(&key);
    drain(&mut a);
    r.cloud.set_offline(true);
    a.save(&key, &doc("Harbour", "Written on a plane.")).unwrap();
    drain(&mut a);
    a.set_source(Source::Space(r.team.clone()));
    let e = a.list().into_iter().find(|e| e.path == key).unwrap();
    assert!(e.waiting);
    assert!(a.sync().unwrap().text.starts_with("OFFLINE"));
    assert!(storage::load(&key).unwrap().blocks.iter().any(|b| b.text == "Written on a plane."));
    r.cloud.set_offline(false);
    r.cloud.advance(200);
    r.cloud.edit_as("ben@example.com", &r.script, &storage::to_markdown(&doc("Harbour", "Ben's rewrite.")));
    a.engine().poke();
    let ev = drain(&mut a);
    let (theirs, by) = ev.iter().find_map(|e| if let Event::Conflict { theirs, by, .. } = e { Some((theirs.clone(), by.clone())) } else { None }).unwrap();
    assert!(theirs.blocks.iter().any(|b| b.text == "Ben's rewrite."));
    assert_eq!(by.name, "Ben Ito");
    assert!(r.cloud.snapshots(&r.script).iter().any(|(k, _, b)| k == "offline" && b.contains("Written on a plane.")));
    assert!(storage::load(&key).unwrap().blocks.iter().any(|b| b.text == "Ben's rewrite."));
    assert!(!a.list().into_iter().find(|e| e.path == key).unwrap().waiting);
}

#[test]
fn opening_offline_uses_this_computers_copy() {
    let _g = sandbox("offline-open");
    let r = room();
    let mut a = store_for(&r.ann, "Ann's Mac");
    drain(&mut a);
    let key = a.key_for(&r.script);
    r.cloud.set_offline(true);
    a.load(&key);
    let ev = drain(&mut a);
    let (_, d, access) = opened(&ev).unwrap();
    assert_eq!(d.meta.title, "Harbour");
    assert_eq!(access, Access::Edit);
    assert!(ev.iter().any(|e| matches!(e, Event::Notice { title, .. } if title == "You're offline")));
}

#[test]
fn viewers_read_only() {
    let _g = sandbox("viewer");
    let r = room();
    let cal = r.cloud.signed_in("cal@example.com", "Cal Diaz");
    let invite = r.ann.invite_member(&r.team, "cal@example.com", nc::Role::Viewer).unwrap();
    cal.accept_invite(&invite).unwrap();
    let mut c = store_for(&cal, "Cal's Mac");
    drain(&mut c);
    let key = c.key_for(&r.script);
    c.load(&key);
    let (_, _, access) = opened(&drain(&mut c)).unwrap();
    assert_eq!(access, Access::ReadOnly { by: None });
    assert_eq!(c.team().unwrap().role, Role::Viewer);
    assert!(!c.spaces().iter().find(|s| s.id == r.team).unwrap().can_write);
}

#[test]
fn scripts_move_between_this_computer_and_the_cloud() {
    let _g = sandbox("move");
    let r = room();
    let mut a = store_for(&r.ann, "Ann's Mac");
    drain(&mut a);
    let local = a.create(&doc("Pilot", "Morning.")).unwrap();
    let key = a.move_to(&local, Some(&r.personal)).unwrap();
    assert!(!local.exists());
    assert!(a.root().join("uploaded").join(local.file_name().unwrap()).exists(), "a copy of the original stays");
    let id = a.id_of(&key).unwrap();
    assert!(r.cloud.body(&id).contains("Morning."));
    assert_eq!(a.place(&key).as_deref(), Some(r.personal.as_str()));
    a.move_to(&key, Some(&r.team)).unwrap();
    assert_eq!(r.cloud.workspace_of(&id), r.team);
    drain(&mut a);
    let back = a.move_to(&key, None).unwrap();
    assert!(back.starts_with(storage::scripts_dir()));
    assert!(storage::load(&back).unwrap().blocks.iter().any(|b| b.text == "Morning."));
    assert!(!r.cloud.exists(&id), "it's in the cloud's Recently deleted");
    assert!(!key.exists());
}

#[test]
fn new_scripts_go_to_the_chosen_space_and_trash_comes_back() {
    let _g = sandbox("create");
    let r = room();
    let mut a = store_for(&r.ann, "Ann's Mac");
    drain(&mut a);
    a.set_source(Source::Space(r.team.clone()));
    let key = a.create(&doc("Second", "New.")).unwrap();
    let id = a.id_of(&key).unwrap();
    assert_eq!(r.cloud.workspace_of(&id), r.team);
    assert_eq!(a.list().len(), 2);
    a.set_starred(&key, true).unwrap();
    drain(&mut a);
    assert!(a.list().iter().find(|e| e.path == key).unwrap().starred);
    a.delete(&key).unwrap();
    assert_eq!(a.list().len(), 1);
    let trash = a.trash().unwrap();
    assert_eq!(trash.len(), 1);
    a.restore(&key);
    drain(&mut a);
    assert_eq!(a.list().len(), 2);
    r.cloud.set_offline(true);
    let fallback = a.create(&doc("Offline new", "x")).unwrap();
    assert!(fallback.starts_with(storage::scripts_dir()));
    assert_eq!(a.source(), Source::Local);
}

#[test]
fn snapshots_and_history_come_from_the_cloud() {
    let _g = sandbox("snaps");
    let r = room();
    let mut a = store_for(&r.ann, "Ann's Mac");
    drain(&mut a);
    let key = a.key_for(&r.script);
    a.load(&key);
    drain(&mut a);
    a.take_snapshot(&key, &doc("Harbour", "Draft one."), Some("Before notes")).unwrap();
    drain(&mut a);
    a.snapshots(&key);
    drain(&mut a);
    let snaps = a.snapshots(&key).unwrap();
    assert_eq!(snaps.len(), 1);
    assert_eq!(snaps[0].label.as_deref(), Some("Before notes"));
    assert!(matches!(a.load_snapshot(&snaps[0].path), Load::Pending));
    let ev = drain(&mut a);
    assert!(ev.iter().any(|e| matches!(e, Event::SnapshotDoc { doc, .. } if doc.blocks.iter().any(|b| b.text == "Draft one."))));
    assert!(matches!(a.load_snapshot(&snaps[0].path), Load::Ready(..)));
    assert!(a.history(&key).is_some());
}

#[test]
fn a_restart_keeps_the_library_and_the_waiting_saves() {
    let _g = sandbox("restart");
    let r = room();
    let key;
    {
        let mut a = store_for(&r.ann, "Ann's Mac");
        drain(&mut a);
        key = a.key_for(&r.script);
        a.load(&key);
        drain(&mut a);
        r.cloud.set_offline(true);
        a.save(&key, &doc("Harbour", "Saved before the battery died.")).unwrap();
        drain(&mut a);
        a.set_source(Source::Space(r.team.clone()));
    }
    let mut again = store_for(&r.ann, "Ann's Mac");
    assert_eq!(again.source(), Source::Space(r.team.clone()));
    assert_eq!(again.list().len(), 1, "listed from this computer's copy while offline");
    assert_eq!(again.spaces().len(), 2);
    r.cloud.set_offline(false);
    again.engine().poke();
    drain(&mut again);
    assert!(r.cloud.body(&r.script).contains("Saved before the battery died."));
}

#[test]
fn signing_out_empties_the_cloud_lists() {
    let _g = sandbox("sign-out");
    let r = room();
    let mut a = store_for(&r.ann, "Ann's Mac");
    drain(&mut a);
    a.set_source(Source::Space(r.team.clone()));
    a.sign_out();
    let ev = drain(&mut a);
    assert!(ev.iter().any(|e| matches!(e, Event::SignedOut)));
    assert_eq!(a.source(), Source::Local);
    assert!(a.team().is_none());
}

// app here---

fn run(app: &mut App, ctx: &egui::Context, frames: usize) {
    for _ in 0..frames {
        let input = RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1320.0, 900.0))), ..Default::default() };
        let _ = ctx.run(input, |c| app.frame(c));
    }
}

#[test]
fn the_app_shows_spaces_accounts_and_the_first_sync_picker() {
    let _g = sandbox("app");
    let r = room();
    let cloud = r.cloud.clone();
    let ctx = egui::Context::default();
    let _ = ctx.run(RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1320.0, 900.0))), ..Default::default() }, |_| {});
    let mut local = crate::backend::LocalBackend::new();
    local.create(&doc("Already here", "Old.")).unwrap();
    let anon = cloud.client();
    let store = store_for(&anon, "Ann's Mac");
    let mut app = App::with_store(&ctx, Box::new(store));
    app.attach_account(fake::account_for(crate::INFO, anon.clone(), "Ann's Mac"));
    run(&mut app, &ctx, 3);
    assert!(!app.debug_signed_in());
    for tab in [9, 10] {
        app.debug_open_settings(tab);
        run(&mut app, &ctx, 2);
    }
    let a = app.account().unwrap();
    a.fill("ann@example.com", "Password123", "");
    a.submit();
    run(&mut app, &ctx, 6);
    assert!(app.debug_signed_in());
    assert!(app.debug_picker_open(), "the first-sync picker asks which scripts to move");
    app.debug_pick_all();
    run(&mut app, &ctx, 3);
    assert!(!app.debug_picker_open());
    let personal = app.debug_spaces().into_iter().find(|s| s.personal).unwrap();
    assert_eq!(personal.scripts, 1);
    for tab in [9, 10] {
        app.debug_open_settings(tab);
        run(&mut app, &ctx, 2);
    }
    app.debug_set_source(Source::Space(r.team.clone()));
    run(&mut app, &ctx, 3);
    assert_eq!(app.debug_entries().len(), 1);
}
