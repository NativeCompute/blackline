pub use ncf::splash::{centre, settle, CARD};

pub struct Splash(ncf::splash::Splash);

impl Default for Splash {
    fn default() -> Self {
        Splash(ncf::splash::Splash::for_os(crate::INFO, ncf::OS))
    }
}

impl Splash {
    pub fn set_status(&mut self, status: crate::update::Status) {
        self.0.set_status(status);
    }

    pub fn is_inline(&self) -> bool {
        self.0.is_inline()
    }

    pub fn show(&mut self, ctx: &eframe::egui::Context) -> bool {
        self.0.show(ctx)
    }
}
