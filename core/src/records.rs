//! The session's persistence records.
//!
//! `docs/field-framework/ARCHITECTURE.md` locks the layout of the two stores a
//! run writes — the `runs` metadata row and the `records` rows keyed by
//! `save_key` — and locks IndexedDB as where they live. IndexedDB itself, the
//! shell's read API, and durability across a page load belong to the goal that
//! owns persistence; what stands here is the same layout held in memory for the
//! life of one core, which is what the restore commands read while that goal is
//! still ahead.
//!
//! Two fields of the locked rows are absent, and deliberately: `written_at` and
//! `updated_at` are wall-clock milliseconds, the core reads no clock, and the
//! document already says they live only outside payloads. The goal that writes
//! these rows to IndexedDB supplies both. Where the order of two writes matters
//! — which autosave slot is the older one — a monotone write counter stands in
//! for the timestamp, which is exact rather than approximate and keeps the core
//! clock-free.

use crate::state::{RecordKind, Step, ANCHORS_PER_RUN};

/// One run's metadata row.
#[derive(Clone, Debug)]
pub struct RunRow {
    pub run_id: String,
    pub form: String,
    pub chapter_index: u8,
    pub step: Step,
    pub branch_nonce: u32,
    /// The monotone record of the largest branch nonce this run has ever used.
    /// It rises on every record write and on import, and no restore lowers it,
    /// which is what makes it the source of truth Branch Recovery increments.
    pub nonce_high: u32,
    pub ended: bool,
}

/// One full save record. `payload` is the canonical bytes, and
/// `payload_sha256` is the digest of exactly those bytes.
#[derive(Clone, Debug)]
pub struct SaveRecord {
    pub save_key: String,
    pub run_id: String,
    pub kind: RecordKind,
    pub save_version: i64,
    pub payload_sha256: String,
    pub payload: String,
    /// Where this write sits in the session's write order.
    pub written_order: u64,
}

/// The two stores together.
#[derive(Clone, Debug, Default)]
pub struct RecordStore {
    runs: Vec<RunRow>,
    records: Vec<SaveRecord>,
    writes: u64,
}

/// The `save_key` of one record: `<run_id>:<kind>:<suffix>`. Anchor suffixes
/// are the anchor identifier as 8-digit zero-padded decimal; the two autosave
/// suffixes are `0` and `1`, alternating so a fault mid-write always leaves the
/// other slot intact.
pub fn save_key(run_id: &str, kind: RecordKind, suffix: u32) -> String {
    match kind {
        RecordKind::Anchor => format!("{run_id}:anchor:{suffix:08}"),
        RecordKind::Auto => format!("{run_id}:auto:{suffix}"),
    }
}

impl RecordStore {
    pub fn new() -> Self {
        RecordStore::default()
    }

    /// Opens or refreshes a run's metadata row. `nonce_high` only ever rises.
    pub fn note_run(
        &mut self,
        run_id: &str,
        form: &str,
        chapter_index: u8,
        step: Step,
        branch_nonce: u32,
    ) {
        match self.runs.iter_mut().find(|row| row.run_id == run_id) {
            Some(row) => {
                if !form.is_empty() {
                    row.form = form.to_string();
                }
                row.chapter_index = chapter_index;
                row.step = step;
                row.branch_nonce = branch_nonce;
                row.nonce_high = row.nonce_high.max(branch_nonce);
            }
            None => self.runs.push(RunRow {
                run_id: run_id.to_string(),
                form: form.to_string(),
                chapter_index,
                step,
                branch_nonce,
                nonce_high: branch_nonce,
                ended: false,
            }),
        }
    }

    /// The largest branch nonce a run has ever used, and 0 for a run with no
    /// row of its own yet.
    pub fn nonce_high(&self, run_id: &str) -> u32 {
        self.runs.iter().find(|row| row.run_id == run_id).map_or(0, |row| row.nonce_high)
    }

    pub fn row(&self, run_id: &str) -> Option<&RunRow> {
        self.runs.iter().find(|row| row.run_id == run_id)
    }

    /// Writes one record, replacing whatever stood under the same key.
    ///
    /// Which key an autosave lands under is derived from run state — never from
    /// this store's own history — so nothing here decides anything the payload
    /// carries. The write order recorded below is read only by this module, for
    /// pruning and for handing records back newest first.
    pub fn write(
        &mut self,
        save_key: String,
        run_id: &str,
        kind: RecordKind,
        payload: String,
        payload_sha256: String,
    ) {
        self.writes += 1;
        let record = SaveRecord {
            save_key,
            run_id: run_id.to_string(),
            kind,
            save_version: crate::state::SAVE_VERSION,
            payload_sha256,
            payload,
            written_order: self.writes,
        };
        match self.records.iter_mut().find(|held| held.save_key == record.save_key) {
            Some(held) => *held = record,
            None => self.records.push(record),
        }
        self.prune_anchors(run_id);
    }

    /// Holds a run to its Anchor-record cap: the write past it deletes the
    /// oldest Anchor record. The metadata of the deleted record is kept, which
    /// is why the payload's own list is not capped.
    fn prune_anchors(&mut self, run_id: &str) {
        loop {
            let mut held: Vec<(u64, usize)> = self
                .records
                .iter()
                .enumerate()
                .filter(|(_, record)| {
                    record.run_id == run_id && record.kind == RecordKind::Anchor
                })
                .map(|(place, record)| (record.written_order, place))
                .collect();
            if held.len() <= ANCHORS_PER_RUN {
                return;
            }
            held.sort_unstable();
            self.records.remove(held[0].1);
        }
    }

    pub fn record(&self, save_key: &str) -> Option<&SaveRecord> {
        self.records.iter().find(|held| held.save_key == save_key)
    }

    /// Every record of one run, newest write first.
    pub fn of_run(&self, run_id: &str) -> Vec<&SaveRecord> {
        let mut found: Vec<&SaveRecord> =
            self.records.iter().filter(|held| held.run_id == run_id).collect();
        found.sort_by(|left, right| right.written_order.cmp(&left.written_order));
        found
    }
}
