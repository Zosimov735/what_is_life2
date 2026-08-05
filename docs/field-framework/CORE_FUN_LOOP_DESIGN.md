# What Is Life 2 — Core Fun Loop Design

Status: approved player-experience authority  
Established: 2026-08-04  
Audience: implementation agents, designers, engineers, content authors, visual and audio owners  
Depends on: `PRODUCT_NORTH_STAR.md`  
Constrains: `PRODUCT_OUTLINE.md`, `AUTOMATION_AND_CONTRACTS.md`, contract content, player copy, visual language, and milestone selection  
First implementation specification: `FIRST_SYNTHETIC_CELL_DESIGN.md`

## 1. Purpose and authority

This document defines the experience that the simulation architecture must produce.

The engineering contracts remain authoritative for causal state, deterministic transitions, persistence, qualification, and record identity. This document is authoritative for the player's object of attachment, the core fun loop, the language used to explain that loop, the order in which complexity may be exposed, and the standard by which additional systems are judged worth building.

Where the internal implementation says `GeneratorSpec`, `AssemblyTemplate`, `Component`, `Route`, `Charge`, `Contract`, or `QualificationRequest`, the player does not automatically receive those names. Internal precision protects the game. It is not the game.

The product must not become:

- a scientifically impressive simulation without a compelling activity;
- a database or debugging interface presented as play;
- an automation editor whose primary fantasy is programming;
- an idle system in which elapsed time replaces decisions;
- a sequence of increasingly elaborate mechanics added before the existing loop is understandable;
- a literal claim to reproduce cellular, organismal, ecological, or evolutionary biology.

The governing product statement is:

> The player creates artificial organisms from biological machines, teaches them simple behaviors, observes emergent systems, discovers hidden relationships, and evolves increasingly complex living systems through experimentation.

## 2. Player role

The player is an omniscient living-systems engineer.

That does not mean the player directly controls every object. The player's power comes from choosing the medium, arranging biological machinery, declaring local responses, introducing new structures or pressures, and understanding enough of the resulting system to guide its development.

The player is not:

- a pilot inside the system;
- a character with a personal narrative;
- a commander issuing continuous unit orders;
- a programmer writing source code;
- a passive observer waiting for numbers to increase;
- a laboratory technician whose reward is correct paperwork.

The player is simultaneously, at different scales:

- a synthetic-cell designer;
- an organismal architect;
- a population and ecological manager;
- an experimentalist;
- an evolutionary pressure;
- an archivist of living-system history.

The feeling of power must come from organization rather than rescue. The player should be able to say, “I made this system capable of doing that without me.”

## 3. Object of attachment

The unit of attachment expands with play.

Early attachment:

> My cell survived.

Intermediate attachment:

> My organism developed a useful adaptation.

Long-term attachment:

> This lineage changed the colony and the medium around it.

End-state attachment:

> I shaped a living system whose history I understand.

The game must therefore preserve continuity across scales. A cell is not discarded merely because organismal play becomes available. An organism is not reduced to a stat block when a population appears. Earlier designs, failures, adaptations, and descendants remain intelligible parts of the larger system.

The machine is not a character, but it must still become personal through:

- recognizable anatomy;
- named or identifiable designs;
- retained lineages;
- unusual successful structures;
- remembered failures;
- adaptations discovered rather than awarded;
- consequences that propagate into later layers.

## 4. Biological depth posture

The long-term target is a deep living-systems simulator. A biology expert should find additional relationships and analogies to appreciate, while a new player should never need expert vocabulary to make the first correct decision.

The standard is:

> Expertise should increase appreciation, not determine basic comprehension.

The game may use biological facts, functional analogies, and systems principles. It must stop following literal biology when literal detail prevents a clear, manipulable, and enjoyable loop. Whenever that happens, the abstraction must remain honest:

- label the simulated quantity by what it does in the game;
- avoid claiming a one-to-one biological equivalent;
- retain conservation and causality where the system depends on them;
- prefer a legible functional subsystem over an inaccurate decorative organelle;
- reserve “genome,” “evolution,” “reproduction,” and similar terms for mechanics that actually support their causal meaning.

The game simulates principles from which life-like behavior can emerge. It is not a research tool and does not make external biological predictions.

## 5. The emotional center: discovering hidden coupling

The central reward is not merely success, failure, or automation.

It is this sequence:

> I was succeeding.  
> I introduced something new.  
> It broke one part of the system.  
> I fixed that part.  
> Something else failed.  
> Then I realized that an earlier choice was coupled to a system I had not understood as related.

The strongest moments come from discovering hidden coupling:

- greater energy capture increases transport or storage demand;
- more signaling improves coordination but consumes resources;
- higher throughput increases loss or overload;
- specialization improves one function while increasing dependency;
- efficiency reduces reserve margin;
- redundancy improves resilience but increases upkeep;
- a local adaptation changes population or environmental behavior later.

The game must not hide these relationships arbitrarily. They should be unanticipated because the player has not yet understood the system, not because the engine withholds the relevant rule.

## 6. Core fun loop

The complete loop is:

```text
Choose a medium and inspect the living system
  -> predict what the system needs
    -> introduce or revise biological machinery
      -> teach local responses
        -> activate autonomous operation
          -> notice a symptom, opportunity, or unexpected behavior
            -> inspect the affected relation
              -> form one hypothesis
                -> make one deliberate change
                  -> restart or continue from a named boundary
                    -> compare the consequence
                      -> retain the adaptation, branch, or assay result
```

The loop is successful only when the player can connect intention, action, consequence, explanation, and revision.

The loop is not:

```text
read instructions -> enter expected solution -> wait -> receive score
```

Nor is it:

```text
place objects -> watch decorative activity -> react to unexplained failure
```

## 7. Core-loop levels and design agenda

Every implementation review must examine each level below. A packet is not player-complete merely because the underlying command exists.

### 7.1 Choose and observe the medium

Player question:

> Where does this living system exist, and what does that environment make possible?

The medium is the first source of constraints, not a decorative background. It may determine nutrient availability, transport conditions, dissipation, movement, crowding, leakage, disturbance, or other law-level effects.

Existing substrate:

- Regime and `ContractSpec`;
- Supply Currents;
- medium motion;
- pressure schedules;
- physical compartments;
- Atlas and contract catalog surfaces.

Required player clarity:

- what resource is available;
- where it enters;
- whether availability is steady, periodic, scarce, or variable;
- what the medium removes or resists;
- what the cell can sense directly;
- what is stable enough to ignore during the first prediction.

Failure to avoid:

- beginning on a regime-selection spreadsheet;
- atmospheric names replacing physical meaning;
- presenting a medium whose first important effect is invisible;
- asking the player to compare many regimes before understanding one organism.

Visual requirement:

The medium should have a stable material identity and a restrained visible resource pattern. Environmental motion, nutrient-bearing flow, and causal boundaries must remain distinct.

Language requirement:

Use a biological, material, or ecological name with a literal expanded explanation. Internal “regime” identifiers may remain in technical details but should not be the primary first-session noun.

### 7.2 Recognize the living system

Player question:

> What is this organism made of, and which parts matter now?

The player must identify the system at a glance before opening the inspector.

Existing substrate:

- physical compartment;
- Components and Node kinds;
- Forms;
- Ports;
- Reserves;
- Modules;
- Routes;
- stored Charge;
- exact selected-object inspection.

Required player clarity:

- the boundary of the living unit;
- where energy or material is stored;
- where nutrients enter;
- which part performs required maintenance;
- which structures move or regulate transport;
- which pathways connect them;
- which parts are active, strained, or failing.

Failure to avoid:

- every object looking like a generic node;
- labels carrying all meaning while geometry remains interchangeable;
- the mobile Form being mistaken for the whole organism;
- the player needing to know code identities to understand anatomy.

Visual requirement:

Each functional role needs stable anatomy. Dynamic state may alter fill, motion, texture, articulation, or damage, but it must not destroy object recognition.

### 7.3 Introduce or revise machinery

Player question:

> What function should I add, remove, connect, or reposition?

The player manipulates functional biological machines rather than abstract configuration records. Early choices should be few, materially distinct, and capable of producing more than one viable solution.

Existing substrate:

- topology edits;
- assembly editing;
- Component and Route limits;
- physical membership;
- positions;
- stocks and material;
- interfaces;
- generator and assembly identity;
- draft preview and exact commit boundaries.

Required player clarity:

- what function a structure contributes;
- what it costs to maintain;
- what it must connect to;
- what new demand or dependency it introduces;
- whether the edit changes the living design or only its starting state.

Failure to avoid:

- exposing every field because it exists;
- requiring the player to reconstruct a real cell;
- hiding cost until after commit;
- allowing visually plausible but mechanically impossible anatomy;
- presenting generator and assembly identity as player goals.

### 7.4 Teach local responses

Player question:

> How should this part respond when it senses a local condition?

The player must not feel as though they are programming. The deterministic ordered policy remains an internal mechanism. The player-facing operation is teaching a response, setting regulation, or establishing an instinct.

Existing substrate:

- eleven local condition kinds;
- ten local action kinds;
- ordered first-match evaluation;
- deterministic target selection;
- local sensing and signals;
- draft preview;
- exact selected rule, target, action, and outcome;
- up to eight rules plus fallback.

Required player clarity:

- what the structure can sense;
- what it cannot know;
- what response will be attempted;
- what target is eligible;
- range, resource, interface, and cooldown limits;
- which response wins when several conditions are true;
- what happens otherwise.

Player grammar:

```text
When [local condition], [biological response].
Otherwise, [resting response].
```

Early play should expose no more rules than are needed to understand the causal loop, even if the engine supports eight.

Failure to avoid:

- code-like syntax as the primary surface;
- unexplained rule order;
- global omniscience hidden inside a friendly label;
- menu options that the selected machinery cannot perform;
- a preview that predicts results independently of Rust.

### 7.5 Activate and manage autonomous operation

Player question:

> Is my living system doing what I expected?

Activation should feel like releasing a constructed living system, not starting a benchmark or pressing “play” on a passive animation.

Existing substrate:

- Design and Commission authority;
- immediate pause;
- 1x, 4x, and 16x scheduling;
- local policy evaluation;
- authoritative action outcomes;
- Routes, storage, upkeep, leakage, pressures, and signals;
- exact frame and event carriage.

Required player clarity within several authoritative steps:

- which structure sensed something;
- which response became active;
- which target was selected;
- what physical action occurred;
- whether resource transfer was requested, accepted, throttled, blocked, or impossible;
- which essential margin changed.

Failure to avoid:

- a delay between the player's change and its first visible consequence;
- movement or particles with no authoritative transfer;
- a simulation-rate change that appears to change physics;
- direct steering that competes with local autonomy;
- a system that operates correctly but cannot be read.

### 7.6 Notice symptoms and opportunities

Player question:

> Does this need my attention now?

Low-level stress should always be available in exact data, but visual salience must communicate urgency.

The required visual urgency ladder is:

1. **Stable:** normal operation; exact margins available on inspection.
2. **Strained:** a margin is weakening or a reserve is trending down; local, restrained cue.
3. **At risk:** failure is likely within the current window without a change; persistent local cue and clear affected relation.
4. **Critical:** an essential function has failed or crossed its grace boundary; unmistakable localized state and evaluator evidence.
5. **Resolved or adapted:** the system has returned to a stable state or demonstrated a retained adaptation; stable final stance, not a celebratory overlay.

Symptoms come first. Explanation follows through inspection.

Failure to avoid:

- every low margin flashing red;
- only showing stress after failure;
- full-screen alarms for local problems;
- evaluator evidence fabricating physical damage;
- exact data hidden because the visual appears healthy.

### 7.7 Diagnose the relationship

Player question:

> Why did this happen, and what can I change?

The diagnosis path is progressive disclosure:

```text
visible symptom
  -> affected structure or pathway
    -> immediate physical reason
      -> preceding local response and input
        -> upstream quantity or dependency
          -> editable boundary
```

Existing substrate:

- selected-object inspection;
- addressed mechanism events;
- policy outcome records;
- criterion margins;
- first-violation traces;
- current-versus-parent comparison;
- branch and engineering-memory records.

Required player clarity:

- the first actionable mismatch, not every log entry;
- the difference between observation and inference;
- the exact object and relation involved;
- a direct path to the relevant anatomy, response, pathway, or starting-state edit.

Failure to avoid:

- debugger stack traces as the default presentation;
- an answer that merely restates the failed criterion;
- automatic root-cause claims unsupported by evidence;
- forcing panel hunting;
- giving the full solution instead of exposing the relationship.

### 7.8 Form a hypothesis and revise

Player question:

> What one change do I think will improve this system?

The player should be encouraged to make a deliberate hypothesis rather than randomly retune many parameters.

Existing substrate:

- policy and assembly drafts;
- exact diffs;
- named restart boundaries;
- retained attempts;
- branch lineage;
- blueprint capture;
- comparison context.

Required player clarity:

- what changed;
- what stayed fixed;
- where the next run begins;
- what prior evidence remains;
- what outcome would support or refute the player's expectation.

Failure to avoid:

- destructive resets;
- silent compatibility repair;
- changing several systems by default;
- lineage language that feels like source control;
- losing the unexpected behavior that motivated the revision.

### 7.9 Assay or qualify

Player question:

> Does the living system maintain its function without my intervention?

Qualification is the proof moment. It should feel like a viability, survival, robustness, or environmental assay depending on scale. The backend remains reproducible and exact. The player-facing experience remains about whether the organism can sustain itself.

Existing substrate:

- immutable qualification request;
- cold authoritative trial execution;
- exact criterion vector;
- independent Throughput, Resilience, Economy, and Complexity evidence;
- first violation;
- complete result records;
- receipt-derived progression.

Required player clarity:

- what has been frozen;
- what conditions will be tested;
- what function counts as survival or success;
- why rescue is disabled;
- which result is functional pass/fail and which readings are tradeoffs;
- what is learned from failure.

Failure to avoid:

- benchmark or CI language as the emotional frame;
- aggregate score replacing evidence;
- a grade compensating for failed function;
- hidden trial mechanics that leak into policy inputs;
- qualification presented before the player has formed an attachment to the candidate.

### 7.10 Preserve, compare, and expand attachment

Player question:

> What did this organism become, and where can that adaptation matter next?

Engineering memory becomes living-system history.

Existing substrate:

- immutable generator and assembly records;
- branches and attempts;
- blueprints;
- linked evidence;
- clone, diff, compatibility, transplant, and comparative-result plans.

Required player clarity:

- ancestor and descendant;
- inherited design versus changed starting state;
- retained adaptation;
- unexpected tool discovered through failure;
- which later system can reuse the principle.

Example:

> Prototype 4 developed starvation tolerance after repeated energy-transport failures.

That statement is useful only when the retained evidence supports it. The game may summarize the history, but it must preserve the literal design and result beneath the summary.

Failure to avoid:

- activity-feed history;
- version numbers without meaning;
- treating a passing result as inherited by every descendant;
- deleting failures that later explain an adaptation;
- locking discoveries to only the layer where they first appeared.

## 8. Developmental scale

The long-term product grows by composing the same systems principles across levels:

```text
synthetic cell
  -> multicellular organism
    -> population or colony
      -> ecosystem
        -> evolutionary system
```

### Synthetic cell

Primary questions:

- Can it acquire usable energy?
- Can it distribute and buffer resources?
- Can it maintain a boundary and essential function?
- Can local responses stabilize it?

### Multicellular organism

Primary questions:

- Can cells specialize?
- Can tissues exchange resource and information?
- Can organism-level homeostasis emerge from local regulation?
- What dependencies appear when functions are distributed?

### Population or colony

Primary questions:

- Can organisms cooperate, compete, or divide labor?
- How do resource strategies change population structure?
- How do local adaptations spread or fail?

### Ecosystem

Primary questions:

- How do living systems alter the medium?
- Which relationships become symbiotic, competitive, or destabilizing?
- How does environmental history constrain later designs?

### Evolutionary system

Primary questions:

- What varies?
- What is inherited?
- What is selected?
- What tradeoffs persist across generations?
- What novel tools emerge from failed or unusual adaptations?

The project may aim for Dwarf Fortress-scale systemic history, but it must reach that scale by layering understandable systems. It must not jump directly to population complexity while the single-cell loop remains opaque.

## 9. Persistent simulation without idle-game drift

Long-term play may allow the player to leave a stable system operating and return to inspect change. The attractive quality is persistence, history, and delayed consequence—not passive accumulation.

Rules:

- elapsed time must not replace a meaningful decision;
- no required early-game progress depends on leaving the game unattended;
- important changes must remain reconstructable and diagnosable;
- background progression may create a new state to reason about, not merely a larger number;
- the player must be able to bound, pause, accelerate, or assay long-running behavior;
- offline or inactive-tab simulation must not be introduced until deterministic custody, event compression, and meaningful return-state explanation exist.

The first synthetic-cell slice contains no offline progression requirement.

## 10. Language system

Player-facing language has three layers.

### Compact biological label

Used during ordinary play.

Examples:

- Energy
- Nutrients
- Membrane gate
- Transport pathway
- Reserve
- Response
- Viability

### Expanded literal explanation

Used on inspection or help.

Examples:

- “Stored usable metabolic resource in this structure.”
- “A directed pathway that can request and accept a limited amount of energy each step.”
- “The local response selected because this structure’s energy fell below the declared fraction.”

### Internal technical identity

Used only in exports, recovery, advanced diagnostics, or contributor documentation.

Examples:

- `Charge`
- `RouteControlState`
- `GeneratorSpec`
- `assembly_hash`
- protocol version

A compact label may be evocative, but it must expand to exact mechanism. A biological label may not imply a mechanic the engine does not own.

## 11. Visual clarity principles

The Field must answer four questions before the player opens a panel:

1. What is this structure?
2. What is it doing?
3. What does it need?
4. Does it need attention now?

The visual system must distinguish:

- boundary from observation;
- nutrient-bearing flow from environmental motion;
- sensing from targeting;
- requested transfer from accepted transfer;
- stored resource from active movement;
- controller state from physical state;
- physical failure from criterion violation;
- live state from draft preview;
- stable strain from immediate danger.

Color reinforces meaning but never carries it alone. Shape, line, texture, motion, fill, and spatial anchoring remain sufficient in reduced-color and low-quality modes.

The presentation should have the material immediacy of a physical toy. World of Goo demonstrates how an engineering system becomes approachable when its material is tactile and its constraints are obvious. Human Resource Machine demonstrates how deterministic logic becomes playful when commands are small, consequences are immediate, and the problem remains visible. What Is Life 2 must apply those lessons without making the player feel that they are programming workers or stacking generic nodes.

## 12. Audio clarity principles

Audio communicates transitions and load, not atmosphere alone.

Useful families:

- nutrient admission;
- accepted transfer cadence;
- membrane or interface switching;
- reserve fill and release;
- growing strain;
- localized failure;
- recovery;
- assay freeze and resolution.

Rules:

- repeated cues aggregate;
- continuous sound scales with authoritative activity;
- selection and preview never resemble physical action;
- failure sounds once and then remains visually legible;
- mute and reduced-sound modes preserve all information.

## 13. Fun-loop proof gate

Before advanced contract, population, reproduction, ecosystem, or evolution implementation expands, the first-cell loop must demonstrate all of the following:

1. A new player can identify the cell boundary, nutrient source, vital structure, reserve, and transport relation without contributor explanation.
2. The player can author one local response without describing the activity as programming.
3. The first committed change produces a visible consequence within several authoritative steps.
4. The player encounters an informative symptom within the intended early-session budget.
5. The player can traverse symptom -> object -> relation -> response -> editable boundary in under twenty seconds.
6. The player can make one deliberate revision and restart in under thirty seconds.
7. The revised system visibly changes before the proof window completes.
8. A hands-off assay clearly distinguishes function from Throughput, Resilience, Economy, and Complexity evidence.
9. The player can explain one unexpected coupling in ordinary language.
10. The retained lineage makes the adaptation useful beyond the original attempt.
11. The player reports the central feeling: “I made this organism, I saw why it changed, and I know what I would try next.”

If the loop fails this gate, agents must improve comprehension, latency, consequence, or revision before adding more biological systems.

## 14. Relationship to implementation milestones

This document does not silently change the active milestone. `MILESTONES.md` remains the sole status authority.

Before implementation begins:

- the milestone ledger must select a bounded first-cell packet or record its dependency on the active engineering-memory work;
- existing accepted source work is preserved;
- the lowest durable transition boundary required for rapid revision must be closed rather than abandoned half-implemented;
- later engineering-memory, transplant, advanced-contract, and product-cutover packets may not be treated as substitutes for proving the fun loop.

The detailed implementation bridge is `FIRST_SYNTHETIC_CELL_DESIGN.md`.

## 15. Agent instructions

Future agents must:

- read this document before modifying player-facing automation, contracts, copy, or visual grammar;
- distinguish internal engineering names from player language;
- inventory existing mechanics before proposing new physics;
- prefer a biological composition of existing authoritative primitives over a parallel cell simulator;
- state which layer of play a new mechanic belongs to;
- state what player question the mechanic answers;
- state the first visible consequence and likely informative failure;
- preserve symptoms-first, explanation-second diagnosis;
- preserve exact evidence beneath biological summaries;
- update `content/copy/catalog.json` and the lexicon for player-facing terminology;
- route all causal behavior through Rust;
- retain renderer parity and reduced modes;
- avoid population, reproduction, or evolution claims until variation, inheritance, and selection are mechanically present;
- record status as specified, source present, integrated, validated, or published without conflating them.

The project succeeds when technical rigor disappears into a living system the player can understand, manipulate, and care about.
