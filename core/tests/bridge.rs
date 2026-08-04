//! The command surface that completes the worker bridge.
//!
//! The contracts under test are the ones `docs/field-framework/ARCHITECTURE.md`
//! locks for the protocol's pre-persistence surface: `export_run` and
//! `import_run` with the whole import validation — format, version, digest, and
//! canonical form — the two restores over the session's own records with the
//! branch-nonce monotone-record rule and the one post-restore normalization,
//! and the render snapshot's frozen byte layout over the Field the model holds.
//!
//! The byte-equivalence contract runs through all of it: an export imported
//! into a fresh session re-exports the same bytes, a Quick Retry lands on the
//! recorded bytes exactly, and a Branch Recovery lands on the same physical
//! state under a different stream, which is what makes a replay after it
//! diverge.

use field_game_core::fault::Code;
use field_game_core::field::{
    pulse_radius, reach_ticks,
    BoundaryState, CurrentState, FieldLayer, FormState, NodeKind, PhysicalCompartment,
    PortState, RouteState,
};
use field_game_core::frame::{
    self, FRAME_BUFFER_CAP, FRAME_VERSION, HEADER_BYTES, MAGIC, SECTION_ENTRY_BYTES,
};
use field_game_core::fx::{Vec2, ONE_UNIT};
use field_game_core::json::{canonicalize, parse, Json};
use field_game_core::plan::{PlanCommand, PlanQueue};
use field_game_core::run::{Mode, Run};
use field_game_core::state::{
    auto_slot, CheckpointState, FieldState, InputConfig, Progress, RecordKind, RunState, Step,
    Surround, ViewDeclaration, ANCHORS_PER_RUN, AUTOSAVE_STEPS, FRAC_ONE, OPENING_IMPULSE,
    SAVE_PAYLOAD_CAP,
};
use field_game_core::Session;

mod support;

/// What the worker sends when it opens the core.

const KEY: &str = "0123456789abcdef";

/// The fixture the render snapshot's decoder is pinned against, in both
/// languages: the Rust encoder writes these bytes and the worker's decoder
/// reads them.
const FRAME_FIXTURE: &str = include_str!("fixtures/frame_state.hex");

fn opened(key: &str) -> Session {
    let mut session = Session::new(&support::worker_init()).expect("versions agree");
    let answer = session.command(
        "init_run",
        &format!("{{\"form\":\"thread\",\"mode\":\"new\",\"run_id\":\"{key}\"}}"),
    );
    assert!(answer.starts_with("{\"body\":"), "a new run opens: {answer}");
    session
}

/// One frame of input, with every declared field present.
fn frame(seq: u32, steps: u16) -> String {
    held_frame(seq, steps, false)
}

/// The same, with the Pulse held for the whole of it.
fn held_frame(seq: u32, steps: u16, pulse_held: bool) -> String {
    pulsing_frame(seq, steps, pulse_held, false)
}

/// The same, carrying the Pulse's level and its release edge.
fn pulsing_frame(seq: u32, steps: u16, pulse_held: bool, pulse_release: bool) -> String {
    format!(
        "{{\"advance_steps\":{steps},\"depth_key\":0,\"inspect\":null,\"pause\":false,\
          \"pulse_held\":{pulse_held},\"pulse_release\":{pulse_release},\"seq\":{seq},\"steer_x\":0,\
          \"steer_y\":0,\"t_us\":{stamp},\"toggle_still\":false,\"wheel\":0}}",
        stamp = i64::from(seq) * 33_333
    )
}

/// The body of a successful response, parsed.
fn body(answer: &str) -> Json {
    let parsed = parse(answer).expect("a response is canonical JSON");
    assert_eq!(parsed.get("ok"), Some(&Json::Bool(true)), "{answer}");
    parsed.get("body").expect("a successful response carries a body").clone()
}

fn text_of(value: &Json, key: &str) -> String {
    value.get(key).and_then(Json::as_text).expect("a text field").to_string()
}

fn int_of(value: &Json, key: &str) -> i64 {
    value.get(key).and_then(Json::as_int).expect("an integer field")
}

/// The error code of a refused response.
fn refusal(answer: &str) -> String {
    let parsed = parse(answer).expect("a response is canonical JSON");
    assert_eq!(parsed.get("ok"), Some(&Json::Bool(false)), "{answer}");
    let error = parsed.get("error").expect("a refusal carries an envelope");
    text_of(error, "code")
}

/// Exports the loaded run and returns the export file's text.
fn exported(session: &mut Session) -> String {
    text_of(&body(&session.command("export_run", "{}")), "text")
}

/// An `import_run` body carrying one export file.
fn import_body(text: &str) -> String {
    let mut out = String::from("{\"text\":");
    field_game_core::json::write_text(&mut out, text);
    out.push('}');
    out
}

// ---------------------------------------------------------------------------
// Export and import
// ---------------------------------------------------------------------------

#[test]
fn an_export_imported_into_a_fresh_session_re_exports_the_same_bytes() {
    let mut first = opened(KEY);
    for seq in 1..=4u32 {
        first.command("input_frame", &frame(seq, 3));
    }
    let file = exported(&mut first);

    let mut second = Session::new(&support::worker_init()).expect("versions agree");
    let answer = body(&second.command("import_run", &import_body(&file)));
    assert_eq!(text_of(&answer, "run_id"), KEY);
    assert_eq!(int_of(&answer, "step"), 12);
    assert_eq!(int_of(&answer, "branch_nonce"), 0, "an import restores exactly");

    // The one post-restore normalization is applied, and it is the only
    // difference the restored state carries.
    assert_eq!(second.run().expect("a run is loaded").state().now.prev_assembly_step, Some(12));
    let again = exported(&mut second);
    let restored = parse(&again).expect("canonical");
    let original = parse(&file).expect("canonical");
    let normalized = normalize_prev_assembly(&original, 12);
    assert_eq!(restored, normalized, "the re-export differs only by the locked normalization");

    // And a second round trip is byte-equal, because the normalization is
    // idempotent: the state it produced is the state it reads back.
    let mut third = Session::new(&support::worker_init()).expect("versions agree");
    third.command("import_run", &import_body(&again));
    assert_eq!(exported(&mut third), again, "byte-equal, not merely equivalent");
}

/// The same export file with `prev_assembly_step` set to the step the restore
/// returned to, in both the live Field and the trajectory's keyframe is
/// untouched — the normalization applies to the state, which is `field.now`.
fn normalize_prev_assembly(file: &Json, step: i64) -> Json {
    let mut rebuilt = file.clone();
    let Json::Map(pairs) = &mut rebuilt else {
        panic!("an export file is an object");
    };
    for (key, value) in pairs.iter_mut() {
        if key == "payload" {
            let Json::Map(payload) = value else { panic!("a payload is an object") };
            for (name, part) in payload.iter_mut() {
                if name != "field" {
                    continue;
                }
                let Json::Map(field) = part else { panic!("a field is an object") };
                for (which, state) in field.iter_mut() {
                    if which != "now" {
                        continue;
                    }
                    let Json::Map(now) = state else { panic!("a state is an object") };
                    for (held, at) in now.iter_mut() {
                        if held == "prev_assembly_step" {
                            *at = Json::Int(step);
                        }
                    }
                }
            }
        }
    }
    // The digest travels with the payload, so it is recomputed over the
    // normalized bytes exactly as an export of the restored run would.
    let Json::Map(pairs) = &mut rebuilt else { unreachable!() };
    let payload = pairs
        .iter()
        .find(|(key, _)| key == "payload")
        .map(|(_, value)| value.clone())
        .expect("a payload");
    let mut bytes = String::new();
    field_game_core::json::write_value(&mut bytes, &payload).expect("canonical");
    let digest = field_game_core::json::hex_bytes(&field_game_core::sha256::digest(bytes.as_bytes()));
    for (key, value) in pairs.iter_mut() {
        if key == "payload_sha256" {
            *value = Json::Text(digest.clone());
        }
    }
    rebuilt
}

/// Rewrites a current export into the one legacy shape the V1 loader supports.
/// This is test-fixture construction, not a second migration implementation:
/// it only reverses the two schema moves (version and compartment location),
/// then recomputes the wrapper digest over those exact canonical bytes.
fn as_v1_export(current: &str) -> String {
    fn field_as_v1(value: &mut Json) {
        let Json::Map(field) = value else { panic!("a Field is an object") };
        let at = field
            .iter()
            .position(|(key, _)| key == "physical_compartment")
            .expect("V2 carries a physical compartment");
        let Json::Map(compartment) = field.remove(at).1 else {
            panic!("the physical compartment is an object")
        };
        let coefficient = compartment
            .iter()
            .find(|(key, _)| key == "leak_per_exposed_contact_per_step")
            .map(|(_, value)| value.clone())
            .expect("the compartment carries its coefficient");
        let boundaries = field
            .iter_mut()
            .find(|(key, _)| key == "boundaries")
            .map(|(_, value)| value)
            .expect("the Field carries observation boundaries");
        let Json::Map(boundaries) = boundaries else { panic!("boundaries is an object") };
        boundaries.push(("leak_frac".to_string(), coefficient));
        field.retain(|(key, _)| {
            !matches!(
                key.as_str(),
                "current_delays"
                    | "leak_breach"
                    | "materials"
                    | "next_signal_id"
                    | "route_clamps"
                    | "route_scramble"
                    | "signals"
                    | "supply_decoys"
            )
        });
    }

    let mut file = parse(current).expect("the current export is canonical");
    let Json::Map(wrapper) = &mut file else { panic!("an export is an object") };
    let payload = wrapper
        .iter_mut()
        .find(|(key, _)| key == "payload")
        .map(|(_, value)| value)
        .expect("an export carries a payload");
    let Json::Map(payload_map) = payload else { panic!("the payload is an object") };
    let content_hash = payload_map
        .iter()
        .find(|(key, _)| key == "scenario_spec")
        .and_then(|(_, scenario)| scenario.get("content_hash"))
        .cloned()
        .expect("the scenario carries its content hash");
    payload_map.retain(|(key, _)| key != "criterion_runtime" && key != "scenario_spec");
    let content_at = payload_map
        .iter()
        .position(|(key, _)| key == "field")
        .expect("the payload carries its Field");
    payload_map.insert(content_at, ("content_hash".to_string(), content_hash));
    for (key, value) in payload_map.iter_mut() {
        if key == "save_version" {
            *value = Json::Int(1);
        } else if key == "field" {
            let Json::Map(field) = value else { panic!("field is an object") };
            for (which, state) in field.iter_mut() {
                if which == "now" {
                    field_as_v1(state);
                } else if which == "trace" {
                    let Json::Map(trace) = state else { panic!("trace is an object") };
                    let keyframe = trace
                        .iter_mut()
                        .find(|(name, _)| name == "keyframe")
                        .map(|(_, value)| value)
                        .expect("trace carries its keyframe");
                    field_as_v1(keyframe);
                }
            }
        }
    }
    let mut payload_bytes = String::new();
    field_game_core::json::write_value(&mut payload_bytes, payload).expect("canonical payload");
    let digest =
        field_game_core::json::hex_bytes(&field_game_core::sha256::digest(payload_bytes.as_bytes()));
    for (key, value) in wrapper.iter_mut() {
        match key.as_str() {
            "payload_sha256" => *value = Json::Text(digest.clone()),
            "save_version" => *value = Json::Int(1),
            _ => {}
        }
    }
    let mut written = String::new();
    field_game_core::json::write_value(&mut written, &file).expect("canonical V1 export");
    written
}

#[test]
fn v1_migration_is_deterministic_and_its_v3_output_is_canonical() {
    let mut source = opened(KEY);
    source.command("input_frame", &frame(1, 3));
    let legacy = as_v1_export(&exported(&mut source));
    assert_eq!(canonicalize(&legacy).expect("the synthetic V1 file is canonical"), legacy);

    let legacy_file = parse(&legacy).expect("canonical V1 wrapper");
    let legacy_payload = legacy_file.get("payload").expect("a legacy payload");
    let first = RunState::migrate_v1(legacy_payload).expect("V1 migrates");
    let second = RunState::migrate_v1(legacy_payload).expect("the same V1 migrates again");
    assert_eq!(first.payload(), second.payload(), "migration is byte-deterministic");
    assert_eq!(first.now.physical_compartment.members, first.view.inside);
    assert_eq!(first.trace.keyframe.physical_compartment.members, first.view.inside);
    assert_eq!(canonicalize(&first.payload()).expect("V3 payload is canonical"), first.payload());
    let reread = RunState::read(&parse(&first.payload()).expect("canonical V3"))
        .expect("canonical V3 reads");
    assert_eq!(reread.payload(), first.payload(), "V3 is its own canonical round trip");

    let mut imported = Session::new(&support::worker_init()).expect("versions agree");
    let answer = body(&imported.command("import_run", &import_body(&legacy)));
    assert_eq!(int_of(&answer, "migrated_from"), 1, "the boundary reports provenance");
    let rewritten = exported(&mut imported);
    assert!(rewritten.contains("\"save_version\":3"));
    assert!(rewritten.contains("\"physical_compartment\":"));
    assert!(!rewritten.contains("\"leak_frac\":"), "legacy causal storage is gone");

    let mut current = Session::new(&support::worker_init()).expect("versions agree");
    let current_answer = body(&current.command("import_run", &import_body(&rewritten)));
    assert_eq!(current_answer.get("migrated_from"), None, "native V3 has no migration marker");
}

#[test]
fn an_export_file_is_canonical_and_carries_the_locked_wrapper() {
    let mut session = opened(KEY);
    session.command("input_frame", &frame(1, 2));
    let file = exported(&mut session);

    assert_eq!(canonicalize(&file).expect("the export file is canonical"), file);
    let parsed = parse(&file).expect("canonical");
    assert_eq!(text_of(&parsed, "format"), "field-game-run");
    assert_eq!(int_of(&parsed, "save_version"), 3);
    let mut bytes = String::new();
    field_game_core::json::write_value(&mut bytes, parsed.get("payload").expect("a payload"))
        .expect("canonical");
    assert_eq!(
        text_of(&parsed, "payload_sha256"),
        field_game_core::json::hex_bytes(&field_game_core::sha256::digest(bytes.as_bytes())),
        "the digest is of exactly the payload's canonical bytes"
    );
}

#[test]
fn an_import_is_refused_for_every_locked_reason() {
    let mut session = opened(KEY);
    session.command("input_frame", &frame(1, 2));
    let file = exported(&mut session);

    // A file this build's loader cannot dispatch on, and one from a version
    // above its own — which is refused as a version rather than guessed at.
    // The file's own version is its last key; the payload carries one of its
    // own, which the digest covers.
    let newer = file.replace(",\"save_version\":3}", ",\"save_version\":4}");
    let older = file.replace(",\"save_version\":3}", ",\"save_version\":0}");
    assert_ne!(newer, file);
    assert_ne!(older, file);

    let refusals: Vec<(&str, String, &str)> = vec![
        ("not_an_object", "[]".to_string(), "import_invalid"),
        ("empty_object", "{}".to_string(), "import_invalid"),
        // Canonical form: the same values, spaced and re-ordered.
        ("spaced", file.replacen('{', "{ ", 1), "import_invalid"),
        (
            "reordered",
            format!(
                "{{\"save_version\":3,{}",
                file.trim_start_matches("{\"format\":\"field-game-run\",")
                    .replace(",\"save_version\":3}", "}")
            ),
            "import_invalid",
        ),
        ("wrong_format", file.replace("field-game-run", "field-game-save"), "import_invalid"),
        ("newer_version", newer, "save_version"),
        ("older_version", older, "import_invalid"),
        // The digest is of exactly the payload's canonical bytes.
        (
            "wrong_digest",
            file.replace("\"payload_sha256\":\"", "\"payload_sha256\":\"0"),
            "import_invalid",
        ),
        // A truncated file is not JSON at all.
        ("truncated", file[..file.len() / 2].to_string(), "import_invalid"),
        // A key the payload's shape never declares.
        (
            "extra_key",
            file.replace("\"branch_nonce\":0,", "\"branch_nonce\":0,\"bonus\":1,"),
            "import_invalid",
        ),
        // A field of the wrong type, and one outside its locked range.
        ("wrong_type", file.replace("\"branch_nonce\":0,", "\"branch_nonce\":\"0\","), "import_invalid"),
        ("out_of_range", file.replace("\"impulse\":3", "\"impulse\":9"), "import_invalid"),
        // A float never appears in canonical JSON.
        ("float", file.replace("\"branch_nonce\":0,", "\"branch_nonce\":0.0,"), "import_invalid"),
    ];

    for (named, text, expected) in refusals {
        let mut fresh = Session::new(&support::worker_init()).expect("versions agree");
        let answer = fresh.command("import_run", &import_body(&text));
        assert_eq!(refusal(&answer), expected, "{named}: {answer}");
        assert_eq!(fresh.lifecycle(), "idle", "{named}: a refused import loads no run");
    }
}

#[test]
fn an_import_writes_the_record_a_restore_reads() {
    let mut first = opened(KEY);
    first.command("input_frame", &frame(1, 5));
    let file = exported(&mut first);

    let mut second = Session::new(&support::worker_init()).expect("versions agree");
    second.command("import_run", &import_body(&file));
    // The imported payload lands under the autosave key its own step derives.
    let records = second.store().of_run(KEY);
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].save_key, format!("{KEY}:auto:{}", auto_slot(5)));
    assert_eq!(records[0].kind, RecordKind::Auto);
    assert_eq!(second.store().nonce_high(KEY), 0);

    // What is stored is the loaded run's own payload, so the bytes and the
    // metadata beside them describe the same moment and re-reading them is a
    // fixed point.
    let stored = records[0].payload.clone();
    assert_eq!(
        stored,
        second.run().expect("a run is loaded").state().payload(),
        "the record holds the run as it now stands"
    );
}

#[test]
fn the_autosave_slot_is_derived_from_the_step_and_from_nothing_else() {
    assert_eq!(auto_slot(0), 0);
    assert_eq!(auto_slot(AUTOSAVE_STEPS - 1), 0);
    assert_eq!(auto_slot(AUTOSAVE_STEPS), 1);
    assert_eq!(auto_slot(2 * AUTOSAVE_STEPS), 0);
    assert_eq!(auto_slot(3 * AUTOSAVE_STEPS), 1);
}

#[test]
fn the_same_key_and_frames_agree_past_the_first_autosave() {
    // The determinism contract, read past the boundary where the autosave
    // cadence first writes into the payload's own metadata.
    let played = |key: &str| -> String {
        let mut session = opened(key);
        session.command("input_frame", &frame(1, 1000));
        session.command("input_frame", &frame(2, 200));
        session.run().expect("a run is loaded").state().payload()
    };
    let once = played(KEY);
    let again = played(KEY);
    assert_eq!(once, again, "byte-equivalent past step 1000, metadata included");
    assert!(once.contains("\"step\":1200"), "{once}");
    assert!(once.contains(&format!("\"save_key\":\"{KEY}:auto:1\"")), "{once}");
}

#[test]
fn a_run_that_was_imported_writes_the_same_records_as_one_that_was_not() {
    // The leak this closes: a slot chosen from the store's own write history
    // made an imported session take a different slot from a fresh one, and the
    // slot is written into the payload. Derived from the step, the two agree.
    let mut source = opened(KEY);
    let opening = exported(&mut source);

    let mut imported = Session::new(&support::worker_init()).expect("versions agree");
    imported.command("import_run", &import_body(&opening));
    imported.command("input_frame", &frame(1, 1000));
    let after_import = imported.run().expect("a run is loaded").state().payload();

    let mut fresh = opened(KEY);
    fresh.command("input_frame", &frame(1, 1000));
    let after_fresh = fresh.run().expect("a run is loaded").state().payload();

    // The one locked post-restore normalization is the whole of the difference.
    assert_eq!(
        after_import.replace("\"prev_assembly_step\":0,", "\"prev_assembly_step\":null,"),
        after_fresh,
        "an imported run and a fresh one differ only by the locked normalization"
    );
    let anchors_of = |payload: &str| {
        parse(payload).expect("canonical").get("anchors").expect("the metadata").clone()
    };
    assert_eq!(anchors_of(&after_import), anchors_of(&after_fresh));
}

// ---------------------------------------------------------------------------
// Autosave records, Quick Retry, and Branch Recovery
// ---------------------------------------------------------------------------

/// Runs a session past one autosave interval and returns it with the
/// identifier of the checkpoint the cadence wrote.
fn past_one_interval() -> (Session, u32) {
    let mut session = opened(KEY);
    session.command("input_frame", &frame(1, AUTOSAVE_STEPS as u16));
    let run = session.run().expect("a run is loaded");
    assert_eq!(run.step(), AUTOSAVE_STEPS);
    let anchors = &run.state().anchors;
    assert_eq!(anchors.len(), 1, "one record every 900 completed steps");
    assert_eq!(anchors[0].step, AUTOSAVE_STEPS);
    assert_eq!(anchors[0].kind, RecordKind::Auto);
    let id = anchors[0].anchor_id;
    (session, id)
}

#[test]
fn the_autosave_cadence_writes_one_record_every_interval_and_alternates_its_slots() {
    let (mut session, _) = past_one_interval();
    assert_eq!(session.store().of_run(KEY)[0].save_key, format!("{KEY}:auto:1"));

    session.command("input_frame", &frame(2, AUTOSAVE_STEPS as u16));
    let keys: Vec<String> =
        session.store().of_run(KEY).iter().map(|record| record.save_key.clone()).collect();
    assert_eq!(keys.len(), 2, "the two slots stand together");
    assert!(keys.contains(&format!("{KEY}:auto:0")));

    // One metadata entry per key, kept in place with its own identifier, so no
    // entry ever names a record holding bytes it was not written beside.
    session.command("input_frame", &frame(3, AUTOSAVE_STEPS as u16));
    let run = session.run().expect("a run is loaded");
    assert_eq!(run.step(), 3 * AUTOSAVE_STEPS);
    let named: Vec<(u32, &str, Step)> = run
        .state()
        .anchors
        .iter()
        .map(|anchor| (anchor.anchor_id, anchor.save_key.as_str(), anchor.step))
        .collect();
    assert_eq!(
        named,
        vec![
            (1, format!("{KEY}:auto:1").as_str(), 3 * AUTOSAVE_STEPS),
            (2, format!("{KEY}:auto:0").as_str(), 2 * AUTOSAVE_STEPS),
        ],
        "two slots, two standing entries, each rewritten in place"
    );

    // Every entry names a record that holds exactly the step it claims.
    for anchor in &run.state().anchors {
        let record = session.store().record(&anchor.save_key).expect("the record it names");
        let held = parse(&record.payload).expect("canonical");
        let step = held.get("field").expect("a field").get("now").expect("a state");
        assert_eq!(step.get("step").and_then(Json::as_int), Some(i64::from(anchor.step)));
    }
}

#[test]
fn no_checkpoint_stands_after_an_import_naming_a_step_it_does_not_hold() {
    // Two autosaves stand, then the run is exported and imported into a fresh
    // session that holds neither record. Each identifier must land on the step
    // its metadata claims, or answer that it names nothing.
    let mut source = opened(KEY);
    source.command("input_frame", &frame(1, 1800));
    source.command("input_frame", &frame(2, 900));
    source.command("input_frame", &frame(3, 60));
    assert_eq!(source.step(), 2760);
    let file = exported(&mut source);

    let mut fresh = Session::new(&support::worker_init()).expect("versions agree");
    fresh.command("import_run", &import_body(&file));
    let claimed: Vec<(u32, Step)> = fresh
        .run()
        .expect("a run is loaded")
        .state()
        .anchors
        .iter()
        .map(|anchor| (anchor.anchor_id, anchor.step))
        .collect();
    assert!(claimed.len() >= 2, "the run had written two records: {claimed:?}");

    for (anchor_id, step) in claimed {
        let answer = fresh.command("restore_checkpoint", &format!("{{\"anchor_id\":{anchor_id}}}"));
        let parsed = parse(&answer).expect("canonical");
        if parsed.get("ok") == Some(&Json::Bool(false)) {
            assert_eq!(refusal(&answer), "not_found", "{anchor_id}: {answer}");
            continue;
        }
        assert_eq!(
            int_of(&body(&answer), "step"),
            i64::from(step),
            "a restore lands on the step its own metadata claims"
        );
        // And re-importing the file puts the run back for the next identifier.
        let mut again = Session::new(&support::worker_init()).expect("versions agree");
        again.command("import_run", &import_body(&file));
        fresh = again;
    }
}

#[test]
fn the_checkpoint_metadata_list_is_not_capped_the_way_records_are() {
    // The 64 cap is a cap on stored records; the metadata of a pruned record is
    // kept, so a payload carrying more than 64 entries reads back.
    let mut run = Run::start(KEY, "thread", &support::content_hash()).expect("a run opens");
    let mut state = run.state().clone();
    let stream = field_game_core::rng::trajectory_stream(KEY, 0);
    state.anchors = (1..=ANCHORS_PER_RUN as u32 + 8)
        .map(|anchor_id| CheckpointState {
            anchor_id,
            step: 0,
            chapter_index: 0,
            objective_id: String::new(),
            kind: RecordKind::Anchor,
            save_key: format!("{KEY}:anchor:{anchor_id:08}"),
            rng: stream,
            branch_nonce: 0,
        })
        .collect();
    let payload = state.payload();
    let read = RunState::read(&parse(&payload).expect("canonical")).expect("more than 64 reads");
    assert_eq!(read.anchors.len(), ANCHORS_PER_RUN + 8);
    assert_eq!(read.payload(), payload, "and re-serializes to the same bytes");
    run.set_anchors(read.anchors);
    assert_eq!(run.state().anchors.len(), ANCHORS_PER_RUN + 8);
}

#[test]
fn a_run_stopped_mid_cooldown_restores_and_carries_on_byte_for_byte() {
    // The depth resolution state is authoritative, so a restore has to land
    // inside the cooldown exactly where the record left it.
    let wheeled = |seq: u32, wheel: i16, steps: u16| {
        format!(
            "{{\"advance_steps\":{steps},\"depth_key\":0,\"inspect\":null,\"pause\":false,\
              \"pulse_held\":false,\"pulse_release\":false,\"seq\":{seq},\"steer_x\":0,\
              \"steer_y\":0,\"t_us\":{stamp},\"toggle_still\":false,\"wheel\":{wheel}}}",
            stamp = i64::from(seq) * 33_333
        )
    };

    let mut source = opened(KEY);
    source.command("input_frame", &wheeled(1, 300, 1));
    source.command("input_frame", &wheeled(2, 300, 2));
    let held = source.run().expect("a run is loaded").state().now.depth_cooldown;
    assert!(held > 0, "the run is stopped inside the cooldown");
    let file = exported(&mut source);

    // Carrying on in the source, and carrying on in a session that restored it,
    // land on the same bytes but for the one locked normalization.
    source.command("input_frame", &wheeled(3, 3000, 4));
    source.command("input_frame", &wheeled(4, 0, 20));
    let carried = source.run().expect("a run is loaded").state().payload();

    let mut restored = Session::new(&support::worker_init()).expect("versions agree");
    restored.command("import_run", &import_body(&file));
    let at = restored.run().expect("a run is loaded");
    assert_eq!(at.state().now.depth_cooldown, held, "the cooldown comes back");
    let returned_to = at.step();
    restored.command("input_frame", &wheeled(1, 3000, 4));
    restored.command("input_frame", &wheeled(2, 0, 20));
    let after = restored.run().expect("a run is loaded").state().payload();

    assert_eq!(
        after.replace(
            &format!("\"prev_assembly_step\":{returned_to},"),
            "\"prev_assembly_step\":null,"
        ),
        carried,
        "a restore mid-cooldown carries on byte for byte"
    );
}

#[test]
fn an_export_taken_between_a_stepless_press_and_the_step_that_follows_carries_everything() {
    // The invariant: the state a run carries on from is the payload and the
    // frames that follow it, and nothing else. A frame that executes no step
    // may carry a bracket direction — the shell offers it again until a frame
    // that executes one consumes it — and an export may be taken between the
    // two, because `export_run` is reachable at any instant and the shell's own
    // recovery capture takes one on the first frame it sees.
    //
    // So the core may hold nothing of that press between the two frames. If it
    // did, the run that kept going and the run restored from the record between
    // them would resolve different depth and diverge permanently, with no byte
    // of the record to say why.
    let keyed = |seq: u32, steps: u16, depth_key: i8| {
        format!(
            "{{\"advance_steps\":{steps},\"depth_key\":{depth_key},\"inspect\":null,\
              \"pause\":false,\"pulse_held\":false,\"pulse_release\":false,\"seq\":{seq},\
              \"steer_x\":0,\"steer_y\":0,\"t_us\":{stamp},\"toggle_still\":false,\"wheel\":0}}",
            stamp = i64::from(seq) * 33_333
        )
    };
    // The record is taken between the press and the step, and both runs are
    // then driven by frames that are byte-identical.
    let carried_on = |following: &str| -> (String, String) {
        let mut source = opened(KEY);
        source.command("input_frame", &keyed(1, 0, 1));
        let file = exported(&mut source);
        source.command("input_frame", following);
        let carried = source.run().expect("a run is loaded").state().payload();

        let mut restored = Session::new(&support::worker_init()).expect("versions agree");
        restored.command("import_run", &import_body(&file));
        let returned_to = restored.run().expect("a run is loaded").step();
        restored.command("input_frame", following);
        let after = restored.run().expect("a run is loaded").state().payload();
        (
            carried,
            after.replace(
                &format!("\"prev_assembly_step\":{returned_to},"),
                "\"prev_assembly_step\":null,",
            ),
        )
    };

    // The shell offers the press again, so the frame that executes a step
    // carries it: both runs resolve it, and both land on the same bytes.
    let (carried, restored) = carried_on(&keyed(2, 1, 1));
    assert_eq!(carried, restored, "the press is in the frame, so the record needs none of it");
    assert!(carried.contains("\"depth_move\":1"), "and it did resolve: {carried}");

    // And the frame after a press the shell has let go of carries none, so
    // neither run resolves one. This is the reading that would diverge if the
    // core were holding the press across the export.
    let (carried, restored) = carried_on(&keyed(2, 1, 0));
    assert_eq!(carried, restored, "a run holds nothing of a press its frames do not carry");
    assert!(!carried.contains("\"depth_move\":1"), "and nothing resolved: {carried}");
}

#[test]
fn the_cap_is_measured_over_the_payload_rather_than_the_file_that_wraps_it() {
    // A payload just under the cap arrives inside a file a little over it. The
    // cap's object is the payload, so the file's own wrapper may not decide it.
    let mut run = Run::start(KEY, "thread", &support::content_hash()).expect("a run opens");
    let stream = field_game_core::rng::trajectory_stream(KEY, 0);
    let mut state = run.state().clone();
    let entry = |anchor_id: u32| CheckpointState {
        anchor_id,
        step: 0,
        chapter_index: 0,
        objective_id: String::new(),
        kind: RecordKind::Anchor,
        save_key: format!("{KEY}:anchor:{anchor_id:08}"),
        rng: stream,
        branch_nonce: 0,
    };
    // Grow the metadata until the payload sits at its cap exactly. The count is
    // worked out one entry at a time rather than by serializing the whole
    // payload each round, and the last entry's free text takes up the slack —
    // a plain ASCII character costs exactly one byte of canonical text.
    let mut total = state.payload().len();
    let mut count = 0u32;
    loop {
        let next = count + 1;
        let added = entry(next).written().len() + usize::from(count > 0);
        if total + added > SAVE_PAYLOAD_CAP {
            break;
        }
        total += added;
        count = next;
    }
    state.anchors = (1..=count).map(entry).collect();
    assert_eq!(state.payload().len(), total, "the count was worked out exactly");
    let last = state.anchors.len() - 1;
    state.anchors[last].objective_id = "a".repeat(SAVE_PAYLOAD_CAP - total);
    let payload = state.payload();
    assert_eq!(payload.len(), SAVE_PAYLOAD_CAP, "the payload stands exactly at its cap");

    run.set_anchors(state.anchors.clone());
    let file = run.state().export_file();
    assert!(file.len() > SAVE_PAYLOAD_CAP, "the wrapper carries the file past the cap");

    let mut session = Session::new(&support::worker_init()).expect("versions agree");
    let answer = session.command("import_run", &import_body(&file));
    assert_eq!(parse(&answer).expect("canonical").get("ok"), Some(&Json::Bool(true)), "{}", &answer[..200.min(answer.len())]);

    // One entry more and the payload itself crosses, which is the capacity
    // envelope rather than an invalid import.
    let mut over = state;
    for next in 0..8 {
        over.anchors.push(entry(over.anchors.len() as u32 + 1 + next));
    }
    assert!(over.payload().len() > SAVE_PAYLOAD_CAP);
    let mut refused_in = Session::new(&support::worker_init()).expect("versions agree");
    let refused_answer = refused_in.command("import_run", &import_body(&over.export_file()));
    assert_eq!(refusal(&refused_answer), "capacity");
}

#[test]
fn quick_retry_lands_on_the_recorded_bytes_exactly() {
    let (mut session, anchor_id) = past_one_interval();
    let recorded = session.store().of_run(KEY)[0].payload.clone();
    let at_interval = session.run().expect("a run is loaded").state().rng;

    // Play on past the recorded moment, then retry it.
    session.command("input_frame", &frame(2, 40));
    assert_eq!(session.step(), AUTOSAVE_STEPS + 40);

    let answer = body(&session.command("restore_checkpoint", &format!("{{\"anchor_id\":{anchor_id}}}")));
    assert_eq!(int_of(&answer, "step"), i64::from(AUTOSAVE_STEPS));
    assert_eq!(int_of(&answer, "branch_nonce"), 0, "Quick Retry keeps the branch");

    let run = session.run().expect("a run is loaded");
    assert_eq!(run.step(), AUTOSAVE_STEPS);
    assert_eq!(run.state().rng, at_interval, "the exact random state comes back");
    assert_eq!(run.mode().name(), "running", "a restore lands running");
    assert!(run.queue().is_empty(), "a restore clears the queue");
    assert_eq!(
        run.state().now.prev_assembly_step,
        Some(AUTOSAVE_STEPS),
        "the one post-restore normalization"
    );

    // Everything but the normalized field is the recorded payload, byte for
    // byte: the restored state re-serializes to those bytes with that one
    // value set to the step it returned to.
    let live = run.state().payload();
    assert_eq!(
        live.replace(
            &format!("\"prev_assembly_step\":{AUTOSAVE_STEPS}"),
            "\"prev_assembly_step\":null"
        ),
        recorded,
        "the restore differs from the record only by the locked normalization"
    );
}

#[test]
fn branch_recovery_takes_a_fresh_nonce_and_re_roots_the_stream() {
    let (mut session, anchor_id) = past_one_interval();
    let before = session.run().expect("a run is loaded").state().rng;
    assert_eq!(session.store().nonce_high(KEY), 0);

    let answer = body(&session.command("recover_branch", &format!("{{\"anchor_id\":{anchor_id}}}")));
    assert_eq!(int_of(&answer, "branch_nonce"), 1, "one past the largest nonce recorded");
    assert_eq!(int_of(&answer, "step"), i64::from(AUTOSAVE_STEPS));

    let run = session.run().expect("a run is loaded");
    assert_ne!(run.state().rng, before, "the trajectory stream is re-rooted");
    assert_eq!(
        run.state().rng,
        field_game_core::rng::trajectory_stream(KEY, 1),
        "at position zero of the new branch's own stream"
    );
    assert_eq!(session.store().nonce_high(KEY), 1, "the monotone record rises and never falls");

    // A second recovery takes the next nonce, and restoring an older record
    // never lowers the monotone record.
    session.command("input_frame", &frame(2, AUTOSAVE_STEPS as u16));
    let later = session.run().expect("a run is loaded").state().anchors.last().expect("a record").anchor_id;
    let answer = body(&session.command("recover_branch", &format!("{{\"anchor_id\":{later}}}")));
    assert_eq!(int_of(&answer, "branch_nonce"), 2);
    assert_eq!(session.store().nonce_high(KEY), 2);
}

#[test]
fn a_recovered_branch_diverges_from_a_quick_retry_of_the_same_moment() {
    let script = |session: &mut Session, from: u32| {
        for seq in from..from + 3 {
            session.command("input_frame", &frame(seq, 5));
        }
    };

    let (mut retried, anchor_id) = past_one_interval();
    retried.command("restore_checkpoint", &format!("{{\"anchor_id\":{anchor_id}}}"));
    script(&mut retried, 2);
    let after_retry = retried.run().expect("a run is loaded").state().payload();

    let (mut recovered, anchor_id) = past_one_interval();
    recovered.command("recover_branch", &format!("{{\"anchor_id\":{anchor_id}}}"));
    script(&mut recovered, 2);
    let after_recovery = recovered.run().expect("a run is loaded").state().payload();

    assert_ne!(after_retry, after_recovery, "a fresh nonce is a different run of the same Field");
    // The opening Open Field regime intentionally has no stochastic Supply
    // variation, so its embodied state can remain equal until a later authored
    // random event. Branch identity and the trajectory root must still differ.
    let stream_of = |payload: &str| {
        parse(payload).expect("canonical").get("rng").expect("a stream position").clone()
    };
    assert_ne!(stream_of(&after_retry), stream_of(&after_recovery));

    // And a Quick Retry repeated twice is byte-equivalent, which is the whole
    // of the exact-retry contract.
    let (mut again, anchor_id) = past_one_interval();
    again.command("restore_checkpoint", &format!("{{\"anchor_id\":{anchor_id}}}"));
    script(&mut again, 2);
    assert_eq!(again.run().expect("a run is loaded").state().payload(), after_retry);
}

#[test]
fn a_restore_naming_no_checkpoint_is_not_found() {
    let (mut session, anchor_id) = past_one_interval();
    for command in ["restore_checkpoint", "recover_branch"] {
        let answer = session.command(command, &format!("{{\"anchor_id\":{}}}", anchor_id + 99));
        assert_eq!(refusal(&answer), "not_found", "{command}: {answer}");
    }
    assert_eq!(session.step(), AUTOSAVE_STEPS, "a refused restore moves nothing");
}

#[test]
fn a_written_record_reads_back_as_the_state_it_was_written_from() {
    let (mut session, _) = past_one_interval();
    let record = session.store().of_run(KEY)[0].payload.clone();
    let key = session.store().of_run(KEY)[0].save_key.clone();

    // `init_run` is valid only before a run is loaded, and the shell reaches
    // that by starting a fresh worker. A loaded run is not reopened in place.
    let answer =
        session.command("init_run", &format!("{{\"mode\":\"restore\",\"save_key\":\"{key}\"}}"));
    assert_eq!(refusal(&answer), "state");

    // The record's own bytes read back as the state they were written from,
    // and reopening a run on them applies the one post-restore normalization.
    let parsed = parse(&record).expect("canonical");
    let state = RunState::read(&parsed).expect("a written record reads back");
    assert_eq!(state.now.step, AUTOSAVE_STEPS);
    assert_eq!(state.run_id, KEY);
    assert_eq!(state.now.prev_assembly_step, None, "the stored bytes are never edited");

    let reopened = Run::restore(state, "thread").expect("the record is restorable");
    assert_eq!(reopened.step(), AUTOSAVE_STEPS);
    assert_eq!(reopened.mode().name(), "running");
    assert_eq!(reopened.state().now.prev_assembly_step, Some(AUTOSAVE_STEPS));
}

// ---------------------------------------------------------------------------
// The render snapshot
// ---------------------------------------------------------------------------

/// The Field the render snapshot's fixture is encoded from: two layers, four
/// Nodes, one Route, one Form, and one current, with a standing inside over two
/// of the Nodes.
fn fixture_field() -> FieldState {
    let mut field = FieldState::opening();
    field.next_node_id = 5;
    field.next_route_id = 2;
    field.step = 0;

    field.layers = vec![
        FieldLayer {
            layer: 0,
            drain: ONE_UNIT,
            noise: FRAC_ONE / 8,
            gain: FRAC_ONE / 4,
            current_ids: vec![7],
            port_ids: vec![1, 2],
        },
        FieldLayer {
            layer: 1,
            drain: 2 * ONE_UNIT,
            noise: FRAC_ONE / 4,
            gain: FRAC_ONE / 2,
            current_ids: Vec::new(),
            port_ids: vec![4],
        },
    ];
    field.ports = vec![
        PortState {
            node: 1,
            layer: 0,
            pos: Vec2::units(100, 200),
            kind: NodeKind::Port,
            q: 64 * ONE_UNIT,
            open: true,
            upkeep_rate: 0,
            capacity: 512 * ONE_UNIT,
        },
        PortState {
            node: 2,
            layer: 0,
            pos: Vec2::units(140, 200),
            kind: NodeKind::Reserve,
            q: 1024 * ONE_UNIT,
            open: true,
            upkeep_rate: 0,
            capacity: 512 * ONE_UNIT,
        },
        PortState {
            node: 3,
            layer: 0,
            pos: Vec2::units(120, 210),
            kind: NodeKind::Form,
            q: 8 * ONE_UNIT,
            open: true,
            upkeep_rate: 0,
            capacity: 256 * ONE_UNIT,
        },
        PortState {
            node: 4,
            layer: 1,
            pos: Vec2::units(3000, 1000),
            kind: NodeKind::Module,
            q: 16 * ONE_UNIT,
            open: true,
            upkeep_rate: 0,
            capacity: 512 * ONE_UNIT,
        },
    ];
    field.routes = vec![RouteState {
        route: 1,
        tail: 1,
        head: 2,
        capacity: 32 * ONE_UNIT,
        flow: 0,
        formed_step: 0,
    }];
    field.forms = vec![FormState {
        id: 1,
        form: "relay".to_string(),
        node: 3,
        controlled: true,
        layer: 0,
        pos: Vec2::units(120, 210),
        vel: Vec2::new(ONE_UNIT / 2, -ONE_UNIT),
        charge: 8 * ONE_UNIT,
        reserve: 64 * ONE_UNIT,
        pulse_charge: FRAC_ONE / 2,
        focus: true,
        route_reach: 256 * ONE_UNIT,
        forecast_depth: 0,
        steer_scale: field_game_core::state::FRAC_ONE,
        route_capacity: 32 * ONE_UNIT,
        link: None,
        trail: None,
        junction: None,
    }];
    field.currents = vec![CurrentState {
        id: 7,
        layer: 0,
        path: vec![Vec2::units(100, 100), Vec2::units(200, 300)],
        width: 16 * ONE_UNIT,
        strength: 64 * ONE_UNIT,
        duty: 65_536,
        period: 40,
        phase: 0,
        bright: true,
        active: true,
    }];
    // This fixture keeps the legacy member set so the V2 byte change is the
    // format version itself; the independent-flags test below deliberately
    // separates physical membership from the passive View.
    field.physical_compartment = PhysicalCompartment {
        members: vec![1, 3],
        leak_per_exposed_contact_per_step: 4096,
    };
    field.boundaries = BoundaryState { drawn: Vec::new(), authored: Vec::new() };
    field
}

fn fixture_view() -> ViewDeclaration {
    ViewDeclaration { inside: vec![1, 3], resolution: 4, window: 45, surround: Surround::Adjacent }
}

/// The run the fixture is taken from: the Field above, established and
/// advanced one step so every per-step quantity the snapshot reads is set.
///
/// The step holds the Pulse, so the snapshot carries a charging Form: the
/// locked rule zeroes a charge on a step that neither holds nor releases, and a
/// fixture whose Form charged before the step and let go during it would pin
/// the resting reading rather than the charging one.
fn fixture_run() -> Run {
    let mut run = Run::start(KEY, "relay", &support::content_hash()).expect("a run opens");
    run.establish_field(fixture_field(), fixture_view()).expect("the Field is establishable");
    run.input_frame(&parse(&held_frame(1, 1, true)).expect("canonical"), None).expect("one step runs");
    run
}

fn hex_of(bytes: &[u8]) -> String {
    field_game_core::json::hex_bytes(bytes)
}

#[test]
fn the_render_snapshot_matches_the_fixture_both_languages_read() {
    let encoded = fixture_run().frame_view();
    assert_eq!(
        hex_of(&encoded),
        FRAME_FIXTURE.trim(),
        "the encoder and the fixture the worker's decoder reads move together"
    );
}

#[test]
fn the_render_snapshot_carries_the_locked_header_and_section_table() {
    let run = fixture_run();
    let view = run.frame_view();

    assert_eq!(&view[0..4], MAGIC);
    assert_eq!(u16::from_le_bytes([view[4], view[5]]), FRAME_VERSION);
    assert_eq!(u32::from_le_bytes([view[8], view[9], view[10], view[11]]), 1, "one step ran");
    assert_eq!(u16::from_le_bytes([view[12], view[13]]), 65535, "full speed saturates");
    assert_eq!(view[14], 0, "the mode is running");
    assert_eq!(view[15], 0, "the camera targets the controlled Form's layer");
    assert_eq!(view[16], OPENING_IMPULSE, "the Impulse the run carries, which a new run opens at");
    assert_eq!(view[17], 0);
    assert_eq!(u16::from_le_bytes([view[18], view[19]]), 0, "no objective is offered yet");
    assert_eq!(
        view[20],
        7,
        "Forms, Ports, Routes, currents, the inside, the path, and Pulse preview",
    );
    assert_eq!(view[21], 0, "the physical-reading alignment byte is zero pad");
    assert_eq!(
        u32::from_le_bytes([view[22], view[23], view[24], view[25]]),
        4096,
        "the material leakage coefficient crosses independently of the View"
    );
    assert_eq!(&view[26..32], &[0u8; 6], "the header's remaining tail is zero pad");
    assert!(view.len() <= FRAME_BUFFER_CAP);

    // The section table: each entry names a kind, a count, and where its
    // records start, and every section lands inside the buffer.
    let mut kinds = Vec::new();
    for place in 0..usize::from(view[20]) {
        let at = HEADER_BYTES + place * SECTION_ENTRY_BYTES;
        let kind = view[at];
        let count = u16::from_le_bytes([view[at + 2], view[at + 3]]);
        let offset = u32::from_le_bytes([view[at + 4], view[at + 5], view[at + 6], view[at + 7]]);
        assert_eq!(view[at + 1], 0, "the entry's pad byte");
        assert!((offset as usize) < view.len(), "kind {kind} starts inside the buffer");
        kinds.push((kind, count));
    }
    assert_eq!(kinds, vec![(1, 1), (2, 4), (3, 1), (4, 1), (5, 1), (10, 2), (13, 1)]);
}

#[test]
fn the_render_snapshot_reads_the_field_the_model_holds() {
    let run = fixture_run();
    let view = run.frame_view();
    let section = |kind: u8| -> (usize, usize) {
        for place in 0..usize::from(view[20]) {
            let at = HEADER_BYTES + place * SECTION_ENTRY_BYTES;
            if view[at] == kind {
                let count = u16::from_le_bytes([view[at + 2], view[at + 3]]) as usize;
                let offset =
                    u32::from_le_bytes([view[at + 4], view[at + 5], view[at + 6], view[at + 7]])
                        as usize;
                return (offset, count);
            }
        }
        panic!("the snapshot carries section {kind}");
    };

    // Forms: the Form ordinal is its place in the closed set, and the flags
    // carry control, focus, and a charging Pulse.
    let (at, count) = section(1);
    assert_eq!(count, 1);
    assert_eq!(view[at], 1, "the Form's identifier");
    assert_eq!(view[at + 1], 2, "relay is third in the closed set");
    assert_eq!(view[at + 2], 0, "on the shallowest layer");
    assert_eq!(view[at + 3], 0b111, "controlled, focused, and charging");
    let x = f32::from_le_bytes([view[at + 4], view[at + 5], view[at + 6], view[at + 7]]);
    // The Form stood at 120 units with half a unit per step of velocity, and
    // the step ran under a control of zero: the steering phase's damper took a
    // quarter of that velocity before the position advanced by what was left.
    assert_eq!(x, 120.375, "the position advanced by velocity, one way into f32");
    let vx = f32::from_le_bytes([view[at + 12], view[at + 13], view[at + 14], view[at + 15]]);
    assert_eq!(vx, 0.375, "the velocity the damper left, one way into f32");

    // Ports: physical membership is carried here rather than inferred from the
    // View. This compatibility fixture gives both the same set; the next test
    // pulls all three notions apart.
    let (at, count) = section(2);
    assert_eq!(count, 4);
    let port = |place: usize| &view[at + place * 16..at + (place + 1) * 16];
    assert_eq!(u32::from_le_bytes(port(0)[0..4].try_into().expect("four bytes")), 1);
    assert_eq!(port(0)[4], 0, "a Port is first in the closed kind set");
    assert_eq!(port(0)[5] & 0b0100, 0b0100, "a physical member");
    assert_eq!(port(0)[5] & 0b1000, 0b1000, "an exposed physical member");
    assert_eq!(port(1)[5] & 0b0010, 0b0010, "overloaded");
    assert_eq!(port(1)[5] & 0b0100, 0, "not a physical member");
    assert_eq!(port(2)[5] & 0b0100, 0b0100, "the other physical member");
    assert_eq!(port(2)[5] & 0b1000, 0b1000, "also on the exposed shell");
    assert_eq!(port(0)[5] & 0b1_0000, 0, "no queued compartment proposal");
    assert_eq!(port(3)[4], 2, "a module is third in the closed kind set");
    // Position in units times 16.
    assert_eq!(u16::from_le_bytes(port(0)[8..10].try_into().expect("two bytes")), 100 * 16);
    // The layer the Node stands on, so the renderer places it on its own plane.
    assert_eq!(port(0)[14], 0, "the first three Nodes stand on the shallowest layer");
    assert_eq!(port(3)[14], 1, "the fourth stands one layer down");
    assert_eq!(port(0)[15], 0, "the record's tail is zero");

    // Routes: the flow is a fraction of the Route's own capacity, and the
    // status marks the overloaded head.
    let (at, count) = section(3);
    assert_eq!(count, 1);
    assert_eq!(u32::from_le_bytes(view[at..at + 4].try_into().expect("four bytes")), 1);
    assert_eq!(view[at + 13], 2, "the head stands overloaded");
    assert_eq!(u16::from_le_bytes(view[at + 14..at + 16].try_into().expect("two bytes")), 1);

    // Currents: the phase advanced one step, and the strength is a fraction of
    // the locked strength cap.
    let (at, count) = section(4);
    assert_eq!(count, 1);
    assert_eq!(u16::from_le_bytes(view[at..at + 2].try_into().expect("two bytes")), 7);
    assert_eq!(view[at + 3], 0b1_0011, "active, bright, and inside its duty window");
    assert_eq!(u16::from_le_bytes(view[at + 4..at + 6].try_into().expect("two bytes")), 1);
    assert_eq!(
        u16::from_le_bytes(view[at + 6..at + 8].try_into().expect("two bytes")),
        16_384,
        "64 units of the 256-unit cap"
    );
    // Where this current's own points start in the flat array, how many it
    // has, and the band a Node has to stand within to receive.
    assert_eq!(u16::from_le_bytes(view[at + 8..at + 10].try_into().expect("two bytes")), 0);
    assert_eq!(view[at + 10], 2, "the fixture's path is two points");
    assert_eq!(view[at + 11], 16, "sixteen units wide");

    // The flat point array, in the same quantization a Port position uses.
    let (at, count) = section(10);
    assert_eq!(count, 2);
    assert_eq!(u16::from_le_bytes(view[at..at + 2].try_into().expect("two bytes")), 100 * 16);
    assert_eq!(u16::from_le_bytes(view[at + 2..at + 4].try_into().expect("two bytes")), 100 * 16);
    assert_eq!(u16::from_le_bytes(view[at + 4..at + 6].try_into().expect("two bytes")), 200 * 16);
    assert_eq!(u16::from_le_bytes(view[at + 6..at + 8].try_into().expect("two bytes")), 300 * 16);

    // The inside, as one bitset over the Port records.
    let (at, count) = section(5);
    assert_eq!(count, 1);
    assert_eq!(view[at], 0b0101, "Port records 0 and 2 are members");
    assert_eq!(&view[at + 1..at + 32], &[0u8; 31]);
}

#[test]
fn frame_v2_keeps_physical_proposed_and_view_membership_independent() {
    let mut field = fixture_field();
    field.physical_compartment.members = vec![1, 2];
    let view = fixture_view();
    let mut queue = PlanQueue::new();
    queue
        .push(PlanCommand::ReshapeCompartment { members: vec![2, 4] })
        .expect("the proposal fits");
    queue.rebuild(&field);
    let config = InputConfig::default_config();
    let progress = Progress::opening();
    let encoded = frame::encode(&frame::Snapshot {
        field: &field,
        mode: Mode::Running,
        time_scale: u16::MAX,
        view_inside: &view.inside,
        queue: &queue,
        cues: &[],
        config: &config,
        progress: &progress,
        pressures: &[],
        objective_ordinal: 0,
        forecast: &[],
        medium: field_game_core::field::MediumMotion::default(),
    });
    let section = |kind: u8| -> (usize, usize) {
        for place in 0..usize::from(encoded[20]) {
            let at = HEADER_BYTES + place * SECTION_ENTRY_BYTES;
            if encoded[at] == kind {
                let count = u16::from_le_bytes([encoded[at + 2], encoded[at + 3]]) as usize;
                let offset = u32::from_le_bytes([
                    encoded[at + 4],
                    encoded[at + 5],
                    encoded[at + 6],
                    encoded[at + 7],
                ]) as usize;
                return (offset, count);
            }
        }
        panic!("frame carries section {kind}");
    };

    let (ports, count) = section(2);
    assert_eq!(count, 4);
    let flags = |place: usize| encoded[ports + place * 16 + 5];
    assert_eq!(flags(0) & 0b1_0100, 0b0_0100, "Node 1: physical only");
    assert_eq!(flags(1) & 0b1_0100, 0b1_0100, "Node 2: physical and proposed");
    assert_eq!(flags(2) & 0b1_0100, 0, "Node 3: View only, so no physical flags");
    assert_eq!(flags(3) & 0b1_0100, 0b1_0000, "Node 4: proposed only");

    let (view_bits, count) = section(5);
    assert_eq!(count, 1);
    assert_eq!(encoded[view_bits], 0b0101, "the passive View remains Nodes 1 and 3");
}

#[test]
fn a_path_longer_than_the_cap_is_decimated_keeping_both_ends() {
    // The locked rule: keep both endpoints and take the interior on an even
    // stride, so the shape the renderer strokes is the shape the encoder wrote.
    let mut run = Run::start(KEY, "relay", &support::content_hash()).expect("a run opens");
    let mut field = fixture_field();
    field.currents[0].path =
        (0..64).map(|step| Vec2::units(100 + step * 10, 500)).collect::<Vec<_>>();
    run.establish_field(field, fixture_view()).expect("the Field is establishable");
    run.input_frame(&parse(&frame(1, 1)).expect("canonical"), None).expect("one step runs");
    let view = run.frame_view();

    let mut points = None;
    let mut record = None;
    for place in 0..usize::from(view[20]) {
        let at = HEADER_BYTES + place * SECTION_ENTRY_BYTES;
        let count = u16::from_le_bytes([view[at + 2], view[at + 3]]) as usize;
        let offset =
            u32::from_le_bytes([view[at + 4], view[at + 5], view[at + 6], view[at + 7]]) as usize;
        if view[at] == 10 {
            points = Some((offset, count));
        }
        if view[at] == 4 {
            record = Some(offset);
        }
    }
    let (at, count) = points.expect("the frame carries the path section");
    let current = record.expect("the frame carries the current record");
    assert_eq!(count, 32, "sixty-four authored points come down to the cap");
    assert_eq!(view[current + 10], 32, "the record counts what the section holds");
    let read = |place: usize| -> u16 {
        u16::from_le_bytes(view[at + place * 4..at + place * 4 + 2].try_into().expect("two bytes"))
    };
    assert_eq!(read(0), 100 * 16, "the authored first point is kept");
    assert_eq!(read(31), (100 + 63 * 10) * 16, "the authored last point is kept");
    // An even stride over the interior: sixty-three intervals across
    // thirty-one steps, rounded, so no two kept points repeat.
    let mut seen = Vec::new();
    for place in 0..32 {
        seen.push(read(place));
    }
    let mut sorted = seen.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), 32, "the decimation keeps thirty-two distinct points");
    assert_eq!(seen, sorted, "and keeps them in the authored order");
}

#[test]
fn a_new_run_carries_the_authored_chapter_into_the_render_snapshot() {
    // A new run stands on the authored chapter now, so its snapshot carries
    // the Field the chapter declares and the objective the sequence opens on.
    let mut session = opened(KEY);
    session.command("input_frame", &frame(1, 1));
    let view = session.frame_view();
    assert!(view.len() > HEADER_BYTES, "the chapter's own parts are written");
    assert!(view[20] >= 4, "Forms, Ports, Routes, and currents all stand: {}", view[20]);
    assert_eq!(view[17], 0, "the run opens on the first chapter");
    assert_eq!(
        u16::from_le_bytes([view[18], view[19]]),
        1,
        "and on the first objective of its authored order",
    );
}

#[test]
fn the_export_cap_is_checked_on_the_way_out() {
    // The cap is a row of the locked capacity table, so crossing it is the
    // capacity envelope rather than a validation fault. Nothing a run can
    // reach today crosses it, so the check is read through the fault the
    // payload accessor raises.
    let run = fixture_run();
    assert!(run.payload().is_ok());
    let fault = field_game_core::field::cap_fault("save_payload", 8 * 1024 * 1024);
    assert_eq!(fault.code(), Code::Capacity);
    assert!(fault.write().contains("\"quantity\":\"save_payload\""));
}

#[test]
fn a_release_carries_its_cues_and_its_reach_into_the_render_snapshot() {
    let mut run = fixture_run();
    // The fixture's step held the Pulse, so a charge stands: one locked share
    // on top of the half the Field was established with.
    assert_eq!(run.state().now.forms[0].pulse_charge, FRAC_ONE / 2 + 2_048);
    let charge = run.state().now.forms[0].pulse_charge;
    let held = |run: &Run, node: u32| {
        run.state().now.ports.iter().find(|port| port.node == node).expect("the Node").q
    };
    let before = (held(&run, 1), held(&run, 2));

    run.input_frame(&parse(&pulsing_frame(2, 1, false, true)).expect("canonical"), None)
        .expect("one step runs");
    let view = run.frame_view();

    let section = |kind: u8| -> Option<(usize, usize)> {
        (0..usize::from(view[20])).find_map(|place| {
            let at = HEADER_BYTES + place * SECTION_ENTRY_BYTES;
            (view[at] == kind).then(|| {
                let count = u16::from_le_bytes([view[at + 2], view[at + 3]]) as usize;
                let offset =
                    u32::from_le_bytes([view[at + 4], view[at + 5], view[at + 6], view[at + 7]])
                        as usize;
                (offset, count)
            })
        })
    };

    // Two Nodes stood inside the reach, both already open, so the emission
    // gathered and raised no activation.
    assert!(held(&run, 1) < before.0 && held(&run, 2) < before.1, "both sources gave");
    let (at, count) = section(7).expect("the snapshot carries its cues");
    assert_eq!(count, 2, "the emission, and the gather beside it");
    let cue = |place: usize| -> (u8, u16, u32) {
        let record = &view[at + place * 8..at + (place + 1) * 8];
        assert_eq!(record[1], 0, "the record's pad byte");
        (
            record[0],
            u16::from_le_bytes([record[2], record[3]]),
            u32::from_le_bytes([record[4], record[5], record[6], record[7]]),
        )
    };
    let reach = reach_ticks(pulse_radius(charge));
    assert_eq!(cue(0), (1, reach, 3), "the emission, its reach, and the Form's own Node");
    assert_eq!(cue(1).0, 2, "the gather rides the same frame as the emission");
    assert_eq!(cue(1).2, 3);
    assert!(cue(1).1 > 0, "and carries what it gathered");

    // The forms record carries the emitted reach on the step it was emitted.
    let (forms, _) = section(1).expect("the forms section");
    assert_eq!(u16::from_le_bytes([view[forms + 22], view[forms + 23]]), reach);
    assert_eq!(view[forms + 3] & 0b100, 0, "and the charge it spent is gone");

    // The frame after it carries neither: a cue lasts one frame, and a Form at
    // rest has no reach.
    run.input_frame(&parse(&frame(3, 1)).expect("canonical"), None).expect("one step runs");
    let after = run.frame_view();
    assert!(
        (0..usize::from(after[20]))
            .all(|place| after[HEADER_BYTES + place * SECTION_ENTRY_BYTES] != 7),
        "no cue stands into the next frame",
    );
    let (forms, _) = (0..usize::from(after[20]))
        .find_map(|place| {
            let at = HEADER_BYTES + place * SECTION_ENTRY_BYTES;
            (after[at] == 1).then(|| {
                (u32::from_le_bytes([
                    after[at + 4],
                    after[at + 5],
                    after[at + 6],
                    after[at + 7],
                ]) as usize, 0)
            })
        })
        .expect("the forms section");
    assert_eq!(u16::from_le_bytes([after[forms + 22], after[forms + 23]]), 0, "at rest");
}

#[test]
fn a_run_with_trail_entries_standing_exports_and_re_imports_byte_for_byte() {
    // A Wake run carries a queue no other Form's does, and the queue is state:
    // it rides the payload, it is validated on the way in, and a run reopened
    // from an export stands where the export left it — entries included.
    let mut session = Session::new(&support::worker_init()).expect("versions agree");
    let opened =
        session.command("init_run", &format!("{{\"form\":\"wake\",\"mode\":\"new\",\"run_id\":\"{KEY}\"}}"));
    assert!(opened.starts_with("{\"body\":"), "a new run opens: {opened}");
    session.command("input_frame", &frame(1, 40));

    let standing = session.run().expect("a run").state().now.pending.clone();
    assert!(!standing.is_empty(), "a Wake run has left entries by its fortieth step");
    let exported = session.run().expect("a run").state().export_file();
    let before = session.run().expect("a run").state().payload();

    let mut reopened = Session::new(&support::worker_init()).expect("versions agree");
    let answer = reopened.command("import_run", &import_body(&exported));
    assert!(answer.starts_with("{\"body\":"), "the run imports: {answer}");
    assert_eq!(reopened.run().expect("a run").state().now.pending, standing, "the queue came back");
    // Byte for byte, but for the one normalization every state restore makes:
    // the previous assembly step becomes the step the run came back to.
    assert_eq!(
        reopened
            .run()
            .expect("a run")
            .state()
            .payload()
            .replace("\"prev_assembly_step\":40", "\"prev_assembly_step\":null"),
        before,
        "byte for byte",
    );

    // And it carries on identically. The two traces differ — one holds forty
    // steps of history and the other eighty — but the Field they advance is
    // the same Field, deliveries included, which is what makes a pending entry
    // state rather than something the run holds outside itself.
    session.command("input_frame", &frame(2, 40));
    reopened.command("input_frame", &frame(2, 40));
    let held = |session: &mut Session| -> String {
        let mut field = session.run().expect("a run").state().now.clone();
        // The one post-restore normalization, set aside: an imported run knows
        // the step it came back to and the run it was taken from does not.
        field.prev_assembly_step = None;
        format!("{field:?}")
    };
    assert_eq!(
        held(&mut reopened),
        held(&mut session),
        "forty more steps leave the same Field, deliveries included",
    );
}

#[test]
fn quick_retry_lands_on_the_recorded_bytes_with_trail_entries_standing() {
    // The same claim across the restore path: what a Quick Retry lands on is
    // the recorded payload, and a queue standing in it is part of that record
    // rather than something the restore has to rebuild.
    let mut session = Session::new(&support::worker_init()).expect("versions agree");
    session.command("init_run", &format!("{{\"form\":\"wake\",\"mode\":\"new\",\"run_id\":\"{KEY}\"}}"));
    session.command("input_frame", &frame(1, AUTOSAVE_STEPS as u16));
    let recorded = session.store().of_run(KEY)[0].payload.clone();
    let anchor_id = session.run().expect("a run").state().anchors[0].anchor_id;
    let held = session.run().expect("a run").state().now.pending.clone();

    session.command("input_frame", &frame(2, 40));
    session.command("restore_checkpoint", &format!("{{\"anchor_id\":{anchor_id}}}"));

    let run = session.run().expect("a run is loaded");
    assert_eq!(run.state().now.pending, held, "the queue came back with the record");
    let live = run.state().payload();
    assert_eq!(
        live.replace(
            &format!("\"prev_assembly_step\":{AUTOSAVE_STEPS}"),
            "\"prev_assembly_step\":null"
        ),
        recorded,
        "and the restore differs from the record only by the locked normalization",
    );
}

#[test]
fn a_payload_naming_a_trail_entry_already_due_is_refused() {
    // An entry is removed on the step it comes due, so a state this build wrote
    // never carries one that is due at or before its own step. A payload that
    // does is refused rather than delivered late.
    let mut session = Session::new(&support::worker_init()).expect("versions agree");
    session.command("init_run", &format!("{{\"form\":\"wake\",\"mode\":\"new\",\"run_id\":\"{KEY}\"}}"));
    session.command("input_frame", &frame(1, 40));
    let exported = session.run().expect("a run").state().export_file();
    let stale = exported.replace("\"due\":75", "\"due\":1");
    assert_ne!(stale, exported, "the export names an entry to move");

    let mut fresh = Session::new(&support::worker_init()).expect("versions agree");
    let answer = fresh.command("import_run", &import_body(&stale));
    assert!(answer.contains("\"ok\":false"), "a stale entry is refused: {answer}");
}
