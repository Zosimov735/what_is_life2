//! What every core test needs of the authored content.
//!
//! The bundle a session opens on is the same bundle the worker hands across:
//! the manifest and every file the manifest lists, in manifest order, with the
//! digest over exactly those bytes. The files are embedded here the way the
//! worker embeds them, so a test never reads the disk at run time and a change
//! to the content reaches the tests without a step between.

#![allow(dead_code)]

pub mod campaign;
pub mod measure;

use field_game_core::fault::quoted;
use field_game_core::json::hex_bytes;
use field_game_core::sha256;

pub const MANIFEST: &str = include_str!("../../../content/manifest.json");
pub const CHAPTER_THE_PULL: &str = include_str!("../../../content/chapters/the_pull.json");
pub const CHAPTER_THE_EDGE: &str = include_str!("../../../content/chapters/the_edge.json");
pub const CHAPTER_THE_LOOP: &str = include_str!("../../../content/chapters/the_loop.json");
pub const CHAPTER_THE_ECHO: &str = include_str!("../../../content/chapters/the_echo.json");
pub const CHAPTER_THE_MESH: &str = include_str!("../../../content/chapters/the_mesh.json");
pub const CHAPTER_THE_BREAK: &str = include_str!("../../../content/chapters/the_break.json");
pub const CHAPTER_THE_REWRITE: &str = include_str!("../../../content/chapters/the_rewrite.json");
pub const CHAPTER_THE_QUIET_EDGE: &str =
    include_str!("../../../content/chapters/the_quiet_edge.json");
pub const FORM_THREAD: &str = include_str!("../../../content/forms/thread.json");
pub const FORM_RING: &str = include_str!("../../../content/forms/ring.json");
pub const FORM_RELAY: &str = include_str!("../../../content/forms/relay.json");
pub const FORM_VAULT: &str = include_str!("../../../content/forms/vault.json");
pub const FORM_LENS: &str = include_str!("../../../content/forms/lens.json");
pub const FORM_KNOT: &str = include_str!("../../../content/forms/knot.json");
pub const FORM_WAKE: &str = include_str!("../../../content/forms/wake.json");
pub const FORM_CHORUS: &str = include_str!("../../../content/forms/chorus.json");
pub const PRESSURE_DRAIN: &str = include_str!("../../../content/pressures/drain.json");
pub const PRESSURE_NOISE: &str = include_str!("../../../content/pressures/noise.json");
pub const PRESSURE_FRACTURE: &str = include_str!("../../../content/pressures/fracture.json");
pub const PRESSURE_FLOOD: &str = include_str!("../../../content/pressures/flood.json");
pub const PRESSURE_INTERFERENCE: &str =
    include_str!("../../../content/pressures/interference.json");
pub const PRESSURE_DRIFT: &str = include_str!("../../../content/pressures/drift.json");

/// A chapter authored for the tests alone, standing in the second slot.
///
/// The campaign-level refusal tests each string-edit one chapter's bytes, and a
/// refusal that edits a shipped chapter makes that chapter's authoring unfree:
/// every literal the edit reaches for is one a re-authoring may not move, and
/// the last chapter agent to stand in that slot authored around seven of them.
/// This file carries those literals instead. It keeps the slot's own `id`, so
/// the closed-set ordering check reads it exactly as it reads the shipped
/// chapter, and it authors nothing a player ever sees.
pub const CHAPTER_SLOT_FIXTURE: &str = include_str!("../fixtures/chapter_slot.json");

/// Where the fixture chapter stands in the manifest's own order.
pub const FIXTURE_PLACE: usize = 1;

/// The manifest's file list with the fixture chapter standing in its slot.
pub fn fixture_files() -> Vec<String> {
    let mut listed: Vec<String> = files().iter().map(|file| file.to_string()).collect();
    listed[FIXTURE_PLACE] = CHAPTER_SLOT_FIXTURE.to_string();
    listed
}

/// The init JSON a session opens on, over one file list of the caller's own.
pub fn worker_init_of(listed: &[String]) -> String {
    format!(
        "{{\"content\":{},\"protocol\":2,\"save_version\":2}}",
        bundle_of(MANIFEST, listed),
    )
}

/// The files the manifest lists, in manifest order.
pub fn files() -> Vec<&'static str> {
    vec![
        CHAPTER_THE_PULL,
        CHAPTER_THE_EDGE,
        CHAPTER_THE_LOOP,
        CHAPTER_THE_ECHO,
        CHAPTER_THE_MESH,
        CHAPTER_THE_BREAK,
        CHAPTER_THE_REWRITE,
        CHAPTER_THE_QUIET_EDGE,
        FORM_THREAD,
        FORM_RING,
        FORM_RELAY,
        FORM_VAULT,
        FORM_LENS,
        FORM_KNOT,
        FORM_WAKE,
        FORM_CHORUS,
        PRESSURE_DRAIN,
        PRESSURE_NOISE,
        PRESSURE_FRACTURE,
        PRESSURE_FLOOD,
        PRESSURE_INTERFERENCE,
        PRESSURE_DRIFT,
    ]
}

/// The locked content hash: the manifest bytes, then every listed file's bytes.
pub fn content_hash() -> String {
    let mut bytes = MANIFEST.as_bytes().to_vec();
    for file in files() {
        bytes.extend_from_slice(file.as_bytes());
    }
    hex_bytes(&sha256::digest(&bytes))
}

/// The bundle, under any digest the caller names.
pub fn bundle_with(hash: &str) -> String {
    let listed: Vec<String> = files().iter().map(|file| quoted(file)).collect();
    format!(
        "{{\"files\":[{}],\"hash\":{},\"manifest\":{}}}",
        listed.join(","),
        quoted(hash),
        quoted(MANIFEST),
    )
}

/// The bundle over one file list of the caller's own, under one manifest, with
/// the digest recomputed over exactly those bytes.
///
/// A test that edits a listed file has to recompute the digest with it, or the
/// hash check in front of the validation is what refuses the edit and the test
/// proves nothing about the validation. The list is the manifest's own order.
pub fn bundle_of(manifest: &str, listed: &[String]) -> String {
    let mut bytes = manifest.as_bytes().to_vec();
    for file in listed {
        bytes.extend_from_slice(file.as_bytes());
    }
    let hash = hex_bytes(&sha256::digest(&bytes));
    let quoted_files: Vec<String> = listed.iter().map(|file| quoted(file)).collect();
    format!(
        "{{\"files\":[{}],\"hash\":{},\"manifest\":{}}}",
        quoted_files.join(","),
        quoted(&hash),
        quoted(manifest),
    )
}

/// The init JSON the worker opens a core with: the versions it speaks, and the
/// authored content it imported.
pub fn worker_init() -> String {
    format!(
        "{{\"content\":{},\"protocol\":2,\"save_version\":2}}",
        bundle_with(&content_hash()),
    )
}

/// The same, under a digest that does not describe the bytes beside it.
pub fn worker_init_with(hash: &str) -> String {
    format!("{{\"content\":{},\"protocol\":2,\"save_version\":2}}", bundle_with(hash))
}
