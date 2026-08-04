//! Smoke test for the core: the version handshake, the closed command set,
//! and the lifecycle gate. Shapes come from
//! `docs/field-framework/ARCHITECTURE.md`.

use field_game_core::{Session, PROTOCOL_VERSION, SAVE_VERSION};

mod support;

/// What the worker sends when it opens the core.

/// A run key: 16 lowercase hex characters.
const KEY: &str = "00112233445566aa";

#[test]
fn versions_are_the_locked_ones() {
    assert_eq!(PROTOCOL_VERSION, 2);
    assert_eq!(SAVE_VERSION, 3);
}

#[test]
fn agreement_opens_a_session_in_idle() {
    let session = Session::new(&support::worker_init()).expect("agreed versions open a session");
    assert_eq!(session.lifecycle(), "idle");
    assert_eq!(session.step(), 0);
}

#[test]
fn disagreement_yields_the_protocol_envelope() {
    let refused = Session::new("{\"protocol\":3,\"save_version\":3}")
        .err()
        .expect("a version the core does not speak is refused");
    assert_eq!(
        refused,
        "{\"code\":\"protocol\",\"detail\":{\"protocol\":2,\"save_version\":3},\
         \"message_key\":null}"
    );

    assert!(Session::new("{}").is_err(), "a missing version is refused");
    assert!(
        Session::new("{\"protocol\":2,\"save_version\":2}").is_err(),
        "a save version the core does not read is refused"
    );
}

#[test]
fn a_new_run_opens_on_the_run_key_the_shell_supplies() {
    let mut session = Session::new(&support::worker_init()).expect("versions agree");
    let answer = session.command(
        "init_run",
        &format!("{{\"form\":\"thread\",\"mode\":\"new\",\"run_id\":\"{KEY}\"}}"),
    );

    // The content hash the build computed is what a run records and what
    // `init_run` reports, and the View is the chapter's own authored opening.
    assert_eq!(
        answer,
        format!(
            "{{\"body\":{{\"branch_nonce\":0,\"chapter_index\":0,\"content_changed\":false,\
             \"content_hash\":\"{}\",\
             \"protocol\":2,\"run_id\":\"{KEY}\",\"save_version\":3,\"step\":0,\
             \"view\":{{\"inside\":[2,3,4],\"resolution\":1,\"surround\":\"adjacent\",\"window\":45}}}},\
             \"ok\":true}}",
            support::content_hash(),
        )
    );
    assert_eq!(session.lifecycle(), "running");
}

#[test]
fn a_restore_naming_no_record_is_not_found() {
    let mut session = Session::new(&support::worker_init()).expect("versions agree");
    assert_eq!(
        session.command("init_run", "{\"mode\":\"restore\",\"save_key\":\"a:auto:0\"}"),
        "{\"error\":{\"code\":\"not_found\",\
         \"detail\":{\"quantity\":\"save_key\",\"save_key\":\"a:auto:0\"},\
         \"message_key\":null},\"ok\":false}"
    );
    assert_eq!(session.lifecycle(), "idle");
}

#[test]
fn a_command_outside_its_states_is_answered_with_state() {
    let mut session = Session::new(&support::worker_init()).expect("versions agree");
    assert_eq!(
        session.command("queue_plan", "{}"),
        "{\"error\":{\"code\":\"state\",\
         \"detail\":{\"actual\":\"idle\",\"expected\":[\"still\"]},\
         \"message_key\":null},\"ok\":false}"
    );
    assert!(session
        .command("export_run", "{}")
        .contains("\"expected\":[\"running\",\"ramp_in\",\"still\",\"ramp_out\",\"suspended\",\"ended\"]"));

    // And once a run is loaded, the command that only opens one is refused.
    session.command("init_run", &format!("{{\"form\":\"ring\",\"mode\":\"new\",\"run_id\":\"{KEY}\"}}"));
    let refused = session.command("init_run", "{}");
    assert!(refused.contains("\"code\":\"state\""), "{refused}");
    assert!(refused.contains("\"actual\":\"running\""), "{refused}");
}

#[test]
fn a_command_outside_the_closed_set_is_a_protocol_fault() {
    let mut session = Session::new(&support::worker_init()).expect("versions agree");
    assert_eq!(
        session.command("step", "{}"),
        "{\"error\":{\"code\":\"protocol\",\"detail\":{\"cmd\":\"step\"},\
         \"message_key\":null},\"ok\":false}"
    );
}

#[test]
fn a_file_that_is_not_an_export_file_is_refused_as_an_invalid_import() {
    let mut session = Session::new(&support::worker_init()).expect("versions agree");
    assert_eq!(
        session.command("import_run", "{\"text\":\"{}\"}"),
        "{\"error\":{\"code\":\"import_invalid\",\"detail\":{\"field\":\"text\"},\
         \"message_key\":\"notice.import_rejected\"},\"ok\":false}"
    );
    assert_eq!(session.lifecycle(), "idle", "a refused import loads no run");
}

#[test]
fn no_frame_and_no_events_exist_before_a_run() {
    let mut session = Session::new(&support::worker_init()).expect("versions agree");
    assert!(session.frame_view().is_empty());
    assert_eq!(session.take_events(), "[]");
}

#[test]
fn a_malformed_body_is_a_validation_fault() {
    let mut session = Session::new(&support::worker_init()).expect("versions agree");
    let refused = session.command("init_run", "{\"mode\":");
    assert!(refused.contains("\"code\":\"validation\""), "{refused}");
}
