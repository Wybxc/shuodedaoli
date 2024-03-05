#![allow(clippy::suspicious_op_assign_impl)]

use std::ops::AddAssign;

use egui::Response;

pub struct Listerner {
    changed: bool,
}

impl Listerner {
    pub fn new() -> Self {
        Self { changed: false }
    }

    pub fn changed(&self) -> bool {
        self.changed
    }
}

impl AddAssign<bool> for Listerner {
    fn add_assign(&mut self, rhs: bool) {
        self.changed |= rhs;
    }
}

impl AddAssign<Response> for Listerner {
    fn add_assign(&mut self, rhs: Response) {
        self.changed |= rhs.changed();
    }
}
