# What Is Life 2 — First Synthetic Cell Design Specification

Status: approved vertical-slice product specification; implementation not yet selected by the milestone ledger  
Established: 2026-08-04  
Audience: implementation agents, game design, systems design, content, copy, rendering, audio, and validation owners  
Product authority: `PRODUCT_NORTH_STAR.md` and `CORE_FUN_LOOP_DESIGN.md`  
Engineering authority: `AUTOMATION_AND_CONTRACTS.md`  
Status authority: `MILESTONES.md`

## 1. Outcome

The first complete player-facing organism is one synthetic cell operating in a stable nutrient medium.

The player:

1. chooses the medium;
2. inspects a dormant or near-dormant cell;
3. identifies its boundary, nutrient source, vital maintenance site, internal transport, and reserve;
4. introduces or revises one functional biological machine;
5. teaches one or more local responses;
6. activates autonomous operation;
7. observes an immediate physical consequence;
8. notices an emergent symptom or unexpected coupling;
9. traces that symptom to an actionable relationship;
10. changes one hypothesis;
11. restarts from a named boundary;
12. demonstrates self-maintenance in a hands-off viability assay;
13. retains a descendant whose useful adaptation can later be applied at organismal, population, or ecological scale.

The first retained discovery is metabolic buffering:

> Resource captured during abundance can preserve function during later scarcity.

The first-cell slice proves that the existing deterministic transport and automation substrate can produce a compelling artificial-life toy before the project expands into more advanced contracts or system layers.

## 2. What the synthetic cell is

The synthetic cell is the entire bounded generator and assembly, not one `Form`, one `Component`, or one visual node.

Its player-visible body includes:

- a causal membrane boundary;
- membrane gates;
- internal metabolic or maintenance structures;
- storage bodies;
- mobile or articulated cellular machinery;
- directed internal transport pathways;
- nutrient-bearing external or boundary-crossing flows;
- local sensing and regulatory responses.

This choice resolves the existing ontology without replacing the engine:

- the **physical compartment** is the cell boundary;
- the **generator** is the organized living design;
- the **assembly** is the cell's starting body state;
- **Components** are biological machines within the cell;
- a **Form** is programmable mobile cellular machinery, not the whole cell;
- **Routes** are intracellular or membrane-linked transport pathways;
- **Supply Currents** are nutrient-bearing environmental inputs.

The first cell is therefore compatible with the existing rule that a Form is not the complete generator.

## 3. Scientific abstraction boundary

The first cell is a functional synthetic-cell abstraction. It is not presented as a literal bacterium, literal eukaryotic cell, or complete minimal cell.

This boundary is biologically necessary. Even experimentally minimized autonomously replicating cells remain complex and incompletely understood. JCVI-syn3.0 contained 473 genes, including 149 genes of unknown function at publication. The robust JCVI-syn3A variant contained 493 genes, and one curated metabolic reconstruction included 338 reactions across nine subsystems. A game that exposes six or eight manipulable functions must not imply that real cellular life consists of six or eight parts.

Primary scientific references:

- Hutchison CA et al. “Design and synthesis of a minimal bacterial genome.” *Science* 351, aad6253 (2016). DOI: 10.1126/science.aad6253.
- Breuer M et al. “Essential metabolism for a minimal cell.” *eLife* 8:e36842 (2019). DOI: 10.7554/eLife.36842.

The design response is functional abstraction:

- represent the player-relevant requirements of cellular life;
- keep many basal processes built into the chassis initially;
- expose a subsystem only when manipulating it creates a clear decision and visible consequence;
- never label a gameplay abstraction as a complete biological account;
- add deeper mechanisms later by replacing or refining a declared abstraction, not by pretending it was already literal.

The cell is intentionally neither prokaryotic nor eukaryotic in taxonomic presentation. It is a designed protocell with prokaryote-scale functional goals and visibly modular biological machinery. This gives the player a coherent first organism without forcing the engine to model molecular interactions, organelle biogenesis, or full gene expression.

## 4. Functional requirements of the first cell

A real cell cannot be reduced to a fixed number of organelles. The first slice instead declares eight functional requirements and decides which are manipulable now.

| Functional requirement | First-slice representation | Player manipulability |
|---|---|---|
| Boundary and selective exchange | physical compartment plus membrane gates | visible and partly editable |
| Energy and nutrient acquisition | nutrient input, mobile uptake machinery, Coupling, accepted resource | directly manipulable |
| Internal distribution | directed transport pathways, interfaces, flow regulation | directly manipulable |
| Reserve and temporal buffering | reserve structure and finite stored metabolic energy | directly manipulable |
| Sensing | local nutrient, energy, target, pathway, overload, and timer readings | directly manipulable through responses |
| Regulation and information | ordered local responses, deterministic target selection, local signals | directly manipulable |
| Synthesis, turnover, and basal maintenance | upkeep, loss, repair-purpose ledger, essential maintenance site | represented but initially not decomposed into molecular machinery |
| Replication and division | absent as an active first-slice mechanic | explicitly deferred |

The initial interactive anatomy may contain more or fewer than six visible objects. The requirement is that these functions are legible, not that each function maps one-to-one onto one node.

### 4.1 Boundary

The boundary separates the cell from the medium and makes internal organization meaningful.

Existing authority:

- `PhysicalCompartment`;
- causal membership;
- boundary leakage;
- opening physical membership;
- separate passive Observation View.

Player-facing name:

- compact: **Membrane**
- expanded: **Cell membrane and enclosed interior**
- internal: `PhysicalCompartment`

The membrane must visibly distinguish:

- inside from outside;
- membrane gates from ordinary internal structures;
- nutrient crossing from internal transport;
- physical leakage from evaluator evidence;
- causal membrane edits from passive microscope or observation selection.

### 4.2 Energy and nutrient acquisition

The cell must detect and acquire an external resource that becomes usable metabolic energy.

Existing authority:

- Supply Current state;
- local Supply sensing;
- seek Supply;
- Coupling;
- accepted Supply cue;
- stored Charge;
- exact Charge ledger;
- movement and upkeep.

Player-facing names:

- `Supply` -> **Nutrients**
- `Charge` -> **Metabolic energy**
- `seek_supply` -> **Move toward nutrients**
- `couple` in nutrient context -> **Bind and absorb**
- accepted Supply -> **Nutrient absorbed**

The game does not call the resource ATP, glucose, proton motive force, carbon, or another literal molecule. It is an abstract usable metabolic resource whose source and sinks are exact.

### 4.3 Internal distribution

Acquisition alone does not sustain the cell. Resource must reach essential structures.

Existing authority:

- Directed Routes;
- endpoint gates;
- requested and accepted flow;
- source-stock limitation;
- capacity throttling;
- destination headroom;
- Route controls;
- simultaneous conserving allocation.

Player-facing names:

- `Route` -> **Transport pathway**
- endpoint interface -> **Gate**
- requested flow -> **Demand**
- accepted flow -> **Delivered energy**
- Route capacity -> **Pathway capacity**
- allocation weight -> **Priority**

The first important systems lesson is:

> Nutrients present in the cell are not equivalent to energy delivered where it is needed.

### 4.4 Reserve and temporal buffering

The cell may store resource during abundance and release it during scarcity.

Existing authority:

- `Reserve` Node kind;
- Vault finite reserve;
- bank and release events;
- periodic nutrient input;
- stored Charge;
- leakage and upkeep;
- Charge thresholds and timer conditions.

Player-facing names:

- `Reserve` -> **Energy reserve**
- Vault in first-cell context -> **Reserve vesicle** or **storage body**
- bank -> **Store excess**
- discharge -> **Release reserve**
- periodic Supply quiet state -> **Nutrient gap**

The first emergent adaptation is not “a bigger number.” It is a new relationship between past and future:

> The cell's current behavior depends on what it stored during earlier abundance.

This is metabolic memory.

### 4.5 Sensing

The cell must act only on locally available information.

Existing authority:

- own Charge fraction;
- own operating margin;
- local Supply state;
- compatible target in range;
- owned Route flow;
- local overload;
- local signal;
- local timer;
- deterministic candidate and target projection.

Player-facing names:

- sensor envelope -> **Receptor range**
- readable candidate -> **Detected structure**
- target -> **Current focus**
- no target -> **Nothing compatible detected**

The player must see what a structure can sense and what it cannot. A response should fail because its input is absent or its target is inaccessible, not because the game silently changes the rule.

### 4.6 Regulation and information

The player teaches responses rather than writing programs.

Existing authority:

- `FrozenLocalPolicy`;
- up to eight ordered rules and fallback;
- beginning-of-phase snapshot;
- first true rule;
- stable-address evaluation;
- deterministic target ordering;
- named outcomes.

Player-facing surface:

```text
When [local condition], [response].
Otherwise, [resting response].
```

Recommended first-slice terms:

| Internal condition/action | Compact player wording |
|---|---|
| `always` | At all times |
| `charge_below` | When energy is low |
| `charge_above` | When energy is abundant |
| `operating_margin_below` | When a vital structure is underfed |
| `supply: present` | When nutrients are nearby |
| `supply: emitting` | When nutrients are flowing |
| `supply: quiet` | During a nutrient gap |
| `target_in_range` | When a compatible structure is within reach |
| `route_flow_below` | When pathway delivery is weak |
| `route_flow_above` | When pathway delivery is strong |
| `overloaded` | When this structure is overloaded |
| `signal_present` | When a local signal is received |
| `timer_elapsed` | After waiting |
| `hold` | Rest here |
| `seek_supply` | Move toward nutrients |
| `seek_port` | Move toward a compatible gate |
| `seek_signal` | Follow the local signal |
| `change_depth` | Move inward or outward |
| `couple` | Bind and exchange |
| `set_interface` | Open or close the gate |
| `set_route` | Regulate the pathway |
| `emit_signal` | Release a signal |
| `use_ability` | Use specialized function |

The exact wording may be refined through the copy catalog and lexicon, but the interface may not revert to code-like `IF/THEN` as the primary ordinary-play presentation.

### 4.7 Basal maintenance

The first cell must pay a continuing cost to remain organized.

Existing authority:

- Component upkeep;
- purpose-typed upkeep ledger;
- layer drain;
- leakage;
- overload;
- operating margin;
- essential receiver Charge criteria.

Player-facing names:

- upkeep -> **Maintenance cost**
- operating margin -> **Vital margin**
- drain -> **Basal demand** or **environmental loss**, according to source
- leakage -> **Membrane loss**
- overload -> **Metabolic strain**

The initial maintenance machinery remains largely built into the cell chassis. It is represented by costs and one or more vital structures, not by a literal ribosome, proteasome, genome, and biosynthetic network. Those may become interactive only when the design can support them as meaningful systems.

### 4.8 Replication and division

Replication is not present in the first slice.

The UI may use descendant, branch, variant, or lineage for engineering records. It must not describe cloning a blueprint as cellular reproduction unless the simulation contains:

- a causal copying event;
- inherited representation;
- embodied resource cost;
- offspring or descendant state;
- variation;
- a selection context.

Replication and evolution remain long-term goals. Their absence is explicit, not hidden by metaphor.

## 5. Current source inventory

The first cell must reuse the existing authoritative substrate.

### 5.1 Existing contract sequence

`content/contracts/manifest.json` currently contains:

1. `intake`
2. `transfer`
3. `buffer`

These already form a biological sequence:

- nutrient acquisition;
- internal delivery;
- resource buffering across scarcity.

The internal ids may remain unchanged for compatibility and record custody.

### 5.2 Existing Node kinds

The current closed Node set is:

- `Port`
- `Reserve`
- `Module`
- `Form`

The first-cell slice should compose these roles rather than immediately add literal organelle classes.

Initial functional interpretation:

| Node kind | First-cell role |
|---|---|
| `Port` | membrane gate or exchange junction |
| `Reserve` | energy storage body |
| `Module` | metabolic, maintenance, or service machinery |
| `Form` | programmable mobile cellular machinery |

A future schema may add differentiated biological hardware only after the first loop demonstrates that the distinction creates a meaningful choice.

### 5.3 Existing programmable vocabulary

Conditions already available in Rust:

- always;
- energy below;
- energy above;
- operating margin below;
- nutrient state;
- target in range;
- pathway flow below;
- pathway flow above;
- overload;
- signal present;
- timer elapsed.

Actions already available in Rust:

- rest;
- seek nutrients;
- seek gate;
- seek signal;
- move between layers;
- bind and exchange;
- open or close gate;
- regulate pathway;
- emit signal;
- use specialized function.

This vocabulary is sufficient to prove nutrient uptake, transport, buffering, and basic homeostasis. No cell-specific TypeScript behavior engine is permitted.

### 5.4 Existing physical and evidence vocabulary

Already represented:

- stored energy;
- finite capacity;
- maintenance;
- leakage;
- accepted and requested flow;
- nutrient emission and quiet phases;
- movement;
- local sensing;
- pathway gating;
- reserve banking and release;
- overload;
- local signals;
- deterministic events;
- provisional criteria;
- immutable viability-assay records;
- lineage and engineering-memory foundations.

The first implementation task is therefore presentation, composition, content tuning, and loop integration before new cellular physics.

## 6. First medium

Internal regime:

- `open_field` for initial acquisition and distribution;
- `periodic_transport` for the first scarcity and buffering challenge.

Player-facing medium:

## Balanced Nutrient Medium

Literal description:

> A still synthetic medium with one visible nutrient source, low background loss, no predation, no hidden disturbance, and enough resource to sustain a correctly organized cell.

Required properties:

- still or visually quiet environmental motion;
- one obvious nutrient-bearing source;
- stable law parameters;
- low enough leakage that the first failure is not dominated by membrane tuning;
- no noise, fracture, crowding, deceptive signals, or withheld schedule in the initial assay;
- a visible and predictable nutrient gap introduced only when buffering is taught;
- resource and medium motion visually distinct.

The medium should be chosen through one simple decision, not a broad Atlas comparison. The first-session choice may be a single selected default with one literal explanation. Medium selection becomes strategically broad only after the player understands the cell.

## 7. Persistent first-cell identity

The target experience is one developing cell lineage across acquisition, distribution, and buffering.

The internal contracts may remain separate authored records, but the player-facing continuity must be honest.

Required identity rules:

- the player begins with one named or identifiable synthetic-cell lineage;
- each accepted design revision creates an explicit descendant;
- the transition from nutrient uptake to internal transport and buffering retains or derives from the exact prior cell design;
- a new contract opening may not pretend to be the same cell if it silently replaces unrelated generator and assembly bytes;
- when existing content requires a new opening, the UI must state that the prior design is being instantiated or adapted into the next assay;
- passing a viability assay does not automatically transfer to descendants.

The initial implementation may use explicit derived attempts across the three existing contract records. It must not fabricate continuity from display metadata alone.

## 8. Player-facing contract sequence

Internal ids remain `intake`, `transfer`, and `buffer`. Player-facing titles should communicate biological function at two levels.

| Internal id | Compact title | Expanded title | Biological question |
|---|---|---|---|
| `intake` | **Feed** | **Nutrient Uptake** | Can the cell detect, reach, absorb, and retain usable energy? |
| `transfer` | **Circulate** | **Internal Transport** | Can acquired energy reach the structures that need it? |
| `buffer` | **Endure** | **Metabolic Buffering** | Can the cell store abundance and survive a predictable nutrient gap? |

Compact titles are verbs. Expanded titles are literal biological functions. Neither is a narrative chapter name.

### 8.1 Feed / Nutrient Uptake

Opening concept:

- one visible cell membrane;
- one nutrient source;
- one mobile uptake apparatus;
- one vital maintenance structure;
- minimal transport relation;
- no useful response installed or one near-working response with one legible defect.

Available responses:

- detect nutrients;
- move toward nutrients;
- bind and absorb;
- rest.

Primary lesson:

> Detection, approach, binding, absorption, storage, and maintenance are separate events.

Likely informative failures:

- nutrients not detected;
- compatible target outside range;
- uptake apparatus arrives late;
- binding occurs but useful resource does not reach the vital structure;
- movement cost exceeds captured energy;
- cell loses more energy than it absorbs.

### 8.2 Circulate / Internal Transport

Opening concept:

- retained or derived cell design;
- membrane gate or internal exchange gate;
- directed pathway;
- source, staging structure, and vital receiver;
- accepted-flow service requirement.

Available responses:

- open or close gate;
- enable, disable, limit, or prioritize pathway;
- bind to compatible structure;
- respond to local energy or pathway flow.

Primary lesson:

> A physical connection, an open gate, a demand, and an accepted delivery are different states.

Likely informative failures:

- pathway exists but gate remains closed;
- pathway is disabled;
- demand is issued but source stock is insufficient;
- capacity is too low;
- destination is full or unavailable;
- one path monopolizes a limited source;
- vital structure remains underfed despite high total cell energy.

### 8.3 Endure / Metabolic Buffering

Opening concept:

- retained or derived transport-capable cell;
- finite reserve structure;
- predictable nutrient-rich and nutrient-gap phases;
- ongoing maintenance and leakage;
- vital service requirement through the complete gap.

Available responses:

- store excess when energy is abundant;
- seek nutrients when available;
- release reserve when a vital margin falls;
- use timer or nutrient phase;
- regulate delivery.

Primary lesson:

> Average abundance can coexist with lethal scarcity; survival depends on managing energy over time.

Likely informative failures:

- reserve never fills;
- reserve releases too early;
- reserve releases too late;
- storage competes with vital maintenance;
- leakage consumes reserve;
- transport capacity prevents reserve recovery;
- the cell survives one gap but cannot restore reserve for the next.

## 9. First survival criterion

The first player-facing survival criterion is:

## Maintain metabolic homeostasis without rescue

Literal meaning:

> Keep the designated vital structure above its minimum metabolic-energy requirement for one complete hands-off viability window while membrane loss remains below the declared ceiling.

The criterion uses existing authoritative sources:

- minimum stored energy at the vital Component;
- accepted delivery where required;
- leakage ratio;
- hands-off service duration;
- exact observation window and grace.

The exact numerical thresholds remain content-owned and are validated by Rust. React does not infer survival.

Player-facing criterion rows:

- **Vital energy:** essential structure remains above its minimum;
- **Energy delivery:** required pathway supplies the structure where applicable;
- **Membrane loss:** leakage remains below the declared limit;
- **Self-sustained time:** the cell maintains function without rescue for the full window.

The first assay should not require growth or division. Survival means finite-horizon self-maintenance under the declared medium, not proof of open-ended life.

## 10. Failure is emergent, not scripted

The content should be near-working but not hard-coded to fail in one exact way.

Requirements:

- at least two mechanically distinct first failures must be plausible;
- the opening must not solve itself;
- the player must not need to rebuild the whole cell before seeing the first relationship;
- each likely failure must produce a different symptom and different revision;
- guidance may point to an observed mismatch but may not state the complete solution.

Preferred early failure family:

> Nutrient energy is present or captured, but the vital structure remains underfed because acquisition, gating, transport, storage, or maintenance is mismatched.

This failure teaches the central systems principle:

> Possessing a resource is not the same as delivering function.

## 11. First emergent behavior

The first surprising behavior is metabolic memory.

The player introduces a reserve to solve one local problem, such as wasted abundance or brief starvation. The resulting cell then behaves differently across time:

- abundance creates stored state;
- stored state changes later response options;
- the cell bridges a nutrient gap;
- the vital structure remains functional despite no current intake;
- recovery after the gap depends on how much reserve remains and how quickly it can refill.

The player did not directly command “be starvation tolerant.” The player assembled storage, sensing, and local responses whose interaction produces starvation tolerance.

The game may summarize a retained lineage fact such as:

> Prototype 4 developed starvation tolerance after repeated energy-transport failures.

That summary must link to exact ancestor, revision, assay, and evidence records.

## 12. First adaptation and later reuse

The first retained adaptation is:

## Metabolic buffering / starvation tolerance

It is selected because it generalizes across later scales.

| Scale | Reuse of the same principle |
|---|---|
| Cell | energy reserve bridges nutrient gaps |
| Multicellular organism | storage tissue or distributed reserves protect vital organs |
| Population | resource banking changes survival and reproduction under scarcity |
| Colony | shared stores alter cooperation and competition |
| Ecosystem | seasonal storage changes resource pressure and population dynamics |
| Evolution | selection trades storage cost against starvation resilience |

The game should explicitly help the player recognize that a principle learned at one layer can become a tool at another. It should not simply award a “buffer upgrade.”

## 13. First 30-minute experience

This is a target, not a scripted mandatory timeline.

### 0–2 minutes: medium and organism recognition

The player sees:

- Balanced Nutrient Medium;
- one synthetic-cell boundary;
- one nutrient source;
- one vital structure;
- one mobile uptake apparatus;
- one reserve or empty reserve slot;
- a small number of transport relations.

The player can answer:

- where nutrients are;
- where the cell is;
- what must stay energized;
- what can move;
- what is currently inactive.

### 2–6 minutes: first biological response

The player selects the mobile apparatus.

The response editor offers a constrained sentence:

> When energy is low, move toward nutrients.  
> Otherwise, rest here.

Preview shows:

- receptor range;
- detected nutrient source;
- selected target;
- movement reach;
- expected local action, clearly marked as preview.

### 6–9 minutes: activation

The player activates the cell.

Within several authoritative steps:

- receptor activity appears;
- target relation locks;
- the apparatus moves;
- nutrient contact or named no-op appears;
- energy and maintenance readings begin changing.

The player can connect the taught response to the physical action.

### 9–14 minutes: first symptom

A meaningful mismatch appears.

Possible examples:

- uptake apparatus absorbs nutrients but the vital structure dims;
- the membrane gate remains closed;
- transport demand exceeds delivery;
- energy is spent on movement faster than it is replenished.

The Field communicates urgency without immediately stating the solution.

### 14–18 minutes: diagnosis

The player selects the stressed structure.

Progressive disclosure shows:

1. **Symptom:** underfed;
2. **Immediate reason:** delivered energy below demand;
3. **Pathway state:** gate closed, capacity limited, source starved, or destination blocked;
4. **Response:** which local rule acted or failed to act;
5. **Editable boundary:** open gate, revise response, alter pathway, reposition machinery, or change starting reserve.

### 18–22 minutes: focused revision

The player changes one relationship.

The exact before/after change is visible in biological language. The named reset states what will be retained and reconstructed. Prior evidence remains.

### 22–25 minutes: visible improvement

The revised cell immediately behaves differently. The player does not wait for the full assay to know that the hypothesis changed the machine.

### 25–30 minutes: first viability or scarcity reveal

The cell either:

- completes a short hands-off viability assay; or
- enters a predictable nutrient gap and reveals the need for metabolic buffering.

The player leaves with one clear next question.

## 14. Visual anatomy

The first cell must read as one bounded living system rather than a loose graph.

### 14.1 Membrane

Required cues:

- stable enclosing contour;
- interior/exterior distinction;
- embedded gates;
- local permeability or leakage indication;
- no overlap with passive View geometry;
- no full-cell warning tint for a local membrane problem.

### 14.2 Nutrient medium

Required cues:

- visible nutrient-bearing material distinct from background medium;
- local concentration or flow direction where authoritative;
- emission and quiet phases;
- admitted nutrient crossing only when accepted;
- no particles at zero accepted uptake.

### 14.3 Vital maintenance structure

Required cues:

- recognizable stable silhouette;
- energy reservoir or internal activity region;
- visible decline from stable to strained to critical;
- exact energy and maintenance values on inspection;
- no physical fracture unless an embodied failure exists.

### 14.4 Mobile uptake or transport apparatus

Required cues:

- receptor region;
- actuator or binding region;
- stored energy;
- selected-target relation;
- movement aligned with authoritative velocity;
- action result localized to actual contact.

### 14.5 Transport pathways

Required channels:

- physical connection;
- direction;
- gate/controller state;
- demand;
- delivered energy.

A pathway that exists but delivers nothing must not look like successful circulation.

### 14.6 Reserve

Required cues:

- finite fill volume;
- bank and release direction;
- current reserve and capacity;
- reserve contribution to vital service;
- visible recovery after scarcity.

### 14.7 Regulation

Required cues:

- restrained receptor activation;
- selected response and target;
- distinct preview treatment;
- no floating code block over the organism;
- exact response sentence in the inspector.

## 15. Symptom and urgency grammar

The first cell uses the global urgency ladder.

| State | Field behavior | Inspector wording | Player expectation |
|---|---|---|---|
| Stable | normal cadence, quiet margin | “Stable” plus exact margin | no immediate action |
| Strained | subdued local contraction, dimming, or slower cadence | “Energy margin falling” | inspect when convenient |
| At risk | persistent local stress and affected pathway emphasis | “Vital energy may cross its minimum in this window” | action likely needed |
| Critical | localized cessation, depletion, or authoritative failure stance | “Vital function failed” with exact first violation | diagnose before next attempt |
| Recovering | staged return of fill, cadence, or gate function | “Recovering” with remaining deficit | verify the repair holds |

Low stress remains visible in data even when the Field remains visually quiet. Salience communicates urgency, not mere existence.

## 16. Diagnosis layers

### Glance layer

Answers:

- Is the cell stable?
- Which region needs attention?
- Is the problem intake, transport, reserve, membrane, or regulation?

### Selected-object layer

Answers:

- What does this structure do?
- What is its current energy, demand, capacity, and margin?
- Which response is active?
- What target and outcome are current?

### Causal-relation layer

Answers:

- What entered?
- What was requested?
- What was delivered?
- Where was it limited?
- What state change preceded the symptom?

### Exact evidence layer

Answers:

- stable ids;
- exact units;
- step;
- criterion relation;
- event window;
- branch and assay identity.

Ordinary play begins at the glance layer. Expert players may remain in exact evidence. No layer contradicts another.

## 17. Player-language dictionary

The following is the target direction. Copy review may refine wording without changing causal meaning.

| Internal term | Compact player term | Expanded explanation |
|---|---|---|
| generator | cell design | frozen anatomy, pathways, and local responses |
| generator revision | cell variant | a descendant design with an exact biological change |
| assembly template | starting cell state | opening positions, energy, gates, material, and membrane membership |
| Component | cellular machinery | one physical structure with a function and local state |
| Form | mobile machinery | a programmable moving structure inside the living design |
| Port | membrane gate / gate | an exchange endpoint that may be open or closed |
| Module | metabolic structure | a cellular machine that performs or receives a required function |
| Reserve | energy reserve | a finite structure that stores usable metabolic energy |
| Route | transport pathway | a directed finite-capacity energy connection |
| Supply Current | nutrient stream | external resource-bearing input |
| Charge | metabolic energy | conserved usable resource stored and transferred by the simulation |
| policy | responses / regulation | ordered local condition-response behavior |
| rule | response | one local condition and biological action |
| fallback | otherwise response | what the structure does when no earlier response applies |
| criterion | viability requirement | an exact relation the cell must maintain |
| qualification | viability assay | a hands-off declared test of the frozen cell |
| grade | evidence profile | separate throughput, resilience, economy, and complexity reading |
| restart | reconstruct starting cell | retain declared design and rebuild the opening state |
| blueprint | cell template | exact reusable living design plus starting cell state |
| branch | descendant variant | a new editable design linked to its source |
| regime | medium | declared physical environment |
| contract | assay / challenge | one declared biological function and its exact requirements |

Prohibited normal first-session language unless expanded or placed in advanced diagnostics:

- schema;
- protocol;
- hash;
- canonical bytes;
- stale guard;
- operation journal;
- generator spec;
- assembly template;
- cold job;
- immutable conflict;
- qualification request.

These concepts remain available where recovery or exact evidence requires them.

## 18. Interaction surface

The first-cell workspace should remain one coherent living-system instrument.

### Top band

Shows:

- medium;
- current viability requirement;
- weakest vital margin;
- Design or Active state;
- Activate/Pause;
- speed;
- reconstruct;
- viability assay.

### Center Field

Shows:

- membrane;
- nutrient source;
- cellular machinery;
- pathways;
- local responses and symptoms;
- direct selection and permitted anatomy editing.

### Right rail

Shows the selected structure:

- biological function;
- energy and capacity;
- maintenance and vital margin;
- gate or pathway ownership;
- installed responses;
- active response, target, and outcome;
- local draft and preview.

### Bottom band

Shows a causal history:

- nutrient phase;
- detection;
- target acquisition;
- binding;
- admitted resource;
- pathway delivery;
- reserve bank/release;
- maintenance;
- leakage;
- first symptom;
- recovery or violation.

At narrow widths, the Field remains primary and the right/bottom regions become dedicated sheets.

## 19. Audio

First-cell audio supports comprehension.

Recommended cues:

- nutrient contact and accepted uptake;
- gate opening and closing;
- internal delivered-energy cadence;
- reserve banking and release;
- strain onset;
- vital-function failure;
- recovery;
- viability-assay freeze and resolution.

No character chirps, narrative stingers, generic heartbeat loop, or constant alarm is required. A subtle living-machine bed may exist only if it follows measured activity and never obscures event information.

## 20. Engineering and biological layer separation

The biological presentation is a projection over exact authority.

Rules:

- internal ids and record schemas remain stable unless the owning engineering packet changes them;
- player copy comes from `content/copy/catalog.json`;
- lexicon changes are reviewed and machine-enforced;
- Rust remains the only owner of sensing, target selection, action admission, movement, transfer, criteria, and assay results;
- TypeScript may select, format, stage drafts, and project geometry;
- renderer effects disappear when the authoritative quantity is zero;
- biological summaries link to exact records;
- no cell-specific shadow simulation is introduced.

## 21. Implementation packets

This specification does not become active until `MILESTONES.md` selects or incorporates the work. Once selected, implementation should proceed in the following bounded order.

### FSC-00 — Ontology and vocabulary bridge

Input:

- current contract ids;
- current copy catalog;
- current lexicon;
- current Node and policy vocabularies.

Work:

- add compact and expanded biological copy;
- define cell, membrane, machinery, nutrient, energy, pathway, reserve, response, and viability terms;
- preserve internal ids;
- remove code-like primary labels from the first-cell route;
- add exact advanced diagnostic expansion.

Handoff:

A future screen can describe the current simulation as one synthetic cell without inventing mechanics.

### FSC-01 — Cell composition and identity

Input:

- physical compartment;
- current Intake opening;
- generator/assembly and attempt/branch authority;
- first-cell scene projection.

Work:

- establish the entire bounded generator as one cell;
- localize membrane, gate, vital structure, mobile machinery, pathway, and nutrient source;
- create honest lineage continuity across the three assays;
- expose stable cell identity and descendant relation;
- ensure the Form is not presented as the whole organism.

Handoff:

The player sees and selects one coherent cell.

### FSC-02 — Balanced Nutrient Medium

Input:

- `open_field`;
- Intake Supply Current;
- existing environmental and compartment laws.

Work:

- present one stable medium;
- minimize irrelevant disturbance;
- show nutrient-bearing input distinctly;
- annotate only the few facts needed for the first prediction;
- add predictable periodic availability as a later state, not initial noise.

Handoff:

The environment is understandable before any response is authored.

### FSC-03 — Teach the first response

Input:

- current local-policy editor;
- pure Rust preview;
- Intake capability envelope.

Work:

- replace programming-first presentation with local biological response sentences;
- constrain initial options;
- show receptor range, detected candidates, selected target, action reach, and likely named no-op;
- preserve ordered evaluation and fallback visibly but quietly.

Handoff:

A new player can author one valid response and predict its first action.

### FSC-04 — Nutrient uptake and internal delivery loop

Input:

- Intake and Transfer mechanics;
- accepted Supply;
- requested/accepted Route evidence;
- selected-object inspection;
- timeline.

Work:

- make detection, approach, contact, admission, storage, demand, delivery, maintenance, and loss physically distinct;
- tune a near-working opening with multiple plausible failures;
- implement symptom-first diagnosis;
- connect each mismatch to one relevant editable boundary.

Handoff:

The player can feed the cell, see why a vital structure remains underfed, revise, and observe a changed consequence.

### FSC-05 — Metabolic buffering and starvation tolerance

Input:

- Buffer periodic medium;
- Vault reserve;
- reserve events;
- full-cycle criteria.

Work:

- introduce one predictable nutrient gap;
- expose store/release responses;
- make reserve state alter later viability;
- show competition between storage, maintenance, transport, leakage, and recovery;
- retain the discovered adaptation in lineage language.

Handoff:

The player creates and recognizes metabolic memory.

### FSC-06 — Viability assay and retained cell history

Input:

- M-021 qualification records;
- first-cell criteria;
- engineering memory and lineage authority.

Work:

- present qualification as a viability assay;
- show frozen cell design, starting state, medium, and requirements;
- retain exact pass/fail and separate evidence axes;
- write no aggregate score;
- create descendant action and adaptation summary from evidence.

Handoff:

The first cell becomes a reusable, inspectable living-system record.

### FSC-07 — Core fun-loop proof

Input:

- integrated first-cell route.

Deferred verification when the validation hold is lifted:

- first-time comprehension;
- first consequence latency;
- first informative symptom timing;
- diagnosis traversal time;
- focused revision time;
- restart and lineage clarity;
- same-state renderer parity;
- biological-language comprehension;
- expert-depth usefulness;
- no-idle behavior;
- viability-assay comprehension;
- post-session explanation and next-hypothesis quality.

Handoff:

The project may expand only after the core loop demonstrates that the player can create, understand, revise, and care about the cell.

## 22. Source ownership

| Layer | Primary current owners | First-cell responsibility |
|---|---|---|
| Rust content | `core/src/content.rs`, `content/contracts/*.json` | compile stable medium, cell opening, capabilities, criteria, and copy keys |
| Rust causal state | `core/src/field.rs`, `core/src/policy.rs`, `core/src/run.rs`, `core/src/state.rs` | preserve exact cell mechanics, conservation, local information, and identity |
| Rust protocol | `core/src/protocol.rs` and engineering/qualification modules | carry exact cell, response, assay, and lineage records |
| Worker | `worker/src/protocol.ts`, `worker/src/entry.ts` | scheduling and transport only; no biological inference |
| Shell | `AutomationWorkbench.tsx`, `FieldSurface.tsx`, ladder/result/engineering surfaces | cell composition, response authoring, symptom diagnosis, assay, lineage |
| Copy | `content/copy/catalog.json`, lexicon tools | compact biological labels plus exact explanations |
| Scene | `app/src/render/scene.ts`, WebGL, Canvas2D | membrane, machinery, nutrient, pathway, reserve, response, symptom, and recovery grammar |
| Archive | `app/src/shell/archive.ts` | durable exact variants, assays, adaptation summaries, and recovery |
| Documentation | this file, `CORE_FUN_LOOP_DESIGN.md`, milestone ledger | prevent scope and ontology drift |

## 23. Non-goals

The first synthetic-cell slice does not require:

- literal molecular species;
- gene expression simulation;
- ribosome or protein-level authoring;
- DNA sequence editing;
- cell growth or division;
- mutation;
- population dynamics;
- predators;
- ecological succession;
- open-ended evolution;
- offline accumulation;
- a new cell-specific simulation core;
- replacement of the existing deterministic ledger;
- a full eukaryotic organelle catalog;
- a fixed one-object-per-biological-function anatomy;
- exposure of engineering-memory recovery language during ordinary play.

These remain future systems only when the existing loop can support and explain them.

## 24. Acceptance boundary

At source level, the first-cell specification is integrated only when:

- the entire bounded generator is represented as one cell;
- the membrane, nutrient source, vital structure, mobile machinery, transport pathway, reserve, and response state are player-readable;
- the three existing contracts form one honest developmental sequence;
- compact and expanded biological language share exact engine authority;
- no normal first-cell screen requires programming, schema, or database vocabulary;
- the first response produces an immediate visible effect;
- at least two emergent failure families are supported;
- diagnosis reaches one actionable biological relationship;
- metabolic buffering produces starvation tolerance through ordinary mechanics;
- the viability assay uses immutable ordinary qualification records;
- the retained descendant and adaptation link to exact evidence;
- renderer, inspector, timeline, and audio tell the same causal story;
- no new advanced system was added to compensate for an unclear first-cell loop.

Validation, publication, and human-readiness claims remain deferred until the milestone ledger lifts the hold and records fresh evidence.

## 25. Agent stop conditions

An agent must stop and correct the owning boundary before continuing when:

- the cell is presented as a single Form;
- a biological label implies a mechanic that does not exist;
- a real-cell fact is used to justify an incomprehensible control;
- TypeScript calculates a target, action, survival state, or adaptation;
- the first session exposes all eight rules or all advanced policy primitives without need;
- visual nutrient motion appears when accepted resource is zero;
- a failure is legible only in logs;
- a tutorial cue gives the full policy solution;
- a contract swap silently replaces the organism while claiming continuity;
- a branch inherits a viability pass;
- an adaptation summary cannot be traced to exact revisions and evidence;
- passive waiting becomes the principal activity;
- reproduction or evolution language appears before their causal mechanics;
- an agent proposes population or ecosystem work to compensate for an unproven single-cell loop.

The immediate product question is not “How much biology can the engine contain?”

It is:

> Can the existing engine make one synthetic cell understandable, surprising, revisable, and worth caring about?
