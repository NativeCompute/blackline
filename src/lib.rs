#[cfg(test)]
mod tests;
#[cfg(test)]
mod teamtests;
#[cfg(test)]
mod uitests;

pub mod alerts;
pub mod anim;
pub mod app;
pub mod backend;
pub mod cards;
pub mod caret;
pub mod chrome;
pub mod editor;
pub mod export;
pub mod fountain;
pub mod icons;
pub mod keys;
pub mod logo;
pub mod model;
pub mod pages;
pub mod people;
pub mod settings;
pub mod splash;
pub mod storage;
pub mod theme;
pub mod ui;
pub mod update;

pub use eframe;

pub const NATIVECOMPUTE_URL: &str = "https://nativecompute.net";
pub const YOUTRACK_INFO_URL: &str = "https://www.jetbrains.com/youtrack/";

pub const WEB: bool = cfg!(target_arch = "wasm32");
