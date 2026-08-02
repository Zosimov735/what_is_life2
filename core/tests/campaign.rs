//! The campaign runner: eight authored chapters, from the opening to the
//! ending.
//!
//! What the run below proves is the goal's own done-when: a campaign of eight
//! chapters of different lengths loads from authored JSON, runs from beginning
//! to ending, and reports every boundary it crosses. The opening chapter is
//! played; the seven after it stand in their own bright currents for the spans
//! they author, which is the placeholder shape the chapter agents replace.
//!
//! The rest of the file holds the parts that are cheaper to test on their own
//! than through a whole campaign: what a transition carries and what it leaves,
//! the optional test's pass-over, the authored events, and the refusals the
//! campaign-level validation makes.

use field_game_core::content::{self, CHAPTER_IDS};
use field_game_core::fault::Code;
use field_game_core::json::{parse, Json};
use field_game_core::state::{FieldState, ObjectiveStage, RecordKind};
use field_game_core::Session;

mod support;

use support::campaign::{play_chapter, Driver};

const KEY: &str = "00112233445566aa";

/// A session opened on one Form over one content bundle, with the events it
/// raised at the open drained.
fn opened_on(init: &str, form: &str) -> (Session, Driver) {
    let mut session = Session::new(init).expect("versions agree");
    let answer = session
        .command("init_run", &format!("{{\"form\":\"{form}\",\"mode\":\"new\",\"run_id\":\"{KEY}\"}}"));
    assert!(answer.contains("\"ok\":true"), "{answer}");
    let mut driver = Driver::new();
    driver.drain(&mut session);
    (session, driver)
}

/// A session opened on one Form over the shipped content.
fn opened(form: &str) -> (Session, Driver) {
    opened_on(&support::worker_init(), form)
}

/// The authored content, read the way a session reads it.
fn authored() -> content::Content {
    let bundle = parse(&support::bundle_with(&support::content_hash())).expect("canonical");
    content::read_bundle(&bundle).expect("the content reads")
}

/// The campaign the fixture chapter stands in, read the way a session reads it.
fn fixture_content() -> content::Content {
    let bundle =
        parse(&support::bundle_of(support::MANIFEST, &support::fixture_files())).expect("canonical");
    content::read_bundle(&bundle).expect("the fixture campaign reads")
}

/// A session opened over the campaign the fixture chapter stands in. The
/// opening chapter is the shipped one, so a run reaches the fixture's slot
/// exactly as a run reaches any second chapter.
fn fixture_opened(form: &str) -> (Session, Driver) {
    opened_on(&support::worker_init_of(&support::fixture_files()), form)
}

/// The chapter a campaign opens on.
fn opening(content: &content::Content) -> &content::Chapter {
    content.chapter(0).expect("the opening chapter")
}

/// The whole campaign, chapter by chapter, each by the driver its id asks for.
fn play_campaign(session: &mut Session, driver: &mut Driver, content: &content::Content) {
    for index in 0..content.chapters.len() {
        play_chapter(session, driver, content.chapter(index as u8).expect("a chapter"));
    }
}

/// How many objectives a driven campaign completes: every required one of
/// every chapter, and no optional test, because a driven run takes none.
fn required_objectives(content: &content::Content) -> usize {
    content
        .chapters
        .iter()
        .flat_map(|chapter| chapter.objectives.iter())
        .filter(|held| held.optional.is_none())
        .count()
}

/// How many Anchors a driven campaign writes, derived from the content rather
/// than counted by hand — so a chapter re-authored behind this test moves no
/// figure here.
///
/// One is written at every transition, and one at every Anchor moment the run
/// reaches. The two merge where they land on the same step: a chapter whose
/// closing completion is itself an Anchor moment writes one record, not two.
/// A chapter closed by letting its last test go has no closing completion, and
/// the last chapter of the campaign has no transition after it.
fn anchors_expected(content: &content::Content) -> usize {
    let last = content.chapters.len() - 1;
    let mut written = 0;
    for (place, chapter) in content.chapters.iter().enumerate() {
        let moments = chapter
            .anchor_moments
            .iter()
            .filter(|id| chapter.objective(id).is_some_and(|held| held.optional.is_none()))
            .count();
        let closing = chapter.objectives.last().filter(|held| held.optional.is_none());
        let merged = place < last
            && closing
                .is_some_and(|held| chapter.anchor_moments.iter().any(|id| id == &held.id));
        written += moments + usize::from(place < last) - usize::from(merged);
    }
    written
}

/// The body of one `objective_changed`, as the two fields these tests read it
/// by: the objective it offers, and the id it left.
fn offer(body: &Json) -> (String, u32, String, Option<String>) {
    let objective = body.get("objective").expect("an objective");
    (
        objective.get("id").and_then(Json::as_text).unwrap_or_default().to_string(),
        objective.get("started_step").and_then(Json::as_int).unwrap_or_default() as u32,
        objective.get("state").and_then(Json::as_text).unwrap_or_default().to_string(),
        body.get("previous_id").and_then(Json::as_text).map(str::to_string),
    )
}

#[test]
fn the_placeholder_campaign_runs_from_its_opening_to_its_ending() {
    let content = authored();
    let (mut session, mut driver) = opened("thread");

    // The opening chapter, played through the phase list the dispatch names for
    // it: its objectives ask for a current, a hold, a Pulse, a Port, a circuit,
    // the pattern carried through the break, and depth.
    play_chapter(&mut session, &mut driver, opening(&content));
    assert_eq!(
        session.run().expect("a run").state().progress.chapter_index,
        1,
        "the opening chapter completed and the run carried into the next",
    );

    // The seven after it, each by whichever driver its own id asks for: a
    // scripted phase list where the dispatch names one, and the rest driver
    // where it does not. Every one is a different length.
    for index in 1..content.chapters.len() {
        play_chapter(&mut session, &mut driver, content.chapter(index as u8).expect("a chapter"));
    }

    // The campaign reached its ending, and the mode the ending leaves.
    let completed = driver.of("run_completed");
    assert_eq!(completed.len(), 1, "one campaign, one ending");
    let body = &completed[0].2;
    // The ending the last chapter authors, resolved through the marks it
    // authors: a chapter with no marks reports its `ending_key` as it stands,
    // and one with marks reports `<ending_key>.<form>.<mark>`. Read off the
    // content rather than written here, so a chapter re-authored behind this
    // moves no literal a reader would have to chase.
    let last = content.chapter(7).expect("a chapter");
    let reached = body.get("ending_id").and_then(Json::as_text).expect("an ending");
    assert_eq!(
        reached,
        last.ending_id("thread", &session.run().expect("a run").state().progress.complete),
        "the ending the last chapter's own marks resolve to",
    );
    assert!(
        reached.starts_with(&last.ending_key),
        "and it is that chapter's own ending key: {reached}",
    );
    assert_eq!(
        body.get("chapter_index").and_then(Json::as_int),
        Some(7),
        "reached on the last chapter of the eight",
    );
    assert_eq!(
        body.get("continuation_unlocked").and_then(Json::as_bool),
        Some(true),
        "and the campaign authored the whole closed set, so the continuation unlocks",
    );
    assert_eq!(session.lifecycle(), "ended");

    // Every chapter was entered, in the closed set's own order, and each one
    // told the shell which it was.
    let chapter_events = driver.of("chapter_changed");
    let entered: Vec<(i64, &str)> = chapter_events
        .iter()
        .map(|(_, _, body)| {
            (
                body.get("chapter_index").and_then(Json::as_int).expect("an index"),
                body.get("title_key").and_then(Json::as_text).expect("a key"),
            )
        })
        .collect();
    assert_eq!(entered.len(), 8, "eight chapters, each announced once");
    for (place, id) in CHAPTER_IDS.iter().enumerate() {
        assert_eq!(entered[place].0, place as i64);
        assert_eq!(entered[place].1, format!("chapter.{id}"));
        let event = chapter_events[place]
            .2
            .get("view")
            .expect("the full View");
        let expected = parse(
            &content.chapter(place as u8).expect("the entered chapter").opening_view.written(),
        )
        .expect("the authored View is canonical");
        assert_eq!(
            event, &expected,
            "chapter {place} announces its authoritative View"
        );
    }

    // The objective line stands on one objective at a time throughout, and
    // clears at the end because the campaign has nothing more to offer.
    let objectives = driver.of("objective_changed");
    let last = objectives.last().expect("the line moved").2.get("objective").expect("one");
    assert_eq!(last.get("state").and_then(Json::as_text), Some("hidden"));
    assert_eq!(last.get("id").and_then(Json::as_text), Some(""));
    // Every required objective of the eight chapters, and no optional test: a
    // test that was not taken was not completed. The count is read off the
    // content rather than written here, so a chapter re-authored behind this
    // moves no figure a reader would have to chase.
    let done = session.run().expect("a run").state().progress.complete.clone();
    assert_eq!(done.len(), required_objectives(&content));
    for chapter in &content.chapters {
        for objective in &chapter.objectives {
            let taken = done.iter().any(|held| held == &objective.id);
            assert_eq!(taken, objective.optional.is_none(), "{}", objective.id);
        }
    }

    // A run that has ended takes no further step, whatever a frame asks for.
    let at = session.run().expect("a run").step();
    driver.rest(&mut session, 300);
    assert_eq!(session.run().expect("a run").step(), at, "an ended run advances nothing");

    // Every boundary wrote its records: an Anchor at each of the seven
    // transitions, and an autosave record beside it.
    let anchors = session.run().expect("a run").state().anchors.clone();
    let written = anchors.iter().filter(|held| held.kind == RecordKind::Anchor).count();
    assert_eq!(written, anchors_expected(&content));
    assert!(
        anchors.iter().any(|held| held.kind == RecordKind::Auto),
        "and the autosave cadence wrote beside them",
    );
}

#[test]
fn a_driven_campaign_passes_over_an_optional_test_at_its_own_span() {
    // The between-two-required shape of an optional test, driven through the
    // campaign rather than read off the derivation. One shape and one chapter's
    // test: the campaign authors seven of them — one in each chapter but the
    // last, which authors a single objective and no test at all — and the
    // reading pinned here is that a test let go is passed over at
    // `started_step + span` with the objective after it installed. What the
    // other six share with this one is that none is terminal, which is the
    // assertion below rather than a claim made by the name of this test.
    let content = authored();
    let (mut session, mut driver) = opened("thread");
    play_campaign(&mut session, &mut driver, &content);
    assert_eq!(
        session.lifecycle(),
        "ended",
        "the campaign reached its ending past every one of its optional tests",
    );

    // Seven optional tests stand across the campaign, and not one of them is
    // the last objective of its chapter: a test a chapter ended on would be a
    // chapter a player could leave by letting something go, and the shape this
    // test pins would be the wrong shape for it.
    let mut tests = 0;
    for index in 0..content.chapters.len() {
        let chapter = content.chapter(index as u8).expect("a chapter");
        tests += chapter.objectives.iter().filter(|held| held.optional.is_some()).count();
        assert!(
            chapter.objectives.last().is_none_or(|held| held.optional.is_none()),
            "chapter {index} ends on an optional test",
        );
    }
    // Every chapter authors at least one, and the count is the content's own
    // rather than a literal here: The Quiet Edge authors two, because its
    // opening tests are what read the continuity the run arrived with.
    assert!(tests >= content.chapters.len(), "the campaign authors {tests} optional tests");

    let told: Vec<(String, u32, String, Option<String>)> =
        driver.of("objective_changed").iter().map(|(_, _, body)| offer(body)).collect();
    let offered_at = |id: &str| -> u32 {
        told.iter()
            .find(|(held, _, state, _)| held == id && state == "active")
            .unwrap_or_else(|| panic!("{id} was offered"))
            .1
    };
    let past = |id: &str| -> &(String, u32, String, Option<String>) {
        told.iter()
            .find(|(_, _, _, previous)| previous.as_deref() == Some(id))
            .unwrap_or_else(|| panic!("{id} was left"))
    };

    // The chapter that offers a test between two required objectives: the
    // sequence installs the one after it at exactly `started_step + span`.
    let mesh = content.chapter(4).expect("a chapter");
    // Found rather than indexed: which place a chapter puts its test in is the
    // chapter's own business, and this test is about the sequence rather than
    // about that place.
    let place = mesh
        .objectives
        .iter()
        .position(|held| held.optional.is_some())
        .expect("the chapter offers a test");
    assert!(place + 1 < mesh.objectives.len(), "and offers it between two objectives");
    let test = &mesh.objectives[place];
    let span = test.optional.expect("an optional test");
    let after = past(&test.id);
    assert_eq!(after.1, offered_at(&test.id) + span, "passed over at its own step");
    assert_eq!(after.0, mesh.objectives[place + 1].id, "and the objective after it stands");
    assert_eq!(after.2, "active");

    // The offers-it-last shape left the shipped campaign when real chapters
    // replaced the placeholders (no authored chapter ends on a test — The
    // Rewrite keeps its Impulse ladder by requiring its close), and what
    // replaced the driven coverage is narrower than the trim note used to say.
    // Ledgered exactly:
    //
    // - **The sequence half is pinned.**
    //   `a_chapter_closed_by_a_pass_over_closes_and_grants_what_a_close_is_worth`
    //   drives `script` over a chapter whose last objective is a test and pins
    //   all four readings: the close fires on the pass-over step and not the
    //   one before it, the close grants three, the objective line goes hidden,
    //   and the test it closed on is *not* recorded complete.
    // - **The run half is pinned by nothing.** That the run then calls
    //   `close_chapter` and raises `chapter_changed` at `offered + span` from a
    //   pass-over is driven by no test in this build: every driven close in the
    //   suite comes from a completion, because no shipped chapter ends on a
    //   test. The path is the same `chapter_complete` flag either way, which is
    //   the argument for it standing — but it is an argument, not a drive, and
    //   this comment is the ledger entry rather than a claim of coverage.
}

#[test]
fn an_ended_run_answers_the_commands_the_ended_state_admits_and_no_others() {
    // The locked command table, read at the one state that had no way of being
    // reached before: `init_run` and `import_run` are valid in `ended`, the
    // queued-change commands are `still`-only, and everything else that is
    // valid in a loaded state is still valid here.
    let content = authored();
    let (mut session, mut driver) = opened("thread");
    play_campaign(&mut session, &mut driver, &content);
    assert_eq!(session.lifecycle(), "ended");

    // A run that ended can still be exported, and the export imports.
    let answer = session.command("export_run", "{}");
    assert!(answer.contains("\"ok\":true"), "{answer}");

    // The queued-change commands are refused with the state envelope.
    let refused = session.command("queue_plan", "{\"plan\":{\"op\":\"cut\",\"route\":1}}");
    assert!(refused.contains("\"code\":\"state\""), "{refused}");
    assert!(refused.contains("\"actual\":\"ended\""), "{refused}");

    // And a fresh run opens from here, which is what `ended` is for.
    let opened = session.command(
        "init_run",
        "{\"form\":\"ring\",\"mode\":\"new\",\"run_id\":\"aabbccdd00112233\"}",
    );
    assert!(opened.contains("\"ok\":true"), "{opened}");
    assert_eq!(session.lifecycle(), "running");
    assert_eq!(session.run().expect("a run").state().progress.chapter_index, 0);
}

#[test]
fn a_transition_carries_the_run_and_establishes_the_next_chapters_field() {
    let content = authored();
    let (mut session, mut driver) = opened("thread");
    play_chapter(&mut session, &mut driver, opening(&authored()));

    let state = session.run().expect("a run").state().clone();
    let opening = content.chapter(0).expect("a chapter");
    let entered = content.chapter(1).expect("a chapter");

    // What the run carries: its key, its branch, its step counter, its Impulse,
    // its completed objectives, and the Form it opened on.
    assert_eq!(state.run_id, KEY);
    assert_eq!(state.branch_nonce, 0);
    assert_eq!(state.progress.chapter_index, 1);
    assert!(state.now.step > 0, "the step counter is the campaign's and does not restart");
    assert_eq!(
        state.progress.complete.len(),
        opening.objectives.iter().filter(|held| held.optional.is_none()).count(),
        "every required objective of the chapter behind it, and no test it let go of",
    );
    assert_eq!(state.progress.impulse, 6, "the Impulse the completions granted, at its cap");
    assert_eq!(session.run().expect("a run").form(), "thread");

    // What it does not: the Field is the next chapter's own, established from
    // its authored content under the Form the run stands on.
    assert_eq!(
        state.now.ports.iter().filter(|port| port.kind != field_game_core::field::NodeKind::Form).count(),
        entered.ports.len(),
    );
    assert_eq!(state.now.routes.len(), entered.routes.len());
    assert_eq!(state.now.currents.len(), entered.currents.len());
    assert_eq!(state.view, entered.opening_view, "under the chapter's authored opening View");
    assert!(state.slate.is_none(), "and under no evaluation record: the candidates named old Nodes");
    assert_eq!(
        state.now.forms[0].route_reach,
        content.form("thread").expect("a Form").route_reach,
        "the Form's own parameters are copied into the new Field",
    );

    // The trajectory restarts on the state the transition leaves, which is what
    // makes the boundary settled rather than replayed across: every step the
    // run has taken since is one the new chapter's Field ran.
    let boundary = driver.of("chapter_changed").last().expect("the transition").1;
    assert_eq!(state.trace.start_step, boundary);
    assert_eq!(state.trace.steps.len() as u32, state.now.step - boundary);
    state.coherent().expect("the state a transition leaves is coherent");
    assert_eq!(
        state.progress.objective.started_step, boundary,
        "and the chapter's opening objective was offered at the boundary",
    );

    // The next chapter's schedule is seated at the step the chapter opened on,
    // so an authored start_step is counted from its own chapter.
    let seated = state.pressures.first().expect("the chapter's primary pressure");
    let requested = entered.pressure_schedule.first().expect("one entry");
    assert_eq!(seated.pressure, requested.pressure);
    assert!(seated.primary, "the chapter's primary pressure is the one it named");
    assert_eq!(seated.start_step, boundary + requested.start_step);
}

#[test]
fn a_quick_retry_across_a_chapter_boundary_lands_at_the_opening_of_the_next() {
    let (mut session, mut driver) = opened("thread");
    play_chapter(&mut session, &mut driver, opening(&authored()));
    // The newest Anchor the run holds, which is the transition's: the opening
    // chapter names an Anchor moment of its own part way through, so the
    // transition's is the last of the two rather than the first.
    let anchor = session
        .run()
        .expect("a run")
        .state()
        .anchors
        .iter()
        .filter(|held| held.kind == RecordKind::Anchor)
        .max_by_key(|held| held.step)
        .expect("the transition wrote an Anchor")
        .clone();
    let at = anchor.step;
    assert_eq!(anchor.chapter_index, 1, "the Anchor names the chapter the run entered");
    assert_eq!(
        at,
        driver.of("chapter_changed").last().expect("the transition").1,
        "and it was written at the step the transition landed on",
    );

    // Play on into the second chapter, then take the Anchor back.
    driver.rest(&mut session, 60);
    assert!(session.run().expect("a run").step() > at);
    let answer =
        session.command("restore_checkpoint", &format!("{{\"anchor_id\":{}}}", anchor.anchor_id));
    assert!(answer.contains("\"ok\":true"), "{answer}");
    let restored = session.run().expect("a run").state().clone();
    assert_eq!(restored.now.step, at);
    assert_eq!(restored.progress.chapter_index, 1);
    assert_eq!(restored.rng, anchor.rng, "Quick Retry restores the exact random state");
    assert_eq!(restored.branch_nonce, anchor.branch_nonce);

    // And the shell is told what it stands on, which is the chapter it entered.
    driver.drain(&mut session);
    let told = driver.of("chapter_changed");
    let last = told.last().expect("a chapter is announced on every reopening");
    assert_eq!(last.2.get("title_key").and_then(Json::as_text), Some("chapter.the_edge"));
    assert_eq!(
        last.2.get("view"),
        Some(&parse(&restored.view.written()).expect("the restored View is canonical")),
        "a reopen announces the restored authoritative View",
    );

    // The chapter's authored event fires again on the restored run, at its own
    // step: the trigger is read off the objective's `started_step` and the step
    // counter, both of which the record carried, so a Quick Retry replays it
    // rather than losing it.
    let content = authored();
    let entered = content.chapter(1).expect("a chapter");
    assert!(!port_open(&session, 4), "the Port stands closed at the chapter's opening");
    driver.rest(&mut session, entered.events[0].at);
    assert_eq!(session.run().expect("a run").step(), at + entered.events[0].at);
    assert!(port_open(&session, 4), "and the event opened it again after the restore");

    // The run carries on from there, and reaches the same boundary again.
    play_chapter(&mut session, &mut driver, entered);
    assert_eq!(session.run().expect("a run").state().progress.chapter_index, 2);
}

/// Whether one Node's Port stands open in the loaded run.
fn port_open(session: &Session, node: u32) -> bool {
    session
        .run()
        .expect("a run")
        .state()
        .now
        .ports
        .iter()
        .find(|port| port.node == node)
        .expect("the authored Port")
        .open
}

#[test]
fn a_campaign_span_replays_byte_for_byte() {
    // The campaign's own byte-equivalence: two runs on the same key, the same
    // Form, and the same frames cross the same boundaries and serialize
    // identically — the transition included, because the transition is derived
    // from the state the frames produced and from nothing else.
    let content = authored();
    let bytes = |chapters: usize| -> (String, u8) {
        let (mut session, mut driver) = opened("ring");
        play_chapter(&mut session, &mut driver, opening(&content));
        for index in 1..chapters {
            play_chapter(&mut session, &mut driver, content.chapter(index as u8).expect("a chapter"));
        }
        let run = session.run().expect("a run");
        (run.state().payload(), run.state().progress.chapter_index)
    };
    let (first, at_first) = bytes(3);
    let (second, at_second) = bytes(3);
    assert_eq!(at_first, 3, "three chapters behind it");
    assert_eq!(at_second, at_first);
    assert_eq!(first, second, "the same frames produce the same bytes across a transition");
}

#[test]
fn an_export_taken_after_a_transition_imports_into_the_chapter_it_stands_in() {
    let (mut session, mut driver) = opened("thread");
    play_chapter(&mut session, &mut driver, opening(&authored()));
    // A few steps into the chapter the run entered, and short of the step its
    // first authored event falls due on: a driven chapter stops within one
    // batch of its boundary, so the rest is kept well inside that event's own
    // span rather than sized against where the batch happened to land.
    driver.rest(&mut session, 10);
    let answer = session.command("export_run", "{}");
    let exported = parse(&answer).expect("canonical");
    let text = exported
        .get("body")
        .and_then(|body| body.get("text"))
        .and_then(Json::as_text)
        .expect("the export file")
        .to_string();
    let at = session.run().expect("a run").step();

    let mut fresh = Session::new(&support::worker_init()).expect("versions agree");
    let quoted = field_game_core::fault::quoted(&text);
    let answer = fresh.command("import_run", &format!("{{\"text\":{quoted}}}"));
    assert!(answer.contains("\"ok\":true"), "{answer}");
    let state = fresh.run().expect("a run").state().clone();
    assert_eq!(state.progress.chapter_index, 1);
    assert_eq!(state.now.step, at);

    // The chapter's authored event fires in the imported run, at its own step:
    // an import carries the objective's `started_step` and the step counter,
    // which is the whole of the trigger, so the disruption lands where it would
    // have landed in the run the export was taken from.
    let content = authored();
    let entered = content.chapter(1).expect("a chapter");
    let boundary = state.progress.objective.started_step;
    let due = boundary + entered.events[0].at;
    assert!(due > at, "the export was taken before the event fell due");
    let mut driver = Driver::new();
    driver.drain(&mut fresh);
    assert!(!port_open(&fresh, 4), "the Port stands closed in the imported run");
    driver.rest(&mut fresh, due - at);
    assert_eq!(fresh.run().expect("a run").step(), due);
    assert!(port_open(&fresh, 4), "and the event opened it at exactly its own step");

    // The imported run carries on inside the chapter it was in, and reaches
    // that chapter's own boundary.
    play_chapter(&mut fresh, &mut driver, entered);
    assert_eq!(fresh.run().expect("a run").state().progress.chapter_index, 2);
}

#[test]
fn an_authored_event_lands_at_its_own_step_and_ends_the_window() {
    // The timing rule, driven against the fixture chapter: `at` steps after the
    // objective it names was offered, on exactly one boundary, with the window
    // ending on it. The two events it reads stand at **different** offsets, so
    // the second half of this test advances the run rather than reading the
    // same step twice.
    let content = fixture_content();
    let chapter = content.chapter(1).expect("a chapter");
    assert!(chapter.events.len() >= 2, "the chapter authors an opening pair of events");
    let content::Effect::PortOpen { node: opened_node, open: true } = chapter.events[0].effect else {
        panic!("the chapter's first event opens a Port");
    };
    assert!(
        chapter.events[1].at > chapter.events[0].at,
        "the second event stands at its own offset, or the advance below is no advance",
    );

    let (mut session, mut driver) = fixture_opened("thread");
    play_chapter(&mut session, &mut driver, opening(&content));
    let opened_at = session.run().expect("a run").state().progress.objective.started_step;
    let port_open = |session: &Session, node: u32| -> bool {
        session
            .run()
            .expect("a run")
            .state()
            .now
            .ports
            .iter()
            .find(|port| port.node == node)
            .expect("the authored Port")
            .open
    };
    let port_open = |session: &Session| port_open(session, opened_node);
    assert!(!port_open(&session), "the Port stands closed as the chapter opens");

    // One step short of the first event, then the step it falls due on.
    let due = opened_at + chapter.events[0].at;
    let short = (due - 1 - session.run().expect("a run").step()) as u16;
    driver.frame(&mut session, short, (0, 0), false, false);
    assert!(!port_open(&session), "and stands closed until the step the event names");
    driver.frame(&mut session, 1, (0, 0), false, false);
    assert_eq!(session.run().expect("a run").step(), due);
    assert!(port_open(&session), "the event opened it at exactly its own step");

    // The trajectory restarts on the state the event leaves: an authored event
    // is a membership boundary like every other change between steps.
    let state = session.run().expect("a run").state().clone();
    assert_eq!(state.trace.start_step, state.now.step);
    state.coherent().expect("the state an event leaves is coherent");

    // And the chapter's second authored event stands at its own step too,
    // whatever that event does: the effect is read off the content, so this
    // reads a Port closed again as readily as a second Port opened. The run is
    // advanced to that step first — the two offsets differ, which the assertion
    // above holds them to.
    let content::Effect::PortOpen { node: second_node, open: second_open } =
        chapter.events[1].effect
    else {
        panic!("the fixture's second event is a Port event");
    };
    let standing_before = session
        .run()
        .expect("a run")
        .state()
        .now
        .ports
        .iter()
        .find(|port| port.node == second_node)
        .expect("the authored Port")
        .open;
    assert_ne!(standing_before, second_open, "the second event has something to change");
    driver.rest(&mut session, chapter.events[1].at - chapter.events[0].at);
    assert_eq!(
        session.run().expect("a run").step(),
        opened_at + chapter.events[1].at,
        "the run stands on the second event's own step",
    );
    let standing = session
        .run()
        .expect("a run")
        .state()
        .now
        .ports
        .iter()
        .find(|port| port.node == second_node)
        .expect("the authored Port")
        .open;
    assert_eq!(standing, second_open, "the second event stands at its own step");
}

#[test]
fn an_authored_route_cut_severs_its_own_route_and_replays_across_the_boundary() {
    // The fourth authored-event kind, driven end to end: it takes one Route out
    // of the Route set at the step it names, on the same boundary-settled
    // carriage every other kind rides, and the run replays across it — which is
    // the whole of what a new event kind has to prove, because an event is a
    // change to authored Field state between two steps and a step that cannot
    // be replayed is a run that cannot be restored.
    let content = fixture_content();
    let chapter = content.chapter(1).expect("a chapter");
    let cut = chapter
        .events
        .iter()
        .find_map(|event| match event.effect {
            content::Effect::RouteCut { route } => Some((event.at, route)),
            _ => None,
        })
        .expect("the fixture authors a route cut");

    // Driven twice on the same key and the same frames, to the same step past
    // the cut: the bytes agree, which is what says the cut is derived from the
    // frames and from nothing else.
    let played = |take_the_record: bool| -> (String, usize, Option<u32>, Option<u32>) {
        let (mut session, mut driver) = fixture_opened("thread");
        play_chapter(&mut session, &mut driver, opening(&content));
        let opened_at = session.run().expect("a run").state().progress.objective.started_step;
        let anchor = take_the_record.then(|| {
            session
                .run()
                .expect("a run")
                .state()
                .anchors
                .iter()
                .filter(|held| held.kind == RecordKind::Anchor)
                .max_by_key(|held| held.step)
                .expect("the transition wrote an Anchor")
                .anchor_id
        });
        let before = session.run().expect("a run").state().now.routes.len();
        assert!(
            session.run().expect("a run").state().now.routes.iter().any(|held| held.route == cut.1),
            "the Route the event names stands before it",
        );

        // One step short of the cut, then the step it falls due on.
        let due = opened_at + cut.0;
        let short = due - 1 - session.run().expect("a run").step();
        driver.rest(&mut session, short);
        assert_eq!(
            session.run().expect("a run").state().now.routes.len(),
            before,
            "nothing is severed before the step the event names",
        );
        driver.frame(&mut session, 1, (0, 0), false, false);
        let state = session.run().expect("a run").state().clone();
        assert_eq!(state.now.step, due);
        assert!(
            !state.now.routes.iter().any(|held| held.route == cut.1),
            "the event severed the Route it names",
        );
        assert_eq!(state.now.routes.len(), before - 1, "and severed exactly that one");
        assert_eq!(
            state.trace.start_step, state.now.step,
            "the cut ends the active window, as every event does",
        );
        state.coherent().expect("the state a severed Route leaves is coherent");

        // Past it: the removal is permanent and the run plays on, the way a
        // committed cut and a Fracture break both leave it.
        driver.rest(&mut session, 120);
        let state = session.run().expect("a run").state().clone();
        assert!(!state.now.routes.iter().any(|held| held.route == cut.1), "and it stays severed");
        (state.payload(), state.now.routes.len(), Some(state.now.step), anchor)
    };

    let (first, standing, at, anchor) = played(true);
    let (second, again, at_again, _) = played(false);
    assert_eq!(standing, again);
    assert_eq!(at, at_again);
    assert_eq!(first, second, "the same frames produce the same bytes across a severed Route");

    // And a Quick Retry back across it: the Anchor the transition wrote is
    // before the cut, so the retried run meets the event again and lands on the
    // same bytes — the trigger is payload state, so a restore replays it rather
    // than losing it.
    let anchor_id = anchor.expect("the transition wrote an Anchor");
    let (mut session, mut driver) = fixture_opened("thread");
    play_chapter(&mut session, &mut driver, opening(&content));
    let opened_at = session.run().expect("a run").state().progress.objective.started_step;
    driver.rest(&mut session, cut.0 + 300);
    let answer = session.command("restore_checkpoint", &format!("{{\"anchor_id\":{anchor_id}}}"));
    assert!(answer.contains("\"ok\":true"), "{answer}");
    let restored = session.run().expect("a run").state().clone();
    assert_eq!(
        restored.now.step, opened_at,
        "the Anchor the transition wrote stands at the chapter's own opening",
    );
    assert!(
        restored.now.routes.iter().any(|held| held.route == cut.1),
        "and the Route stands again in the record the retry landed on",
    );
    driver.drain(&mut session);
    let due = opened_at + cut.0;
    let short = due - 1 - session.run().expect("a run").step();
    driver.rest(&mut session, short);
    driver.frame(&mut session, 1, (0, 0), false, false);
    driver.rest(&mut session, 120);
    let replayed = session.run().expect("a run").state().clone();
    assert_eq!(Some(replayed.now.step), at);
    assert!(
        !replayed.now.routes.iter().any(|held| held.route == cut.1),
        "the event severed the Route again after the retry",
    );
    assert_eq!(replayed.now.routes.len(), standing);
}

#[test]
fn an_optional_test_is_passed_over_when_its_span_runs_out() {
    // The optional test, read through the sequence rather than through the
    // Field: an objective the chapter does not require stands for the span it
    // authors and is let go of afterwards, and the chapter carries on.
    let mut progress = field_game_core::state::Progress::opening();
    let chapter = optional_chapter();
    assert!(content::offer_opening(&mut progress, &chapter, 0));
    assert_eq!(progress.objective.id, "objective.the_pull.required");

    // The required objective completes, and the optional test is offered next.
    progress.complete.push("objective.the_pull.required".to_string());
    let offered = chapter.offered(&progress).expect("the optional test is offered");
    assert_eq!(offered.id, "objective.the_pull.optional");
    progress.objective = field_game_core::state::ObjectiveState {
        id: offered.id.clone(),
        state: ObjectiveStage::Active,
        progress: 0,
        target: Some(offered.condition.target()),
        started_step: 100,
        completed_step: None,
    };

    // While it stands it is what the sequence offers, and the chapter is not
    // complete.
    assert_eq!(chapter.offered(&progress).map(|held| held.id.as_str()), Some("objective.the_pull.optional"));

    // Past its span the sequence moves on. What was passed over never enters
    // the completed list, and is never offered again.
    let mut ordered = progress.clone();
    ordered.objective.id = "objective.the_pull.after".to_string();
    ordered.objective.started_step = 160;
    assert_eq!(
        chapter.offered(&ordered).map(|held| held.id.as_str()),
        Some("objective.the_pull.after"),
        "the objective after it is what stands",
    );
    assert!(
        !ordered.complete.iter().any(|held| held == "objective.the_pull.optional"),
        "a test that was not taken was not completed",
    );
}

#[test]
fn the_step_that_passes_a_test_over_installs_the_next_objective() {
    // The pass-over driven through the script itself rather than read off the
    // derivation: the step at `started_step + span` is the one that moves the
    // line, and the objective it moves to is the one the sequence offers next.
    let chapter = optional_chapter();
    let mut progress = field_game_core::state::Progress::opening();
    progress.complete.push("objective.the_pull.required".to_string());
    progress.objective = standing("objective.the_pull.optional", &chapter, 100);

    let mut field = FieldState::opening();
    let mut cues = Vec::new();

    // One step short of the span: nothing moves.
    field.step = 159;
    let outcome = script(&mut progress, &field, &chapter, &mut cues);
    assert_eq!(progress.objective.id, "objective.the_pull.optional");
    assert!(outcome.changed.is_empty(), "the test still stands");

    // The step the span runs out on.
    field.step = 160;
    let outcome = script(&mut progress, &field, &chapter, &mut cues);
    assert_eq!(progress.objective.id, "objective.the_pull.after");
    assert_eq!(progress.objective.started_step, 160, "offered at the step it was passed over");
    assert_eq!(progress.objective.state, ObjectiveStage::Active);
    assert!(!outcome.chapter_complete, "a required objective still stands after it");
    let told = outcome.changed.first().expect("the line moved");
    assert_eq!(told.1.as_deref(), Some("objective.the_pull.optional"), "naming what it left");
    assert!(
        !progress.complete.iter().any(|held| held == "objective.the_pull.optional"),
        "a test that was not taken was not completed",
    );

    // And it is never offered again, however far the run goes.
    field.step = 400;
    script(&mut progress, &field, &chapter, &mut cues);
    assert_eq!(progress.objective.id, "objective.the_pull.after");
}

#[test]
fn a_chapter_closed_by_a_pass_over_closes_and_grants_what_a_close_is_worth() {
    // The Impulse edge, pinned: a chapter closes when its sequence has nothing
    // more to offer, and a close is worth three whichever trigger closed it.
    // A close that is itself a completion is paid by that completion; a close
    // by pass-over has no completion to pay it, so the close grants.
    let mut chapter = optional_chapter();
    chapter.objectives.pop();
    let mut progress = field_game_core::state::Progress::opening();
    progress.complete.push("objective.the_pull.required".to_string());
    progress.impulse = 0;
    progress.objective = standing("objective.the_pull.optional", &chapter, 0);

    let mut field = FieldState::opening();
    let mut cues = Vec::new();
    field.step = 59;
    assert!(!script(&mut progress, &field, &chapter, &mut cues).chapter_complete);
    assert_eq!(progress.impulse, 0, "nothing is granted while the test stands");

    field.step = 60;
    let outcome = script(&mut progress, &field, &chapter, &mut cues);
    assert!(outcome.chapter_complete, "letting the last test go closes the chapter");
    assert_eq!(progress.impulse, 3, "and the close grants what a close is worth");
    assert_eq!(progress.objective.state, ObjectiveStage::Hidden);
    assert!(
        !progress.complete.iter().any(|held| held == "objective.the_pull.optional"),
        "the test it closed on is still not completed",
    );
}

#[test]
fn a_close_that_is_a_completion_is_paid_once() {
    // The other half of the same rule: a chapter closed by completing its last
    // objective grants three, not six — the completion's grant is the close's.
    let mut chapter = optional_chapter();
    chapter.objectives.truncate(1);
    let mut progress = field_game_core::state::Progress::opening();
    progress.impulse = 0;
    progress.objective = standing("objective.the_pull.required", &chapter, 0);

    let mut field = FieldState::opening();
    field.step = 1;
    let mut cues = Vec::new();
    let raised = [field_game_core::field::Cue { kind: field_game_core::field::CUE_PULSE_EMITTED, a: 0, b: 0 }];
    let reading = content::StepReading { field: &field, cues: &raised };
    let outcome =
        content::advance_objectives(&mut progress, &field, &chapter, &reading, &mut cues);
    assert!(outcome.chapter_complete);
    assert_eq!(progress.impulse, 3, "one close, one grant");
    assert_eq!(progress.complete, vec!["objective.the_pull.required".to_string()]);
}

/// The objective state one id stands in, offered at a step.
fn standing(id: &str, chapter: &content::Chapter, at: u32) -> field_game_core::state::ObjectiveState {
    let objective = chapter.objective(id).expect("an authored objective");
    field_game_core::state::ObjectiveState {
        id: id.to_string(),
        state: ObjectiveStage::Active,
        progress: 0,
        target: Some(objective.condition.target()),
        started_step: at,
        completed_step: None,
    }
}

/// One step of the authored sequence against a Field nothing happens in, so
/// what is under test is the sequence rather than the Field.
fn script(
    progress: &mut field_game_core::state::Progress,
    field: &FieldState,
    chapter: &content::Chapter,
    cues: &mut Vec<field_game_core::field::Cue>,
) -> content::ScriptOutcome {
    let reading = content::StepReading { field, cues: &[] };
    content::advance_objectives(progress, field, chapter, &reading, cues)
}

/// A chapter with one required objective, one optional test, and one more
/// required objective after it — the shape the pass-over rule is read against.
fn optional_chapter() -> content::Chapter {
    let condition = || content::Condition::PulseReleased { count: 1 };
    content::Chapter {
        id: "the_pull".to_string(),
        title_key: "chapter.the_pull".to_string(),
        ending_key: "ending.the_pull".to_string(),
        ending_marks: Vec::new(),
        events: Vec::new(),
        layers: Vec::new(),
        forms: Vec::new(),
        ports: Vec::new(),
        routes: Vec::new(),
        currents: Vec::new(),
        authored_boundaries: Vec::new(),
        objectives: vec![
            content::Objective {
                id: "objective.the_pull.required".to_string(),
                condition: condition(),
                optional: None,
            },
            content::Objective {
                id: "objective.the_pull.optional".to_string(),
                condition: condition(),
                optional: Some(60),
            },
            content::Objective {
                id: "objective.the_pull.after".to_string(),
                condition: condition(),
                optional: None,
            },
        ],
        anchor_moments: Vec::new(),
        opening_view: field_game_core::state::ViewDeclaration::opening(),
        pressure_schedule: Vec::new(),
    }
}

/// The campaign with the fixture chapter standing in its slot and one edit made
/// to the fixture's own bytes, the digest recomputed over what that leaves — so
/// what is under test is the validation rather than the hash check in front of
/// it.
///
/// **Why a fixture and not the shipped chapter.** Each refusal below has to
/// reach for a literal that is present and then break it. Aimed at a shipped
/// chapter, that makes the chapter's authoring unfree: the last agent to author
/// this slot recorded that its Fracture stands on route 1 and its opening View
/// is `[2, 3]` because tests here needed those exact strings to exist. The
/// fixture carries the literals instead, so a chapter may be re-authored
/// without a refusal test following it around. It keeps the slot's own `id`, so
/// the closed-set ordering check reads it as it reads any chapter, and
/// `the_shipped_campaign_loads_as_it_stands` keeps one test on the real bytes.
fn edited(from: &str, to: &str) -> Result<content::Content, field_game_core::fault::Fault> {
    let mut files = support::fixture_files();
    let place = support::FIXTURE_PLACE;
    let held = files[place].replace(from, to);
    assert_ne!(held, files[place], "the edit found something to change");
    files[place] = held;
    let bundle = support::bundle_of(support::MANIFEST, &files);
    content::read_bundle(&parse(&bundle).expect("canonical"))
}

/// The same, expecting a refusal, and returning the detail it named.
fn refused(from: &str, to: &str) -> String {
    let fault = edited(from, to).expect_err("the edited campaign is refused");
    assert_eq!(fault.code(), Code::ContentInvalid);
    fault.detail().expect("a named diagnostic").to_string()
}

#[test]
fn the_shipped_campaign_loads_as_it_stands() {
    // The one test here that reads the shipped bytes: every authored chapter,
    // Form, and pressure table the manifest lists, through the whole of
    // `read_bundle` and the campaign-level validation behind it. The refusals
    // below run against a fixture, and this is what says the fixture is not
    // standing in for content that would itself be refused.
    let content = authored();
    assert_eq!(content.chapters.len(), CHAPTER_IDS.len(), "all eight chapters load");
    for (place, id) in CHAPTER_IDS.iter().enumerate() {
        assert_eq!(&content.chapters[place].id, id, "in the closed set's own order");
    }
}

#[test]
fn the_fixture_campaign_is_read_as_it_stands() {
    // The control for the refusals below: the same path, with an edit that
    // changes nothing the validation reads, loads.
    assert!(edited("\"period\": 30", "\"period\": 31").is_ok());
}

#[test]
fn two_chapters_naming_one_objective_are_refused() {
    // The second chapter would open with its only objective already behind it,
    // because the completed list is the campaign's rather than the chapter's.
    let detail =
        refused("objective.the_edge.hold_the_current", "objective.the_pull.follow_current");
    assert_eq!(detail, "{\"reason\":\"objectives\"}");
}

#[test]
fn a_chapter_out_of_the_closed_sets_order_is_refused() {
    // The manifest names the order chapters are played in, and the closed set
    // names the order they stand in. A campaign that disagrees with the closed
    // set is refused rather than played in the order it happens to list.
    let manifest = support::MANIFEST.replace(
        "\"the_edge\",\n    \"the_loop\"",
        "\"the_loop\",\n    \"the_edge\"",
    );
    assert_ne!(manifest, support::MANIFEST, "the edit found the order to swap");
    // The fixture keeps the slot's own id, so swapping the two slots is a
    // reordering of the closed set exactly as it would be for shipped bytes.
    let mut files = support::fixture_files();
    files.swap(1, 2);
    let bundle = support::bundle_of(&manifest, &files);
    let fault = content::read_bundle(&parse(&bundle).expect("canonical"))
        .expect_err("a reordered campaign is refused");
    assert_eq!(fault.code(), Code::ContentInvalid);
    assert_eq!(fault.detail().expect("a diagnostic"), "{\"reason\":\"chapters\"}");
}

#[test]
fn a_pressure_aimed_at_something_the_chapter_never_placed_is_refused() {
    // The schedule-target gap: a pressure aimed at a Node that never stands
    // runs every stage and changes nothing at all.
    let detail = refused(
        "\"target\": { \"t\": \"route\", \"id\": 1 }",
        "\"target\": { \"t\": \"route\", \"id\": 9 }",
    );
    assert_eq!(detail, "{\"reason\":\"pressure_schedule\"}");
}

#[test]
fn a_chapter_that_pays_better_nearer_the_surface_is_refused() {
    // Reward monotonicity: the deeper layer carries the greater loss and the
    // greater reward, so a depth choice is a choice.
    let detail = refused(
        "{ \"layer\": 1, \"drain\": 65536, \"noise\": 8192, \"gain\": 65536 }",
        "{ \"layer\": 1, \"drain\": 65536, \"noise\": 8192, \"gain\": 16384 }",
    );
    assert_eq!(detail, "{\"reason\":\"layers\"}");
}

#[test]
fn an_event_naming_something_the_chapter_never_placed_is_refused() {
    let detail = refused("\"kind\": \"set_port_open\", \"node\": 4", "\"kind\": \"set_port_open\", \"node\": 9");
    assert_eq!(detail, "{\"reason\":\"node\"}");
    let detail = refused(
        "{ \"objective\": \"objective.the_edge.hold_the_current\", \"at\": 40",
        "{ \"objective\": \"objective.the_edge.never_authored\", \"at\": 40",
    );
    assert_eq!(detail, "{\"reason\":\"objective\"}");
    // The fourth event kind names a Route, and a Route the chapter never placed
    // is a cut that severs nothing — the same gap, at the kind that was added
    // last.
    let detail = refused("\"kind\": \"set_route_cut\", \"route\": 3", "\"kind\": \"set_route_cut\", \"route\": 9");
    assert_eq!(detail, "{\"reason\":\"route\"}");
}

#[test]
fn a_chapter_of_optional_tests_alone_is_refused() {
    // A chapter is completed by meeting what it requires, so it has to require
    // something.
    let detail = refused("\"optional\": null", "\"optional\": 600");
    assert_eq!(detail, "{\"reason\":\"objectives\"}");
}

#[test]
fn a_chapter_that_names_no_ending_is_refused() {
    // Every chapter names the ending its completion reaches, so a campaign
    // truncated for testing still ends on one.
    let detail = refused("\"ending_key\": \"ending.the_edge\"", "\"ending_key\": \"the_edge\"");
    assert_eq!(detail, "{\"reason\":\"ending_key\"}");
}

#[test]
fn a_chapter_whose_field_cannot_be_established_is_refused_at_load() {
    // The malformed-definition check a load can honestly make: every chapter
    // establishes, under every Form the campaign authors.
    let detail = refused("\"inside\": [2, 3]", "\"inside\": [2, 9]");
    assert_eq!(detail, "{\"id\":9,\"quantity\":\"node\"}");
}
