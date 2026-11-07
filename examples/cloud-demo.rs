use blackline::app::App;
use blackline::backend::{Source, Store};
use blackline::cloud::CloudStore;
use blackline::model::{Document, Element};
use blackline::storage;
use ncf::cloud::fake::{self, FakeCloud};
use ncf::cloud::{self as nc, Device, Engine, Heartbeat, Outbox, Worker};

fn script(title: &str, author: &str, scenes: &[(&str, &str, &str, &str)]) -> String {
    let mut d = Document::default();
    d.blocks.clear();
    d.meta.title = title.into();
    d.meta.author = author.into();
    for (heading, action, who, line) in scenes {
        d.push(Element::SceneHeading, heading);
        d.push(Element::Action, action);
        d.push(Element::Character, who);
        d.push(Element::Dialogue, line);
    }
    storage::to_markdown(&d)
}

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let home = std::env::temp_dir().join(format!("blackline-cloud-demo-{}", std::process::id()));
    let _ = std::fs::create_dir_all(home.join("blackline"));
    std::env::set_var("XDG_DATA_HOME", &home);
    let light = args.iter().any(|a| a == "--light");
    let _ = std::fs::write(
        home.join("blackline/settings.conf"),
        format!("splash = no\nonboarded = blue-pages\ncloud_asked = yes\nlight_mode = {}\n", if light { "yes" } else { "no" }),
    );
    let pick = |flag: &str| args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned().unwrap_or_default();
    let (view, settings, open) = (pick("--home"), pick("--settings"), pick("--open"));

    let cloud = FakeCloud::new();
    let ann = cloud.signed_in("ann@example.com", "Ann Lee");
    let ben = cloud.signed_in("ben@example.com", "Ben Ito");
    let cal = cloud.signed_in("cal@example.com", "Cal Diaz");
    let room = ann.create_workspace("Writers room").unwrap();
    for (who, c, role) in [("ben@example.com", &ben, nc::Role::Editor), ("cal@example.com", &cal, nc::Role::Viewer)] {
        let i = ann.invite_member(&room, who, role).unwrap();
        c.accept_invite(&i).unwrap();
    }
    let personal = ann.me().unwrap().personal_workspace;
    let harbour = script("Harbour", "Ann Lee & Ben Ito", &[
        ("EXT. HARBOUR - NIGHT", "Rain hammers the pier. A lone lamp swings.", "MARA", "They said the boat would wait."),
        ("INT. CUSTOMS HOUSE - CONTINUOUS", "Ledgers everywhere. A kettle screams.", "OSKAR", "Boats don't wait. People do."),
    ]);
    let h = ann.create_project(&room, "blackline", "Harbour", &harbour, &serde_json::json!({"pages": 2, "scenes": 2})).unwrap();
    let ledger = script("The Ledger", "Ben Ito", &[("INT. BANK VAULT - DAY", "Dust in the light.", "IRIS", "Count it again.")]);
    let l = ben.create_project(&room, "blackline", "The Ledger", &ledger, &serde_json::json!({"pages": 1, "scenes": 1})).unwrap();
    ben.save_project(&l.id, 1, &ledger.replace("Count it again.", "Count it again. Slowly."), None, None, None).unwrap();
    let night = script("Night Shift", "Ann Lee", &[("INT. DINER - 3 AM", "Neon hum. One customer.", "JO", "Coffee's on the house tonight.")]);
    ann.create_project(&personal, "blackline", "Night Shift", &night, &serde_json::json!({"pages": 1, "scenes": 1})).unwrap();
    if open == "locked" {
        cloud.lock_as("ben@example.com", &h.id, "Ben's PC");
    }

    let root = storage::library_root().join("cloud");
    let device = Device { name: "Ann's Mac".into(), ..Device::load_or_create(&root) };
    let worker = Worker::new(ann.clone(), device, Outbox::load(root.join("outbox.json")), Heartbeat::sync());
    let mut store = CloudStore::with_engine(ann.clone(), Engine::inline(worker), root);
    for _ in 0..4 {
        let _ = store.events();
    }
    let local = blackline::backend::LocalBackend::new();
    drop(local);
    let _ = storage::save(&storage::scripts_dir().join("old-draft.md"), &storage::from_markdown(&script("Old Draft", "Ann Lee", &[("INT. ATTIC - DAY", "Boxes.", "ANN", "Found it.")])));
    match view.as_str() {
        "team" => store.set_source(Source::Space(room.clone())),
        "personal" => store.set_source(Source::Space(personal.clone())),
        _ => {}
    }
    let key = store.key_for(&h.id);

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default().with_title("Blackline · NativeCloud demo").with_inner_size([1320.0, 860.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Blackline",
        options,
        Box::new(move |cc| {
            let mut app = App::with_store(&cc.egui_ctx, Box::new(store));
            app.attach_account(fake::account_for(blackline::INFO, ann, "Ann's Mac"));
            if let Some(a) = app.account() {
                a.sync_settings = false;
            }
            match settings.as_str() {
                "account" => app.debug_open_settings(9),
                "teams" => {
                    if let Some(a) = app.account() {
                        a.select_team(Some(&room));
                    }
                    app.debug_open_settings(10);
                }
                _ => {}
            }
            if !open.is_empty() {
                app.debug_open_from_home(key);
            }
            let _ = &cloud;
            Ok(Box::new(app))
        }),
    )
}
