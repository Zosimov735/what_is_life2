//! The explicit random state: the generator, the split, and the draw.
//!
//! `docs/field-framework/FRAMEWORK.md` requires that distinct part sequences
//! yield independent streams and the same sequence always yields the same
//! stream; `docs/field-framework/ARCHITECTURE.md` names the construction that
//! delivers it and the roots the run's streams hang from.

use field_game_core::json::{hex16, is_hex};
use field_game_core::rng::{block, run_stream, trajectory_stream, Part, RngState};

const KEY: &str = "0123456789abcdef";
const OTHER_KEY: &str = "fedcba9876543210";

/// The frozen stream identity.
///
/// These are golden vectors, pinned as literals rather than recomputed: every
/// value a run ever draws hangs off them, and every save records a position in
/// a stream they define. Changing any one of them silently changes what a
/// recorded run replays into, which invalidates every save ever written and
/// every framework value ever reproduced from one. A failure here is never a
/// stale expectation to update — it is a change to the generator, the split,
/// or the roots that must be undone or version-gated.
#[test]
fn the_generator_and_the_roots_are_frozen() {
    // The block function, at the counter's low half and across into its high
    // half — the second pair proves the high 64 bits reach the rounds.
    assert_eq!(
        block(0x0123_4567_89ab_cdef, 0),
        (0xb4bf_78fa_b4ff_5716, 0x52f2_1f94_3428_08c4)
    );
    assert_eq!(
        block(0x0123_4567_89ab_cdef, (1u128 << 64) | 2),
        (0x7017_900a_d03c_ad4b, 0x96ba_35d4_1413_03a2)
    );

    // The root of a run key, and the live trajectory stream of its first
    // branch, both as their keys.
    assert_eq!(hex16(RngState::root(KEY).key), "5ff6c45c9b635362");
    assert_eq!(hex16(trajectory_stream(KEY, 0).key), "fde2381b87ec0e4a");
    assert_eq!(trajectory_stream(KEY, 0).ctr, 0);
    assert_eq!(trajectory_stream(KEY, 0).half, 0);

    // The first four words that stream yields.
    let mut stream = trajectory_stream(KEY, 0);
    let words: Vec<String> = (0..4).map(|_| hex16(stream.next_word())).collect();
    assert_eq!(
        words,
        vec![
            "655bae281d2cd633".to_string(),
            "5d12703015c9ad07".to_string(),
            "0fc3c3cb9106c614".to_string(),
            "6efa1d1b463427cc".to_string(),
        ]
    );
}

#[test]
fn the_root_is_a_pure_function_of_the_run_key() {
    assert_eq!(RngState::root(KEY), RngState::root(KEY));
    assert_ne!(RngState::root(KEY), RngState::root(OTHER_KEY));
    let root = RngState::root(KEY);
    assert_eq!(root.ctr, 0);
    assert_eq!(root.half, 0);
}

#[test]
fn the_same_part_sequence_always_yields_the_same_stream() {
    let root = RngState::root(KEY);
    let once = root.split(&[Part::Name("evaluation"), Part::Number(3)]);
    let again = root.split(&[Part::Name("evaluation"), Part::Number(3)]);
    assert_eq!(once, again);
}

#[test]
fn distinct_part_sequences_yield_distinct_streams() {
    let root = RngState::root(KEY);
    let named = [
        root.split(&[Part::Name("trajectory")]),
        root.split(&[Part::Name("evaluation"), Part::Number(1)]),
        root.split(&[Part::Name("evaluation"), Part::Number(2)]),
        root.split(&[Part::Name("candidate"), Part::Number(1), Part::Name("baseline")]),
        root.split(&[Part::Name("candidate"), Part::Number(1), Part::Name("cut-impact")]),
        // A name and a number are tagged apart, so these are not the same
        // sequence written two ways.
        root.split(&[Part::Name("1")]),
        root.split(&[Part::Number(1)]),
    ];
    for (position, stream) in named.iter().enumerate() {
        for other in &named[position + 1..] {
            assert_ne!(stream, other, "each named stream is its own");
        }
    }
}

#[test]
fn a_split_starts_at_position_zero_and_a_parent_position_changes_the_child() {
    let root = RngState::root(KEY);
    let child = root.split(&[Part::Name("trajectory")]);
    assert_eq!(child.ctr, 0);
    assert_eq!(child.half, 0);

    let mut advanced = root;
    advanced.next_word();
    assert_ne!(advanced.split(&[Part::Name("trajectory")]), child);
}

#[test]
fn the_branch_nonce_re_roots_the_run_stream() {
    assert_ne!(run_stream(KEY, 0), run_stream(KEY, 1));
    assert_eq!(run_stream(KEY, 1), run_stream(KEY, 1));
    assert_ne!(trajectory_stream(KEY, 0), trajectory_stream(KEY, 1));
    assert_ne!(trajectory_stream(KEY, 0), trajectory_stream(OTHER_KEY, 0));
    // The trajectory stream is the run stream's own split, not the run stream.
    assert_ne!(trajectory_stream(KEY, 0), run_stream(KEY, 0));
}

#[test]
fn the_stream_yields_both_halves_of_a_block_before_the_counter_moves() {
    let mut stream = RngState::root(KEY);
    let start = stream;
    let first = stream.next_word();
    assert_eq!(stream.half, 1);
    assert_eq!(stream.ctr, 0, "the counter holds while the second half is unread");
    let second = stream.next_word();
    assert_eq!(stream.half, 0);
    assert_eq!(stream.ctr, 1);
    assert_ne!(first, second);

    // Replaying from the recorded position yields the same words.
    let mut replayed = start;
    assert_eq!(replayed.next_word(), first);
    assert_eq!(replayed.next_word(), second);
}

#[test]
fn a_draw_stays_inside_its_range_and_covers_it() {
    let mut stream = RngState::root(KEY);
    let mut seen = [0u32; 6];
    for _ in 0..600 {
        let value = stream.draw(6);
        assert!(value < 6);
        seen[value as usize] += 1;
    }
    assert!(seen.iter().all(|count| *count > 40), "every value of the range is drawn: {seen:?}");

    // A range of one consumes a word and yields the only value in it.
    let mut single = RngState::root(KEY);
    let before = single;
    assert_eq!(single.draw(1), 0);
    assert_ne!(single, before);
}

#[test]
fn a_position_is_written_as_the_locked_hex_widths() {
    let state = RngState { key: 1, ctr: 2, half: 1 };
    assert_eq!(
        state.write(),
        "{\"ctr\":\"00000000000000000000000000000002\",\
         \"half\":1,\"key\":\"0000000000000001\"}"
    );
    let live = trajectory_stream(KEY, 0);
    let written = live.write();
    let key = written.split("\"key\":\"").nth(1).unwrap().trim_end_matches("\"}");
    assert!(is_hex(key, 16));
}
