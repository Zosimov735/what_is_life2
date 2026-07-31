//! The one error envelope, and the closed code set behind it.
//!
//! `docs/field-framework/ARCHITECTURE.md` locks a single error shape for
//! worker responses, persistence failures, content validation, and import
//! rejection: `{ code, message_key, detail }`, where `message_key` is a
//! copy-catalog key for a fault a player is shown and null for a
//! developer-only one, and `detail` is machine-readable and never shown.

use crate::json::{write_text, Obj};

/// The closed code set of version 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Code {
    Protocol,
    State,
    Validation,
    Impulse,
    Capacity,
    NotFound,
    SaveCorrupt,
    SaveVersion,
    ImportInvalid,
    ContentInvalid,
    WorkerRestart,
    Internal,
}

impl Code {
    /// The code as the envelope spells it.
    pub fn name(self) -> &'static str {
        match self {
            Code::Protocol => "protocol",
            Code::State => "state",
            Code::Validation => "validation",
            Code::Impulse => "impulse",
            Code::Capacity => "capacity",
            Code::NotFound => "not_found",
            Code::SaveCorrupt => "save_corrupt",
            Code::SaveVersion => "save_version",
            Code::ImportInvalid => "import_invalid",
            Code::ContentInvalid => "content_invalid",
            Code::WorkerRestart => "worker_restart",
            Code::Internal => "internal",
        }
    }

    /// The copy-catalog key the player sees for this code, where one is
    /// locked, and none for a developer-only code.
    pub fn message_key(self) -> Option<&'static str> {
        match self {
            Code::SaveCorrupt => Some("notice.anchor_recovered"),
            Code::SaveVersion => Some("notice.import_version"),
            Code::ImportInvalid => Some("notice.import_rejected"),
            Code::ContentInvalid => Some("notice.content_invalid"),
            Code::WorkerRestart | Code::Internal => Some("notice.run_resumed"),
            _ => None,
        }
    }
}

/// One error envelope, built and then written.
#[derive(Clone, Debug)]
pub struct Fault {
    code: Code,
    /// Already-canonical JSON, or none for a null detail.
    detail: Option<String>,
}

impl Fault {
    pub fn new(code: Code) -> Self {
        Fault { code, detail: None }
    }

    /// The common detail shape: one reason, machine-readable.
    pub fn because(code: Code, reason: &str) -> Self {
        let mut detail = String::new();
        let mut object = Obj::new(&mut detail);
        object.text("reason", reason);
        object.end();
        Fault { code, detail: Some(detail) }
    }

    /// A validation fault naming the field that failed.
    pub fn field(field: &str) -> Self {
        let mut detail = String::new();
        let mut object = Obj::new(&mut detail);
        object.text("field", field);
        object.end();
        Fault { code: Code::Validation, detail: Some(detail) }
    }

    /// A fault carrying an already-canonical detail object.
    pub fn detailed(code: Code, detail: String) -> Self {
        Fault { code, detail: Some(detail) }
    }

    pub fn code(&self) -> Code {
        self.code
    }

    /// The already-canonical detail object, where the fault carries one.
    pub fn detail(&self) -> Option<&str> {
        self.detail.as_deref()
    }

    /// The envelope, canonical: `{ code, detail, message_key }`.
    pub fn write(&self) -> String {
        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        object.text("code", self.code.name());
        match &self.detail {
            Some(detail) => object.raw("detail", detail),
            None => object.null("detail"),
        };
        match self.code.message_key() {
            Some(key) => object.text("message_key", key),
            None => object.null("message_key"),
        };
        object.end();
        out
    }

    /// The envelope as the whole of a failed response.
    pub fn response(&self) -> String {
        format!("{{\"error\":{},\"ok\":false}}", self.write())
    }
}

/// The `state` fault's detail: the states the command is valid in, and the one
/// the session is actually in.
pub fn state_fault(actual: &str, expected: &[&str]) -> Fault {
    let mut detail = String::new();
    let mut object = Obj::new(&mut detail);
    object.text("actual", actual);
    let mut states = object.list("expected");
    for name in expected {
        states.text(name);
    }
    states.end();
    object.end();
    Fault::detailed(Code::State, detail)
}

/// A successful response with an already-canonical body.
pub fn ok_response(body: &str) -> String {
    format!("{{\"body\":{body},\"ok\":true}}")
}

/// A string as a canonical JSON value, for details built by hand.
pub fn quoted(value: &str) -> String {
    let mut out = String::new();
    write_text(&mut out, value);
    out
}
