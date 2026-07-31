//! The message layer of the core, in plain Rust.
//!
//! No binding type appears here, so `cargo test` exercises the same code the
//! browser runs. The command set is closed at nine and the error code set at
//! twelve, exactly as `docs/field-framework/ARCHITECTURE.md` locks them.
//!
//! What this answers is the whole command surface that stands before durable
//! persistence: the version handshake, `init_run` in both its modes,
//! `input_frame`, `export_run`, `import_run` with the locked import
//! validation, the two restores over the session's own record store, and the
//! three queued-change commands, which are valid only in `still` and answer
//! the `state` envelope everywhere else.
//!
//! The three queued-change commands are whole: `queue_plan` reads one entry of
//! the locked union and hands it to the run, which validates it against the
//! projection every earlier entry has been applied to; `undo_plan` takes the
//! most recent entry back, and on an empty queue succeeds and changes nothing;
//! and `commit_plan` is a transaction — all of the queue or none of it — which
//! an empty queue passes through, spending nothing and leaving Still Mode by
//! the mode table's own committed exit.

use crate::content::{self, Chapter, Content};
use crate::fault::{ok_response, state_fault, Code, Fault};
use crate::json::{canonicalize, hex_bytes, parse, Json, Obj};
use crate::plan::PlanCommand;
use crate::read;
use crate::records::RecordStore;
use crate::run::{Mode, Run};
use crate::sha256;
use crate::state::{
    auto_slot, RecordKind, RunState, EXPORT_FORMAT, SAVE_PAYLOAD_CAP, SAVE_VERSION,
};

/// The protocol version this build speaks. Version 1 everywhere in version 1.
pub const PROTOCOL_VERSION: u32 = 1;

/// The lifecycle state of a session that has not loaded a run.
const IDLE: &str = "idle";

/// The loaded lifecycle states, in mode-table order.
const LOADED: &[&str] = &[
    "running", "ramp_in", "still", "ramp_out", "suspended", "ended",
];

/// The nine commands of version 1, each with the lifecycle states it is valid
/// in. The set is closed: an unknown command is a `protocol` fault.
const COMMANDS: [(&str, &[&str]); 9] = [
    ("init_run", &["idle", "ended"]),
    ("input_frame", LOADED),
    ("queue_plan", &["still"]),
    ("undo_plan", &["still"]),
    ("commit_plan", &["still"]),
    ("restore_checkpoint", LOADED),
    ("recover_branch", LOADED),
    ("export_run", LOADED),
    ("import_run", &["idle", "ended"]),
];

/// A session over the WASM boundary: the state a `Core` instance holds.
pub struct Session {
    run: Option<Run>,
    store: RecordStore,
    /// The authored content the worker handed over at construction, or the
    /// fault reading it produced. The document puts content validation at
    /// `init_run`, so a bundle that does not read is held here and answered
    /// there rather than refusing the session outright.
    content: Result<Content, Fault>,
    /// Events the session itself raised, beside the run's own.
    events: Vec<String>,
}

impl Session {
    /// Completes the version handshake the worker opens.
    ///
    /// `init_json` carries the versions the worker speaks. Agreement on both
    /// is the whole of the handshake; a disagreement is returned as the
    /// `protocol` error envelope and the caller never gets a session.
    pub fn new(init_json: &str) -> Result<Self, String> {
        let spoken = parse(init_json).ok();
        let protocol = spoken.as_ref().and_then(|body| body.get("protocol")).and_then(Json::as_int);
        let save_version =
            spoken.as_ref().and_then(|body| body.get("save_version")).and_then(Json::as_int);
        if protocol != Some(i64::from(PROTOCOL_VERSION)) || save_version != Some(SAVE_VERSION) {
            let mut detail = String::new();
            let mut object = Obj::new(&mut detail);
            object.int("protocol", i64::from(PROTOCOL_VERSION));
            object.int("save_version", SAVE_VERSION);
            object.end();
            return Err(Fault::detailed(Code::Protocol, detail).write());
        }
        // The authored content arrives with the versions, in the one call the
        // locked WASM surface has for opening a core: the worker imports the
        // bytes statically and hands them across with the digest the build
        // embedded, and nothing is ever fetched.
        let content = match spoken.as_ref().and_then(|body| body.get("content")) {
            Some(value) => content::read_bundle(value),
            None => Err(Fault::because(Code::ContentInvalid, "content")),
        };
        Ok(Self { run: None, store: RecordStore::new(), content, events: Vec::new() })
    }

    /// The content hash this build reports, and the empty digest when no
    /// content read.
    fn content_hash(&self) -> String {
        match &self.content {
            Ok(content) => content.hash.clone(),
            Err(_) => hex_bytes(&sha256::digest(&[])),
        }
    }

    /// The chapter a loaded run stands on, and none when the run's own content
    /// hash is not this build's.
    ///
    /// A run restored under a different hash carries on — that is the locked
    /// behaviour, and `content_changed` says so — but the authored sequence
    /// does not run against a Field it was not authored for: the objective ids
    /// and the Nodes they name belong to the content the run was opened under.
    fn chapter(&self) -> Option<&Chapter> {
        let content = self.content.as_ref().ok()?;
        let run = self.run.as_ref()?;
        if run.state().content_hash != content.hash {
            return None;
        }
        content.chapter(run.state().progress.chapter_index)
    }

    /// The lifecycle state, as the state error reports it: the mode of the
    /// loaded run, or idle before one is loaded.
    pub fn lifecycle(&self) -> &'static str {
        self.run.as_ref().map_or(IDLE, |run| run.mode().name())
    }

    /// The completed-step counter of the loaded run, and 0 before one loads.
    pub fn step(&self) -> u32 {
        self.run.as_ref().map_or(0, Run::step)
    }

    /// Answers one command. The return value is the inner response — `ok` with
    /// a body, or `ok` false with an error envelope — and the worker wraps it
    /// in the message envelope with the correlation id.
    pub fn command(&mut self, kind: &str, body_json: &str) -> String {
        match self.answer(kind, body_json) {
            Ok(body) => ok_response(&body),
            Err(fault) => fault.response(),
        }
    }

    fn answer(&mut self, kind: &str, body_json: &str) -> Result<String, Fault> {
        let Some((_, valid_in)) = COMMANDS.iter().find(|(name, _)| *name == kind) else {
            let mut detail = String::new();
            let mut object = Obj::new(&mut detail);
            object.text("cmd", kind);
            object.end();
            return Err(Fault::detailed(Code::Protocol, detail));
        };

        let lifecycle = self.lifecycle();
        if !valid_in.contains(&lifecycle) {
            return Err(state_fault(lifecycle, valid_in));
        }

        let body = parse(body_json).map_err(|reason| Fault::because(Code::Validation, reason))?;
        if !body.is_map() {
            return Err(Fault::because(Code::Validation, "body_not_an_object"));
        }

        match kind {
            "init_run" => self.init_run(&body),
            "input_frame" => {
                // Disjoint fields: the campaign is read out of the content and
                // the run is advanced, and neither borrow reaches the other.
                // The whole campaign crosses rather than one chapter of it,
                // because a step can be the one that carries the run into the
                // next chapter and the step after it belongs to that one.
                let campaign = match (self.content.as_ref(), self.run.as_ref()) {
                    (Ok(content), Some(run)) if run.state().content_hash == content.hash => {
                        Some(content)
                    }
                    _ => None,
                };
                let run = self.run.as_mut().ok_or_else(|| state_fault(IDLE, LOADED))?;
                let ran = run.input_frame(&body, campaign)?;
                self.store_pending();
                self.drain_events();
                self.autosave();
                // No response to a valid frame crosses the boundary: the frame
                // event the worker raises is its acknowledgement. The count
                // here is the core's own answer to the worker, exact where the
                // event's `u8` saturates.
                let mut out = String::new();
                let mut object = Obj::new(&mut out);
                object.int("steps_run", i64::from(ran));
                object.end();
                Ok(out)
            }
            "export_run" => {
                read::exact_keys(&body, "body", &[])?;
                self.loaded()?.export()
            }
            "import_run" => self.import_run(&body),
            "restore_checkpoint" => self.restore_by_id(&body, false),
            "recover_branch" => self.restore_by_id(&body, true),
            "queue_plan" => self.queue_plan(&body),
            "undo_plan" => {
                read::exact_keys(&body, "body", &[])?;
                let run = self.loaded()?;
                // An empty queue succeeds and changes nothing; the shell reads
                // `remaining: 0` and leaves Still Mode on the second Escape.
                let remaining = run.undo_plan();
                let mut out = String::new();
                let mut object = Obj::new(&mut out);
                object.raw("queue", &run.queue().written(run.impulse()));
                object.int("remaining", remaining as i64);
                object.end();
                Ok(out)
            }
            "commit_plan" => {
                read::exact_keys(&body, "body", &[])?;
                let run = self.loaded()?;
                let applied = run.commit_plan()?;
                let mut out = String::new();
                let mut object = Obj::new(&mut out);
                object.int("applied", i64::from(applied));
                object.int("impulse", i64::from(run.impulse()));
                // The slate the run stands under once the commit has answered:
                // the one an applying commit just reassembled, the one already
                // standing when a commit applied nothing, and ordinal 0 for a
                // run standing under no slate at all. The `review_ready` the
                // commit raises carries the record itself, and rides after this
                // response as the locked event ordering has it.
                object.int(
                    "slate_ordinal",
                    run.standing_slate().map_or(0, |slate| i64::from(slate.ordinal)),
                );
                object.end();
                // The record the reassembly raised is drained here, after the
                // answer is built and before it is returned, so the caller
                // reads the response first and the `review_ready` after it.
                self.drain_events();
                Ok(out)
            }
            _ => Err(state_fault(lifecycle, valid_in)),
        }
    }

    /// Queues one proposed change.
    ///
    /// The body carries the tagged union and nothing else; the run holds the
    /// entry to the preconditions its variant declares, against the projection
    /// every earlier entry has been applied to, and to what the queue would
    /// then cost. A refused entry is not queued, and the queue the answer
    /// carries is the queue as it stands.
    fn queue_plan(&mut self, body: &Json) -> Result<String, Fault> {
        read::exact_keys(body, "body", &["plan"])?;
        let plan = PlanCommand::read(read::at(body, "plan")?)?;
        let run = self.loaded()?;
        run.queue_plan(plan)?;
        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        object.raw("queue", &run.queue().written(run.impulse()));
        object.end();
        Ok(out)
    }

    /// Opens a run. A new run takes its key from the shell — the only
    /// nondeterministic input a run ever takes — and a restore names a
    /// persistence record.
    fn init_run(&mut self, body: &Json) -> Result<String, Fault> {
        let mode = read::text(body, "mode")?;
        match mode {
            "new" => {
                read::exact_keys(body, "body", &["form", "mode", "run_id"])?;
                let run_id = read::text(body, "run_id")?;
                let form = read::text(body, "form")?;
                let content = self.content.as_ref().map_err(Fault::clone)?;
                // The run key and the Form id are the body's, so they are held
                // to the body's own envelope before the content is asked for
                // anything: a Form outside the closed set is a validation fault
                // whatever the content authors.
                let mut run = Run::start(run_id, form, &content.hash)?;
                let chapter = content
                    .chapter(0)
                    .ok_or_else(|| Fault::because(Code::ContentInvalid, "chapters"))?;
                let authored = content
                    .form(form)
                    .ok_or_else(|| Fault::because(Code::ContentInvalid, "forms"))?;
                let (field, view) = content::establish(chapter, authored)?;
                run.establish_field(field, view)
                    .map_err(|fault| read::recode(fault, Code::ContentInvalid))?;
                run.set_schedule(content.pressures.clone());
                run.open_chapter(chapter);
                run.open_schedule(chapter);
                let state = run.state();
                self.store.note_run(
                    &state.run_id,
                    form,
                    state.progress.chapter_index,
                    state.now.step,
                    state.branch_nonce,
                );
                let answer = opened_body(&run, false);
                self.run = Some(run);
                self.drain_events();
                Ok(answer)
            }
            "restore" => {
                read::exact_keys(body, "body", &["mode", "save_key"])?;
                let key = read::text(body, "save_key")?.to_string();
                let (state, changed) = self.recorded(&key)?;
                let form = self.store.row(&state.run_id).map_or(String::new(), |row| row.form.clone());
                let run = Run::restore(state, &form)?;
                self.note(&run);
                let answer = opened_body(&run, changed);
                self.run = Some(run);
                self.reopen_chapter();
                self.drain_events();
                Ok(answer)
            }
            _ => Err(Fault::field("mode")),
        }
    }

    /// Restores from a checkpoint the run's own metadata names.
    ///
    /// Quick Retry keeps the branch nonce and the recorded random state
    /// exactly. Branch Recovery restores the same physical state, then takes
    /// the nonce one past the largest the run has recorded and re-roots the
    /// trajectory stream, so stochastic readings legitimately re-draw. Both
    /// land in `running` with the plan queue cleared, and both apply the one
    /// post-restore normalization.
    fn restore_by_id(&mut self, body: &Json, branching: bool) -> Result<String, Fault> {
        read::exact_keys(body, "body", &["anchor_id"])?;
        let anchor_id = read::int(body, "anchor_id", 0, i64::from(u32::MAX))? as u32;
        let run = self.run.as_ref().ok_or_else(|| state_fault(IDLE, LOADED))?;
        let Some(anchor) = run.anchor(anchor_id) else {
            let mut detail = String::new();
            let mut object = Obj::new(&mut detail);
            object.int("anchor_id", i64::from(anchor_id));
            object.text("quantity", "anchor");
            object.end();
            return Err(Fault::detailed(Code::NotFound, detail));
        };
        let key = anchor.save_key.clone();
        let form = run.form().to_string();
        let of_run = run.state().run_id.clone();

        // A checkpoint of one run never restores another. The run key is inside
        // the `save_key` already, and one session holds one run, so this cannot
        // be reached today; it is checked rather than relied on, because the
        // metadata travels in a payload that an import can bring from anywhere.
        if self.store.record(&key).map(|record| record.run_id.as_str()) != Some(of_run.as_str()) {
            let mut detail = String::new();
            let mut object = Obj::new(&mut detail);
            object.text("quantity", "save_key");
            object.text("save_key", &key);
            object.end();
            return Err(Fault::detailed(Code::NotFound, detail));
        }

        let (state, _) = self.recorded(&key)?;
        let mut restored = Run::restore(state, &form)?;
        if branching {
            let nonce = self.store.nonce_high(restored.state().run_id.as_str()).saturating_add(1);
            restored.rebranch(nonce);
        }
        self.note(&restored);
        let answer = restored_body(&restored);
        self.run = Some(restored);
        // A restore lands the run on a step it has already been at, and the
        // objective that stood there is the one the record carried. The shell
        // learns which the same way it learns it on any other reopening — the
        // chapter's own events — because a restore that moved the run
        // backwards would otherwise leave the surface showing an objective the
        // run no longer stands on.
        self.reopen_chapter();
        self.drain_events();
        Ok(answer)
    }

    /// Raises the chapter's own events on a run that was reopened, so a fresh
    /// worker session tells the shell what it holds — and binds the authored
    /// pressure tables back onto it, because a record carries the staged list
    /// and never the content that drives it.
    ///
    /// A run whose content hash has moved gets neither the chapter nor the
    /// tables: `content_changed` already says its authored sequence is not
    /// trusted, and a staged pressure with no table reads as spent, leaves at
    /// the next step, and says so through the ordinary `pressure_changed`
    /// event rather than standing frozen forever.
    fn reopen_chapter(&mut self) {
        let Some(chapter) = self.chapter().cloned() else {
            return;
        };
        let schedule = self
            .content
            .as_ref()
            .map(|content| content.pressures.clone())
            .unwrap_or_default();
        if let Some(run) = self.run.as_mut() {
            run.set_schedule(schedule);
            run.open_chapter(&chapter);
            run.open_schedule(&chapter);
        }
    }

    /// Writes every record the authored sequence asked for, each with the
    /// payload taken at the step that asked for it.
    fn store_pending(&mut self) {
        let Some(run) = self.run.as_mut() else {
            return;
        };
        let written = run.take_records();
        if written.is_empty() {
            return;
        }
        let run_id = run.state().run_id.clone();
        for (key, kind, payload) in written {
            let digest = hex_bytes(&sha256::digest(payload.as_bytes()));
            self.store.write(key, &run_id, kind, payload, digest);
        }
        self.note_loaded();
    }

    /// Takes the events the loaded run raised into the session's own queue, in
    /// the order their causes occurred.
    fn drain_events(&mut self) {
        let Some(run) = self.run.as_mut() else {
            return;
        };
        let raised = run.take_events();
        self.events.extend(raised);
    }

    fn note_loaded(&mut self) {
        let Some(run) = self.run.as_ref() else {
            return;
        };
        let state = run.state();
        let noted = (
            state.run_id.clone(),
            run.form().to_string(),
            state.progress.chapter_index,
            state.now.step,
            state.branch_nonce,
        );
        self.store.note_run(&noted.0, &noted.1, noted.2, noted.3, noted.4);
    }

    /// Reads one stored record back, in the locked load order: parse; the save
    /// version at most this build's; re-serialize through the core and compare
    /// bytes and digest; then schema, enum, range, and cap validation. Any
    /// failure marks the record invalid.
    fn recorded(&self, key: &str) -> Result<(RunState, bool), Fault> {
        let Some(record) = self.store.record(key) else {
            let mut detail = String::new();
            let mut object = Obj::new(&mut detail);
            object.text("quantity", "save_key");
            object.text("save_key", key);
            object.end();
            return Err(Fault::detailed(Code::NotFound, detail));
        };
        if record.save_version > SAVE_VERSION {
            return Err(Fault::because(Code::SaveVersion, "record"));
        }
        read_payload(&record.payload, &record.payload_sha256, Code::SaveCorrupt, &self.content_hash())
    }

    /// Imports a run from an export file.
    ///
    /// The locked order: the file is canonical bytes or it is refused; `format`
    /// matches; `save_version` at most this build's, and above it the
    /// `save_version` envelope rather than a guess; the payload's digest is
    /// re-verified through the core; then schema and caps. The run then loads
    /// exactly — same branch nonce, same random state — and the imported
    /// payload is written into the older autosave slot.
    fn import_run(&mut self, body: &Json) -> Result<String, Fault> {
        read::exact_keys(body, "body", &["text"])?;
        let text = read::text(body, "text")?.to_string();
        // Canonicalization is implemented once, in the core, and an export file
        // is canonical bytes: parsing and re-emitting has to give back exactly
        // what arrived. Key order, spacing, a duplicate key, a float, and a
        // number written any other way all fail here.
        let rewritten = canonicalize(&text)
            .map_err(|reason| Fault::because(Code::ImportInvalid, reason))?;
        if rewritten != text {
            return Err(Fault::because(Code::ImportInvalid, "not_canonical"));
        }
        let file = parse(&text).map_err(|reason| Fault::because(Code::ImportInvalid, reason))?;
        read::exact_keys(&file, "text", &["format", "payload", "payload_sha256", "save_version"])
            .map_err(|fault| read::recode(fault, Code::ImportInvalid))?;
        let format = read::text(&file, "format")
            .map_err(|fault| read::recode(fault, Code::ImportInvalid))?;
        if format != EXPORT_FORMAT {
            return Err(Fault::because(Code::ImportInvalid, "format"));
        }
        let version = read::int(&file, "save_version", 0, i64::from(u32::MAX))
            .map_err(|fault| read::recode(fault, Code::ImportInvalid))?;
        if version > SAVE_VERSION {
            return Err(Fault::because(Code::SaveVersion, "import"));
        }
        if version != SAVE_VERSION {
            // The migration envelope dispatches on the version; version 1 is
            // identity and no earlier version exists to migrate from.
            return Err(Fault::because(Code::ImportInvalid, "save_version"));
        }
        let digest = read::hex(&file, "payload_sha256", 64)
            .map_err(|fault| read::recode(fault, Code::ImportInvalid))?
            .to_string();
        let payload = read::at(&file, "payload")
            .map_err(|fault| read::recode(fault, Code::ImportInvalid))?;
        let mut bytes = String::new();
        crate::json::write_value(&mut bytes, payload)
            .map_err(|reason| Fault::because(Code::ImportInvalid, reason))?;
        // The cap is stated over the payload, so the payload is what is
        // measured — the export file is that plus its fixed wrapper, and a
        // payload exactly at the cap arrives in a file a little larger.
        if bytes.len() > SAVE_PAYLOAD_CAP {
            return Err(crate::field::cap_fault("save_payload", SAVE_PAYLOAD_CAP as i64));
        }

        let (state, _) = read_payload(&bytes, &digest, Code::ImportInvalid, &self.content_hash())?;
        let form = state
            .now
            .forms
            .iter()
            .find(|held| held.controlled)
            .or_else(|| state.now.forms.first())
            .map_or(String::new(), |held| held.form.clone());
        let mut run = Run::restore(state, &form)?;
        let run_id = run.state().run_id.clone();

        // The imported payload lands under the derived autosave key for the
        // step it carries, and that key's metadata entry is rewritten in place
        // so it describes the payload now standing there. Nothing is appended:
        // an import adds no history the run did not have. What is stored is the
        // loaded run's own payload, so the bytes and the metadata beside them
        // always describe the same moment.
        let key = run.rewrite_checkpoint(RecordKind::Auto, auto_slot(run.step()));
        let stored = run.payload()?;
        let stored_digest = hex_bytes(&sha256::digest(stored.as_bytes()));
        self.note(&run);
        self.store.write(key, &run_id, RecordKind::Auto, stored, stored_digest);

        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        object.int("branch_nonce", i64::from(run.state().branch_nonce));
        object.text("run_id", &run.state().run_id);
        object.int("step", i64::from(run.state().now.step));
        object.end();
        self.run = Some(run);
        self.reopen_chapter();
        self.drain_events();
        Ok(out)
    }

    /// Writes the autosave record the cadence is due, if one is: one `auto`
    /// record every 900 completed steps. The metadata rides in the payload, so
    /// it is noted before the bytes are taken.
    fn autosave(&mut self) {
        let Some(run) = self.run.as_ref() else {
            return;
        };
        let due = run.autosave_intervals();
        if due == 0 || due <= self.last_auto_interval() {
            return;
        }
        // Never inside Still Mode. A due autosave runs at the exit, which is
        // what the next frame after the ramp completes is; no run reaches
        // `still` yet, so this guard stands unexercised until the goal that
        // owns Still Mode makes it reachable.
        if run.mode() == Mode::Still {
            return;
        }
        let run_id = run.state().run_id.clone();
        let slot = auto_slot(run.step());

        // A payload past its cap is refused rather than stored, and the
        // metadata note is rolled back with it so the two never disagree.
        let written = {
            let run = self.run.as_mut().expect("a run is loaded");
            let held = run.state().anchors.clone();
            let key = run.note_checkpoint(RecordKind::Auto, slot);
            match run.payload() {
                Ok(payload) => Some((key, payload)),
                Err(_) => {
                    run.set_anchors(held);
                    None
                }
            }
        };
        let Some((key, payload)) = written else {
            return;
        };
        let digest = hex_bytes(&sha256::digest(payload.as_bytes()));
        self.store.write(key, &run_id, RecordKind::Auto, payload, digest);
        let noted = {
            let run = self.run.as_ref().expect("a run is loaded");
            let state = run.state();
            (
                run.form().to_string(),
                state.progress.chapter_index,
                state.now.step,
                state.branch_nonce,
            )
        };
        self.store.note_run(&run_id, &noted.0, noted.1, noted.2, noted.3);
    }

    /// The autosave interval the newest `auto` record of the loaded run stands
    /// at, and 0 when none stands.
    fn last_auto_interval(&self) -> u32 {
        let Some(run) = self.run.as_ref() else {
            return 0;
        };
        run.state()
            .anchors
            .iter()
            .filter(|anchor| anchor.kind == RecordKind::Auto)
            .map(|anchor| anchor.step / crate::state::AUTOSAVE_STEPS)
            .max()
            .unwrap_or(0)
    }

    /// Refreshes the run's metadata row, which is what raises `nonce_high`.
    fn note(&mut self, run: &Run) {
        let state = run.state();
        self.store.note_run(
            &state.run_id,
            run.form(),
            state.progress.chapter_index,
            state.now.step,
            state.branch_nonce,
        );
    }

    fn loaded(&mut self) -> Result<&mut Run, Fault> {
        self.run.as_mut().ok_or_else(|| state_fault(IDLE, LOADED))
    }

    /// The loaded run, for a caller that reads its state directly rather than
    /// through a command.
    pub fn run(&self) -> Option<&Run> {
        self.run.as_ref()
    }

    /// The session's records, for a caller reading the store directly.
    pub fn store(&self) -> &RecordStore {
        &self.store
    }

    /// The render snapshot for the most recent step, as locked bytes. A
    /// session holding no run has no frame to hand out.
    pub fn frame_view(&self) -> Vec<u8> {
        self.run.as_ref().map(Run::frame_view).unwrap_or_default()
    }

    /// The events raised since the last call, canonical JSON, in the order
    /// their causes occurred. The campaign raises four of the seven —
    /// `objective_changed`, `checkpoint_written`, `chapter_changed`, and
    /// `run_completed` — the pressure system raises `pressure_changed`, the
    /// slate, Echo, and inspections raise `review_ready`, and the frame event
    /// is the worker's own.
    pub fn take_events(&mut self) -> String {
        let raised = std::mem::take(&mut self.events);
        format!("[{}]", raised.join(","))
    }
}

/// Reads one save payload back and holds it to the locked load order's last
/// two steps: the digest of exactly these bytes, then schema and caps. The
/// caller names the envelope a failure answers with.
fn read_payload(
    bytes: &str,
    digest: &str,
    code: Code,
    hash: &str,
) -> Result<(RunState, bool), Fault> {
    if hex_bytes(&sha256::digest(bytes.as_bytes())) != digest {
        return Err(Fault::because(code, "payload_sha256"));
    }
    let payload = parse(bytes).map_err(|reason| Fault::because(code, reason))?;
    let state = RunState::read(&payload).map_err(|fault| read::recode(fault, code))?;
    state.coherent().map_err(|fault| read::recode(fault, code))?;
    // A restore under a different content hash continues, and says so; the
    // framework reproducibility of pre-restore records is no longer claimed.
    let changed = state.content_hash != hash;
    Ok((state, changed))
}

/// The answer `init_run` returns once a run is open.
fn opened_body(run: &Run, content_changed: bool) -> String {
    let state = run.state();
    let mut out = String::new();
    let mut object = Obj::new(&mut out);
    object.int("branch_nonce", i64::from(state.branch_nonce));
    object.int("chapter_index", i64::from(state.progress.chapter_index));
    object.bool("content_changed", content_changed);
    object.text("content_hash", &state.content_hash);
    object.int("protocol", i64::from(PROTOCOL_VERSION));
    object.text("run_id", &state.run_id);
    object.int("save_version", SAVE_VERSION);
    object.int("step", i64::from(state.now.step));
    object.raw("view", &state.view.written());
    object.end();
    out
}

/// The answer both restores return.
fn restored_body(run: &Run) -> String {
    let state = run.state();
    let mut out = String::new();
    let mut object = Obj::new(&mut out);
    object.int("branch_nonce", i64::from(state.branch_nonce));
    object.int("step", i64::from(state.now.step));
    object.raw("view", &state.view.written());
    object.end();
    out
}
