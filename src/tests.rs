use crate::export;
use crate::model::{guess_element, wrap, Document, Element};
use crate::storage;

fn sample() -> Document {
    let mut d = Document::default();
    d.blocks.clear();
    d.meta.title = "The Long Way Down".into();
    d.meta.author = "A. Writer".into();
    d.meta.draft = "First Draft".into();
    d.push(Element::SceneHeading, "INT. WAREHOUSE - NIGHT");
    d.push(Element::Action, "Rain hammers the corrugated roof. MARIA moves between the crates, one hand on the wall, counting doors under her breath until she reaches the one that isn't locked.");
    d.push(Element::Character, "MARIA (V.O.)");
    d.push(Element::Parenthetical, "(barely audible)");
    d.push(Element::Dialogue, "Three, four... there you are.");
    d.push(Element::Shot, "ANGLE ON THE DOOR");
    d.push(Element::Transition, "SMASH CUT TO:");
    d.reseed_ids();
    d
}

#[test]
fn markdown_round_trips() {
    let mut a = sample();
    a.normalize();
    let md = storage::to_markdown(&a);
    println!("--- markdown ---\n{md}");
    let b = storage::from_markdown(&md);
    assert_eq!(a.blocks.len(), b.blocks.len(), "block count");
    for (x, y) in a.blocks.iter().zip(b.blocks.iter()) {
        assert_eq!(x.element, y.element, "element for {:?}", x.text);
        assert_eq!(x.text, y.text, "text");
    }
    assert_eq!(a.meta.title, b.meta.title);
    assert_eq!(a.meta.author, b.meta.author);
    assert_eq!(a.meta.draft, b.meta.draft);
}

#[test]
fn wrapping_respects_columns() {
    let long = "Rain hammers the corrugated roof and nobody in this building has slept for two days straight.";
    for l in wrap(long, 35) {
        assert!(l.chars().count() <= 35, "line too wide: {l:?}");
    }
}

#[test]
fn layout_matches_industry_indents() {
    let d = sample();
    let lines = export::compose(&d);
    let find = |e: Element| lines.iter().find(|l| l.element == Some(e)).unwrap().indent;
    assert_eq!(find(Element::SceneHeading), 0);
    assert_eq!(find(Element::Action), 0);
    assert_eq!(find(Element::Character), 22);
    assert_eq!(find(Element::Parenthetical), 16);
    assert_eq!(find(Element::Dialogue), 10);
    assert_eq!(find(Element::Transition), 45);
    let txt = export::to_plain_text(&d);
    println!("--- plain text ---\n{txt}");
    assert!(txt.contains("                      MARIA (V.O.)"));
}

#[test]
fn pagination_is_sane() {
    let mut d = sample();
    for _ in 0..300 {
        d.push(Element::Action, "They run.");
    }
    d.reseed_ids();
    let pages = export::paginate(&export::compose(&d));
    assert!(pages.len() > 5, "expected multiple pages, got {}", pages.len());
    for p in &pages {
        assert!(p.len() <= 60, "page overflow: {}", p.len());
    }
}

#[test]
fn pdf_is_written() {
    let d = sample();
    let path = std::env::temp_dir().join(format!("ns-test-{}.pdf", std::process::id()));
    export::to_pdf(&d, &path).expect("pdf export");
    let bytes = std::fs::read(&path).unwrap();
    assert!(bytes.starts_with(b"%PDF"), "not a pdf");
    assert!(bytes.len() > 1000, "pdf too small: {}", bytes.len());
    println!("pdf bytes: {}", bytes.len());
}

#[test]
fn paste_guessing() {
    assert_eq!(guess_element("INT. CAR - DAY", None), Element::SceneHeading);
    assert_eq!(guess_element("(beat)", None), Element::Parenthetical);
    assert_eq!(guess_element("CUT TO:", None), Element::Transition);
    assert_eq!(guess_element("MARIA", None), Element::Character);
    assert_eq!(guess_element("Not now.", Some(Element::Character)), Element::Dialogue);
    assert_eq!(guess_element("She runs for the door.", Some(Element::Action)), Element::Action);
}

#[test]
fn slugs_are_filenames() {
    assert_eq!(storage::slugify("The Long Way Down"), "the-long-way-down");
    assert_eq!(storage::slugify("  Episode 2: Fallout!  "), "episode-2-fallout");
    assert_eq!(storage::slugify("///"), "untitled");
}

// new here---

use crate::fountain;
use crate::model::{base_character, complete, eighths_label};
use crate::settings::{AfterExport, Settings, SortBy};

pub(crate) static VAULT: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub(crate) fn sandbox(name: &str) -> std::sync::MutexGuard<'static, ()> {
    let guard = VAULT.lock().unwrap_or_else(|e| e.into_inner());
    let dir = std::env::temp_dir().join(format!("blackline-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    std::env::set_var("XDG_DATA_HOME", &dir);
    guard
}

fn three_scenes() -> Document {
    let mut d = Document::default();
    d.blocks.clear();
    d.meta.title = "Three".into();
    d.push(Element::Action, "FADE IN on nothing in particular.");
    d.push(Element::SceneHeading, "INT. ONE - DAY");
    d.push(Element::Character, "MARIA (V.O.)");
    d.push(Element::Dialogue, "One.");
    d.push(Element::SceneHeading, "EXT. TWO - NIGHT");
    d.push(Element::Character, "COLE");
    d.push(Element::Parenthetical, "(beat)");
    d.push(Element::Dialogue, "Two, and then some.");
    d.push(Element::Character, "MARIA (CONT'D)");
    d.push(Element::Dialogue, "Two again.");
    d.push(Element::SceneHeading, "INT. THREE - DAWN");
    d.push(Element::Action, "Three.");
    d.reseed_ids();
    d
}

#[test]
fn a_scene_card_round_trips_through_the_file() {
    let mut a = three_scenes();
    a.blocks[1].note = "Maria speaks first.".into();
    a.blocks[1].tint = Some(3);
    a.blocks[4].note = "Cole answers --> twice".into();
    a.meta.starred = true;
    a.normalize();
    let md = storage::to_markdown(&a);
    assert!(md.contains("<!-- scene: tint=3 | Maria speaks first. -->"), "{md}");
    assert!(md.contains("starred: yes"));
    let b = storage::from_markdown(&md);
    assert_eq!(b.blocks.len(), a.blocks.len());
    assert_eq!(b.blocks[1].note, "Maria speaks first.");
    assert_eq!(b.blocks[1].tint, Some(3));
    assert_eq!(b.blocks[4].note, "Cole answers - -> twice", "a comment must not be able to close itself");
    assert_eq!(b.blocks[4].tint, None);
    assert!(b.meta.starred);
    assert!(b.blocks.iter().all(|x| !x.text.contains("<!--")));
}

#[test]
fn files_from_the_first_blackline_still_open_unchanged() {
    let old = "---\ntitle: Old\nauthor: Me\ncontact: \ndraft: One\n---\n\n## INT. ROOM - DAY\n\nA room.\n\n**BOB**\n\n> Hi.\n\n`CUT TO:`\n\n";
    let d = storage::from_markdown(old);
    assert_eq!(d.blocks.len(), 5);
    assert!(!d.meta.starred);
    assert_eq!(storage::to_markdown(&d), old);
    let d = storage::from_markdown("<!-- a note to self -->\n\n## INT. X - DAY\n");
    assert_eq!(d.blocks[0].element, Element::Action);
    assert_eq!(d.blocks[1].element, Element::SceneHeading);
}

const A_REAL_LIBRARY: [(&str, &str); 3] = [
    (
        "the-long-way-down.md",
        "---\ntitle: The Long Way Down\nauthor: A. Writer\ncontact: a.writer@example.com · 555-0100\ndraft: Second Draft — 3 March\n---\n\n## INT. WAREHOUSE - NIGHT\n\nRain hammers the corrugated roof. MARIA moves between the crates, counting doors under her breath — “four, five” — until one isn’t locked.\n\n**MARIA (V.O.)**\n\n*(barely audible)*\n\n> Three, four... there you are.\n\n### ANGLE ON THE DOOR\n\nIt swings inward on its own. Café light spills across the floor.\n\n`SMASH CUT TO:`\n\n## EXT. ROOFTOP - CONTINUOUS\n\n**COLE**\n\n> You came back.\n\n**MARIA (CONT'D)**\n\n> I never left.\n\n`FADE OUT.`\n\n",
    ),
    (
        "untitled-script.md",
        "---\ntitle: Untitled Script\nauthor: \ncontact: \ndraft: \n---\n\n## INT. KITCHEN - DAY\n\nNothing yet.\n\n",
    ),
    (
        "pilot.md",
        "---\ntitle: Pilot\nauthor: Sam\ncontact: \ndraft: First Draft\nstarred: yes\n---\n\n## INT. OFFICE - MORNING\n<!-- scene: tint=2 | Sam meets the team. -->\n\nPhones ring.\n\n**SAM**\n\n> Morning.\n\n",
    ),
];

#[test]
fn an_existing_library_is_never_rewritten_by_the_app() {
    let _g = sandbox("golden");
    storage::ensure_dirs().unwrap();
    let dir = storage::scripts_dir();
    for (name, text) in A_REAL_LIBRARY {
        std::fs::write(dir.join(name), text).unwrap();
    }
    let listing = || {
        let mut names: Vec<String> = std::fs::read_dir(storage::scripts_dir())
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    };
    let before = listing();

    let ctx = eframe::egui::Context::default();
    let mut app = crate::app::App::with_context(&ctx);
    assert_eq!(listing(), before, "starting up added or removed a file");
    for (name, text) in A_REAL_LIBRARY {
        assert_eq!(std::fs::read_to_string(dir.join(name)).unwrap(), text, "{name} changed on start-up");
    }

    for (name, text) in A_REAL_LIBRARY {
        let path = dir.join(name);
        app.debug_open(path.clone());
        assert_eq!(app.path().as_deref(), Some(path.as_path()));
        app.debug_save();
        assert_eq!(app.path().as_deref(), Some(path.as_path()), "{name} was renamed");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), text, "{name} changed on save");
    }
    assert_eq!(listing(), before, "saving added or removed a file");
}

#[test]
fn scenes_and_cast_are_counted_the_way_a_schedule_would() {
    let d = three_scenes();
    let sc = d.scenes();
    assert_eq!(sc.len(), 3);
    assert_eq!(sc[0].number, 1);
    assert_eq!(sc[0].start, 1, "the action before the first heading is not a scene");
    assert_eq!(sc[1].cast, vec!["COLE".to_string(), "MARIA".to_string()]);
    let cast = d.cast();
    assert_eq!(cast[0].name, "MARIA", "extensions are not separate people");
    assert_eq!(cast[0].cues, 2);
    assert_eq!(cast[0].scenes, 2);
    assert_eq!(cast[1].name, "COLE");
    assert_eq!(cast[1].words, 4);
    assert_eq!(base_character("  maria (V.O.) ^"), "MARIA");
    assert_eq!(d.scene_of(d.blocks[7].id), Some(1));
    assert_eq!(d.scene_of(d.blocks[0].id), None);
}

#[test]
fn a_scene_moves_whole() {
    let mut d = three_scenes();
    assert!(d.move_scene(0, 3), "first scene to the end");
    let order: Vec<String> = d.scenes().iter().map(|s| s.heading.clone()).collect();
    assert_eq!(order, vec!["EXT. TWO - NIGHT", "INT. THREE - DAWN", "INT. ONE - DAY"]);
    assert_eq!(d.blocks.last().unwrap().text, "One.", "its dialogue came with it");
    assert!(d.move_scene(2, 0), "and back to the front");
    assert_eq!(d.scenes()[0].heading, "INT. ONE - DAY");
    assert_eq!(d.blocks[0].element, Element::Action, "the prologue stays put");
    assert!(!d.move_scene(1, 1) && !d.move_scene(1, 2), "a move onto itself is no move");
    assert!(d.remove_scene(1));
    assert_eq!(d.scenes().len(), 2);
    assert_eq!(d.cast().iter().find(|c| c.name == "COLE"), None);
}

#[test]
fn page_breaks_and_scene_lengths_come_from_the_printed_layout() {
    let mut d = sample();
    for i in 0..40 {
        d.push(Element::SceneHeading, &format!("INT. ROOM {i} - DAY"));
        for _ in 0..3 {
            d.push(Element::Action, "They run, and they keep running until the corridor ends in a door.");
        }
    }
    d.reseed_ids();
    let pages = export::page_count(&d);
    let starts = export::page_starts(&d);
    assert_eq!(starts.len(), pages - 1, "every page after the first starts somewhere");
    assert_eq!(starts[0].0, 2);
    let lens = export::scene_lengths(&d);
    assert_eq!(lens.len(), d.scenes().len());
    assert!(lens.iter().all(|(_, pg, e)| *pg >= 1 && *e >= 1));
    let total: usize = lens.iter().map(|x| x.2).sum();
    assert!((total as i64 - (pages * 8) as i64).abs() <= (pages * 8) as i64 / 3, "{total} eighths for {pages} pages");
    assert_eq!(eighths_label(11), "1 3/8");
    assert_eq!(eighths_label(3), "3/8");
    assert_eq!(eighths_label(16), "2");
}

#[test]
fn smart_type_finishes_names_places_and_transitions() {
    let mut d = three_scenes();
    let own = d.new_block(Element::Character, "");
    let own_id = own.id;
    d.blocks.push(own);
    assert_eq!(complete(&d, own_id, Element::Character, "MA").as_deref(), Some("RIA"));
    assert_eq!(complete(&d, own_id, Element::Character, "co").as_deref(), Some("LE"));
    assert_eq!(complete(&d, own_id, Element::Character, "MARIA"), None, "nothing left to add");
    assert_eq!(complete(&d, own_id, Element::Character, "ZED"), None);
    assert_eq!(complete(&d, own_id, Element::SceneHeading, "e").as_deref(), Some("XT. "));
    assert_eq!(
        complete(&d, own_id, Element::SceneHeading, "EXT. T").as_deref(),
        Some("WO - "),
        "a place already used, ready for its time of day"
    );
    assert_eq!(complete(&d, own_id, Element::SceneHeading, "INT. NEW - NI").as_deref(), Some("GHT"));
    assert_eq!(complete(&d, own_id, Element::Transition, "SMA").as_deref(), Some("SH CUT TO:"));
    assert_eq!(complete(&d, own_id, Element::Action, "MA"), None, "action is left alone");
}

#[test]
fn find_and_replace_keep_each_element_s_capitals() {
    let mut d = three_scenes();
    assert_eq!(d.find("two").len(), 3);
    let n = d.replace_all("two", "four");
    assert_eq!(n, 3);
    assert_eq!(d.blocks[4].text, "EXT. FOUR - NIGHT", "a heading stays in capitals");
    assert_eq!(d.blocks[7].text, "four, and then some.");
    assert_eq!(d.replace_all("", "x"), 0);
    let (s, n) = crate::model::replace_folded("Ab ab AB", "ab", "c");
    assert_eq!((s.as_str(), n), ("c c c", 3));
}

#[test]
fn fountain_is_read_into_typed_blocks() {
    let src = "Title: **The Long Way Down**\nCredit: written by\nAuthor: A. Writer\nDraft date: 1 May\n\nINT. WAREHOUSE - NIGHT #1#\n\n= Maria finds the door.\n\nRain hammers the roof. [[a note]]\n\nMARIA (V.O.)\n(barely audible)\nThree, four...\nthere you are.\n\n@McCLANE\nYippee.\n\n/* cut this\nwhole bit */\n\nCUT TO:\n\n.FLASHBACK\n\n> THE END <\n\n!SHOUTING IS ACTION\n\n> BURN TO WHITE\n";
    let d = fountain::parse(src);
    assert_eq!(d.meta.title, "The Long Way Down");
    assert_eq!(d.meta.author, "A. Writer");
    assert_eq!(d.meta.draft, "1 May");
    let kinds: Vec<Element> = d.blocks.iter().map(|b| b.element).collect();
    use Element::*;
    assert_eq!(
        kinds,
        vec![SceneHeading, Action, Character, Parenthetical, Dialogue, Character, Dialogue, Transition, SceneHeading, Action, Action, Transition]
    );
    assert_eq!(d.blocks[0].text, "INT. WAREHOUSE - NIGHT", "the scene number is dropped");
    assert_eq!(d.blocks[0].note, "Maria finds the door.");
    assert_eq!(d.blocks[1].text, "Rain hammers the roof.", "notes are dropped");
    assert_eq!(d.blocks[4].text, "Three, four... there you are.", "one speech, one block");
    assert_eq!(d.blocks[5].text, "MCCLANE");
    assert_eq!(d.blocks[9].text, "THE END");
    assert!(d.blocks.iter().all(|b| !b.text.contains("cut this")), "the boneyard is dropped");
}

#[test]
fn final_draft_round_trips() {
    let mut a = three_scenes();
    a.meta.author = "A. Writer & Co".into();
    a.blocks[1].note = "Maria <speaks> first.".into();
    a.push(Element::Shot, "ANGLE ON THE DOOR");
    a.push(Element::Transition, "SMASH CUT TO:");
    a.normalize();
    let x = export::to_fdx(&a);
    assert!(x.starts_with("<?xml"));
    assert!(x.contains("<Paragraph Type=\"Scene Heading\">"));
    assert!(x.contains("Maria &lt;speaks&gt; first."));
    let b = fountain::parse_fdx(&x);
    assert_eq!(b.meta.title, "Three");
    assert_eq!(b.meta.author, "A. Writer & Co");
    assert_eq!(b.blocks.len(), a.blocks.len());
    for (p, q) in a.blocks.iter().zip(b.blocks.iter()) {
        assert_eq!(p.element, q.element, "{}", p.text);
        assert_eq!(p.text, q.text);
    }
    assert_eq!(b.blocks[1].note, "Maria <speaks> first.");
}

#[test]
fn fountain_export_carries_the_synopsis_and_reads_back() {
    let mut a = three_scenes();
    a.blocks[1].note = "First.".into();
    let f = export::to_fountain(&a);
    assert!(f.contains("= First."));
    let b = fountain::parse(&f);
    assert_eq!(b.scenes().len(), 3);
    assert_eq!(b.scenes()[0].synopsis, "First.");
    assert_eq!(b.cast()[0].name, "MARIA");
}

#[test]
fn the_pdf_can_carry_scene_numbers() {
    let d = three_scenes();
    let path = std::env::temp_dir().join(format!("ns-numbers-{}.pdf", std::process::id()));
    export::to_pdf_with(&d, &path, true).expect("pdf");
    let bytes = std::fs::read(&path).unwrap();
    assert!(bytes.starts_with(b"%PDF"));
    let lines = export::compose(&d);
    let numbered: Vec<usize> = lines.iter().filter_map(|l| l.scene).collect();
    assert_eq!(numbered, vec![1, 2, 3]);
}

#[test]
fn settings_round_trip_and_old_look_keys_are_ignored() {
    let mut s = Settings::default();
    assert!(!s.light_mode, "dark by default");
    s.light_mode = true;
    s.sort_by = SortBy::Length;
    s.after_export = AfterExport::Reveal;
    s.session_goal = 750;
    s.page_px = 19.0;
    s.scene_numbers = true;
    s.show_details = true;
    s.element_colors = false;
    s.character_colors = true;
    s.character_colors_reading = true;
    s.colour_seed = 7;
    s.youtrack_url = "https://example.youtrack.cloud/issues?q=for:me&sort=updated".into();
    assert_eq!(Settings::parse(&s.serialize()), s, "a link full of = and & survives");
    let d = Settings::default();
    assert_eq!(d.youtrack_url, "", "no tracker until you set one");
    assert!(d.element_colors && !d.character_colors && !d.show_details);
    assert_eq!(Settings::parse("youtrack_url =\n").youtrack_url, "", "an empty link stays empty");
    assert_eq!(
        Settings::parse("youtrack_url = https://team.youtrack.cloud\n").youtrack_url,
        "https://team.youtrack.cloud",
        "a link already set is kept"
    );

    let old = "theme = frostbite\nlight_mode = yes\nanimations = no\nblur = always\nglass_opacity = 0.80\nmatch_tesseract = yes\npage_px = 18\n";
    let o = Settings::parse(old);
    assert!(o.light_mode && !o.animations);
    assert_eq!(o.page_px, 18.0);
    let again = o.serialize();
    for gone in ["theme", "blur", "glass_opacity", "match_tesseract"] {
        assert!(!again.lines().any(|l| l.starts_with(&format!("{gone} ="))), "{gone} is not written back");
    }
    let g = Settings::parse("page_px = 900\nglass_opacity = -3\nsession_goal = x\n=\n");
    assert_eq!(g.page_px, 26.0);
    assert_eq!(g.session_goal, 0);
}

#[test]
fn snapshots_are_kept_per_script_and_follow_a_rename() {
    let _g = sandbox("snapshots");
    storage::ensure_dirs().unwrap();
    let d = three_scenes();
    let path = storage::unique_path("Three", None);
    storage::save(&path, &d).unwrap();
    storage::take_snapshot(&path, &d).unwrap();
    storage::take_snapshot(&path, &d).unwrap();
    let snaps = storage::list_snapshots(&path);
    assert_eq!(snaps.len(), 2, "two taken in the same second are both kept");
    assert!(snaps[0].pages >= 1);
    let back = storage::load(&snaps[0].path).unwrap();
    assert_eq!(back.scenes().len(), 3);
    let moved = storage::unique_path("Four", None);
    storage::follow_rename(&path, &moved);
    assert_eq!(storage::list_snapshots(&moved).len(), 2);
    assert!(storage::list_snapshots(&path).is_empty());
}

#[test]
fn imports_land_as_ordinary_scripts() {
    let _g = sandbox("import");
    let dir = std::env::temp_dir().join(format!("ns-import-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("my-heist_film.fountain");
    std::fs::write(&f, "EXT. BANK - DAY\n\nNobody moves.\n\nROSA\nNow.\n").unwrap();
    let d = storage::import_file(&f).unwrap();
    assert_eq!(d.meta.title, "my heist film", "named after the file when it has no title page");
    assert_eq!(d.scenes().len(), 1);
    assert_eq!(d.cast()[0].name, "ROSA");
    let x = dir.join("x.fdx");
    std::fs::write(&x, export::to_fdx(&three_scenes())).unwrap();
    assert_eq!(storage::import_file(&x).unwrap().scenes().len(), 3);
    assert!(storage::import_file(&dir.join("missing.fountain")).is_err());
}

#[test]
fn the_library_lists_pages_scenes_and_stars() {
    let _g = sandbox("library");
    storage::ensure_dirs().unwrap();
    let mut d = three_scenes();
    d.meta.starred = true;
    storage::save(&storage::unique_path("Three", None), &d).unwrap();
    let e = storage::list_scripts();
    assert_eq!(e.len(), 1);
    assert!(e[0].starred);
    assert_eq!(e[0].scenes, 3);
    assert_eq!(e[0].pages, 1);
    assert_eq!(e[0].preview, "FADE IN on nothing in particular.");
}

#[test]
fn odd_input_never_panics() {
    for s in [
        "", "---", "---\n", "##", "**", "``", "*()*", ">", "<!--", "<!-- scene: -->", "<!-- scene: tint=x -->",
        "## \u{1F3AC} ÉTÉ - JOUR\n<!-- scene: tint=99 | ü -->", "\u{0}\u{FFFF}", "#\n#\n#",
    ] {
        let d = storage::from_markdown(s);
        let _ = export::compose(&d);
        let _ = export::scene_lengths(&d);
        let _ = storage::to_markdown(&d);
        let _ = fountain::parse(s);
        let _ = fountain::parse_fdx(s);
        let _ = d.cast();
        let _ = complete(&d, 0, Element::SceneHeading, s);
    }
    let d = storage::from_markdown("## A\n<!-- scene: tint=99 | x -->\n");
    assert_eq!(d.blocks[0].tint, Some(99));
    let _ = crate::theme::palette(true).group(99);
}

#[test]
fn the_palette_is_complete_in_both_casts() {
    for dark in [true, false] {
        let p = crate::theme::palette(dark);
        assert_ne!(p.text, p.solid);
        assert_eq!(p.dark, dark);
    }
}

#[test]
fn the_icon_is_an_svg_drawn_from_the_mark() {
    let s = crate::logo::svg();
    assert!(s.starts_with("<svg"));
    assert_eq!(s.matches("<polygon").count(), 2, "the slab and the line");
    assert!(s.contains("#3B4CFF"), "the line is the signal blue");
}

#[test]
fn a_library_from_before_the_rename_stays_where_it_is() {
    let home = std::env::temp_dir().join(format!("blackline-rename-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    std::fs::create_dir_all(&home).unwrap();
    let fresh = if cfg!(windows) { "Blackline" } else { "blackline" };
    assert_eq!(storage::library_in(&home), home.join(fresh));
    std::fs::create_dir_all(home.join("northstar/scripts")).unwrap();
    assert_eq!(storage::library_in(&home), home.join("northstar"));
    assert!(!home.join(fresh).exists());
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn every_speaker_gets_a_colour_of_their_own_that_does_not_move() {
    crate::theme::set_palette(true);
    let names: Vec<String> = ["MARIA", "COLE", "JONAH", "ADA", "THE WRITER", "RUTH", "KAI"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let a = crate::theme::character_colors(&names, 0);
    let distinct: std::collections::HashSet<_> = a.values().map(|c| c.to_array()).collect();
    assert_eq!(distinct.len(), names.len(), "no two speakers share a colour");
    let mut more = names.clone();
    more.push("NEWCOMER".into());
    let b = crate::theme::character_colors(&more, 0);
    for n in &names {
        assert_eq!(a[n], b[n], "{n} kept their colour");
    }
    let c = crate::theme::character_colors(&names, 1);
    assert_ne!(a["MARIA"], c["MARIA"]);
    for dark in [true, false] {
        crate::theme::set_palette(dark);
        for col in crate::theme::character_colors(&names, 3).values() {
            let l = 0.299 * col.r() as f32 + 0.587 * col.g() as f32 + 0.114 * col.b() as f32;
            if dark {
                assert!(l > 120.0, "light enough on a dark page: {col:?}");
            } else {
                assert!(l < 150.0, "dark enough on a white page: {col:?}");
            }
        }
    }
    assert_eq!(three_scenes().speakers(), vec!["MARIA".to_string(), "COLE".to_string()]);
}

#[test]
fn the_palette_is_nativecomputes() {
    let p = crate::theme::palette(true);
    assert_eq!(p.backdrop, eframe::egui::Color32::BLACK);
    assert_eq!(p.text, eframe::egui::Color32::from_rgb(0xF2, 0xF3, 0xF7));
    assert_eq!(p.primary, eframe::egui::Color32::from_rgb(0x3B, 0x4C, 0xFF));
    assert_eq!(p.primary_light, eframe::egui::Color32::from_rgb(0x6E, 0x7B, 0xFF));
    assert_eq!(crate::theme::palette(false).primary, p.primary);
}

const _: () = assert!(crate::theme::R_PILL <= 2.0 && crate::theme::R_ISLAND <= 2.0);

#[test]
fn a_tint_is_exactly_as_strong_as_asked() {
    use eframe::egui::Color32;
    let c = Color32::from_rgb(200, 100, 50);
    let t = crate::theme::tint(c, 0.05);
    assert_eq!(t.a(), 13);
    assert_eq!((t.r(), t.g(), t.b()), (10, 5, 3), "premultiplied at 5%, not boosted");
    assert_eq!(crate::theme::tint(c, 0.0), Color32::TRANSPARENT);
}

#[test]
fn a_page_selection_reads_like_a_print_dialog() {
    use export::{describe_pages, parse_page_range as r};
    assert_eq!(r("1-3, 7, 10-", 12).unwrap(), vec![1, 2, 3, 7, 10, 11, 12]);
    assert_eq!(r("3", 12).unwrap(), vec![3]);
    assert_eq!(r("-2", 12).unwrap(), vec![1, 2], "an open start is page 1");
    assert_eq!(r("7 3;3,1", 12).unwrap(), vec![1, 3, 7], "sorted, without repeats");
    assert_eq!(r("10-99", 12).unwrap(), vec![10, 11, 12], "a range past the end stops at it");
    assert_eq!(r("2\u{2013}4", 12).unwrap(), vec![2, 3, 4], "an en dash is a dash");
    for bad in ["", "0", "5-2", "abc", "13", "1-x"] {
        assert!(r(bad, 12).is_err(), "{bad:?} should be refused");
    }
    assert_eq!(r("20", 12).unwrap_err(), "There are only 12 pages");
    assert_eq!(describe_pages(&[1, 2, 3, 7, 10, 11, 12]), "1-3, 7, 10-12");
    assert_eq!(describe_pages(&[4]), "4");
}

fn long_speeches() -> Document {
    let mut d = Document::default();
    d.blocks.clear();
    d.meta.title = "Voices".into();
    d.push(Element::SceneHeading, "INT. HALL - NIGHT");
    for k in 0..40 {
        d.push(Element::Action, "Somebody crosses the hall and the floor answers every step.");
        d.push(Element::Character, if k % 2 == 0 { "MARIA" } else { "COLE (V.O.)" });
        d.push(Element::Parenthetical, "(low)");
        d.push(Element::Dialogue, "Say it once, say it plainly, and then say nothing at all for the rest of the night.");
    }
    d.reseed_ids();
    d
}

#[test]
fn a_pdf_plan_keeps_chosen_pages_their_numbers_and_their_voices() {
    let d = long_speeches();
    let total = export::page_count(&d);
    assert!(total >= 3, "{total}");
    let inks: std::collections::HashMap<String, [u8; 3]> =
        [("MARIA".to_string(), [200, 0, 0]), ("COLE".to_string(), [0, 0, 200])].into_iter().collect();
    let opts = export::PdfOptions {
        pages: Some(vec![2, total]),
        voices: Some(inks),
        ..export::PdfOptions::default()
    };
    let plan = export::pdf_plan(&d, &opts);
    assert_eq!(plan.iter().map(|p| p.number).collect::<Vec<_>>(), vec![2, total], "numbered as in the whole script");
    for page in &plan {
        for (line, ink) in &page.lines {
            match line.element {
                Some(Element::Action) | Some(Element::SceneHeading) => assert_eq!(*ink, None, "{}", line.text),
                Some(Element::Character) | Some(Element::Parenthetical) | Some(Element::Dialogue) => {
                    assert!(ink.is_some(), "speech carries its speaker's ink: {}", line.text)
                }
                _ => {}
            }
        }
    }
    let all = export::pdf_plan(&d, &export::PdfOptions { voices: opts.voices.clone(), ..Default::default() });
    for w in all.windows(2) {
        if let Some((l, ink)) = w[1].lines.iter().find(|(l, _)| l.element.is_some()) {
            if l.element == Some(Element::Dialogue) {
                assert!(ink.is_some(), "carried over the break");
            }
        }
    }
    assert!(export::pdf_plan(&d, &export::PdfOptions::default())
        .iter()
        .all(|p| p.lines.iter().all(|(_, i)| i.is_none())));
}

#[test]
fn a_pdf_of_some_pages_has_just_those_pages() {
    let d = long_speeches();
    let dir = std::env::temp_dir();
    let count = |path: &std::path::Path| {
        let b = std::fs::read(path).unwrap();
        let t = String::from_utf8_lossy(&b);
        t.matches("/Type /Page\n").count() + t.matches("/Type/Page\n").count()
            + t.matches("/Type /Page>>").count() + t.matches("/Type/Page/").count()
            + t.matches("/Type /Page/").count() + t.matches("/Type /Page ").count()
    };
    let whole = dir.join(format!("ns-whole-{}.pdf", std::process::id()));
    export::to_pdf_opts(&d, &whole, &export::PdfOptions::default()).unwrap();
    let some = dir.join(format!("ns-some-{}.pdf", std::process::id()));
    let inks = crate::theme::character_inks(&d.speakers(), 0);
    export::to_pdf_opts(
        &d,
        &some,
        &export::PdfOptions {
            title_page: false,
            pages: Some(vec![2]),
            voices: Some(inks),
            scene_numbers: true,
            notes: false,
        },
    )
    .unwrap();
    let (w, s) = (count(&whole), count(&some));
    assert_eq!(w, 1 + export::page_count(&d), "title page and every page");
    assert_eq!(s, 1, "one chosen page, no title page");
    assert!(export::to_pdf_opts(
        &d,
        &some,
        &export::PdfOptions { title_page: false, pages: Some(vec![]), ..Default::default() }
    )
    .is_err(), "an empty PDF is refused");
}

#[test]
fn character_inks_read_on_paper_whatever_the_app_theme() {
    let names: Vec<String> = (0..9).map(|k| format!("SPEAKER {k}")).collect();
    for dark in [true, false] {
        crate::theme::set_palette(dark);
        let inks = crate::theme::character_inks(&names, 5);
        let app = crate::theme::character_colors(&names, 5);
        for n in &names {
            let [r, g, b] = inks[n];
            let l = 0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32;
            assert!(l < 140.0, "{n} is dark enough for white paper ({l})");
            let c = app[n];
            let top = |x: [u8; 3]| (0..3).max_by_key(|&i| x[i]).unwrap();
            assert_eq!(top([r, g, b]), top([c.r(), c.g(), c.b()]), "{n} keeps its hue");
        }
    }
    assert!(Settings::parse("scene_numbers = yes\n").pdf_scene_numbers);
    assert!(!Settings::parse("scene_numbers = yes\npdf_scene_numbers = no\n").pdf_scene_numbers);
}

// acts here---

fn two_acts() -> Document {
    let mut d = Document::default();
    d.blocks.clear();
    d.meta.title = "Pilot".into();
    d.push(Element::Act, "ACT ONE");
    d.push(Element::SceneHeading, "INT. DINER - DAY");
    d.push(Element::Action, "Coffee steams.");
    d.push(Element::SceneHeading, "EXT. LOT - DAY");
    d.push(Element::Character, "MARIA");
    d.push(Element::Dialogue, "We're late.");
    d.push(Element::Act, "ACT TWO");
    d.push(Element::SceneHeading, "INT. CAR - NIGHT");
    for k in 0..30 {
        d.push(Element::Action, &format!("The road unrolls, mile {k} of it, headlights on the white line."));
    }
    d.push(Element::SceneHeading, "EXT. MOTEL - NIGHT");
    d.push(Element::Action, "A sign buzzes.");
    d.reseed_ids();
    d
}

#[test]
fn acts_are_titled_and_closed_in_words() {
    use crate::model::{act_title, end_of};
    assert_eq!(act_title(1), "ACT ONE");
    assert_eq!(act_title(5), "ACT FIVE");
    assert_eq!(act_title(20), "ACT TWENTY");
    assert_eq!(act_title(21), "ACT 21");
    assert_eq!(end_of("ACT TWO"), "END OF ACT TWO");
    assert_eq!(end_of("teaser"), "END OF TEASER");
    assert_eq!(end_of(""), "END OF ACT");
}

#[test]
fn an_act_round_trips_through_the_file_as_a_level_one_heading() {
    let a = two_acts();
    let md = storage::to_markdown(&a);
    assert!(md.contains("\n# ACT ONE\n\n## INT. DINER - DAY\n"), "{md}");
    assert!(md.contains("\n# ACT TWO\n"));
    let b = storage::from_markdown(&md);
    let kinds = |d: &Document| d.blocks.iter().map(|b| (b.element, b.text.clone())).collect::<Vec<_>>();
    assert_eq!(kinds(&a), kinds(&b));
    let mut c = two_acts();
    c.blocks[0].text.clear();
    let back = storage::from_markdown(&storage::to_markdown(&c));
    assert_eq!(back.blocks[0].element, Element::Act);
    assert_eq!(back.blocks[0].text, "");
}

#[test]
fn scenes_end_where_an_act_begins_and_acts_span_their_scenes() {
    let d = two_acts();
    let scenes = d.scenes();
    assert_eq!(scenes.len(), 4);
    assert_eq!(scenes[1].end, 6);
    assert_eq!(d.blocks[scenes[1].end].element, Element::Act);
    let acts = d.acts();
    assert_eq!(acts.len(), 2);
    assert_eq!((acts[0].number, acts[0].title.as_str(), acts[0].start, acts[0].end), (1, "ACT ONE", 0, 6));
    assert_eq!((acts[1].start, acts[1].end), (6, d.blocks.len()));
    assert_eq!(d.scene_of(d.blocks[6].id), None);
    assert_eq!(d.scene_of(d.blocks[8].id), Some(2));
    assert_eq!(d.act_of(d.blocks[8].id), Some(1));
    assert_eq!(d.act_of(d.blocks[0].id), Some(0));
    assert!(sample().acts().is_empty());
    assert_eq!(sample().scenes().len(), 1);
}

#[test]
fn every_act_starts_a_page_centred_and_ends_with_its_end_of_line() {
    let d = two_acts();
    let lines = export::compose(&d);
    let act_lines: Vec<&export::Line> = lines.iter().filter(|l| l.element == Some(Element::Act)).collect();
    let texts: Vec<&str> = act_lines.iter().map(|l| l.text.as_str()).collect();
    assert_eq!(texts, ["ACT ONE", "END OF ACT ONE", "ACT TWO", "END OF ACT TWO"]);
    for l in &act_lines {
        assert_eq!(l.indent, (60 - l.text.len()) / 2, "{} is centred", l.text);
    }
    assert!(act_lines[1].is_act_end() && act_lines[1].block.is_none());
    assert!(act_lines[0].is_act_title());

    let pages = export::paginate(&lines);
    let first = |pg: &Vec<export::Line>| pg.iter().find(|l| !l.text.trim().is_empty()).map(|l| l.text.clone());
    assert_eq!(first(&pages[0]).as_deref(), Some("ACT ONE"));
    let two = pages.iter().position(|pg| first(pg).as_deref() == Some("ACT TWO")).expect("ACT TWO opens a page");
    assert_eq!(two, 1, "act one fits on one page, so act two is page 2");
    assert!(pages[0].iter().any(|l| l.text == "END OF ACT ONE"));
    assert!(pages.len() >= 3, "act two runs over");
    let starts = export::page_starts(&d);
    assert_eq!(starts[0], (2, d.blocks[6].id));
    assert_eq!(export::page_of_block(&d, d.blocks[6].id), Some(2));
}

#[test]
fn an_end_of_line_is_never_alone_at_the_top_of_a_page() {
    for fill in 40..60 {
        let mut d = Document::default();
        d.blocks.clear();
        d.push(Element::Act, "ACT ONE");
        d.push(Element::SceneHeading, "INT. HALL - DAY");
        for k in 0..fill {
            d.push(Element::Character, "MARIA");
            d.push(Element::Dialogue, &format!("Line {k}."));
        }
        d.reseed_ids();
        for pg in export::paginate(&export::compose(&d)) {
            let first = pg.iter().find(|l| !l.text.trim().is_empty()).unwrap();
            assert!(!first.is_act_end(), "END OF opened a page with {fill} speeches");
            assert!(pg.len() <= crate::model::LINES_PER_PAGE + 4);
        }
    }
}

#[test]
fn acts_survive_fountain_and_final_draft_without_doubling_their_close() {
    let d = two_acts();
    let f = export::to_fountain(&d);
    assert!(f.contains("# ACT ONE\n\n>ACT ONE<"), "{f}");
    assert!(f.contains(">END OF ACT ONE<\n\n===\n\n# ACT TWO"), "{f}");
    assert!(f.trim_end().ends_with(">END OF ACT TWO<"));
    let back = crate::fountain::parse(&f);
    let acts: Vec<String> = back.blocks.iter().filter(|b| b.element == Element::Act).map(|b| b.text.clone()).collect();
    assert_eq!(acts, ["ACT ONE", "ACT TWO"]);
    assert!(!back.blocks.iter().any(|b| b.text.contains("END OF")), "the close is not kept twice");
    assert_eq!(back.scenes().len(), 4);

    let x = export::to_fdx(&d);
    assert!(x.contains("<Paragraph Type=\"New Act\">\n      <Text>ACT ONE</Text>"));
    assert!(x.contains("<Paragraph Type=\"End of Act\">\n      <Text>END OF ACT TWO</Text>"));
    let back = crate::fountain::parse_fdx(&x);
    let acts: Vec<String> = back.blocks.iter().filter(|b| b.element == Element::Act).map(|b| b.text.clone()).collect();
    assert_eq!(acts, ["ACT ONE", "ACT TWO"]);
    assert!(!back.blocks.iter().any(|b| b.text.contains("END OF")));

    let other = "Title: X\n\n# Teaser\n\nEXT. SEA - DAY\n\nWaves.\n\n>END OF TEASER<\n\n===\n\n>**_ACT ONE_**<\n\nINT. SHIP - DAY\n\nCreaks.\n\n## A sequence\n\n>THE END<\n";
    let back = crate::fountain::parse(other);
    let kinds: Vec<(Element, &str)> = back.blocks.iter().map(|b| (b.element, b.text.as_str())).collect();
    assert_eq!(
        kinds,
        [
            (Element::Act, "TEASER"),
            (Element::SceneHeading, "EXT. SEA - DAY"),
            (Element::Action, "Waves."),
            (Element::Act, "ACT ONE"),
            (Element::SceneHeading, "INT. SHIP - DAY"),
            (Element::Action, "Creaks."),
            (Element::Action, "THE END"),
        ]
    );
}

#[test]
fn a_pdf_with_acts_is_written() {
    let bytes = export::pdf_bytes(&two_acts(), &export::PdfOptions::default()).unwrap();
    assert_eq!(&bytes[..5], b"%PDF-");
    let txt = export::to_plain_text(&two_acts());
    assert!(txt.contains(&format!("{}ACT ONE\n", " ".repeat(26))));
    assert!(txt.contains("END OF ACT TWO"));
}

// chosen colours here---

#[test]
fn chosen_colours_ride_in_the_front_matter_and_only_when_there_are_some() {
    let mut d = sample();
    d.meta.set_voice("MARIA", Some(212));
    d.meta.set_voice("COLE", Some(384));
    let md = storage::to_markdown(&d);
    assert!(md.contains("\nvoices: MARIA=212; COLE=24\n---\n"), "{md}");
    let back = storage::from_markdown(&md);
    assert_eq!(back.meta.voices, vec![("MARIA".to_string(), 212), ("COLE".to_string(), 24)]);
    assert_eq!(back.meta.voice("COLE"), Some(24));
    let mut e = back.clone();
    e.meta.set_voice("MARIA", None);
    assert_eq!(e.meta.voices, vec![("COLE".to_string(), 24)]);
    assert!(!storage::to_markdown(&sample()).contains("voices"));
    let odd = "---\ntitle: X\nvoices: MARIA=; =40; BOB=12; bob=99; SAL=x\n---\n\n## INT. A - DAY\n";
    assert_eq!(storage::from_markdown(odd).meta.voices, vec![("BOB".to_string(), 12)]);
}

#[test]
fn custom_colours_keep_the_chosen_hue_and_deal_the_rest_as_random_does() {
    use crate::theme;
    let names: Vec<String> = ["MARIA", "COLE", "JUNE"].iter().map(|s| s.to_string()).collect();
    for dark in [true, false] {
        theme::set_palette(dark);
        let random = theme::character_colors(&names, 3);
        assert_eq!(theme::voice_colors(&names, 3, &[]), random);
        let chosen = vec![("COLE".to_string(), 120u16), ("NOBODY".to_string(), 10)];
        let custom = theme::voice_colors(&names, 3, &chosen);
        assert_eq!(custom["COLE"], theme::voice_of_hue(120));
        assert_eq!(custom["MARIA"], random["MARIA"], "the others are dealt as before");
        assert_eq!(custom["JUNE"], random["JUNE"]);
        assert!(!custom.contains_key("NOBODY"), "a colour for someone who never speaks is ignored");
        let inks = theme::voice_inks(&names, 3, &chosen);
        assert_eq!(theme::voice_inks(&names, 3, &[]), theme::character_inks(&names, 3));
        let [r, g, b] = inks["COLE"];
        assert!(g > r && g > b, "120 is green on paper too: {:?}", inks["COLE"]);
        assert!((r as u32 + g as u32 + b as u32) < 3 * 160, "deep enough to read as ink");
    }
}

#[test]
fn the_colour_mode_is_remembered_and_written_only_once_chosen() {
    use crate::settings::Settings;
    let plain = Settings::default().serialize();
    assert!(!plain.contains("character_colors_mode"), "a file that never chose stays as it was");
    let s = Settings {
        custom_colors: true,
        ..Default::default()
    };
    let text = s.serialize();
    assert!(text.contains("character_colors_mode = custom"));
    assert!(Settings::parse(&text).custom_colors);
    assert!(!Settings::parse(&plain).custom_colors);
}

// shortcuts here---

#[test]
fn every_default_shortcut_reads_back_and_the_specific_chord_is_heard_first() {
    use crate::keys::{self, Command, Keymap};
    use eframe::egui::{Key, KeyboardShortcut, Modifiers};
    for web in [false, true] {
        for cmd in Command::ALL {
            assert_eq!(Command::from_slug(&cmd.slug()), Some(cmd));
            for k in cmd.defaults(web) {
                assert_eq!(keys::parse(&keys::text(&k)), Some(k), "{}", keys::text(&k));
            }
        }
        let order = Keymap::default().listen(&Command::ALL, web);
        for (i, (_, a)) in order.iter().enumerate() {
            for (_, b) in &order[i + 1..] {
                let more = |x: &KeyboardShortcut| x.modifiers.shift as u8 + x.modifiers.alt as u8;
                if a.logical_key == b.logical_key {
                    assert!(more(a) >= more(b), "{} before {}", keys::text(a), keys::text(b));
                }
            }
        }
    }
    assert_eq!(keys::text(&KeyboardShortcut::new(Modifiers::COMMAND, Key::Comma)), "Ctrl+,");
    assert_eq!(keys::parse("Ctrl++"), Some(KeyboardShortcut::new(Modifiers::COMMAND, Key::Plus)));
    assert_eq!(keys::parse("ctrl+shift+enter"), Some(KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::SHIFT, Key::Enter)));
    assert_eq!(keys::parse("Hyper+Q"), None);
    assert_eq!(Keymap::default().label(Command::NewScript, true), "Ctrl+Alt+N");
    assert_eq!(Keymap::default().label(Command::Element(3), true), "Alt+3");
    assert_eq!(Keymap::default().label(Command::Element(3), false), "Ctrl+3");
}

#[test]
fn a_chosen_shortcut_is_kept_only_while_it_differs_from_the_default() {
    use crate::keys::{self, Command, Keymap};
    use crate::settings::Settings;
    use eframe::egui::{Key, KeyboardShortcut, Modifiers};
    let alt_d = KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::ALT, Key::D);
    let mut s = Settings::default();
    assert!(!s.serialize().contains("key."), "nothing chosen, nothing written");
    s.keys.set(Command::ToggleDetails, alt_d, true);
    assert_eq!(s.keys.chords(Command::ToggleDetails, true), vec![alt_d]);
    let text = s.serialize();
    assert!(text.contains("key.details = Ctrl+Alt+D\n"), "{text}");
    let back = Settings::parse(&text);
    assert_eq!(back.keys, s.keys);
    s.keys.set(Command::ToggleDetails, KeyboardShortcut::new(Modifiers::COMMAND, Key::I), true);
    assert!(s.keys.is_default());
    let km = Keymap::default();
    let ctrl_s = KeyboardShortcut::new(Modifiers::COMMAND, Key::S);
    assert_eq!(km.taken_by(&ctrl_s, Command::Undo, &Command::ALL, false), Some(Command::Save));
    assert_eq!(km.taken_by(&ctrl_s, Command::Save, &Command::ALL, false), None);
    let refuse = |m: Modifiers, k: Key, web: bool| keys::refused(&KeyboardShortcut::new(m, k), web);
    assert!(refuse(Modifiers::NONE, Key::Q, false).is_some(), "a plain key is for typing");
    assert!(refuse(Modifiers::SHIFT, Key::Enter, false).is_some());
    assert!(refuse(Modifiers::NONE, Key::F2, false).is_none(), "function keys are fine bare");
    assert!(refuse(Modifiers::COMMAND, Key::W, true).is_some(), "a browser keeps Ctrl+W");
    assert!(refuse(Modifiers::COMMAND, Key::Num4, true).is_some());
    assert!(refuse(Modifiers::ALT, Key::Num4, true).is_none());
    assert!(refuse(Modifiers::COMMAND, Key::V, false).is_some());
    assert!(refuse(Modifiers::ALT, Key::ArrowUp, false).is_some());
    assert!(refuse(Modifiers::COMMAND | Modifiers::ALT, Key::D, true).is_none());
}

#[test]
fn text_is_cut_to_the_width_it_is_given_not_a_guessed_count() {
    let ctx = eframe::egui::Context::default();
    crate::theme::install_fonts(&ctx);
    let _ = ctx.run(Default::default(), |ctx| {
        let font = crate::theme::font_display(22.0);
        let long = "FIFINE AM8 REVIEW - HOW TO INSTANTLY IMPROVE YOUR VOICEOVER AUDIO";
        let w = |s: &str| crate::theme::tracked_width(ctx, s, font.clone(), 0.0);
        for room in [60.0, 180.0, 400.0] {
            let cut = crate::ui::elide_to_width(ctx, long, font.clone(), 0.0, room);
            assert!(w(&cut) <= room, "{cut:?} is {} wide in {room}", w(&cut));
            assert!(cut.ends_with('\u{2026}'));
        }
        assert_eq!(crate::ui::elide_to_width(ctx, "PILOT", font.clone(), 0.0, 400.0), "PILOT");
        let ro = "Geabiștii Prime – O noapte sinistră în țară";
        let cut = crate::ui::elide_to_width(ctx, ro, crate::theme::font_bold(15.0), 0.0, 150.0);
        let head = cut.trim_end_matches('\u{2026}');
        assert!(ro.starts_with(head), "{cut:?}");
        let hud = crate::theme::font_hud(10.5);
        let cut = crate::ui::elide_to_width(ctx, "5 PAGES / 11 SCENES / SAVED BY YOU", hud.clone(), 0.8, 120.0);
        assert!(crate::theme::tracked_width(ctx, &cut, hud, 0.8) <= 120.0);
    });
}

fn speech_across_a_break(action_paras: usize, words: usize) -> Document {
    let mut md = String::from("---\ntitle: Break\n---\n\n## INT. ROOM - DAY\n\n");
    for k in 0..action_paras {
        md.push_str(&format!("Something happens, number {k}.\n\n"));
    }
    md.push_str("**MARIA (V.O.)**\n\n> ");
    let speech: Vec<String> = (0..words).map(|k| format!("word{k}")).collect();
    md.push_str(&speech.join(" "));
    md.push_str("\n\nShe stops.\n");
    storage::from_markdown(&md)
}

#[test]
fn a_speech_broken_by_a_page_carries_more_and_contd() {
    let d = speech_across_a_break(24, 160);
    let pages = export::paginate(&export::compose(&d));
    assert!(pages.len() >= 2);
    let foot = pages[0].iter().rev().find(|l| !l.text.trim().is_empty()).unwrap();
    assert_eq!(foot.text, export::MORE, "the page ends with (MORE)");
    assert!(foot.block.is_none(), "(MORE) is layout, never stored");
    let top = pages[1].iter().find(|l| !l.text.trim().is_empty()).unwrap();
    assert_eq!(top.text, "MARIA (V.O.) (CONT'D)");
    assert_eq!(top.indent, Element::Character.indent_cols());
    assert_eq!(pages[1][1].element, Some(Element::Dialogue));
    let left = pages[0].iter().filter(|l| l.element == Some(Element::Dialogue)).count();
    assert!(left >= 2);
    assert!(pages.iter().all(|p| p.len() <= crate::model::LINES_PER_PAGE));
    assert!(!storage::to_markdown(&d).contains("CONT'D"));
    assert!(!storage::to_markdown(&d).contains("(MORE)"));
}

#[test]
fn a_speech_that_fits_has_no_more_and_a_lone_cue_still_moves() {
    let d = speech_across_a_break(2, 20);
    let lines = export::paginate(&export::compose(&d)).concat();
    assert!(!lines.iter().any(|l| l.text == export::MORE || l.text.contains("CONT'D")));
    for n in 20..30 {
        let d = speech_across_a_break(n, 30);
        for page in export::paginate(&export::compose(&d)) {
            let last = page.iter().rev().find(|l| !l.text.trim().is_empty()).unwrap();
            assert!(
                !(last.element == Some(Element::Character) && last.block.is_some()),
                "{n}: a cue stranded at the foot"
            );
        }
    }
}

#[test]
fn the_update_check_compares_versions_and_reads_github() {
    use crate::update::{newer, parse};
    assert!(newer("2.0.0", "2.0.1"));
    assert!(newer("2.0.1", "v2.1.0"));
    assert!(newer("2.1.0", "10.0.0"), "numbers, not text");
    assert!(newer("2.0", "2.0.1"));
    assert!(!newer("2.0.0", "v2.0.0"));
    assert!(!newer("2.1.0", "2.0.9"));
    assert!(!newer("2.0.0", "2.0.0-rc1"));
    let reply = r#"{"url":"https://api.github.com/x","html_url":"https://github.com/NativeCompute/blackline/releases/tag/v2.1.0","tag_name":"v2.1.0","author":{"html_url":"https://github.com/someone"}}"#;
    let r = parse(reply, crate::INFO.repo).unwrap();
    assert_eq!(r.version, "2.1.0");
    assert_eq!(r.url, "https://github.com/NativeCompute/blackline/releases/tag/v2.1.0");
    assert!(parse("not json", crate::INFO.repo).is_none());
    assert!(parse("{}", crate::INFO.repo).is_none());
    assert!(Settings::default().check_updates);
    assert!(!Settings::parse("check_updates = no\n").check_updates);
}

#[test]
fn the_app_icon_is_drawn_from_the_mark() {
    let n = 64u32;
    let px = crate::logo::rgba(n);
    assert_eq!(px.len(), (n * n * 4) as usize);
    let at = |x: u32, y: u32| {
        let i = ((y * n + x) * 4) as usize;
        [px[i], px[i + 1], px[i + 2], px[i + 3]]
    };
    assert_eq!(at(0, 0)[3], 0, "the tile's corner is rounded off");
    assert_eq!(at(4, n / 2), [0, 0, 0, 255], "the tile is black");
    assert_eq!(at(26, 32), [0xF2, 0xF3, 0xF7, 255], "the slab is white");
    assert_eq!(at(42, 32), [0x3B, 0x4C, 0xFF, 255], "the line is the signal blue");
    let ico = crate::logo::ico();
    assert_eq!(&ico[..4], &[0, 0, 1, 0]);
    assert_eq!(u16::from_le_bytes([ico[4], ico[5]]), 6, "six sizes");
    assert_eq!(ico[6 + 16 * 5], 0, "the last is 256 px, written as 0");
    let shipped = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/packaging/blackline.ico")).unwrap();
    assert_eq!(shipped, ico, "packaging/blackline.ico is regenerated from the mark");
}

// blue pages format here---

const A_BLUE_PAGES_FILE: &str = "---\ntitle: Crossfire\nauthor: Sam\ncontact: \ndraft: Blue\nstorylines: 0=Maria; 2=The heist\nrevision: blue\nrevision_date: 2026-11-04\nrevision_base: 2026-11-04_10-00-00.md\nlocked: yes\n---\n\n## INT. KITCHEN - NIGHT [[too dark?]]\n<!-- scene: tint=2 | They argue. -->\n\nThe kettle screams. [[cut the kettle]]\n\n**MARIA**\n\n> I said stop.\n\n**JONAS ^ [[louder]]**\n\n*(over her [[maybe cut]])*\n\n> I'm not stopping!\n\n### ON THE KETTLE [[insert]]\n\n`CUT TO: [[or SMASH CUT]]`\n\n# ACT TWO [[midpoint]]\n\n";

#[test]
fn every_blue_pages_marker_round_trips_exactly() {
    let d = storage::from_markdown(A_BLUE_PAGES_FILE);
    let cue = d.blocks.iter().find(|b| b.text == "JONAS").expect("the dual cue keeps its name");
    assert!(cue.dual);
    assert_eq!(cue.remark, "louder");
    assert!(!d.blocks.iter().find(|b| b.text == "MARIA").unwrap().dual);
    let heading = &d.blocks[0];
    assert_eq!(heading.text, "INT. KITCHEN - NIGHT");
    assert_eq!(heading.remark, "too dark?");
    assert_eq!(heading.note, "They argue.");
    assert_eq!(heading.tint, Some(2));
    let paren = d.blocks.iter().find(|b| b.element == Element::Parenthetical).unwrap();
    assert_eq!(paren.text, "(over her)");
    assert_eq!(paren.remark, "maybe cut");
    assert_eq!(d.blocks.iter().find(|b| b.element == Element::Transition).unwrap().remark, "or SMASH CUT");
    assert_eq!(d.meta.storyline(2), Some("The heist"));
    assert_eq!(d.meta.revision, Some(0));
    assert!(d.meta.revising() && d.meta.locked);
    assert_eq!(storage::to_markdown(&d), A_BLUE_PAGES_FILE);
}

#[test]
fn text_that_only_looks_like_a_marker_is_left_exactly_as_it_was() {
    let lookalikes = [
        "Two spaces before  [[this]] stay text.",
        "Glued[[on]] stays text.",
        "Odd [[a]] b]] stays text.",
        "Padded [[ inside ]] stays text.",
        "[[only a bracket]]",
        "**BOB^**",
        "**BOB  ^**",
        "> She says [[nothing]]",
    ];
    for line in lookalikes {
        let file = format!("---\ntitle: T\nauthor: \ncontact: \ndraft: \n---\n\n## INT. X - DAY\n\n{line}\n\n");
        let d = storage::from_markdown(&file);
        assert_eq!(storage::to_markdown(&d), file, "{line:?} must re-save unchanged");
    }
    let d = storage::from_markdown("**BOB^**\n");
    assert!(!d.blocks[0].dual, "a caret glued to the name is not a dual marker");
}

#[test]
fn a_script_without_the_new_things_writes_none_of_them() {
    let mut d = sample();
    d.normalize();
    let md = storage::to_markdown(&d);
    for key in ["storylines:", "revision", "locked:", "[[", " ^**"] {
        assert!(!md.contains(key), "{key} must only appear once it is used");
    }
    d.meta.set_storyline(1, "  ");
    d.blocks[1].remark = "   ".into();
    assert_eq!(storage::to_markdown(&d), md, "blank names and notes write nothing");
}

// dual dialogue here---

fn argument() -> Document {
    let mut d = Document::default();
    d.blocks.clear();
    d.meta.title = "Crossfire".into();
    d.push(Element::SceneHeading, "INT. KITCHEN - NIGHT");
    d.push(Element::Action, "They face each other across the table.");
    d.push(Element::Character, "MARIA");
    d.push(Element::Dialogue, "I said stop. I said it twice and you kept going anyway.");
    d.push(Element::Character, "JONAS");
    d.push(Element::Parenthetical, "(over her)");
    d.push(Element::Dialogue, "I'm not stopping!");
    d.push(Element::Action, "Silence.");
    d.reseed_ids();
    d
}

#[test]
fn two_speeches_pair_and_unpair() {
    let mut d = argument();
    assert!(d.dual_pairs().is_empty());
    let jonas = d.blocks[4].id;
    assert_eq!(d.toggle_dual(jonas), Ok(true));
    assert_eq!(d.dual_pairs(), vec![crate::model::DualPair { left: 2, right: 4, end: 7 }]);
    let maria_line = d.blocks[3].id;
    assert_eq!(d.toggle_dual(maria_line), Ok(false), "either speech can unpair them");
    assert!(d.dual_pairs().is_empty());
    let maria = d.blocks[2].id;
    assert_eq!(d.toggle_dual(maria), Ok(true), "from the first speech it pairs with the next");
    assert!(d.blocks[4].dual);
    let silence = d.blocks[7].id;
    assert!(d.toggle_dual(silence).is_err(), "action is not a speech");
}

#[test]
fn a_dual_pair_prints_side_by_side_on_shared_rows() {
    let mut d = argument();
    d.blocks[4].dual = true;
    let lines = export::compose(&d);
    let rows: Vec<&export::Line> = lines.iter().filter(|l| l.dual).collect();
    assert_eq!(rows[0].text, "MARIA");
    assert_eq!(rows[0].indent, Element::Character.dual_indent());
    let right = rows[0].right.as_ref().expect("JONAS sits on the same row");
    assert_eq!(right.text, "JONAS");
    assert_eq!(right.indent, crate::model::DUAL_RIGHT + Element::Character.dual_indent());
    assert!(rows.iter().all(|l| l.text.chars().count() <= crate::model::DUAL_COLS));
    let left_rows = 1 + crate::model::wrap("I said stop. I said it twice and you kept going anyway.", crate::model::DUAL_COLS).len();
    assert_eq!(rows.len(), left_rows.max(3), "the pair is as tall as the taller speech");
    let text = export::to_plain_text(&d);
    assert!(text.lines().any(|l| l.contains("MARIA") && l.contains("JONAS")), "{text}");
    let single = export::compose(&argument()).len();
    assert!(lines.len() < single, "side by side takes fewer rows than one after the other");
}

#[test]
fn a_dual_pair_is_never_split_across_pages() {
    for filler in 40..56 {
        let mut d = argument();
        d.blocks.truncate(2);
        for k in 0..filler {
            d.push(Element::Action, &format!("Beat {k}."));
        }
        d.push(Element::Character, "MARIA");
        d.push(Element::Dialogue, "One. Two. Three. Four. Five. Six. Seven. Eight. Nine. Ten.");
        d.push(Element::Character, "JONAS");
        d.push(Element::Dialogue, "Ten. Nine. Eight. Seven. Six. Five. Four. Three. Two. One.");
        let n = d.blocks.len();
        d.blocks[n - 2].dual = true;
        d.reseed_ids();
        let pages = export::paginate(&export::compose(&d));
        let holding: Vec<usize> = pages
            .iter()
            .enumerate()
            .filter(|(_, p)| p.iter().any(|l| l.dual))
            .map(|(k, _)| k)
            .collect();
        assert_eq!(holding.len(), 1, "with {filler} beats the pair landed on pages {holding:?}");
        assert!(pages.iter().all(|p| p.len() <= crate::model::LINES_PER_PAGE));
    }
}

#[test]
fn dual_dialogue_survives_fountain_and_final_draft() {
    let mut d = argument();
    d.blocks[4].dual = true;
    let f = export::to_fountain(&d);
    assert!(f.contains("@JONAS ^"), "{f}");
    let back = crate::fountain::parse(&f);
    assert_eq!(back.dual_pairs().len(), 1, "{f}");
    let x = export::to_fdx(&d);
    assert!(x.contains("<DualDialogue>"));
    let back = crate::fountain::parse_fdx(&x);
    assert_eq!(back.dual_pairs().len(), 1);
    assert!(
        !back.blocks.iter().any(|b| b.element == Element::Action && b.text == "MARIA"),
        "the wrapper is not read as a line of action"
    );
    assert_eq!(back.blocks.len(), d.blocks.len());
}

// script notes here---

#[test]
fn notes_ride_through_fountain_on_every_kind_of_line() {
    let mut d = argument();
    d.blocks[1].remark = "shorter".into();
    d.blocks[4].dual = true;
    d.blocks[4].remark = "louder".into();
    d.blocks[5].remark = "maybe cut".into();
    let f = export::to_fountain(&d);
    assert!(f.contains("@JONAS ^ [[louder]]"), "{f}");
    assert!(f.contains("(over her) [[maybe cut]]"), "{f}");
    let back = crate::fountain::parse(&f);
    let kinds: Vec<Element> = back.blocks.iter().map(|b| b.element).collect();
    assert_eq!(kinds, d.blocks.iter().map(|b| b.element).collect::<Vec<_>>(), "{f}");
    let remarks: Vec<&str> = back.blocks.iter().map(|b| b.remark.as_str()).collect();
    assert_eq!(remarks, d.blocks.iter().map(|b| b.remark.as_str()).collect::<Vec<_>>());
    assert!(back.blocks[4].dual);
    assert!(back.blocks.iter().all(|b| !b.text.contains("[[")));
}

#[test]
fn a_fountain_note_on_its_own_line_belongs_to_the_line_before() {
    let f = "INT. HOUSE - DAY\n\nShe waits.\n[[too quiet?]]\n\nHe knocks.\n\n[[multi\nline note]]\nGone.\n";
    let d = crate::fountain::parse(f);
    let waits = d.blocks.iter().find(|b| b.text == "She waits.").unwrap();
    assert_eq!(waits.remark, "too quiet?");
    assert!(d.blocks.iter().all(|b| !b.text.contains("too quiet")));
    assert!(d.blocks.iter().all(|b| !b.text.contains("line note")), "notes across lines are still left out");
}

#[test]
fn the_pdf_can_list_every_note_with_its_page() {
    let mut d = argument();
    d.blocks[1].remark = "the kettle".into();
    let rows = export::endnotes(&d);
    assert_eq!(rows[0], (true, "NOTES".to_string()));
    assert!(rows.iter().any(|(bold, t)| *bold && t.starts_with("PAGE 1 — They face")));
    assert!(rows.iter().any(|(_, t)| t.trim() == "the kettle"));
    let with = export::pdf_bytes(&d, &export::PdfOptions { notes: true, ..Default::default() }).unwrap();
    let without = export::pdf_bytes(&d, &export::PdfOptions::default()).unwrap();
    assert!(with.len() > without.len());
    assert!(export::endnotes(&argument()).is_empty(), "no notes, no notes page");
}

fn take() -> Document {
    let mut d = Document::default();
    d.blocks.clear();
    d.push(Element::SceneHeading, "INT. KITCHEN - NIGHT");
    d.push(Element::Action, "Rain on the glass.");
    d.push(Element::Character, "MARIA");
    d.push(Element::Dialogue, "I said stop.");
    d.push(Element::Action, "Silence.");
    d.reseed_ids();
    d
}

#[test]
fn a_revision_marks_only_what_moved() {
    let base = take();
    assert!(crate::revision::changed(&base, &base).is_empty());

    let mut now = base.clone();
    now.blocks[3].text = "I said  stop. ".into();
    assert!(crate::revision::changed(&base, &now).is_empty(), "spacing alone is not a change");

    now.blocks[3].text = "I said stop. Now.".into();
    let edited = now.blocks[3].id;
    assert_eq!(crate::revision::changed(&base, &now), [edited].into_iter().collect());

    let mut now = base.clone();
    let mut thunder = now.blocks[1].clone();
    thunder.id = 99;
    thunder.text = "Thunder.".into();
    now.blocks.insert(2, thunder);
    assert_eq!(crate::revision::changed(&base, &now), [99].into_iter().collect());

    let mut now = base.clone();
    let after = now.blocks[2].id;
    now.blocks.remove(1);
    assert_eq!(crate::revision::changed(&base, &now), [after].into_iter().collect(), "a cut marks the line after it");

    let mut now = base.clone();
    now.blocks.pop();
    let last = now.blocks.last().unwrap().id;
    assert_eq!(crate::revision::changed(&base, &now), [last].into_iter().collect(), "a cut at the end marks the line before");

    let mut now = base.clone();
    now.blocks[3].element = Element::Action;
    let moved = now.blocks[3].id;
    assert_eq!(crate::revision::changed(&base, &now), [moved].into_iter().collect(), "a new element type counts");
}

#[test]
fn searching_inside_a_script_reads_lines_and_notes() {
    let mut d = take();
    d.blocks[3].remark = "Make the stop land harder".into();
    d.blocks[1].text = format!("{} Rain on the roof. {}", "word ".repeat(30), "word ".repeat(30));
    let text = storage::to_markdown(&d);

    let (hits, total) = storage::find_in(&text, "STOP", 5);
    assert_eq!(total, 2, "the line and its note");
    assert_eq!((hits[0].block, hits[0].note), (3, false));
    assert_eq!((hits[1].block, hits[1].note), (3, true));
    let first: String = hits[0].line.chars().skip(hits[0].start).take(hits[0].len).collect();
    assert_eq!(first, "stop");

    let (hits, _) = storage::find_in(&text, "rain  on the", 5);
    assert_eq!(hits.len(), 1);
    let long = &hits[0];
    assert_eq!(long.block, 1);
    assert!(long.line.starts_with('…') && long.line.ends_with('…'), "{}", long.line);
    let found: String = long.line.chars().skip(long.start).take(long.len).collect();
    assert_eq!(found, "Rain on the");

    let (hits, total) = storage::find_in(&text, "o", 1);
    assert_eq!(hits.len(), 1);
    assert!(total > 1);
    assert!(storage::find_in(&text, "", 5).0.is_empty());
    assert!(storage::find_in(&text, "nowhere at all", 5).0.is_empty());
}

#[test]
fn scene_headings_split_into_side_place_and_time() {
    use crate::report::split_heading;
    let s = |a: &str, b: &str, c: &str| (a.to_string(), b.to_string(), c.to_string());
    assert_eq!(split_heading("INT. KITCHEN - NIGHT"), s("INT.", "KITCHEN", "NIGHT"));
    assert_eq!(split_heading("ext.  rooftop - day"), s("EXT.", "ROOFTOP", "DAY"));
    assert_eq!(split_heading("INT./EXT. CAR - MOVING - DAWN"), s("INT./EXT.", "CAR - MOVING", "DAWN"));
    assert_eq!(split_heading("I/E. PORCH - LATER"), s("INT./EXT.", "PORCH", "LATER"));
    assert_eq!(split_heading("MONTAGE"), s("", "MONTAGE", ""));
}

#[test]
fn the_reports_count_what_is_on_the_page() {
    let mut d = Document::default();
    d.blocks.clear();
    d.push(Element::SceneHeading, "INT. KITCHEN - NIGHT");
    d.push(Element::Character, "MARIA");
    d.push(Element::Dialogue, "One two three four.");
    d.push(Element::Character, "JONAS (V.O.)");
    d.push(Element::Parenthetical, "(quiet)");
    d.push(Element::Dialogue, "Five six.");
    d.push(Element::SceneHeading, "EXT. ROOF - NIGHT");
    d.push(Element::Character, "MARIA");
    d.push(Element::Dialogue, "Seven eight.");
    d.push(Element::SceneHeading, "INT. KITCHEN - DAY");
    d.push(Element::Action, "Empty.");
    d.reseed_ids();
    let r = crate::report::build(&d);

    assert_eq!(r.speakers.len(), 2);
    let maria = &r.speakers[0];
    assert_eq!((maria.name.as_str(), maria.cues, maria.lines, maria.words, maria.scenes, maria.first_scene, maria.first_page), ("MARIA", 2, 2, 6, 2, 1, 1));
    let jonas = &r.speakers[1];
    assert_eq!((jonas.name.as_str(), jonas.cues, jonas.words), ("JONAS", 1, 2));
    assert!((maria.share - 0.75).abs() < 1e-4 && (jonas.share - 0.25).abs() < 1e-4);

    let lengths = export::scene_lengths(&d);
    assert_eq!(r.scenes.len(), 3);
    for (row, (id, page, eighths)) in r.scenes.iter().zip(&lengths) {
        assert_eq!((row.id, row.page, row.eighths), (*id, *page, *eighths), "the same figures as the printed layout");
    }
    assert_eq!(r.scenes[0].cast, vec!["MARIA".to_string(), "JONAS".to_string()]);

    let kitchen = r.places.iter().find(|p| p.name == "KITCHEN").unwrap();
    assert_eq!((kitchen.scenes, kitchen.sides.clone()), (2, vec!["INT.".to_string()]));
    assert_eq!(kitchen.eighths, r.scenes[0].eighths + r.scenes[2].eighths);
    let night = r.times.iter().find(|t| t.name == "NIGHT").unwrap();
    assert_eq!(night.scenes, 2);
    assert_eq!(r.times.iter().map(|t| t.eighths).sum::<usize>(), r.eighths);

    let text = r.text(0, "Test");
    assert!(text.starts_with("Test · Characters\n\nCHARACTER"), "{text}");
    assert!(text.contains("MARIA      2     2      6      75%"), "{text}");
    assert!(r.text(2, "Test").contains("KITCHEN"));
}

#[test]
fn beat_guides_land_on_the_usual_pages() {
    let beats = crate::cards::beat_pages(110 * 8);
    let pages: Vec<usize> = beats.iter().map(|b| b.2).collect();
    assert_eq!(pages, vec![12, 28, 56, 83, 100]);
    assert_eq!(beats[2].0, "Midpoint");
    assert_eq!(crate::cards::beat_pages(0).iter().map(|b| b.2).collect::<Vec<_>>(), vec![1; 5]);

    let spans = vec![(0, 8), (8, 4), (12, 20)];
    assert_eq!(crate::cards::beat_scene(&spans, 32, 0.10), Some(0));
    assert_eq!(crate::cards::beat_scene(&spans, 32, 0.30), Some(1));
    assert_eq!(crate::cards::beat_scene(&spans, 32, 0.90), Some(2));
}
