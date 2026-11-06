#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use blackline::{app, logo, smoke, storage, theme};
use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

fn emit(args: &[String], flag: &str, default: &str, bytes: impl FnOnce() -> Vec<u8>) -> bool {
    let Some(i) = args.iter().position(|a| a == flag) else {
        return false;
    };
    let path = args.get(i + 1).filter(|p| !p.starts_with('-')).cloned().unwrap_or_else(|| default.to_string());
    match std::fs::write(&path, bytes()) {
        Ok(()) => println!("wrote {path}"),
        Err(e) => {
            eprintln!("could not write {path}: {e}");
            std::process::exit(1);
        }
    }
    true
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if emit(&args, "--emit-icon", "blackline.svg", || {
        theme::set_palette(true);
        logo::svg().into_bytes()
    }) || emit(&args, "--emit-ico", "blackline.ico", logo::ico)
        || emit(&args, "--emit-icns", "Blackline.icns", logo::icns)
        || emit(&args, "--emit-info-plist", "Info.plist", || logo::info_plist().into_bytes())
    {
        return;
    }
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("blackline {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    if let Some(demo) = ncf::updater::demo_from_args(&args) {
        std::process::exit(ncf::updater::run_demo(blackline::INFO, demo));
    }
    if let Some(plan) = ncf::updater::from_args(&args, ncf::OS) {
        match plan {
            Ok(plan) => std::process::exit(ncf::updater::run(blackline::INFO, plan)),
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(2);
            }
        }
    }
    ncf::updater::tidy("blackline");

    let update_smoke = args.iter().position(|a| a == "--smoke-update").map(|i| {
        let version = args.get(i + 2).cloned().unwrap_or_default();
        if let Some(source) = args.get(i + 3) {
            std::env::set_var("NCF_UPDATE_SOURCE", source);
        }
        let dir = std::path::PathBuf::from(args.get(i + 1).cloned().unwrap_or_else(|| "blackline-smoke".to_string()));
        let dir = std::fs::create_dir_all(&dir).and_then(|_| dir.canonicalize()).unwrap_or(dir);
        std::env::set_var("NCF_UPDATE_SHOT", dir.join("updater.png"));
        (dir, version)
    });
    let smoke = args
        .iter()
        .position(|a| a == "--smoke")
        .map(|i| std::path::PathBuf::from(args.get(i + 1).cloned().unwrap_or_else(|| "blackline-smoke".to_string())))
        .or_else(|| update_smoke.as_ref().map(|(dir, _)| dir.clone()));
    let only = args
        .iter()
        .position(|a| a == "--renderer")
        .and_then(|i| args.get(i + 1))
        .map(|r| r.to_ascii_lowercase());
    if let Some(dir) = &smoke {
        smoke::catch_logs();
        if let Err(e) = smoke::prepare(dir) {
            eprintln!("smoke: {e}");
            std::process::exit(2);
        }
    }

    let wants_splash = storage::read_settings().splash;
    let (size, min) = ncf::app::first_size(wants_splash, ncf::OS);
    let builder = eframe::egui::ViewportBuilder::default()
        .with_title("Blackline")
        .with_icon(std::sync::Arc::new(eframe::egui::IconData {
            rgba: logo::rgba(128),
            width: 128,
            height: 128,
        }))
        .with_app_id("blackline")
        .with_drag_and_drop(true)
        .with_inner_size(size)
        .with_min_inner_size(min);
    let viewport = ncf::chrome::viewport(builder, ncf::OS);

    let files: Vec<std::path::PathBuf> = args
        .iter()
        .skip(1)
        .filter(|a| !a.starts_with('-'))
        .map(std::path::PathBuf::from)
        .filter(|p| p.is_file())
        .collect();

    let started = Arc::new(AtomicBool::new(false));
    let mut failures = Vec::new();
    let choices: Vec<eframe::Renderer> = renderers()
        .into_iter()
        .filter(|r| only.as_deref().map(|o| r.to_string().to_ascii_lowercase() == o).unwrap_or(true))
        .collect();
    for renderer in choices {
        let options = eframe::NativeOptions {
            viewport: viewport.clone(),
            renderer,
            centered: ncf::chrome::native_frame(ncf::OS),
            ..Default::default()
        };
        let files = files.clone();
        let flag = started.clone();
        let smoke_dir = smoke.clone();
        let update_version = update_smoke.as_ref().map(|(_, v)| v.clone());
        let run = std::panic::catch_unwind(AssertUnwindSafe(|| {
            eframe::run_native(
                "Blackline",
                options,
                Box::new(move |cc| {
                    flag.store(true, Ordering::SeqCst);
                    let mut app = app::App::new(cc);
                    app.arrivals = files;
                    match (smoke_dir, update_version) {
                        (Some(dir), Some(version)) => Ok(Box::new(smoke::UpdateSmoke::new(app, dir, version)) as Box<dyn eframe::App>),
                        (Some(dir), None) => Ok(Box::new(smoke::Smoke::new(app, dir, renderer.to_string())) as Box<dyn eframe::App>),
                        (None, _) => Ok(Box::new(app) as Box<dyn eframe::App>),
                    }
                }),
            )
        }));
        let failure = match run {
            Ok(Ok(())) => {
                if let (Some(dir), Some(_)) = (&smoke, &update_smoke) {
                    std::process::exit(smoke::finish_update(dir));
                }
                if let Some(dir) = &smoke {
                    std::process::exit(smoke::finish(dir));
                }
                return;
            }
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
    #[cfg(target_os = "macos")]
    return vec![eframe::Renderer::Wgpu, eframe::Renderer::Glow];
    #[cfg(not(any(windows, target_os = "macos")))]
    return vec![eframe::Renderer::Glow];
}

fn report(failures: &[String], started: bool) {
    let what = if started {
        "Blackline stopped unexpectedly. Your scripts are saved as you type, so nothing should be lost."
    } else {
        "Blackline couldn't open its window. This usually means the graphics driver is missing or too old. Updating it usually fixes this; in a virtual machine, turn on 3D acceleration."
    };
    let detail = failures.join("\n");
    let mut text = format!("{what}\n\n{detail}");
    if let Some(log) = storage::write_error_log(&detail) {
        text.push_str(&format!("\n\nThis was also written to {}", log.display()));
    }
    if let Some(dir) = std::env::args().position(|a| a == "--smoke").and_then(|i| std::env::args().nth(i + 1)) {
        let _ = std::fs::write(std::path::Path::new(&dir).join("report.txt"), format!("FAIL\n{text}\n"));
        eprintln!("{text}");
        std::process::exit(1);
    }
    ncf::platform::alert("Blackline", &text);
}
