# What Is Life 2 — Milestone Ledger

Status: canonical execution state
Updated: 2026-08-03

Exactly one milestone may be active. A completed entry is never rewritten to
hide a failed check, blocked platform gate, or panel dissent.

## Active

### M-003 — Immutable GeneratorSpec and embodied state split

Selected by: science, game-design, and engine panel

Outcome: frozen generator rules and declared topology constraints live in an
immutable `GeneratorSpec`; positions, inventories, active Routes, compartment
state, and other changing quantities remain in embodied `FieldState`.

Why now: M-002 removed the View from causal state, but authored rules and
changing state still share broad structures and initialization paths. That
ambiguity blocks honest inheritance, Holdout, Renewal, and specification-cost
work and makes later mechanic changes harder to reason about.

Scope:

- introduce an immutable `GeneratorSpec` owned by authoritative Rust state;
- move frozen local rules, declared Component kinds, topology constraints, and
  addressed trial inputs into the specification;
- keep positions, inventories, active Routes, compartment state, and other
  changing quantities in embodied state;
- make transition, replay, perturbation, save, and worker paths consume the
  split explicitly;
- preserve deterministic V2 save compatibility while versioning any necessary
  V3 boundary once, end to end;
- expose only player-meaningful specification and embodied readings in the UI.

Non-goals:

- no typed material, local recruitment, or genuine autonomous Renewal;
- no synchronous Route allocation or segment-based Supply capture correction;
- no entropy, capacity, or information-threshold claim;
- no Atlas integration or broad candidate-ranking redesign;
- no second simulation path outside the existing Rust/WASM Worker architecture.

Completion gates:

- implementation and direct inspection precede validation;
- immutable specification data cannot be changed through embodied transition or
  plan APIs;
- replay and perturbation preserve one frozen specification while changing only
  embodied state and explicit external inputs;
- save/export/import remains deterministic across the split;
- direct play continues through one representative chapter without changing
  the established visual and control surface;
- focused build, type, protocol, migration, and direct-play checks pass;
- exact results and documentation are published to `main`.

Panel resolution:

- Science requires the split before any genomic-specification analogy.
- Game design requires no interruption to the playable Field while the internal
  ownership model changes.
- Engine requires one explicit migration boundary and compile-visible ownership
  rather than parallel compatibility structures.

Implementation progress:

- [`27932ad`](https://github.com/Zosimov735/what_is_life2/commit/27932ad) moves
  authored content identity and pressure rules behind private `GeneratorSpec`
  fields. Live transitions, replay, ranking, protocol, restore, and save paths
  now read those rules through the specification while the canonical V2 save
  bytes remain unchanged.
- Focused evidence: every Rust test target compiles; 30 bridge, frame, export,
  import, and migration checks pass; rebuilt development WASM opens Thread and
  enters Still Mode with both Physical Compartment and Observation View present
  and no browser warning or error entries.

## Queued roadmap

This is the complete planned development queue, not a list that is replaced
whenever the active milestone changes. IDs remain attached to their outcomes.
At each gate the panel may split, combine, or reorder queued work, but it must
record that change here rather than deleting history. Only the milestone under
**Active** is authorized for implementation.

| ID | Milestone | Depends on | Bounded outcome and explicit boundary | Required completion evidence |
|---|---|---|---|---|
| M-004 | Fast deterministic validation pipeline | M-003 | Generate content once, build release WASM once, build the production app once, and execute the complete unchanged validation inventory in parallel Rust shards and read-only web stages. No simulation, content, UI, save, assertion, seed, or coverage reduction. | Exact pre/post test inventories; one release WASM and one production build in logs; identical content/output manifests from repeated builds; every shard assigned exactly once; clean GitHub wall time under 15 minutes. |
| M-005 | Order-independent Transport V4 | M-004 | Replace sequential Route execution with a versioned simultaneous proportional allocator and one-step latency; use point-to-segment Supply capture; make resource rejection and sharing explicit. No new screen. | Route-ID permutations produce identical trajectories; every Charge delta closes; multi-hop cannot traverse twice in one step; migration and replay remain deterministic. |
| M-006 | Truthful chassis and Regime catalogs | M-005 | Generate comparable Form/Regime measurements from content, rename `Current` to **Supply Stream**, fully realize three first-slice chassis, and mark every other unfinished mechanism `Pending`. | Catalog values match engine measurements; implemented abilities have causal and resource evidence; Form choice cannot set global compartment physics; direct comparison-screen inspection passes. |
| M-007 | Number 2 render foundation | M-003, M-006 | Add quality tiers, effective-DPR and render-area caps, cached environmental art, bounded particles/compositing, and idle/hidden animation suspension. No target-Mac performance claim. | Pixel/pass/particle counters meet budgets; Atlas stops while idle/hidden; supported tiers preserve composition; simulation bytes are identical across visual tiers. |
| M-008 | First Fun Atlas Expedition — Steady Transport | M-004–M-007 | Ship Atlas → three truthful Forms → active commissioning → passive View → one paid Route/compartment edit → displaced Supply Stream → quantified result in 6–8 minutes. Other destinations are `Model Pending`; no Ensemble or life claim. | Exact production playthrough; units/windows match engine ledgers; physical, proposed, and View outlines coexist; cold-read players can explain Form, Charge, Supply, View, and intervention. |
| M-009 | Immutable hosted static preview | M-008 | Host the exact CI static artifact over HTTPS to close managed-browser QA without introducing an application server or remote gameplay dependency. | Hosted per-file hashes match CI; direct supported-browser play, reload, save/restore, and request audit pass; only packaged assets are requested; this is not macOS evidence. |
| M-010 | Thin Tauri 2 development package | M-009 | Wrap the same static assets and Worker/WASM core in Tauri 2/WKWebView. No localhost server, native simulation IPC, signing, or distribution claim. | A macOS runner or target Mac launches an unsigned/ad-hoc `.app`; module Worker, WASM, WebGL/Canvas fallback, audio, input, persistence, and offline relaunch pass. A build alone is not a launch pass. |
| M-011 | Exact ledgers and Observe Bench | M-005, M-008 | Add Component/Route/Supply/leakage provenance plus passive Boundary Flow, retention, response-lag, window, surround, and Measurement Grain instruments. Decorative Phase remains hidden. | Readings recompute from retained traces with quantity/unit/View/window; arbitrary instrument changes preserve causal bytes, RNG, plan, budget, trace source, and future trajectory. |
| M-012 | Typed Intervention Bench | M-005, M-011 | Expose only supported tool-specific controls one causal mechanism at a time; remove universal parameter fiction and disclose target, onset, duration, cost, and provenance. | Per-tool preview/commit/undo traces agree with actual ledger deltas; unsupported tools are absent or `Pending`; live commits are costed and trace-ending; clone work leaves live state unchanged. |
| M-013 | Paired Divergence Replay | M-004, M-012 | Replay baseline and intervention from one Anchor under keyed common randomness and one frozen control policy; show the first recorded divergence without claiming complete causation. | Identical branches match until onset; changed action yields a reproducible divergence timeline; unrelated draw order cannot drift; replay is read-only and loaded on demand. |
| M-014 | Durable Archive and analysis jobs | M-004, M-013 | Store specifications, scenarios, anchors, seeds, controls, interventions, analyses, evidence, and failures in versioned IndexedDB; keep long jobs off the live path. No remote service dependency. | Export/import and migration hashes round-trip; cancel/restart recovery works; archived provenance reproduces results; live cadence and the hot-frame cap remain intact. |
| M-015 | Descriptive Ensemble Overlay | M-011, M-014 | Run declared repeated realizations and report traces, pass count/rate, median, observed range, failure modes, and observed trial-set support—never entropy or confidence claims. | Seed/input manifests replay; mixed specifications/regimes are rejected; raw results recompute the summary; live play remains responsive. |
| M-016 | Sealed Holdout and trust transition | M-004, M-012, M-015 | Commit the design and criterion before revealing an independent suite; disable steering, Pulses, rescue, edits, adaptive retry, and suite-specific recompilation. This completes the first full Atlas trust loop. | Pre-reveal hashes match; external-control logs are empty; first attempts and failures remain durable; pass/fail recomputes from Archive; claims stay limited to the declared suite. |
| M-017 | Complete remaining Form mechanisms | M-005, M-011, M-016 | Implement Ring's local seal, locally sensed Lens forecast, Knot junction, conserving Wake cache, and explicit Chorus formation/handoff—or keep unfinished chassis unavailable. | Each ability has local-sensing limits, direct causal traces, conservation, comparative measurements, and hands-off disclosure; none changes global compartment physics. |
| M-018 | Regime and disturbance expansion | M-016, M-017 | Add destinations one physical law at a time, including crowded/diffusive and Vestige-style Fields; separate medium motion from Supply delivery; add only honest disturbance operations. | Every destination versions its lawset, input schedule, criterion, and status; different lawsets are never pooled; each is directly playable with an independent visual/performance budget. |
| M-019 | Measured runtime optimization | M-018 | Profile representative Active Field, Atlas, Worker, and analysis loads; change only measured hot paths while preserving the architecture and deterministic outputs. | Profiles precede changes; Active Field targets 60 rendered frames/s over 30 steps/s, Atlas 30 frames/s and zero idle animation; no latency, hot-frame, artifact-size, or trace regression. Remote results remain remote. |
| M-020 | Typed material, addressed signals, and local maintenance | M-004, M-005, M-018 | Add material pools, degradation, locally available failure signals, detection, recruitment intermediates, positioning state, and local timers while keeping resource, matter, control, and information distinct. | Separate ledgers close; local policies cannot read omniscient experiment state; signal reach/decoding and degradation traces are visible and deterministic. |
| M-021 | External Substitution Assay | M-012, M-020 | Implement experimenter-supplied replacement with an explicit transferred-property policy. It remains a control and is never called Renewal. | Position, kind, state, topology, material, and control supplied versus reset are reported; cloned comparison and external-organization ledger reproduce. |
| M-022 | Genuine autonomous Renewal | M-016, M-020, M-021 | Remove a Component and its Routes, then require frozen local rules to detect loss, recruit material, position a replacement, reconnect, and recover function without rescue. | No player/controller event or automatic topology inheritance; local-information audit; detection/recruitment/reconnection/recovery timings; material and Charge costs; multi-seed positive and negative controls. |
| M-023 | Batch Inheritance Assay | M-014, M-022 | Copy a declared specification externally, partition embodied state, and test recovery across batches while separating inherited, reconstructed, and experimenter-supplied organization. No Life Cycle claim. | Copy/partition provenance, deterministic batch replay, recovery distributions, and an explicit supplied-information ledger pass. |
| M-024 | Constrained Open Field compiler and Archive graph | M-014–M-023 | Compile bounded scenarios from lawset, generator, initial distribution, input schedule, control contract, criterion, interventions, and Views; connect revisions and assays in Archive. No arbitrary code execution. | Canonical compile hash; compile→run→archive→restore reproducibility; authored topology/placement is encoded or disclosed; invalid combinations fail with literal explanations. |
| M-025 | Scientific claim eligibility gate | M-015, M-023, M-024 | Audit the implementation against the preprint. Keep entropy/capacity surfaces disabled unless alphabet, projection, distribution, code, estimator, uncertainty, and unseen-support treatment are declared. | Claim-by-claim evidence matrix; terminology audit; known synthetic estimator recovery if enabled; hash length is never reported as description cost. A disabled-state rationale is a valid result. |
| M-026 | Number 2 product polish, audio, and accessibility | Stable mechanics through M-025 | Complete coherent Atlas/Field/bench/Archive visuals, state-driven sound, onboarding, scalable UI, remapping, captions, reduced motion, contrast, focus, and non-color cues. No mechanic may exist only in decoration. | Full-session play, viewport/tier captures, keyboard-only completion, focus and screen-reader audit, audio-equivalence review, and copy review pass without simulation changes. |
| M-027 | Release persistence and recovery freeze | M-024, M-026 | Freeze save/replay/schema behavior, atomic persistence, corruption recovery, diagnostics, content-change policy, and release support boundaries. | Migrations from every published version; size caps; restart/sleep/wake/corruption/export/import/rollback rehearsals; exact release diagnostics and recovery instructions. |
| M-028 | Representative Mac profiling and release candidates | M-010, M-019, M-027 | Profile the intended laptop, make evidence-driven remediation, and produce matching hosted-web and unsigned Mac candidates. | Target-Mac launch/frame/input/memory/thermal/energy/audio/offline/long-job traces; two consecutive green candidate runs; exact source/content/tool/output provenance; clean upgrade and rollback rehearsal. |
| M-029 | Signed macOS and matching static release | M-028; Apple credentials | Apply hardened runtime, Developer ID signature, timestamp, notarization, and stapling; publish the matching static release and checksums. This gate pauses if credentials or Mac authority are unavailable. | Gatekeeper launch on a clean Mac; signature/notarization/staple verification; tagged source and both artifacts match the evidence manifest; no credential enters the repository. |

Not scheduled or implied:

- A **Life Cycle** claim requires a later panel-approved causal-copying and
  heritable-representation contract. M-023 alone does not earn that language.
- A native Tauri Rust simulation host requires target-Mac evidence that
  Worker/WASM is the dominant bottleneck and a superseding architecture
  decision. Packaging convenience alone cannot schedule it.

### Roadmap-wide rules

- Implementation always precedes new or updated validation. No TDD, ever.
- A later milestone may not silently absorb a missing dependency or strengthen a
  scientific claim beyond its evidence.
- Remote evidence cannot satisfy a target-Mac gate.
- A blocked direct inspection, credential, signing, or hardware gate remains
  visible in this ledger.
- Every completion row keeps its main commit, failed-run history, exact evidence,
  panel dissent, and next selection.

## Completed

| ID | Outcome | Main commit | Direct evidence | Validation | Panel review / next |
|---|---|---|---|---|---|
| M-002 | Physical Compartment is causal and paid; Observation View is independent, immediate, free, and passive. The Number 2 Still surface now renders both with distinct readings and authored field texture. | [`f187f6e`](https://github.com/Zosimov735/what_is_life2/commit/f187f6e) | Direct play selected Thread, entered Still Mode, switched from View to Physical Compartment, preserved budget 3 / queued edits 0 / cost 0, and produced no browser warnings or errors. Same-state source/implementation evidence is recorded in `design-qa.md` and `design-qa-comparison.png`; narrow 319 × 699 inspection found no region collisions after correction. | Release WASM and Vite production builds passed; TypeScript passed; focused validation passed 99 Vitest checks, 30 bridge checks, 1 budget assertion with 6 measurement checks ignored, and lexicon validation over 181 files / 281 entries. A later full suite passed build, typecheck, bridge, budget, campaign, and canonical JSON sections before the user explicitly stopped exhaustive validation in favor of rapid milestone implementation during the slow chapter simulations. | Panel selected M-003, the immutable `GeneratorSpec` / embodied-state split. Exhaustive campaign regression is deferred until the gameplay baseline is implemented far enough to evaluate. |
| M-001 | Reproducible remote-to-laptop static build baseline | [`9df748b`](https://github.com/Zosimov735/what_is_life2/commit/9df748ba76a83df566afaec9263b73b4b79448f4) | Clean Ubuntu 24.04 run [`30765055465`](https://github.com/Zosimov735/what_is_life2/actions/runs/30765055465); exact toolchain doctor; eight-file production artifact, 1,423,134 bytes unpacked and 472,476 bytes archived; worker bundle contains content hash `6aa41dc6…`; WASM is 662,243 bytes. macOS package, signing, launch, frame-time, memory, and energy evidence remain pending. | Release Rust/WASM build passed; app and test TypeScript passed; 534 Rust checks passed and 6 were ignored; 30 Vitest files / 313 checks passed; 35 tool checks passed and 76 lost-document checks remained explicitly skipped; lexicon checked 181 files with no violations. The inherited unoptimized core suite consumed about 25 minutes and is recorded as an iteration-cost issue. | Panel selected M-002, the compile-enforced physical-compartment / passive-View split. |
