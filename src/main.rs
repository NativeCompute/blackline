#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use blackline::{app, logo, splash, storage, theme};

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--emit-icon") {
        let path = args
            .get(i + 1)
            .cloned()
            .unwrap_or_else(|| "blackline.svg".to_string());
        theme::set_palette(true);
        match std::fs::write(&path, logo::svg()) {
            Ok(()) => println!("wrote {path}"),
            Err(e) => eprintln!("could not write {path}: {e}"),
        }
        return Ok(());
    }
    if let Some(i) = args.iter().position(|a| a == "--emit-ico") {
        let path = args
            .get(i + 1)
            .cloned()
            .unwrap_or_else(|| "blackline.ico".to_string());
        match std::fs::write(&path, logo::ico()) {
            Ok(()) => println!("wrote {path}"),
            Err(e) => eprintln!("could not write {path}: {e}"),
        }
        return Ok(());
    }
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("blackline {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    let small = storage::read_settings().splash;
    let size: [f32; 2] = if small {
        [splash::CARD.x, splash::CARD.y]
    } else {
        [1320.0, 900.0]
    };

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Blackline")
            .with_icon(std::sync::Arc::new(eframe::egui::IconData {
                rgba: logo::rgba(128),
                width: 128,
                height: 128,
            }))
            .with_app_id("blackline")
            .with_transparent(true)
            .with_decorations(false)
            .with_drag_and_drop(true)
            .with_inner_size(size)
            .with_min_inner_size(if small { size } else { [880.0, 580.0] }),
        ..Default::default()
    };

    let files: Vec<std::path::PathBuf> = args
        .iter()
        .skip(1)
        .filter(|a| !a.starts_with('-'))
        .map(std::path::PathBuf::from)
        .filter(|p| p.is_file())
        .collect();

    eframe::run_native(
        "Blackline",
        options,
        Box::new(move |cc| {
            let mut app = app::App::new(cc);
            app.arrivals = files;
            Ok(Box::new(app))
        }),
    )
}
