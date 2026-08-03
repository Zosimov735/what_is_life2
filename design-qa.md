# Number 2 Visual Overhaul — Design QA

Final result: passed

## Target

- Source: `docs/field-framework/assets/number-2/physical-compartment-and-view.png`
- Source dimensions: 1487 × 1058 px
- Canonical state: Still Mode with the passive Observation View and causal
  Physical Compartment visible together
- Implementation capture: `design-qa-implementation.png`
- Implementation viewport: 1280 × 720 CSS px at DPR 1
- Narrow capture: `design-qa-mobile.png`
- Narrow viewport: 319 × 699 CSS px at DPR 1

## Same-State Comparison

- Combined evidence: `design-qa-comparison.png`
- Layout: normalized source on the left, implementation on the right
- Comparison dimensions: 2560 × 720 px
- The source was center-cropped to the implementation's 16:9 viewport and
  scaled to 1280 × 720 before both images were joined. No implementation crop
  or independent visual judgment was used.

## Comparison History

1. The first direct-play capture used the browser's 319 × 699 default viewport.
   The mode controls and View protocol collided, and the coordinate-profile
   control touched the View region. The narrow breakpoint now reserves distinct
   vertical bands for mode, protocol, profile, and budget surfaces.
2. The first desktop comparison matched composition and hierarchy but muted too
   much of the source texture's amber and violet variation. The final treatment
   raises authored texture presence while keeping live causal marks dominant.
3. The final 1280 × 720 comparison retains the source's graphite field,
   filament density, restrained instrument typography, heavy material edge,
   and thin violet observational layer without copying conceptual labels that
   conflict with the product mechanics.

## Interaction Evidence

- Thread opens into the live Field and Space enters Still Mode.
- Observation View is initially pressed and remains a free analysis surface.
- Physical Compartment resolves to one control and changes to
  `aria-pressed="true"` when selected.
- Merely switching tools leaves the displayed Intervention Budget at 3, queued
  physical edits at 0, and cost at 0.
- The browser reported no warning or error console entries during the flow.

## Findings

- P0: none.
- P1: none.
- P2: none.
- P3: live geometry follows the deterministic chapter state rather than the
  mock-up's illustrative node placement; this is intentional and preserves the
  simulation as the source of truth.
