//! The locked fixed-point arithmetic, and the two Node relations built on it.
//!
//! `docs/field-framework/ARCHITECTURE.md` locks every simulation quantity as a
//! Q32.16 binary fixed-point integer stored in `i64` — raw value = quantity ×
//! 65536 — and locks the four operations below on `i64` with `i128`
//! intermediates: `fixed_mul` shifting right by 16 with an arithmetic shift
//! (rounding toward negative infinity), `fixed_div` shifting left by 16 with
//! the quotient truncated toward zero, `isqrt` by the restoring
//! shift-and-subtract method, and `clamp01` on a `Frac`. Distance and adjacency
//! are locked in the same section, one rule for every Node.
//!
//! No float appears here or anywhere else in the step logic, and no
//! transcendental function exists: magnitudes go through `isqrt`, and a curved
//! response, if one is ever needed, is an authored piecewise-linear table.

use crate::state::{Fx, Frac, FRAC_ONE};

/// One distance or Charge unit, raw. Every quantity is the unit times 65536.
pub const ONE_UNIT: Fx = 65_536;

/// The raw span the capacity caps hold every stored quantity below: 2^28, the
/// bound the overflow-safety argument rests on. 4096 units of stored Charge and
/// the 4096-unit width of a layer plane are both exactly this value.
pub const STORED_BOUND: Fx = 1 << 28;

/// The raw distance one layer of separation contributes: 512 units.
pub const LAYER_SEPARATION: Fx = 33_554_432;

/// The raw distance inside which two Nodes are adjacent: 256 units.
pub const ADJACENT_WITHIN: Fx = 16_777_216;

/// Narrows an intermediate back to `Fx`.
///
/// The caps hold every stored quantity below 2^28 raw and every windowed sum
/// below 2^46, so no result of a locked operation on locked state can leave
/// `i64`. A value that does is a defect rather than a fault in any input, and
/// the core traps on it instead of wrapping — which is why the crate builds
/// with overflow checks on in every profile.
fn narrow(value: i128) -> Fx {
    Fx::try_from(value).expect("a locked quantity stays inside the carry width")
}

/// `fixed_mul(a, b)` = `(i128(a) * i128(b)) >> 16`, an arithmetic shift, so the
/// result rounds toward negative infinity.
pub fn fixed_mul(a: Fx, b: Fx) -> Fx {
    narrow((i128::from(a) * i128::from(b)) >> 16)
}

/// `fixed_div(a, b)` = `(i128(a) << 16) / i128(b)`, the quotient truncated
/// toward zero. A zero divisor is a validated precondition, not a result.
pub fn fixed_div(a: Fx, b: Fx) -> Fx {
    debug_assert!(b != 0, "a locked division names a nonzero divisor");
    narrow((i128::from(a) << 16) / i128::from(b))
}

/// `isqrt(x)` — the largest `s` with `s * s <= x`, by the restoring
/// shift-and-subtract method, 64 result bits.
pub fn isqrt(value: u128) -> u64 {
    let mut remainder = value;
    let mut root: u128 = 0;
    // The highest even power of two, walked down to the first that the
    // remainder can carry.
    let mut bit: u128 = 1 << 126;
    while bit > remainder {
        bit >>= 2;
    }
    while bit != 0 {
        if remainder >= root + bit {
            remainder -= root + bit;
            root = (root >> 1) + bit;
        } else {
            root >>= 1;
        }
        bit >>= 2;
    }
    // A `u128` input has a root inside 64 bits by construction.
    root as u64
}

/// `clamp01(x)` = `min(65536, max(0, x))`: the raw form of [0, 1].
pub fn clamp01(value: Frac) -> Frac {
    value.clamp(0, FRAC_ONE)
}

/// Holds a value inside an inclusive raw range. The ranges this enforces are
/// locked; the clamp is how a locked range is kept rather than a rule of its
/// own.
pub fn hold(value: Fx, low: Fx, high: Fx) -> Fx {
    value.clamp(low, high)
}

/// A position on a layer plane: raw `Fx` per axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Vec2 {
    pub x: Fx,
    pub y: Fx,
}

impl Vec2 {
    pub fn new(x: Fx, y: Fx) -> Self {
        Vec2 { x, y }
    }

    /// The position in whole units, for a fixture or a bound written in units.
    pub fn units(x: i64, y: i64) -> Self {
        Vec2 { x: x * ONE_UNIT, y: y * ONE_UNIT }
    }

    /// Reads a position out of a payload. Both axes are raw `Fx`, and the
    /// shape declares exactly the two of them.
    pub fn read(value: &crate::json::Json, key: &str) -> Result<Self, crate::fault::Fault> {
        use crate::read;
        let found = read::map(value, key)?;
        read::exact_keys(found, key, &["x", "y"])?;
        Ok(Vec2 {
            x: read::int(found, "x", i64::MIN, i64::MAX)?,
            y: read::int(found, "y", i64::MIN, i64::MAX)?,
        })
    }

    pub fn write(&self, out: &mut String) {
        let mut object = crate::json::Obj::new(out);
        object.int("x", self.x);
        object.int("y", self.y);
        object.end();
    }

    pub fn written(&self) -> String {
        let mut out = String::new();
        self.write(&mut out);
        out
    }
}

/// The locked distance between two placed Nodes:
/// `d(a, b) = isqrt(dx*dx + dy*dy) + 33554432 * |layer(a) - layer(b)|`,
/// computed on raw values with `i128` intermediates.
pub fn distance(first: Vec2, first_layer: u8, second: Vec2, second_layer: u8) -> Fx {
    let dx = i128::from(first.x) - i128::from(second.x);
    let dy = i128::from(first.y) - i128::from(second.y);
    let square = (dx * dx + dy * dy) as u128;
    let plane = i128::from(isqrt(square));
    let depth = i128::from(first_layer.abs_diff(second_layer)) * i128::from(LAYER_SEPARATION);
    narrow(plane + depth)
}

/// True when the locked distance between two placed Nodes is at most `radius`.
///
/// This is `distance(..) <= radius` decided without taking the root, and it is
/// the same predicate on every input rather than an approximation of it. For a
/// nonnegative integer radius `w` and a nonnegative integer square `S`:
///
/// ```text
/// isqrt(S) <= w   iff   sqrt(S) < w + 1   iff   S < (w + 1)^2
/// ```
///
/// because `isqrt` floors and `w` is an integer. The depth term is exact
/// already, so it is subtracted from the radius first; a radius the depth term
/// alone exhausts puts the two Nodes outside it whatever their positions.
///
/// The reason to decide it this way is the step budget: current delivery tests
/// every Node against every path point of every current, and taking a root at
/// each of those would cost more than the whole step is allowed. A test pins
/// this function against `distance` across the boundary and beyond it.
pub fn within(first: Vec2, first_layer: u8, second: Vec2, second_layer: u8, radius: Fx) -> bool {
    if radius < 0 {
        return false;
    }
    let depth = i128::from(first_layer.abs_diff(second_layer)) * i128::from(LAYER_SEPARATION);
    let plane_allowed = i128::from(radius) - depth;
    if plane_allowed < 0 {
        return false;
    }
    let dx = i128::from(first.x) - i128::from(second.x);
    let dy = i128::from(first.y) - i128::from(second.y);
    let square = dx * dx + dy * dy;
    let edge = plane_allowed + 1;
    square < edge * edge
}

/// The locked adjacency rule: `adj(a, b)` exactly when `d(a, b) <= 16777216`
/// — 256 units. One rule for every Node, so static Nodes have effectively
/// fixed adjacency and moving Forms recompute theirs per step.
pub fn adjacent(first: Vec2, first_layer: u8, second: Vec2, second_layer: u8) -> bool {
    within(first, first_layer, second, second_layer, ADJACENT_WITHIN)
}
