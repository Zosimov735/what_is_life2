# What Is Life 2 — Product Outline

Status: current product contract  
Date: 2026-08-02

## The promise

The player pilots a luminous commissioning chassis through an unfamiliar
physical regime, activates and connects Components into a generator, declares
a measurable function, predicts how the system will respond, and then removes
their own hand from the controls to see whether the organization holds.

The fantasy is not “embody a literal real-world object” or “protect the glowing
center.” It is:

> Build something whose behavior you understand well enough to trust when you
> are no longer steering it.

The game earns mystery from the Field's visual world, not from ambiguous
controls. Every important reading has a quantity, unit, causal rule, and visible
failure condition.

## The player role

The player is simultaneously:

- a pilot during commissioning;
- a systems builder in Still Mode;
- an experimentalist at the Observe and Intervention benches;
- a forecaster during Divergence Replay;
- a validator during Ensemble and Holdout trials;
- an archivist who carries successful generators between regimes.

These are different modes of engagement with one causal system. They are not
separate minigames with unrelated currencies.

## The central loop

1. **Enter a regime.** Choose a destination in the Atlas and read its actual
   physical conditions and function contract.
2. **Choose a Form.** Select the steerable commissioning chassis whose measured
   movement, storage, reach, and ability fit the regime.
3. **Commission.** Pilot through finite Supply, activate Components, create
   Directed Routes, and establish the required function.
4. **Measure.** Move a passive View, choose an instrument, and inspect literal
   readings without altering the system.
5. **Predict.** State what should happen under a declared disturbance or change.
6. **Intervene.** Apply one typed, bounded causal edit or run it on a clone.
7. **Diagnose.** Use Divergence Replay to find the first material departure from
   baseline.
8. **Release control.** Freeze the generator specification and run Ensemble and
   Holdout trials with direct rescue disabled.
9. **Archive or revise.** Preserve the evidence, failed branches, and successful
   design; then carry the same generator into a new regime.

## What makes it fun

### Spatial mastery

The Form is responsive and pleasurable to steer. Supply, failing Components,
physical compartments, moving hazards, and construction reach turn the Field into a
readable space rather than a dashboard with animated wallpaper.

### Systems with visible consequences

Routes carry finite resource. Components have operating margins. Physical
compartments leak according to authoritative membership and a declared leakage coefficient. Changes propagate
through an actual ledger. The player can point to why a result occurred.

### Constrained invention

Still Mode offers a small number of meaningful edits. Reach, capacity, material,
upkeep, and intervention limits force tradeoffs. A successful design feels
owned because it was assembled under pressure, not selected from a solution
menu.

### Prediction and reveal

The strongest dramatic beat is a forecast being confirmed, refuted, or broken
in an unexpected place. Divergence Replay turns failure into an answerable
question: where did the altered run first stop behaving like the baseline?

### The trust transition

Commissioning lets the player rescue the system. Validation does not. The
moment direct control is removed converts skilled piloting into a scientific
claim about the generator. That transition supplies the game's emotional arc.

### Beautiful physical variation

Each Atlas destination changes laws or regime parameters, not merely color.
Crowding, transport variability, Supply schedule, medium motion, dissipation,
compartment materials, and disturbance families produce visibly and
mechanically different challenges.

## Product structure beyond an eight-chapter campaign

The existing eight chapters remain useful as legacy authored scenarios and
regression content. They are not the target product shell.

The target structure is an Atlas with several ways to play:

- **Expeditions:** authored regime contracts with a beginning, escalation, and
  Holdout gate.
- **Open Field:** choose a regime, function contract, Form, and intervention
  limits; build freely.
- **Bench studies:** load an archived generator and run observation,
  counterfactual, or perturbation jobs without replaying an expedition.
- **Renewal trials:** test external substitution first, then genuine local
  recruitment, positioning, and reconnection.
- **Batch Inheritance Assays:** externally copy a specification and partition
  state, then examine recovery. Life Cycle claims remain reserved until a
  heritable representation and causal copying mechanism are real.
- **Daily or seeded Fields:** compare generators under a shared regime seed and
  declared contract without turning scientific evidence into one collapsed value.
- **Archive challenges:** transplant a frozen generator into regimes it was not
  tuned for and explain why it continues or fails.

Progression unlocks new regimes, instruments, intervention types, and complete
chassis abilities. It should not rely on invisible percentage upgrades that
weaken the legibility of the simulation.

## The game objects

- **Form:** the bright steerable commissioning chassis and one Component of the
  generator.
- **Generator specification:** the frozen local rules, topology constraints,
  Component types, and declared inputs that can be copied into a trial.
- **Embodied generator state:** current positions, inventories, operating
  states, Routes, physical compartment, and local material at time `t`.
- **Component:** a discrete physical site with type, state, stored resource, and
  local rules.
- **Directed Route:** a finite-capacity transfer channel.
- **Supply Stream:** an external source of stored usable resource; it does not
  push objects unless a separate medium-velocity law exists.
- **Physical compartment:** authoritative causal membership and a declared
  leakage law; rendered geometry is derived.
- **Observation View:** a passive analysis selection and protocol. It never
  changes physics.
- **Instrument:** a pure reading over recorded state or history.
- **Intervention:** an explicit causal edit with a typed target and parameters.
- **Regime:** a declared physical environment and its scheduled inputs.

## Screen family

### Atlas

A navigable field of regimes. Each selectable destination reports literal
conditions and an implementation status. Atmospheric names may be subtitles;
they never replace the physical contract.

### Form selection

The selected object is visibly the one the player will steer. The comparison
panel shows the same measured fields for every chassis and distinguishes
implemented abilities from proposed ones.

### Active Field

Most of the screen is the simulated space. The HUD shows only readings required
by the active contract: Route throughput, lowest operating margin, leakage,
grace time, Supply schedule, or another literal criterion.

### Still Mode

The Field pauses cleanly. The player queues structural changes, sees exact
targets and costs, then commits or undoes. A passive View remains a visibly
separate tool.

### Observe Bench

The player chooses a View and instrument. Measurements may be spatial, temporal,
or comparative, but changing the observation protocol is free and noncausal.

### Intervention Bench

Tools expose only parameters that apply to their target. There is no universal
“dose / width / duration” panel. The player chooses whether an intervention
applies to a cloned replay or the live Field.

### Divergence, Ensemble, Holdout, and Archive

These are cold-path analysis surfaces. They summarize jobs rather than streaming
every simulated state into the renderer. They preserve seeds, specifications,
controls, interventions, and outcome contracts so evidence can be reproduced.

## Failure

Failure should usually remain recoverable during commissioning and informative
during analysis. The system identifies the violated criterion and the earliest
causal divergence it can support. It does not collapse everything into a
single aggregate meter.

A Holdout failure is not a punishment screen. It returns the failed seed,
condition, criterion, first divergence, and a branch point from which the player
can revise the generator.

## Visual and audio direction: Number 2

The Number 2 style is graphite-dark, materially dense without literal
imitation, and
dense with fine filament, translucent gel, mineral haze, luminous resource
motion, and restrained instrument typography. Static complexity is baked into
textures; live causal objects remain crisp and interactive.

Sound should reveal load, transfer cadence, instability, intervention, and
release from control. It should not be a constant synthetic drone. Important
events use distinct, learnable motifs whose intensity follows measured state.

## Scientific posture

The game is an artificial coarse-grained world, not a molecular-life simulator
and not a proof that its generators satisfy any external category. It uses the supplied preprint to
discipline distinctions among specification, environment, runtime randomness,
physical compilation, coarse observation, and ensemble-level function.

Small gameplay ensembles do not estimate Shannon entropy or Hartley support by
themselves. A View is not identical to a coarse-graining: only a defined state
projection or measurement grain can play that role, while membership, window,
and surround are held as analysis-protocol variables. These limits are part of
the design, not caveats hidden after the fact.

## First vertical slice

The first complete Number 2 slice contains:

1. one Atlas destination with explicit regime data;
2. three fully implemented Forms with measured comparison fields;
3. one three-Component circulation contract;
4. separate physical-compartment and passive-View controls;
5. one moving Supply-path disturbance;
6. one typed Route intervention;
7. one Divergence Replay;
8. a twelve-seed Ensemble summary;
9. a hands-off Holdout gate;
10. an Archive record containing the specification, seeds, controls, and
    evidence.

That slice proves the complete loop before expanding the Atlas or adding Life
Cycle claims.
