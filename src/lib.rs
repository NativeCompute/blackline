#[cfg(test)]
mod tests;
#[cfg(test)]
mod teamtests;
#[cfg(test)]
mod uitests;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod cloudtests;

pub mod app;
pub mod backend;
pub mod cards;
pub mod chrome;
#[cfg(not(target_arch = "wasm32"))]
pub mod cloud;
pub mod editor;
pub mod export;
pub mod fountain;
pub mod keys;
pub mod logo;
pub mod model;
pub mod pages;
pub mod people;
pub mod report;
pub mod revision;
pub mod settings;
#[cfg(not(target_arch = "wasm32"))]
pub mod smoke;
pub mod splash;
pub mod storage;
pub mod theme;
pub mod ui;

pub use eframe;
pub use ncf::{alerts, anim, caret, icons, update};

pub const MARK: ncf::mark::AppMark = ncf::mark::AppMark {
    name: "Blackline",
    parts: &[ncf::mark::slab((0.27, 0.83), (0.49, 0.17)), ncf::mark::line((0.585, 0.83), (0.805, 0.17))],
};

pub const INFO: ncf::AppInfo = ncf::AppInfo {
    name: "Blackline",
    id: "blackline",
    version: env!("CARGO_PKG_VERSION"),
    tagline: "Screenwriting in industry format",
    repo: "NativeCompute/blackline",
    mark: MARK,
};

pub const NATIVECOMPUTE_URL: &str = ncf::NATIVECOMPUTE_URL;
pub const YOUTRACK_INFO_URL: &str = "https://www.jetbrains.com/youtrack/";
pub const MAC_INSTALL: &str = "curl -fsSL https://github.com/NativeCompute/blackline/releases/latest/download/install-macos.sh | bash";

pub const WEB: bool = cfg!(target_arch = "wasm32");
