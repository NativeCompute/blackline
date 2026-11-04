pub use ncf::splash::{centre, CARD};

pub struct Splash(ncf::splash::Splash);

impl Default for Splash {
    fn default() -> Self {
        Splash(ncf::splash::Splash::new(crate::INFO))
    }
}

impl Splash {
    pub fn set_status(&mut self, status: crate::update::Status) {
        self.0.set_status(status);
    }

    pub fn show(&mut self, ctx: &eframe::egui::Context) -> bool {
        self.0.show(ctx)
    }
}
