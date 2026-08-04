//! The explicit random state.
//!
//! `docs/field-framework/FRAMEWORK.md` states the interface — a storable value
//! sigma with `split` and `draw` — and
//! `docs/field-framework/ARCHITECTURE.md` locks the one construction behind
//! it: Philox2x64-10 as the generator, FNV-1a-64 absorption with SplitMix64
//! finalization as the split, and the named roots the run's streams hang from.
//! Both block function and split are implemented exactly as those documents
//! write them; nothing here chooses anything.
//!
//! No system randomness and no clock is read. A run's whole nondeterministic
//! input is its run key, and every stream is a pure function of that key, the
//! branch nonce, and the part sequence naming the stream.

use crate::json::{hex16, hex32, Obj};

/// The Philox multiplier.
const MULTIPLIER: u64 = 0xD2B7_4407_B1CE_6E93;

/// The Weyl constant the key advances by each round.
const WEYL: u64 = 0x9E37_79B9_7F4A_7C15;

/// The FNV-1a-64 offset basis, where every absorption starts.
const FNV_BASIS: u64 = 0xCBF2_9CE4_8422_2325;

/// The FNV-1a-64 prime.
const FNV_PRIME: u64 = 0x0000_0100_0000_01B3;

/// One position in one stream: `{ key, ctr, half }`.
///
/// The default is the zero position of the zero key. It is a valid position
/// like any other and belongs to no named stream, so it is what a caller with
/// nothing staged threads through the step seam — a step that draws nothing
/// never reads it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RngState {
    pub key: u64,
    pub ctr: u128,
    pub half: u8,
}

/// One part of a stream name: a name or an integer, in the order the stream is
/// named. Distinct sequences yield independent streams.
#[derive(Clone, Copy, Debug)]
pub enum Part<'a> {
    Name(&'a str),
    Number(i64),
}

/// The Philox2x64-10 block function, normative as
/// `docs/field-framework/ARCHITECTURE.md` writes it. Public so the golden
/// vectors that pin the stream identity can name it directly.
pub fn block(key: u64, ctr: u128) -> (u64, u64) {
    let mut low = ctr as u64;
    let mut high = (ctr >> 64) as u64;
    let mut round_key = key;
    for _ in 0..10 {
        let product = u128::from(low) * u128::from(MULTIPLIER);
        let next_low = ((product >> 64) as u64) ^ round_key ^ high;
        high = product as u64;
        low = next_low;
        round_key = round_key.wrapping_add(WEYL);
    }
    (low, high)
}

impl RngState {
    /// The root state of a run: the 16 ASCII bytes of the run key absorbed
    /// from the FNV basis with no tag, finalized with the mix.
    pub fn root(run_id: &str) -> Self {
        let mut hash = FNV_BASIS;
        for byte in run_id.as_bytes() {
            hash = absorb(hash, *byte);
        }
        RngState { key: mix64(hash), ctr: 0, half: 0 }
    }

    /// The stream named by a part sequence, fully determined by this state and
    /// those parts.
    pub fn split(&self, parts: &[Part]) -> Self {
        let mut hash = FNV_BASIS;
        for byte in self.key.to_le_bytes() {
            hash = absorb(hash, byte);
        }
        for byte in self.ctr.to_le_bytes() {
            hash = absorb(hash, byte);
        }
        hash = absorb(hash, self.half);
        for part in parts {
            match part {
                Part::Name(name) => {
                    hash = absorb(hash, 0x01);
                    for byte in (name.len() as u32).to_le_bytes() {
                        hash = absorb(hash, byte);
                    }
                    for byte in name.as_bytes() {
                        hash = absorb(hash, *byte);
                    }
                }
                Part::Number(value) => {
                    hash = absorb(hash, 0x02);
                    for byte in value.to_le_bytes() {
                        hash = absorb(hash, byte);
                    }
                }
            }
        }
        RngState { key: mix64(hash), ctr: 0, half: 0 }
    }

    /// The next 64-bit word of the stream, advancing the position.
    pub fn next_word(&mut self) -> u64 {
        let (first, second) = block(self.key, self.ctr);
        if self.half == 0 {
            self.half = 1;
            first
        } else {
            self.half = 0;
            self.ctr = self.ctr.wrapping_add(1);
            second
        }
    }

    /// The next uniform integer in [0, m), exactly uniform: words that would
    /// bias the result are rejected, and the words consumed are part of the
    /// position.
    pub fn draw(&mut self, m: u64) -> u64 {
        debug_assert!(m >= 1, "a draw covers at least one value");
        let span = u128::from(m);
        let remainder = (1u128 << 64) % span;
        let limit = (1u128 << 64) - remainder;
        loop {
            let word = u128::from(self.next_word());
            if word < limit {
                return (word % span) as u64;
            }
        }
    }

    /// Draws from an event-addressed child stream without moving the parent.
    /// Paired worlds sharing this parent therefore share every unaffected
    /// `(event, object, step)` draw even when an intervention changes iteration
    /// membership elsewhere.
    pub fn addressed(&self, event: &str, object: u32, step: u32, m: u64) -> u64 {
        let mut child = self.split(&[
            Part::Name("event"),
            Part::Name(event),
            Part::Number(i64::from(object)),
            Part::Number(i64::from(step)),
        ]);
        child.draw(m)
    }

    /// Reads a stream position out of a payload: the key and counter as
    /// fixed-width lowercase hex, the half as the integer 0 or 1.
    pub fn read(value: &crate::json::Json, key: &str) -> Result<Self, crate::fault::Fault> {
        use crate::read;
        let found = read::map(value, key)?;
        read::exact_keys(found, key, &["ctr", "half", "key"])?;
        let counter = read::hex(found, "ctr", 32)?;
        let stream = read::hex(found, "key", 16)?;
        let half = read::int(found, "half", 0, 1)? as u8;
        Ok(RngState {
            key: u64::from_str_radix(stream, 16).map_err(|_| crate::fault::Fault::field("key"))?,
            ctr: u128::from_str_radix(counter, 16).map_err(|_| crate::fault::Fault::field("ctr"))?,
            half,
        })
    }

    /// The state as canonical JSON: the key and counter as fixed-width hex,
    /// the half as the integer 0 or 1.
    pub fn write(&self) -> String {
        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        object.text("ctr", &hex32(self.ctr));
        object.int("half", i64::from(self.half));
        object.text("key", &hex16(self.key));
        object.end();
        out
    }
}

/// One absorbed byte of FNV-1a-64.
fn absorb(hash: u64, byte: u8) -> u64 {
    (hash ^ u64::from(byte)).wrapping_mul(FNV_PRIME)
}

/// The SplitMix64 finalizer, normative as the document writes it.
fn mix64(value: u64) -> u64 {
    let mut z = value;
    z ^= z >> 30;
    z = z.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z ^= z >> 27;
    z = z.wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    z
}

/// The run's own stream root: the branch split of the root state. Branch
/// Recovery changes only the nonce, so stochastic readings re-draw.
pub fn run_stream(run_id: &str, branch_nonce: u32) -> RngState {
    RngState::root(run_id).split(&[Part::Name("branch"), Part::Number(i64::from(branch_nonce))])
}

/// The live trajectory stream: the one the step function draws from, and the
/// one whose position a Run carries and a restore returns to exactly.
pub fn trajectory_stream(run_id: &str, branch_nonce: u32) -> RngState {
    run_stream(run_id, branch_nonce).split(&[Part::Name("trajectory")])
}
