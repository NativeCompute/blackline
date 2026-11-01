#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use blackline::{app, logo, splash, storage, theme};
use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

fn main() {
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
        return;
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
        return;
    }
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("blackline {}", env!("CARGO_PKG_VERSION"));
        return;
    }

    let small = storage::read_settings().splash;
    let size: [f32; 2] = if small {
        [splash::CARD.x, splash::CARD.y]
    } else {
        [1320.0, 900.0]
    };

    let viewport = eframe::egui::ViewportBuilder::default()
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
        .with_min_inner_size(if small { size } else { [880.0, 580.0] });

    let files: Vec<std::path::PathBuf> = args
        .iter()
        .skip(1)
        .filter(|a| !a.starts_with('-'))
        .map(std::path::PathBuf::from)
        .filter(|p| p.is_file())
        .collect();

    let started = Arc::new(AtomicBool::new(false));
    let mut failures = Vec::new();
    for renderer in renderers() {
        let options = eframe::NativeOptions {
            viewport: viewport.clone(),
            renderer,
            ..Default::default()
        };
        let files = files.clone();
        let flag = started.clone();
        let run = std::panic::catch_unwind(AssertUnwindSafe(|| {
            eframe::run_native(
                "Blackline",
                options,
                Box::new(move |cc| {
                    flag.store(true, Ordering::SeqCst);
                    let mut app = app::App::new(cc);
                    app.arrivals = files;
                    Ok(Box::new(app))
                }),
            )
        }));
        let failure = match run {
            Ok(Ok(())) => return,
            Ok(Err(e)) => e.to_string(),
            Err(p) => p
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_else(|| "unknown error".to_string()),
        };
        failures.push(format!("{renderer}: {failure}"));
        if started.load(Ordering::SeqCst) {
            break;
        }
    }
    report(&failures, started.load(Ordering::SeqCst));
}

fn renderers() -> Vec<eframe::Renderer> {
    #[cfg(windows)]
    return vec![eframe::Renderer::Glow, eframe::Renderer::Wgpu];
    #[cfg(not(windows))]
    return vec![eframe::Renderer::Glow];
}

fn report(failures: &[String], started: bool) {
    let what = if started {
        "Blackline stopped unexpectedly. Your scripts are saved as you type, so nothing should be lost."
    } else {
        "Blackline couldn't open its window. This usually means the graphics driver is missing or too old. Updating it usually fixes this; in a virtual machine, turn on 3D acceleration."
    };
    let detail = failures.join("\n");
    let root = storage::library_root();
    let log = root.join("error.log");
    let line = format!(
        "{} blackline {}\n{}\n\n",
        chrono::Local::now().to_rfc3339(),
        env!("CARGO_PKG_VERSION"),
        detail
    );
    let saved = std::fs::create_dir_all(&root).is_ok()
        && std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log)
            .and_then(|mut f| std::io::Write::write_all(&mut f, line.as_bytes()))
            .is_ok();
    let mut text = format!("{what}\n\n{detail}");
    if saved {
        text.push_str(&format!("\n\nThis was also written to {}", log.display()));
    }
    alert(&text);
}

#[cfg(windows)]
fn alert(text: &str) {
    #[link(name = "user32")]
    extern "system" {
        fn MessageBoxW(
            window: *mut std::ffi::c_void,
            text: *const u16,
            caption: *const u16,
            kind: u32,
        ) -> i32;
    }
    let wide = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
    let (text, caption) = (wide(text), wide("Blackline"));
    unsafe {
        MessageBoxW(std::ptr::null_mut(), text.as_ptr(), caption.as_ptr(), 0x10);
    }
}

#[cfg(not(windows))]
fn alert(text: &str) {
    eprintln!("{text}");
}
