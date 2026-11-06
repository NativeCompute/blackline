pub use ncf::splash::{centre, settle, CARD};

pub struct Splash(ncf::splash::Splash);

impl Default for Splash {
    fn default() -> Self {
        Splash(ncf::splash::Splash::for_os(crate::INFO, ncf::OS))
    }
}

impl Splash {
    pub fn replay(status: crate::update::Status) -> Splash {
        let mut s = ncf::splash::Splash::inline(crate::INFO);
        if let crate::update::Status::Newer(r) = &status {
            s.set_offer(ncf::splash::Offer { version: r.version.clone(), can_update: false });
        }
        s.set_status(status);
        Splash(s)
    }

    pub fn set_status(&mut self, status: crate::update::Status) {
        self.0.set_status(status);
    }

    pub fn wants_offer(&self) -> bool {
        self.0.wants_offer()
    }

    pub fn set_offer(&mut self, offer: ncf::splash::Offer) {
        self.0.set_offer(offer);
    }

    pub fn take_choice(&mut self) -> Option<ncf::splash::Choice> {
        self.0.take_choice()
    }

    pub fn is_inline(&self) -> bool {
        self.0.is_inline()
    }

    pub fn show(&mut self, ctx: &eframe::egui::Context) -> bool {
        self.0.show(ctx)
    }
}
