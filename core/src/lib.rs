//! `field_game_core` — the authoritative simulation core.
//!
//! The WASM surface is the one exported type `Core` with the four methods
//! `docs/field-framework/ARCHITECTURE.md` locks. Each is a thin adapter over
//! [`protocol::Session`], which is plain Rust so the native test run and the
//! browser run exercise the same code.
//!
//! The crate imports nothing outside itself: no system randomness, no clock,
//! and no floating point in state or step logic. A run's whole
//! nondeterministic input is the run key the shell hands it.

pub mod content;
pub mod criterion;
pub mod analysis;
pub mod coord;
pub mod engineering;
pub mod fault;
pub mod field;
pub mod field_inspect;
pub mod frame;
pub mod fx;
pub mod instrument;
pub mod json;
pub mod perturb;
pub mod plan;
pub mod policy;
pub mod pressure;
pub mod protocol;
pub mod rank;
pub mod read;
pub mod records;
pub mod rng;
pub mod run;
pub mod sha256;
pub mod slate;
pub mod state;

use wasm_bindgen::prelude::*;

pub use protocol::{Session, PROTOCOL_VERSION};
pub use state::SAVE_VERSION;

/// The core, as the worker holds it.
#[wasm_bindgen]
pub struct Core {
    session: Session,
}

#[wasm_bindgen]
impl Core {
    /// Opens a core. `init_json` carries the versions the worker speaks; a
    /// disagreement throws the `protocol` error envelope and yields no core.
    #[wasm_bindgen(constructor)]
    pub fn new(init_json: &str) -> Result<Core, JsError> {
        match Session::new(init_json) {
            Ok(session) => Ok(Core { session }),
            Err(envelope) => Err(JsError::new(&envelope)),
        }
    }

    /// Answers one command, returning the response as canonical JSON.
    pub fn command(&mut self, kind: &str, body_json: &str) -> String {
        self.session.command(kind, body_json)
    }

    /// The render snapshot for the most recent step, as locked bytes.
    pub fn frame_view(&self) -> Vec<u8> {
        self.session.frame_view()
    }

    /// The events raised since the previous call, as canonical JSON.
    pub fn take_events(&mut self) -> String {
        self.session.take_events()
    }
}
