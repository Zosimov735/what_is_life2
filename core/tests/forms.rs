//! The eight starting Forms: what each one authors, and what each one does.
//!
//! Every Form's promise is one sentence in the copy catalog, and every promise
//! this build keeps is kept by authored parameters reaching the rules that read
//! them. So the tests here are in two halves. The first reads the authored
//! files and pins what they declare — the orderings the promises name, and the
//! one parameter the document says only `lens` carries. The second drives the
//! rules and pins what the declarations do: the Charge a Form can stand on,
//! the links a reach admits, and the Forms a Field opens with. Physical
//! compartment membership and leakage are chapter parameters, so the same
//! chapter must establish them identically under every Form.
//!
//! What is deliberately not here is a branch on a Form's name. Nothing in the
//! core asks which Form was selected; the Field is established from the Form's
//! data, and these tests read the same data the establishment does.

use field_game_core::content::{self, Ability, FormContent};
use field_game_core::field::{self, NodeKind, Unstaged};
use field_game_core::fx::{distance, Vec2, ONE_UNIT};
use field_game_core::json::{parse, Json};
use field_game_core::run::FORMS;
use field_game_core::slate::TAU_DEFAULT;
use field_game_core::state::{ControlState, FieldState, ViewDeclaration, FRAC_ONE};
use field_game_core::{coord, Session};

mod support;

use support::measure::{circuit_with_form, played, view_of};

const KEY: &str = "aabbccdd00112233";

/// The window the one reading below is taken over, and the inside it is taken
/// of: the coordinate tests' own, because the fixture is theirs.
const WINDOW: u16 = 8;
const INSIDE: [u32; 3] = [2, 3, 4];

/// One rendered frame at the 60-frames-per-second target, in microseconds.
const FRAME_US: i64 = 16_667;

/// How long a ramp takes, in microseconds. Locked.
const RAMP_US: i64 = 250_000;

/// The authored content, read as the worker hands it over.
fn authored() -> content::Content {
    let bundle = parse(&support::bundle_with(&support::content_hash())).expect("canonical");
    content::read_bundle(&bundle).expect("the content reads")
}

/// One Form's authored parameters.
fn form_of(content: &content::Content, id: &str) -> FormContent {
    content.form(id).unwrap_or_else(|| panic!("{id} is authored")).clone()
}

/// The Field one Form opens the opening chapter on, and the View it stands
/// under.
fn opened_field(content: &content::Content, id: &str) -> (FieldState, ViewDeclaration) {
    let chapter = content.chapter(0).expect("the opening chapter");
    content::establish(chapter, &form_of(content, id)).expect("a Field stands")
}

/// One step over a Field, with nothing staged and no control asked for.
fn one_step(field: &mut FieldState) -> field::StepOutcome {
    let mut unstaged = Unstaged::default();
    field::advance(field, ControlState::default(), FRAC_ONE, &mut unstaged.staging())
}

/// Where a Node stands in the Port list.
fn node_q(field: &FieldState, node: u32) -> i64 {
    field.ports.iter().find(|port| port.node == node).map(|port| port.q).expect("a Node")
}

/// A session opened on one Form.
fn opened(form: &str) -> Session {
    let mut session = Session::new(&support::worker_init()).expect("versions agree");
    let answer = session.command(
        "init_run",
        &format!("{{\"form\":\"{form}\",\"mode\":\"new\",\"run_id\":\"{KEY}\"}}"),
    );
    assert!(answer.contains("\"ok\":true"), "{answer}");
    session
}

/// One frame of input, with every declared field present.
fn frame(seq: u32, steps: u16, toggle_still: bool, t_us: i64) -> String {
    format!(
        "{{\"advance_steps\":{steps},\"depth_key\":0,\"inspect\":null,\"pause\":false,\
          \"pulse_held\":false,\"pulse_release\":false,\"seq\":{seq},\"steer_x\":0,\
          \"steer_y\":0,\"t_us\":{t_us},\"toggle_still\":{toggle_still},\"wheel\":0}}"
    )
}

/// Takes a session into Still Mode, where the queued-change commands stand.
fn into_still(session: &mut Session) {
    session.command("input_frame", &frame(1, 1, true, 1_000_000));
    assert_eq!(session.lifecycle(), "ramp_in");
    session.command("input_frame", &frame(2, 1, false, 1_000_000 + RAMP_US + FRAME_US));
    assert_eq!(session.lifecycle(), "still");
}

// ---------------------------------------------------------------------------
// What the eight files declare
// ---------------------------------------------------------------------------

#[test]
fn every_starting_form_authors_its_own_parameters() {
    let content = authored();
    assert_eq!(content.forms.len(), FORMS.len(), "one file per Form of the closed set");

    let mut seen: Vec<(String, [i64; 7])> = Vec::new();
    println!("\nthe authored parameters, in whole units where they are quantities");
    println!(
        "{:<8} {:>6} {:>9} {:>8} {:>8} {:>7} {:>8} {:>6}",
        "form", "reach", "forecast", "steer", "route", "upkeep", "capacity", "links",
    );
    for id in FORMS {
        let form = form_of(&content, id);
        let links: usize = form
            .abilities
            .iter()
            .map(|ability| match ability {
                Ability::LinkedForms { offsets, .. } => offsets.len(),
                Ability::Trail { .. } | Ability::Junction { .. } => 0,
            })
            .sum();
        println!(
            "{id:<8} {:>6} {:>9} {:>8} {:>8} {:>7} {:>8} {:>6}",
            form.route_reach / ONE_UNIT,
            form.forecast_depth,
            form.steer_scale,
            form.route_capacity / ONE_UNIT,
            form.upkeep_rate / ONE_UNIT,
            form.capacity / ONE_UNIT,
            links,
        );
        let vector = [
            form.route_reach,
            i64::from(form.forecast_depth),
            form.steer_scale,
            form.route_capacity,
            form.upkeep_rate,
            form.capacity,
            form.reserve,
        ];
        for (held, other) in &seen {
            assert_ne!(other, &vector, "{id} and {held} author the same parameters");
        }
        seen.push((id.to_string(), vector));
    }
}

#[test]
fn the_authored_parameters_stand_in_the_order_the_promises_name() {
    // Each assertion below is one promise, read as an ordering over the
    // authored numbers. They are stated against the whole set rather than
    // pairwise, so a later change that quietly makes a Form the strongest at
    // everything fails here.
    let content = authored();
    let by = |pick: fn(&FormContent) -> i64| -> Vec<(String, i64)> {
        let mut held: Vec<(String, i64)> =
            FORMS.iter().map(|id| (id.to_string(), pick(&form_of(&content, id)))).collect();
        held.sort_by_key(|(_, value)| *value);
        held
    };

    // Ring's short reach and Relay's long routes are the two ends of the reach.
    let reach = by(|form| form.route_reach);
    assert_eq!(reach.first().map(|(id, _)| id.as_str()), Some("ring"));
    assert_eq!(reach.last().map(|(id, _)| id.as_str()), Some("relay"));

    // Vault preserves stored state and Lens holds little charge: the two ends
    // of the Form's own overload threshold.
    let held = by(|form| form.capacity);
    assert_eq!(held.first().map(|(id, _)| id.as_str()), Some("lens"));
    assert_eq!(held.last().map(|(id, _)| id.as_str()), Some("vault"));

    // Knot pays the high upkeep, and Vault carries the reserve.
    let upkeep = by(|form| form.upkeep_rate);
    assert_eq!(upkeep.last().map(|(id, _)| id.as_str()), Some("knot"));
    let reserve = by(|form| form.reserve);
    assert_eq!(reserve.last().map(|(id, _)| id.as_str()), Some("vault"));

    // Thread responds twice as fast as the standard chassis and Vault is the
    // slow end of the authored steering range.
    let steering = by(|form| form.steer_scale);
    assert_eq!(steering.first().map(|(id, _)| id.as_str()), Some("vault"));
    assert_eq!(steering.last().map(|(id, _)| id.as_str()), Some("thread"));

    // The document's own line: only `lens` declares a Forecast depth above 0
    // in version 2.
    for id in FORMS {
        let depth = form_of(&content, id).forecast_depth;
        assert_eq!(depth > 0, id == "lens", "{id} declares a Forecast depth of {depth}");
    }
}

// ---------------------------------------------------------------------------
// What the declarations do
// ---------------------------------------------------------------------------

#[test]
fn the_field_a_run_opens_on_carries_form_parameters_and_the_chapters_compartment() {
    let content = authored();
    let chapter = content.chapter(0).expect("the opening chapter");
    for id in FORMS {
        let form = form_of(&content, id);
        let (field, _) = opened_field(&content, id);
        let controlled = field.forms.iter().find(|held| held.controlled).expect("one is steered");
        assert_eq!(controlled.form, id);
        assert_eq!(controlled.route_reach, form.route_reach);
        assert_eq!(controlled.forecast_depth, form.forecast_depth);
        assert_eq!(controlled.reserve, form.reserve);
        assert_eq!(
            field.physical_compartment, chapter.physical_compartment,
            "{id} opens the chapter-authored members and leakage coefficient",
        );
        // The Form's own Node carries the threshold and the upkeep rate.
        let node = field
            .ports
            .iter()
            .find(|port| port.node == controlled.node)
            .expect("the Form's own Node");
        assert_eq!(node.kind, NodeKind::Form);
        assert_eq!(node.capacity, form.capacity);
        assert_eq!(node.upkeep_rate, form.upkeep_rate);
    }
}

#[test]
fn the_same_chapter_compartment_leaks_identically_under_every_form() {
    // One step over the same chapter-authored physical compartment, with the
    // same Charge on every member. The opening View begins with the same member
    // list as authored initial data, but clearing that local View value cannot
    // enter the transition: `advance` accepts no View.
    let content = authored();
    let chapter = content.chapter(0).expect("the opening chapter");
    let standing = 512 * ONE_UNIT;
    let mut leaked: Vec<(&str, i64)> = Vec::new();
    for id in FORMS {
        let (mut field, mut view) = opened_field(&content, id);
        assert_eq!(view.inside, chapter.physical_compartment.members);
        view.inside.clear();
        assert!(view.inside.is_empty());
        assert_eq!(field.physical_compartment, chapter.physical_compartment);
        for port in &mut field.ports {
            if field.physical_compartment.members.contains(&port.node) {
                port.q = standing;
            }
        }
        leaked.push((id, one_step(&mut field).ledger.leakage));
    }
    println!("\nwhat one step of the same physical compartment leaks, raw");
    for (id, held) in &leaked {
        println!("  {id:<8} {held:>8}");
    }
    let expected = leaked.first().expect("one Form").1;
    assert!(expected > 0, "the authored compartment has exposed contacts");
    assert!(
        leaked.iter().all(|(_, held)| *held == expected),
        "Form selection cannot change chapter leakage: {leaked:?}",
    );
}

#[test]
fn the_threshold_a_form_authors_decides_what_it_can_stand_on() {
    // The same Charge pressed onto each Form's own Node, and one step run. A
    // Form standing above its threshold sheds a quarter of the excess; a Form
    // whose threshold is above what it holds sheds nothing. That is the whole
    // of "preserves stored state" against "holds little charge" as version 2
    // expresses them.
    let content = authored();
    let pressed = 1024 * ONE_UNIT;
    let mut held: Vec<(&str, i64)> = Vec::new();
    for id in FORMS {
        let (mut field, _) = opened_field(&content, id);
        let node = field.forms.iter().find(|form| form.controlled).expect("one is steered").node;
        for port in &mut field.ports {
            if port.node == node {
                port.q = pressed;
            }
        }
        // Long enough for the quarter-of-the-excess rule to settle: what it
        // leaves standing is the threshold the Form authored, or everything
        // pressed onto it when the threshold stands above that.
        for _ in 0..60 {
            one_step(&mut field);
        }
        held.push((id, node_q(&field, node)));
    }
    println!("\nwhat sixty steps leave standing on a Form pressed to 1024 units");
    for (id, standing) in &held {
        println!("  {id:<8} {:>6} units", standing / ONE_UNIT);
    }
    let of = |id: &str| held.iter().find(|(name, _)| *name == id).expect("a Form").1;
    for (id, standing) in &held {
        let form = form_of(&content, id);
        if form.upkeep_rate > 0 {
            // A Form that pays cannot settle on its threshold: the Node phase
            // takes its rate every step, from whatever the decay leaves.
            assert!(
                *standing < form.capacity.min(pressed),
                "{id} pays upkeep and so stands below the threshold it authored",
            );
            continue;
        }
        assert_eq!(
            standing / ONE_UNIT,
            form.capacity.min(pressed) / ONE_UNIT,
            "{id} settles on the threshold it authored",
        );
    }
    assert_eq!(of("vault"), pressed, "Vault's threshold stands above what was pressed onto it");
    assert!(of("lens") * 2 < pressed, "Lens sheds most of what it cannot hold");
    assert!(of("chorus") < of("thread"), "a Chorus Form holds less than a Thread");
    // Knot's threshold stands above what was pressed onto it, so nothing was
    // shed and every raw unit missing is a raw unit paid: sixty steps at the
    // rate it authors, off what it was pressed with, and no other term.
    assert_eq!(
        of("knot"),
        pressed - 60 * form_of(&content, "knot").upkeep_rate,
        "Knot stands on what it was pressed with less what it paid",
    );
}

#[test]
fn the_forecast_depth_a_form_authors_decides_how_far_a_reading_stands() {
    // Lens's promise is two halves; this is the half the threshold above does
    // not carry. The Field's declared Forecast depth is the controlled Form's
    // own, and Horizon is the larger of it and the retained span — so the
    // authored parameter reaches a reading rather than sitting in a file.
    //
    // The Field is the coordinate tests' own circuit, whose retained span is 1
    // whatever Form stands beside it, so what each Form's authored depth does
    // to the reading is the whole of what changes between these eight.
    let content = authored();
    let view = view_of(&INSIDE, WINDOW);
    let mut read: Vec<(&str, i64)> = Vec::new();
    for id in FORMS {
        let depth = form_of(&content, id).forecast_depth;
        let state = played(circuit_with_form(depth), usize::from(WINDOW), view.clone());
        let horizon = coord::of(&state, &view, TAU_DEFAULT).horizon.value.expect("a reading");
        read.push((id, horizon));
    }
    println!("\nhow far each Form's own Horizon stands, in steps");
    for (id, horizon) in &read {
        println!("  {id:<8} {horizon:>3}");
    }
    for (id, horizon) in &read {
        let expected = if *id == "lens" { i64::from(field::FORECAST_DEPTH_CAP) } else { 1 };
        assert_eq!(*horizon, expected, "{id} stands a Horizon of {horizon}");
    }
}

#[test]
fn the_reach_a_form_authors_decides_which_links_it_may_form() {
    // The reachability decision, read off the authored geometry: how many pairs
    // of the opening chapter's Nodes stand inside each Form's reach. The
    // numbers are the chapter's as authored today; the point of pinning them is
    // that a chapter re-authoring moves a figure a reader can check rather than
    // one that drifts.
    let content = authored();
    let mut counted: Vec<(&str, usize, usize)> = Vec::new();
    for id in FORMS {
        let form = form_of(&content, id);
        let (field, _) = opened_field(&content, id);
        // Every pair of placed Nodes, the Form's own included, under the locked
        // distance rule the `connect` precondition reads. The deep pairs are
        // counted apart: a pair with one end a layer down carries the rule's
        // own 512-unit term, which is the stretch the chapter reserves.
        let (mut within, mut deep) = (0, 0);
        for (place, first) in field.ports.iter().enumerate() {
            for second in &field.ports[place + 1..] {
                if distance(first.pos, first.layer, second.pos, second.layer) > form.route_reach {
                    continue;
                }
                within += 1;
                if first.layer != second.layer {
                    deep += 1;
                }
            }
        }
        counted.push((id, within, deep));
    }
    println!("\npairs of the opening chapter's Nodes inside each Form's reach");
    println!("{:<8} {:>6} {:>6}", "form", "pairs", "deep");
    for (id, within, deep) in &counted {
        println!("  {id:<8} {within:>4} {deep:>6}");
    }
    let of = |id: &str| counted.iter().find(|(name, _, _)| *name == id).expect("a Form").1;
    let deep_of = |id: &str| counted.iter().find(|(name, _, _)| *name == id).expect("a Form").2;
    // Every Form can form at least one link in the opening chapter, which is
    // the floor the chapter's own authoring has to keep.
    for (id, within, _) in &counted {
        assert!(*within > 0, "{id} can form no link at all in the opening chapter");
    }
    assert!(of("relay") > of("knot"), "Relay reaches pairs Knot cannot");
    assert!(of("knot") > of("ring"), "Knot's dense routing reaches more than Ring's short reach");
    assert_eq!(of("ring"), 2, "Ring reaches only the two closest pairs");
    assert_eq!(of("relay"), 12, "Relay reaches every pair on the surface, and two below it");

    // The chapter's own reachability policy, stated as an ordering rather than
    // as a table: a link across the layer boundary is the deliberate stretch,
    // and only the widest reach the campaign authors admits one.
    for (id, _, deep) in &counted {
        assert_eq!(*deep > 0, *id == "relay", "{id} reaches {deep} pairs across the layer");
    }
    assert_eq!(deep_of("relay"), 2, "and Relay reaches the two nearest of them");
}

#[test]
fn a_short_reach_refuses_the_link_a_long_reach_forms() {
    // The same command, the same two Nodes, and the two Forms at the ends of
    // the reach ladder: the refusal is the Form's parameter reaching the
    // locked precondition, and nothing about the command changes with it.
    // The two Ports the chapter places furthest apart with no Route standing
    // between them: 874 units of plane, which is past every reach but Relay's.
    let far = "{\"from\":2,\"op\":\"connect\",\"to\":5}";

    let mut relay = opened("relay");
    into_still(&mut relay);
    let formed = relay.command("queue_plan", &format!("{{\"plan\":{far}}}"));
    assert!(formed.contains("\"ok\":true"), "Relay forms the long link: {formed}");

    let mut ring = opened("ring");
    into_still(&mut ring);
    let refused = ring.command("queue_plan", &format!("{{\"plan\":{far}}}"));
    assert!(refused.contains("\"ok\":false"), "Ring's reach refuses it: {refused}");
    assert!(refused.contains("\"reason\":\"reach\""), "and says why: {refused}");

    // The near link stands for both, so the refusal is about the distance
    // rather than about the Nodes.
    let near = "{\"from\":2,\"op\":\"connect\",\"to\":6}";
    let mut ring = opened("ring");
    into_still(&mut ring);
    let formed = ring.command("queue_plan", &format!("{{\"plan\":{near}}}"));
    assert!(formed.contains("\"ok\":true"), "Ring forms the near link: {formed}");
}

#[test]
fn only_the_form_that_authors_links_opens_with_more_than_one_form() {
    let content = authored();
    for id in FORMS {
        let form = form_of(&content, id);
        let (field, _) = opened_field(&content, id);
        let Some(Ability::LinkedForms { offsets, separation }) = form.abilities.first() else {
            assert_eq!(field.forms.len(), 1, "{id} opens with the one Form the chapter places");
            continue;
        };
        assert_eq!(id, "chorus", "the linked Forms are Chorus's authored shape");
        assert_eq!(field.forms.len(), 1 + offsets.len());
        assert_eq!(
            field.forms.iter().filter(|held| held.controlled).count(),
            1,
            "several Forms stand, and exactly one of them is steered",
        );
        let controlled = field.forms.iter().find(|held| held.controlled).expect("one is steered");
        for held in field.forms.iter().filter(|held| !held.controlled) {
            // Every linked Form is a Form of the Field: its own Node, of the
            // Form kind, mirroring it.
            let node = field
                .ports
                .iter()
                .find(|port| port.node == held.node)
                .expect("a linked Form has its own Node");
            assert_eq!(node.kind, NodeKind::Form);
            assert_eq!(node.pos, held.pos);
            assert_eq!(node.capacity, form.capacity);
            assert_eq!(held.form, form.id);
            assert_eq!(held.route_reach, form.route_reach);
            // They open standing together, which is what the separation
            // parameter is measured against.
            assert!(
                distance(controlled.pos, controlled.layer, held.pos, held.layer) <= *separation,
                "a linked Form opens inside the authored separation",
            );
        }
    }
}

#[test]
fn the_first_playable_frame_is_the_selected_forms_own() {
    // The frame the shell draws before a single step runs, taken through the
    // whole path a shell takes it through. No two Forms produce the same one,
    // and the byte that says which Form stands is the Form's own place in the
    // closed set.
    let mut seen: Vec<(&str, Vec<u8>)> = Vec::new();
    for (place, id) in FORMS.iter().enumerate() {
        let session = opened(id);
        let view = session.frame_view();
        assert_eq!(&view[0..4], b"FGF1");
        // The forms section is the first the encoder writes, and a Form record
        // opens with the Form identifier and then the Form's own ordinal.
        let sections = usize::from(view[20]);
        assert!(sections > 0);
        let kind = view[32];
        assert_eq!(kind, 1, "the forms section stands first");
        let count = u16::from_le_bytes([view[34], view[35]]);
        let at = u32::from_le_bytes([view[36], view[37], view[38], view[39]]) as usize;
        assert_eq!(view[at + 1], place as u8, "{id} carries its own place in the closed set");
        assert_eq!(usize::from(count), if *id == "chorus" { 4 } else { 1 });
        for (held, other) in &seen {
            assert_ne!(other, &view, "{id} and {held} open on the same frame");
        }
        seen.push((id, view));
    }
}

#[test]
fn the_form_a_run_opened_on_is_recorded_and_replays_byte_for_byte() {
    // Form selection is part of `init_run`, so it is part of what a run is. Two
    // runs on the same key and the same frames agree byte for byte when they
    // opened on the same Form, and disagree when they did not.
    let drive = |form: &str| -> String {
        let mut session = opened(form);
        for seq in 1..=4u32 {
            let body = format!(
                "{{\"advance_steps\":30,\"depth_key\":0,\"inspect\":null,\"pause\":false,\
                  \"pulse_held\":false,\"pulse_release\":false,\"seq\":{seq},\"steer_x\":4096,\
                  \"steer_y\":-2048,\"t_us\":0,\"toggle_still\":false,\"wheel\":0}}"
            );
            let answer = session.command("input_frame", &body);
            assert!(answer.contains("\"ok\":true"), "{answer}");
        }
        session.run().expect("a run").state().payload()
    };

    for id in FORMS {
        assert_eq!(drive(id), drive(id), "{id} replays byte for byte");
        // The payload names the Form it opened on, which is what makes an
        // import able to reopen the run it was taken from.
        let payload = parse(&drive(id)).expect("canonical");
        let forms = payload
            .get("field")
            .and_then(|field| field.get("now"))
            .and_then(|now| now.get("forms"))
            .and_then(|forms| match forms {
                Json::List(items) => Some(items.clone()),
                _ => None,
            })
            .expect("the Forms of the Field");
        assert!(forms
            .iter()
            .all(|form| form.get("form").and_then(|held| held.as_text()) == Some(id)));
    }
    let mut payloads: Vec<String> = FORMS.iter().map(|id| drive(id)).collect();
    payloads.sort();
    payloads.dedup();
    assert_eq!(payloads.len(), FORMS.len(), "no two Forms run into the same state");
}

#[test]
fn an_ability_this_build_cannot_apply_is_refused_rather_than_ignored() {
    // The whole reason abilities are read strictly: an entry that reached the
    // Field as nothing would be a promise the run does not keep.
    let content = authored();
    let chorus = support::FORM_CHORUS;
    let refused = |text: String| {
        let mut files: Vec<String> = support::files().iter().map(|held| held.to_string()).collect();
        let place = files.iter().position(|held| held == chorus).expect("the authored file");
        files[place] = text;
        let mut bytes = support::MANIFEST.as_bytes().to_vec();
        for file in &files {
            bytes.extend_from_slice(file.as_bytes());
        }
        let hash = field_game_core::json::hex_bytes(&field_game_core::sha256::digest(&bytes));
        let listed: Vec<String> =
            files.iter().map(|file| field_game_core::fault::quoted(file)).collect();
        let bundle = format!(
            "{{\"files\":[{}],\"hash\":{},\"manifest\":{}}}",
            listed.join(","),
            field_game_core::fault::quoted(&hash),
            field_game_core::fault::quoted(support::MANIFEST),
        );
        let parsed = parse(&bundle).expect("canonical");
        content::read_bundle(&parsed).err()
    };

    assert!(
        refused(chorus.replace("linked_forms", "linked_regions")).is_some(),
        "an ability kind this build does not know is refused",
    );
    assert!(
        refused(chorus.replace("\"separation\": 16777216,", "")).is_some(),
        "an ability missing a parameter its kind declares is refused",
    );
    // An offset that would put a linked Form off the plane is refused where it
    // is resolved, which is at establishment against the chapter's placement.
    let mut off = form_of(&content, "chorus");
    off.abilities = vec![Ability::LinkedForms {
        offsets: vec![Vec2::new(-1_000_000_000, 0)],
        separation: 16_777_216,
    }];
    let chapter = content.chapter(0).expect("the opening chapter");
    assert!(
        content::establish(chapter, &off).is_err(),
        "an offset that lands off the plane is content this build refuses",
    );
}

// ---------------------------------------------------------------------------
// What the authored abilities do
// ---------------------------------------------------------------------------

/// One step over a Field under a named control, and what the step accounted.
fn steered_step(
    field: &mut FieldState,
    control: ControlState,
) -> field::StepOutcome {
    let mut unstaged = Unstaged::default();
    field::advance(field, control, FRAC_ONE, &mut unstaged.staging())
}

#[test]
fn the_steering_scale_a_form_authors_decides_how_far_a_gesture_carries_it() {
    // The same gesture, held for two seconds of steps, over each Form's own
    // opening Field: what differs is the scale each Form authors, and what it
    // moves is how far the gesture carries. Thread's promise is the fast one
    // and Vault's is the slow one, so they are the two ends of this ordering.
    let content = authored();
    let control = ControlState { steer_x: 32767, ..ControlState::default() };
    let mut carried: Vec<(&str, i64)> = Vec::new();
    for id in FORMS {
        let (mut field, _) = opened_field(&content, id);
        let opened = field.forms.iter().find(|form| form.controlled).expect("one is steered").pos.x;
        for _ in 0..60 {
            steered_step(&mut field, control);
        }
        let ended = field.forms.iter().find(|form| form.controlled).expect("one is steered").pos.x;
        carried.push((id, (ended - opened) / ONE_UNIT));
    }
    println!("\nhow far two seconds of one gesture carries each Form, in units");
    for (id, held) in &carried {
        println!("  {id:<8} {held:>6}");
    }
    let of = |id: &str| carried.iter().find(|(name, _)| *name == id).expect("a Form").1;
    assert!(of("thread") > of("vault") * 3, "Thread's gesture carries it far past Vault's");
    for (id, held) in &carried {
        if *id == "thread" || *id == "vault" {
            continue;
        }
        assert!(held < &of("thread"), "{id} is not faster than the Form whose promise is speed");
        assert!(held > &of("vault"), "{id} is not slower than the Form whose promise is delay");
    }
    // The measured order is the authored order, so no Form is quietly faster
    // than the parameter it declares.
    let mut by_parameter: Vec<(&str, i64)> =
        FORMS.iter().map(|id| (*id, form_of(&content, id).steer_scale)).collect();
    by_parameter.sort_by_key(|(id, value)| (*value, *id));
    let mut by_measure = carried.clone();
    by_measure.sort_by_key(|(id, value)| (*value, *id));
    assert_eq!(
        by_parameter.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
        by_measure.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
    );
}

#[test]
fn the_trail_wake_authors_is_left_behind_it_and_comes_due_where_it_stood() {
    // Wake's own promise, driven: the Form leaves an entry every period, the
    // entry stands in the Field's own queue, and it delivers where it was left
    // — through the ledger's `wake` source and at the step it comes due, never
    // at the step it was left.
    let content = authored();
    let wake = form_of(&content, "wake");
    let Some(Ability::Trail { period, delay, magnitude, .. }) = wake.abilities.first() else {
        panic!("wake authors a Trail");
    };
    let (mut field, _) = opened_field(&content, "wake");
    let node = field.forms.iter().find(|form| form.controlled).expect("one is steered").node;
    let opening = node_q(&field, node);

    let mut left_at = 0;
    let mut delivered_at = 0;
    let mut delivered = 0;
    for step in 1..=(u32::from(*period) + u32::from(*delay)) {
        let outcome = steered_step(&mut field, ControlState::default());
        assert_eq!(outcome.ledger.residual(), 0, "step {step} balances");
        assert!(outcome.ledger.balanced(), "and balances per Node");
        if left_at == 0 && !field.pending.is_empty() {
            left_at = step;
            assert_eq!(outcome.ledger.wake, 0, "leaving an entry moves no Charge");
        }
        if outcome.ledger.wake > 0 {
            delivered_at = step;
            delivered = outcome.ledger.wake;
        }
    }
    assert_eq!(left_at, u32::from(*period), "the first entry is left on the period's own step");
    assert_eq!(
        delivered_at,
        u32::from(*period) + u32::from(*delay),
        "and comes due the authored delay later",
    );
    assert_eq!(delivered, *magnitude, "the whole magnitude reached the Field");
    // The Form stood where it left the entry, so its own Node is what stood
    // inside the reach when the entry came due.
    assert!(node_q(&field, node) > opening, "the Charge arrived where the Form had stood");
}

#[test]
fn a_chorus_running_flat_out_stands_separated_and_rejoins_when_it_stops() {
    // Chorus's promise, both halves, from the authored numbers alone: the
    // linked Forms follow the one that is steered, and a leader that runs flat
    // out opens more distance than the authored separation admits — so the
    // vulnerability is reachable in the Field the chapter places, and it closes
    // again as soon as the gesture does.
    let content = authored();
    let chorus = form_of(&content, "chorus");
    let Some(Ability::LinkedForms { separation, .. }) = chorus.abilities.first() else {
        panic!("chorus authors links");
    };
    let (mut field, _) = opened_field(&content, "chorus");
    let apart = |field: &FieldState| -> Vec<i64> {
        let anchor = field.forms.iter().find(|form| form.controlled).expect("one is steered");
        field
            .forms
            .iter()
            .filter(|form| !form.controlled)
            .map(|form| distance(anchor.pos, anchor.layer, form.pos, form.layer))
            .collect()
    };
    assert!(apart(&field).iter().all(|held| held <= separation), "the set opens standing together");

    let control = ControlState { steer_x: 32767, steer_y: -16384, ..ControlState::default() };
    for _ in 0..90 {
        steered_step(&mut field, control);
    }
    let running = apart(&field);
    println!("\nhow far a running Chorus stands from its own Forms, raw: {running:?}");
    assert!(
        running.iter().any(|held| held > separation),
        "a Chorus at full deflection opens past its authored separation",
    );
    // Every linked Form moved: following is what makes the set a set, and the
    // distance above is a lag rather than a Form left standing.
    assert!(field.forms.iter().filter(|form| !form.controlled).all(|form| form.vel.x != 0));

    for _ in 0..120 {
        steered_step(&mut field, ControlState::default());
    }
    assert!(
        apart(&field).iter().all(|held| held <= separation),
        "and the set closes again as soon as the gesture does: {:?}",
        apart(&field),
    );
}

#[test]
fn the_route_capacity_a_form_authors_is_what_a_link_it_forms_carries() {
    // Relay's promise has two halves, and this is the second: it moves Charge
    // across long routes, and what it loses when one is cut is the liability
    // that balances the reach. A Route the player forms takes the controlled
    // Form's own authored capacity — the document's named extension point,
    // read at the commit and by no new rule — so Relay's link carries more per
    // step than any other Form's, and a cut takes more away with it.
    let content = authored();
    let mut authored_capacities: Vec<(&str, i64)> = Vec::new();
    for id in FORMS {
        authored_capacities.push((id, form_of(&content, id).route_capacity));
    }
    println!("\nwhat a link each Form forms carries, in whole units");
    for (id, held) in &authored_capacities {
        println!("  {id:<8} {:>4}", held / ONE_UNIT);
    }
    let of = |id: &str| authored_capacities.iter().find(|(name, _)| *name == id).expect("a Form").1;
    for (id, held) in &authored_capacities {
        assert!(*held > 0, "{id} forms a link that carries something");
        if *id == "relay" {
            continue;
        }
        assert!(of("relay") > *held, "Relay's link carries more than {id}'s");
    }

    // Driven rather than argued: the same two Nodes, the same command, and the
    // Route each Form leaves standing.
    let formed = |form: &str| -> i64 {
        let mut session = opened(form);
        into_still(&mut session);
        let near = "{\"from\":2,\"op\":\"connect\",\"to\":6}";
        let queued = session.command("queue_plan", &format!("{{\"plan\":{near}}}"));
        assert!(queued.contains("\"ok\":true"), "{form} forms the near link: {queued}");
        let committed = session.command("commit_plan", "{}");
        assert!(committed.contains("\"ok\":true"), "{form} commits it: {committed}");
        let state = session.run().expect("a run").state();
        state.now.routes.iter().map(|route| route.capacity).max().expect("a Route stands")
    };
    assert_eq!(formed("relay"), of("relay"), "the Route Relay formed carries what Relay authors");
    assert_eq!(formed("ring"), of("ring"), "and Ring's carries what Ring authors");
    assert!(formed("relay") > formed("ring"), "which is the divergence the promise names");
}
