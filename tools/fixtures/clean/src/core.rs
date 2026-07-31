//! Fixture: neutral identifiers in a core source file.

pub struct FormState {
    pub charge: f32,
    pub depth: u8,
}

impl FormState {
    /// Moves stored charge along a route and returns the remaining charge.
    pub fn transfer_charge(&mut self, amount: f32) -> f32 {
        self.charge = (self.charge - amount).max(0.0);
        self.charge
    }
}
