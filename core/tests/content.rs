//! The authored-content pipeline, from the bundle the worker hands over to the
//! hash `init_run` reports.

use field_game_core::content::{self, CHAPTER_IDS, CONTENT_VERSION};
use field_game_core::fault::{Code, Fault};
use field_game_core::field;
use field_game_core::json::parse;
use field_game_core::Session;

mod support;

const KEY: &str = "00112233445566aa";

fn opened(session: &mut Session) -> String {
    session.command(
        "init_run",
        &format!("{{\"form\":\"thread\",\"mode\":\"new\",\"run_id\":\"{KEY}\"}}"),
    )
}

#[test]
fn the_hash_init_run_reports_is_the_digest_over_the_bytes_that_arrived() {
    let mut session = Session::new(&support::worker_init()).expect("versions agree");
    let answer = opened(&mut session);
    let body = parse(&answer).expect("canonical");
    let reported = body
        .get("body")
        .and_then(|body| body.get("content_hash"))
        .and_then(|held| held.as_text())
        .expect("a content hash");
    assert_eq!(reported, support::content_hash(), "the hash is the one over the content");
    assert_eq!(
        body.get("body").and_then(|body| body.get("content_changed")).and_then(|h| h.as_bool()),
        Some(false),
        "a fresh run stands on the content it was opened with",
    );
    // And the run records it, which is what a restore compares against.
    let run = session.run().expect("a run is loaded");
    assert_eq!(run.state().spec.content_hash(), support::content_hash());
}

#[test]
fn a_digest_that_does_not_describe_the_bytes_beside_it_is_refused() {
    // The build embeds the digest and the worker hands both across. A generated
    // file that has gone stale is exactly this case, and it cannot pass.
    let stale = "0".repeat(64);
    let mut session =
        Session::new(&support::worker_init_with(&stale)).expect("the versions still agree");
    let answer = opened(&mut session);
    assert!(answer.contains("\"code\":\"content_invalid\""), "{answer}");
    assert!(answer.contains("\"reason\":\"content_hash\""), "{answer}");
    assert!(answer.contains("\"message_key\":\"notice.content_invalid\""), "{answer}");
    assert_eq!(session.lifecycle(), "idle", "and no run is opened on it");
}

#[test]
fn a_session_opened_without_content_answers_at_init_run() {
    // The document puts content validation at `init_run`, so a bundle that does
    // not read is held rather than refusing the session outright.
    let mut session =
        Session::new("{\"protocol\":2,\"save_version\":2}").expect("the versions agree");
    let answer = opened(&mut session);
    assert!(answer.contains("\"code\":\"content_invalid\""), "{answer}");
}

#[test]
fn the_bundle_reads_into_the_chapter_the_manifest_lists() {
    let bundle = parse(&support::bundle_with(&support::content_hash())).expect("canonical");
    let content = content::read_bundle(&bundle).expect("the authored content reads");
    assert_eq!(content.version, CONTENT_VERSION);
    assert_eq!(content.chapters.len(), 8, "the eight chapters of the campaign");
    assert_eq!(
        content.chapters.iter().map(|held| held.id.as_str()).collect::<Vec<_>>(),
        CHAPTER_IDS.to_vec(),
        "in the closed set's own order",
    );
    assert_eq!(content.forms.len(), 8, "and all eight starting Forms");
    let chapter = content.chapter(0).expect("the opening chapter");
    assert_eq!(chapter.id, "the_pull");
    assert!(CHAPTER_IDS.contains(&chapter.id.as_str()));
    assert_eq!(chapter.title_key, "chapter.the_pull");
    assert_eq!(chapter.objectives.len(), 9, "the nine objectives of the opening chapter");
    assert_eq!(
        chapter.objectives.iter().map(|held| held.id.as_str()).collect::<Vec<_>>(),
        vec![
            "objective.the_pull.follow_current",
            "objective.the_pull.hold_center",
            "objective.the_pull.release_pulse",
            "objective.the_pull.open_port",
            "objective.the_pull.close_loop",
            "objective.the_pull.carry_pattern",
            "objective.the_pull.take_the_depth",
            "objective.the_pull.open_the_deep_port",
            "objective.the_pull.hold_the_moving_current",
        ],
    );
    assert_eq!(chapter.anchor_moments, vec!["objective.the_pull.carry_pattern".to_string()]);
}

#[test]
fn the_chapter_establishes_a_field_that_passes_every_locked_check() {
    let bundle = parse(&support::bundle_with(&support::content_hash())).expect("canonical");
    let content = content::read_bundle(&bundle).expect("the content reads");
    let chapter = content.chapter(0).expect("a chapter");
    for form in &content.forms {
        let (established, view) = content::establish(chapter, form).expect("a Field stands");
        field::validate(&established).expect("the whole locked validation");
        field::establishable(&established).expect("every non-Port Node is established open");
        field::establishable_view(&view, &established).expect("the opening View stands");
        assert!(field::within_caps(&established));
        assert_eq!(
            established.physical_compartment, chapter.physical_compartment,
            "the run's physical compartment is the chapter's under {}",
            form.id,
        );
        // One controlled Form, whatever else the Form's abilities stand beside
        // it: the linked Forms of `linked_forms` are Forms of the Field and
        // none of them is steered.
        let linked: usize = form
            .abilities
            .iter()
            .map(|ability| match ability {
                content::Ability::LinkedForms { offsets, .. } => offsets.len(),
                content::Ability::Trail { .. } => 0,
            })
            .sum();
        assert_eq!(established.forms.len(), 1 + linked);
        assert_eq!(established.forms.iter().filter(|held| held.controlled).count(), 1);
        for held in &established.forms {
            assert_eq!(held.form, form.id);
        }
    }
}

#[test]
fn every_chapter_explicitly_authors_a_compartment_separate_from_its_opening_view() {
    let bundle = parse(&support::bundle_with(&support::content_hash())).expect("canonical");
    let content = content::read_bundle(&bundle).expect("the content reads");
    let form = content.forms.first().expect("a Form");

    for chapter in &content.chapters {
        assert!(
            !chapter.physical_compartment.members.is_empty(),
            "{} explicitly authors causal members",
            chapter.id,
        );
        assert_eq!(
            chapter.physical_compartment.leak_per_exposed_contact_per_step, 64,
            "{} explicitly authors the provisional coefficient",
            chapter.id,
        );
        assert_eq!(
            chapter.physical_compartment.members, chapter.opening_view.inside,
            "{} begins with coincident physical and observational selections",
            chapter.id,
        );

        let (field, mut view) = content::establish(chapter, form).expect("the chapter stands");
        let causal = field.physical_compartment.clone();
        view.inside.clear();
        assert!(view.inside.is_empty(), "the local analysis selection was cleared");
        assert_eq!(
            field.physical_compartment, causal,
            "clearing {}'s View cannot rewrite its causal compartment",
            chapter.id,
        );
    }
}

#[test]
fn the_edge_authors_a_real_physical_reshape_condition() {
    let bundle = parse(&support::bundle_with(&support::content_hash())).expect("canonical");
    let content = content::read_bundle(&bundle).expect("the content reads");
    let chapter = content.chapter(1).expect("The Edge");
    let objective = chapter
        .objectives
        .iter()
        .find(|held| held.id == "objective.the_edge.draw_the_edge")
        .expect("the compartment objective");
    assert!(matches!(
        &objective.condition,
        content::Condition::CompartmentReshaped { steps: 1 },
    ));
}

#[test]
fn every_authored_current_is_wide_enough_to_draw_what_it_delivers() {
    // A carry-forward from the ledger: the drawn band must not promise delivery
    // beyond the catchment, so the narrowest authored width is eight units.
    let bundle = parse(&support::bundle_with(&support::content_hash())).expect("canonical");
    let content = content::read_bundle(&bundle).expect("the content reads");
    let chapter = content.chapter(0).expect("a chapter");
    for current in &chapter.currents {
        assert!(
            current.width >= content::MIN_CURRENT_WIDTH,
            "current {} is narrower than the drawn band admits",
            current.id,
        );
        assert!(current.period >= 1, "the beat is driven by the authored period");
    }
    // One current per layer beyond the opening one, so no layer carries the
    // worst shape the step budget warns about.
    for layer in 0..2u8 {
        let standing = chapter.currents.iter().filter(|held| held.layer == layer).count();
        assert!(standing <= 2, "layer {layer} carries {standing} currents");
    }
}

#[test]
fn the_deeper_layer_authors_the_larger_reward_and_the_larger_cost() {
    // The first strategic choice is the depth choice, and it is authored: a
    // deeper layer scales delivery up and takes Drain for it.
    let bundle = parse(&support::bundle_with(&support::content_hash())).expect("canonical");
    let content = content::read_bundle(&bundle).expect("the content reads");
    let chapter = content.chapter(0).expect("a chapter");
    assert_eq!(chapter.layers.len(), 2);
    assert!(chapter.layers[1].gain > chapter.layers[0].gain, "deeper delivers more");
    assert!(chapter.layers[1].drain > chapter.layers[0].drain, "and costs more");
    assert_eq!(chapter.layers[0].drain, 0, "the opening layer removes no Charge by depth");
}

#[test]
fn a_run_restored_under_a_different_hash_carries_on_and_says_so() {
    // The locked behaviour: the run continues, `content_changed` is true, and
    // framework reproducibility of pre-restore records is no longer claimed.
    let mut session = Session::new(&support::worker_init()).expect("versions agree");
    opened(&mut session);
    let exported = session.command("export_run", "{}");
    let text = parse(&exported)
        .expect("canonical")
        .get("body")
        .and_then(|body| body.get("text"))
        .and_then(|held| held.as_text())
        .expect("an export file")
        .to_string();

    // A session whose content differs reads the same file back.
    let other = support::worker_init_with(&support::content_hash());
    let mut fresh = Session::new(&other).expect("versions agree");
    let quoted = field_game_core::fault::quoted(&text);
    let answer = fresh.command("import_run", &format!("{{\"text\":{quoted}}}"));
    assert!(answer.contains("\"ok\":true"), "{answer}");
}

#[test]
fn an_authored_id_outside_the_closed_set_is_content_invalid() {
    let manifest = support::MANIFEST.replace("the_pull", "the_pool");
    let bundle = format!(
        "{{\"files\":[{}],\"hash\":{},\"manifest\":{}}}",
        support::files()
            .iter()
            .map(|file| field_game_core::fault::quoted(file))
            .collect::<Vec<_>>()
            .join(","),
        field_game_core::fault::quoted(&hash_of(&manifest)),
        field_game_core::fault::quoted(&manifest),
    );
    let parsed = parse(&bundle).expect("canonical");
    let fault = content::read_bundle(&parsed).expect_err("an id outside the set is refused");
    assert_eq!(fault.code(), Code::ContentInvalid);
}

fn hash_of(manifest: &str) -> String {
    let mut bytes = manifest.as_bytes().to_vec();
    for file in support::files() {
        bytes.extend_from_slice(file.as_bytes());
    }
    field_game_core::json::hex_bytes(&field_game_core::sha256::digest(&bytes))
}

/// The bundle, with one edit made to the chapter's own bytes and the digest
/// recomputed over what that leaves — so what is under test is the chapter's
/// own validation rather than the hash check in front of it.
fn edited(from: &str, to: &str) -> Result<field_game_core::content::Content, Fault> {
    let chapter = support::CHAPTER_THE_PULL.replace(from, to);
    assert_ne!(chapter, support::CHAPTER_THE_PULL, "the edit found something to change");
    let mut files: Vec<String> = support::files()
        .iter()
        .map(|file| field_game_core::fault::quoted(file))
        .collect();
    files[0] = field_game_core::fault::quoted(&chapter);
    let mut bytes = support::MANIFEST.as_bytes().to_vec();
    bytes.extend_from_slice(chapter.as_bytes());
    for file in support::files().iter().skip(1) {
        bytes.extend_from_slice(file.as_bytes());
    }
    let hash = field_game_core::json::hex_bytes(&field_game_core::sha256::digest(&bytes));
    let bundle = format!(
        "{{\"files\":[{}],\"hash\":{},\"manifest\":{}}}",
        files.join(","),
        field_game_core::fault::quoted(&hash),
        field_game_core::fault::quoted(support::MANIFEST),
    );
    content::read_bundle(&parse(&bundle).expect("canonical"))
}

/// The same, expecting a refusal, and returning the detail it named.
fn refused(from: &str, to: &str) -> String {
    let fault = edited(from, to).expect_err("the edited chapter is refused");
    assert_eq!(fault.code(), Code::ContentInvalid);
    fault.detail().expect("a named diagnostic").to_string()
}

#[test]
fn the_authored_chapter_is_read_as_it_stands() {
    // The control for the refusals below: the same path, unedited, reads.
    assert!(edited("\"period\": 30", "\"period\": 31").is_ok());
}

#[test]
fn a_chapter_may_author_an_opening_view_different_from_its_physical_compartment() {
    let content = edited("\"inside\": [2, 3, 4],", "\"inside\": [2, 3],")
        .expect("View membership is independent authored analysis data");
    let chapter = content.chapter(0).expect("the edited chapter");
    assert_eq!(chapter.opening_view.inside, vec![2, 3]);
    assert_eq!(chapter.physical_compartment.members, vec![2, 3, 4]);
}

#[test]
fn a_physical_compartment_naming_an_unplaced_node_is_refused() {
    let detail = refused(
        "\"physical_compartment\": {\n    \"members\": [2, 3, 4],",
        "\"physical_compartment\": {\n    \"members\": [2, 3, 44],",
    );
    assert_eq!(detail, "{\"reason\":\"physical_compartment\"}");
}

#[test]
fn a_condition_naming_a_current_the_chapter_never_placed_is_refused() {
    let detail = refused(
        "{ \"kind\": \"in_current\", \"current\": 1, \"steps\": 1800 }",
        "{ \"kind\": \"in_current\", \"current\": 9, \"steps\": 1800 }",
    );
    assert_eq!(detail, "{\"reason\":\"current\"}", "the field is named");
}

#[test]
fn a_condition_naming_a_layer_the_chapter_never_placed_is_refused() {
    let detail = refused("\"kind\": \"hold_position\",\n        \"layer\": 0", "\"kind\": \"hold_position\",\n        \"layer\": 6");
    assert_eq!(detail, "{\"reason\":\"layer\"}");
}

#[test]
fn a_condition_holding_a_point_off_the_plane_is_refused() {
    let detail = refused(
        "\"pos\": { \"x\": 99614720, \"y\": 130547712 },\n        \"radius\"",
        "\"pos\": { \"x\": 999614720, \"y\": 130547712 },\n        \"radius\"",
    );
    assert_eq!(detail, "{\"reason\":\"pos\"}");
}

#[test]
fn a_condition_naming_a_port_the_chapter_never_placed_is_refused() {
    let detail = refused(
        "{ \"kind\": \"ports_open\", \"ports\": [2] }",
        "{ \"kind\": \"ports_open\", \"ports\": [77] }",
    );
    assert_eq!(detail, "{\"reason\":\"ports\"}");
}

#[test]
fn a_condition_naming_a_node_that_is_open_from_the_start_is_refused() {
    // Node 6 is the reserve, and every kind other than `port` is established
    // open, so an objective asking for it to be opened could never be anything
    // a player did.
    let detail = refused(
        "{ \"kind\": \"ports_open\", \"ports\": [2] }",
        "{ \"kind\": \"ports_open\", \"ports\": [6] }",
    );
    assert_eq!(detail, "{\"reason\":\"ports\"}");
}

#[test]
fn a_condition_naming_a_route_the_chapter_never_placed_is_refused() {
    let detail = refused(
        "{ \"kind\": \"routes_flowing\", \"routes\": [2, 3, 4] }",
        "{ \"kind\": \"routes_flowing\", \"routes\": [2, 3, 40] }",
    );
    assert_eq!(detail, "{\"reason\":\"routes\"}");
}

#[test]
fn a_condition_naming_a_node_the_chapter_never_placed_is_refused() {
    let detail = refused("\"nodes\": [2, 3, 4],", "\"nodes\": [2, 3, 44],");
    assert_eq!(detail, "{\"reason\":\"nodes\"}");
}

#[test]
fn a_pattern_condition_naming_an_unplaced_route_is_refused_by_that_field() {
    let detail = refused(
        "\"kind\": \"pattern_held\",\n        \"routes\": [2, 3, 4],",
        "\"kind\": \"pattern_held\",\n        \"routes\": [2, 3, 41],",
    );
    assert_eq!(detail, "{\"reason\":\"routes\"}");
}
