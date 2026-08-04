# What Is Life 2 - Human Playtest Guide

Status: playtest contract
Date: 2026-08-03

Open `http://localhost:5173/?field_run` for the authored campaign shortcut.
This must show Form selection first, then the persistent campaign rail after
commissioning begins. `?field_stand_in` is a developer-only renderer diagnostic
with no campaign and is not a playtest entry point.

This guide tells a tester what the game currently is, what each action should
do, and where an unexpected result is most likely to be a defect. It is a
companion to the mechanics contract, not a replacement for it.

## What the game is

What Is Life 2 is a systems-life game about commissioning a local generator,
observing whether it sustains an explicit function, disturbing it, and finally
removing the player's control. The bright object is a steerable **Form**. The
luminous network is the larger embodied generator: Components store usable
resource, Directed Routes transfer it, Supply Streams deliver it, and physical
compartments affect leakage.

The central question is not whether a structure looks convincing. It is whether a
frozen local organization continues to meet a measurable function under a
declared environment and control contract.

## Persistent campaign position

The running Field carries a top campaign rail. It names the current chapter,
shows `Chapter N / 8` and `Objective N / total`, reports elapsed campaign time,
and draws separate chapter and overall progress lines. The clock is the
authoritative continuous campaign step counter at 30 steps per second; it does
not reset at chapter transitions and stops in Still Mode.

The rail is a position instrument rather than a walkthrough. Use the detailed
table and chapter sections below for expected time bands and exact completion
conditions. Report any case where the rail disagrees with the objective line,
the elapsed clock advances in Still Mode, a chapter count changes unexpectedly,
or the run advances out of the order below.

## Campaign map and expected clock

Simulation time runs at **30 steps per second** while the Field is active.
Still Mode stops simulation time. Travel, searching, reading Why, optional
objectives, and any resettable hold that loses its condition add to real play
time. The ranges below are navigation aids, not speedrun requirements.

| Campaign position | Chapter | Expected chapter time | Approximate campaign clock | What changes |
|---|---|---:|---:|---|
| 1 of 8 | The Pull | 25-30 min | 0:00-0:30 | Learn Supply, stored Q, Coupling, Ports, Routes, depth, and Drift. |
| 2 of 8 | The Edge | 18-35 min | 0:30-1:05 | Separate the causal Physical Compartment from the passive Observation View. |
| 3 of 8 | The Loop | 36-50 min | 1:05-1:55 | Build and sustain a closed circuit through closure and Flood. |
| 4 of 8 | The Echo | 44-55 min | 1:55-2:50 | Bank Charge, survive repeated Supply outages, and use a deep spare path. |
| 5 of 8 | The Mesh | 36-60 min | 2:50-3:50 | Coordinate multiple Forms and two simultaneous patterns under Interference. |
| 6 of 8 | The Break | 44-60 min | 3:50-4:50 | Repair a severed route, then survive a permanent Fracture. |
| 7 of 8 | The Rewrite | 56-70 min | 4:50-6:00 | Rebuild a dependency while Supply, upkeep, and topology fail together. |
| 8 of 8 | The Quiet Edge | 50-80 min | 6:00-7:10 | Close the final ring, lose both supplies, then leave it hands-off for 18.2 min. |

The complete campaign is therefore a long-form run of roughly **five to seven
hours**, depending on route knowledge, Form, optional objectives, and resets.
An Anchor is written at authored milestones and every chapter transition. The
run keeps its selected Form, intervention budget, completed objectives, keyed
random state, and campaign step across a transition; each new chapter replaces
the Field itself.

## How progression actually works

Only one required objective is active at a time. Its sentence is a command; its
progress bar is authoritative; Why contains the full mechanical condition.
There is no hidden confirmation button. The objective completes on the first
simulation step that satisfies its condition or fills its required duration.

| Objective language | What the game is measuring | How to advance it | What should happen on screen |
|---|---|---|---|
| Enter, follow, hold, or take a current | The controlled Form is inside the named Supply Stream on the correct depth layer. | Steer the near-white Form into the moving band and follow it when it moves. | Cyan delivery filaments reach the Form; the progress bar rises. Most current counts are cumulative, so leaving pauses rather than erases progress. |
| Store a number of CU | The Form Component's actual stored Q has reached the stated quantity. | Stay connected to Supply; do not release Coupling merely to make the timer move. | The amber reservoir arc around the Form fills. The objective bar follows stored Q directly and can fall if Q is spent or lost. |
| Approach a Port | The Form is within the authored world-space distance of the named dormant Port. | Steer toward the hollow circular Component. | The objective completes on arrival; the next objective asks for Coupling. |
| Open a Port | Every named Port is open. | Hold E until the hollow Port is inside the reach circle and visibly locks, then release E. | The segmented gate resolves into an active Component; connected Routes may begin moving Q. |
| Start or run Routes | Every named Directed Route moved Q on the same simulation step. | Open both endpoints, provide Q at the tail, and remove any cut or blockage. | Route ticks and beads move tail to head. A line without moving ticks is connected geometry, not successful transport. |
| Hold, carry, sustain, or close a pattern | All named Routes flow together and named Components have no unrelieved overload for a continuous duration. | Keep Supply and relief paths active. Repair closures or cuts immediately. | The progress bar rises only while the whole pattern is valid. A miss resets this hold to zero unless Why explicitly says the count is cumulative. |
| Reshape the compartment | Physical membership differs from the chapter's opening compartment. | Enter Still Mode, select the Physical Compartment tool, drag a thick-edge handle across a Component, and commit with Enter. | A mint proposed edge appears before commit; the warm mineral edge replaces it after commit and the intervention budget falls. Moving the violet View does not count. |
| Optional Port or pattern | The condition is available for a fixed offer span and then passes without blocking the campaign. | Complete it before its offer expires, or ignore it and continue when the next objective appears. | There is no failure screen. Completion may improve reserves or an ending mark; expiry simply advances. |
| Let it run | The closed pattern remains valid under neutral input for the full hands-off window. | Release every key. Do not steer, Couple, change depth, enter Still Mode, hand off, or rescue. | The criterion rail enters hands-off countdown; the Field continues without player action. |

### Progress that appears stuck

1. Open Why and identify whether the condition is cumulative or continuous.
2. Confirm the controlled Form is the near-white chassis, not another cool-grey
   Form in a Chorus field.
3. For a current objective, look for a delivery filament to the Form. Merely
   touching the glow around the band may be outside its true capture width.
4. For a Route objective, look for moving directional ticks on every required
   line. A bright endpoint alone does not prove flow.
5. For a pattern objective, look for a red overload ring, a closed segmented
   Port, a broken Route, or an inactive Supply Stream.
6. If the objective is hidden, leave Still Mode. Hiding it while paused is
   intentional; hiding it while the simulation is running is not.
7. If all visible conditions are met for five active seconds and the bar has not
   moved, record the objective text, chapter, Form, step, and a screenshot.

## Starting a run

1. On the **Atlas**, select an implemented Regime with the pointer or arrow
   keys. Press Enter or activate the selected destination.
2. On **Select Form**, choose one of the eight chassis and confirm it. The panel
   at right states its measured operating contract.
3. Begin commissioning. The Field opens with the selected lawset and Form.
4. Read the objective and its **Why** explanation before changing the Field.

The opening is intentionally dark and sparse. Structure, flow, pressure, local
materials, and intervention effects should become legible as the run develops.

## Controls

### Atlas and Form selection

- Pointer: select an entry or activate a control.
- Arrow keys: move the current selection.
- Enter: open the selected Regime or confirm the selected Form.

### Active Field

- `WASD` or Arrow keys: steer the controlled Form.
- Pointer motion: inspect only. It must never steer the Form.
- Hold `E`: extend the coupling radius.
- Release `E`: apply the coupling Pulse.
- Mouse wheel or `[` / `]`: change depth.
- Space: enter Still Mode.
- Pointer: inspect literal Field objects and activate **Why** or other UI.

Pointer presses never charge or release a Pulse. Keyboard navigation and the
Why control remain usable while steering and Coupling are keyboard-only.

### Still Mode

- `C`: select the causal Physical Compartment tool.
- `V`: select the passive Observation View tool.
- Drag Component to Component: queue a Directed Route.
- Drag a Route endpoint: queue a redirect.
- Select a Route and press Delete: queue removal.
- Drag a physical-compartment handle: queue a paid boundary edit.
- Enter: commit queued physical interventions.
- Escape: undo the newest queued intervention; press Escape again with an empty
  queue to leave Still Mode.
- Space: leave Still Mode directly.

The objective is hidden in Still Mode so the intervention and observation
surfaces have enough room. This is intended.

## Complete visual dictionary

The renderer uses no floating names over Field objects. Shape, hue, motion, and
local state marks are therefore the legend. Inspection text is authoritative
when a mark is ambiguous.

### Atlas, Form selection, and transitions

| What you see | What it means |
|---|---|
| Large constellation-like map on Atlas | The available Regime catalogue. Its connecting filaments are navigation structure, not the campaign map and not Field Routes. |
| Bright boxed Atlas destination | Currently selected Regime. Only entries marked Implemented can begin a playable run. |
| Regime detail block below/beside the Atlas | The selected environmental lawset: Supply, dissipation, transport, medium motion, compartment, interventions, and criterion. It describes rules, not campaign progress. |
| Form radio list | Eight chassis choices for the entire campaign. Changing selection here does not start a run until Begin commissioning. |
| Form contract panel | Quantitative ability summary for the selected chassis. These are starting capabilities, not upgrades or scores. |
| Full-screen chapter review | The previous chapter completed and the next Field is being established. Clearing it does not erase the Anchor written at the transition. |
| Ending review | The Quiet Edge completed the campaign. The run should remain ended and accept no further simulation input. |

### Field background and depth

| What you see | What it means | Causal status |
|---|---|---|
| Near-black graphite field | The play surface, not empty space and not a resource. | Decorative substrate. |
| Sparse dust, faint etched lines, low glints | Atmosphere and scale texture. They may move with the camera. | Decorative; not inspectable and never a target. |
| Dark translucent plane haze | Another depth layer seen through the current camera layer. Deeper marks lose contrast and appear smaller. | Shows authoritative layer separation. |
| Broad directional medium streaks | The Regime has medium motion. Their direction is the environmental push; drag and collisions can move Forms. | Causal environmental motion, but not Supply. |
| Darkening or settling of the whole Field | The short ramp into Still Mode. | Simulation time is slowing to a pause. |

### Components, Forms, and stored Q

| Shape or mark | Object/state | Meaning |
|---|---|---|
| Near-white faceted polygon with a pale-blue chassis ring | Controlled Form | This is the only Form moved by WASD/arrow steering. The pointed facet indicates travel direction. |
| Cool-grey faceted polygon | Uncontrolled linked Form | Present in Chorus fields. It retains local state but does not take steering until a Handoff. |
| Circular Component | Port | An interface used by Routes. A Port must be open at both ends before a Route can carry. |
| Hollow dark circle with three separated gate arcs | Dormant Port | Closed. Move into Coupling reach, hold E until it locks, then release. |
| Filled/glowing circle with an intact outer ring | Open Port | Participates in Routes. Opening is persistent unless a chapter event closes it. |
| Broken dim Route with a transverse mark at one end | Dormant wiring | The marked endpoint Port is closed, so no Q can cross. Open that exact Port with Coupling. |
| Continuous Route with a small directional notch | Operational wiring | Both endpoint gates are open. The notch points from the tail toward the receiving head; moving ticks appear when Q actually flows. |
| Square Component | Reserve | Stores Q. An amber inner arc shows its reserved fraction. Vault can discharge its isolated reserve through Coupling. |
| Diamond Component | Module | A functional processing or dependency Component. Chapter events may cut Routes around it or require replacement. |
| Warm amber center or bloom inside any Component | Stored Q | Brighter/larger means more Q relative to visible capacity. It is not intervention budget. |
| Amber arc around the controlled Form | Form reservoir | The fraction of the Form Component's Q capacity currently filled. This is the visual answer to Store N CU. |
| Pale-cyan expanding ring around a Component | Recent Supply delivery | A Supply Stream delivered Q to this recipient on a recent step. |
| Solid coral/red outer ring | Overload | The Component holds more than its threshold. If no outgoing Route is actively carrying, pattern objectives enter recoverable setback. |
| Warm mineral outer ring on a Component | Physical-compartment shell contact | The Component is on the causal material boundary and contributes exposed leakage contacts. |

### Supply Streams and Routes

| What you see | Object/state | Meaning |
|---|---|---|
| Thick bright cyan-white moving band | Bright Supply Stream | Authored source of Q. It delivers to eligible Components inside its true capture width; it does not push the Form. |
| Dim slate-blue moving band | Ordinary or deep Supply Stream | Another Supply source, often stronger and paired with deeper-layer drain. |
| Cyan-to-amber curved filaments from a band to a Component | Active Supply delivery path | Shows exactly which visible recipient is geometrically eligible for delivery. Travelling beads move toward the recipient. |
| Thin graphite line between Components | Directed Route geometry | Tail-to-head connection. Geometry alone does not mean Q is moving. |
| Pale moving ticks/beads along a Route | Route flow | Q moved along the Route on the current step. Their direction is tail to head and their density/brightness follows flow. |
| Route warming from graphite toward pale blue | Increasing flow fraction | More of that Route's own capacity is being used. |
| Red Route treatment | Overloaded or pressure-stressed Route | Capacity or pressure is compromising transport. Inspect it for the exact state. |
| Broken/dashed mint line | Queued new Route or redirect | Preview only. It becomes causal only after Enter commits the queue. |
| Ochre/cut mark on a Route | Queued removal or authored break | The line is cut or will be cut. A missing Route never routes around itself. |
| Two crossbars over a Route | Clamp | Temporary capacity intervention is active. |
| Magenta disruption on a Route | Scramble | Temporary delivery uncertainty is active. |
| Gold timing mark | Delay | Input timing is shifted for the declared duration. |
| Coral breach arc | Breach | Temporary physical leakage intervention is active. |

### Coupling, before and after release

| Coupling mark | Meaning before release | Expected result on release |
|---|---|---|
| Dashed circular reach centered on the Form | True world-space reach for the current E hold. It grows from 8 to 192 field units. | Only objects inside this circle can be affected. |
| Rotating sweep and stable radius ticks | The hold is active and reach is still being communicated. | No effect yet; release is the action edge. |
| Curved tether from Form to a locked object | That exact object is an authoritative target for this release. | The target receives the effect indicated by its local animation. |
| Warm inward beads | Gather target | One quarter of eligible nearby non-Form Q moves into the Form, bounded by available stock and headroom. |
| Cyan iris around a hollow Port | Open target | The exact core-projected dormant Port opens; this lock is not guessed from screen distance. |
| Outward ochre chevrons | Interference target | Reachable Interference is pressed back. |
| Reach circle with no tether and No connection in the strip | Nothing effective is in range. | The emission occurs but gathers nothing, opens nothing, and suppresses nothing. |
| Compact Coupling strip above the objective | Predicted release summary | Shows reach and only nonzero Draw, Release reserve, Open Ports, and Suppress Interference effects. |

### Boundaries, View, and Still Mode

| What you see | Meaning | What interaction does |
|---|---|---|
| Thick warm mineral-grey closed edge with a faint filled interior | Physical Compartment | Determines causal membership and leakage. Editing costs intervention budget and changes retained history. |
| Thin pale-violet closed outline with no fill | Observation View | Determines what instruments measure. Editing is immediate, free, and noncausal. |
| Mint broken boundary | Proposed Physical Compartment | Queue preview. Nothing causal has changed yet. |
| Faint violet candidate outlines | Ranked observation candidates | Alternative measurement apertures. Sparsity groups equal tiers; the brightest outline is focused. |
| Circular handle on a Component | Port handle | Start a new Route from that Component. |
| Handle at either end of a Route | Route endpoint handle | Redirect that endpoint. |
| Handle on a thick boundary vertex | Compartment handle | Change physical membership. |
| Violet forecast strip near the foot of the Field | Forward envelope for the active View | Passive predicted range; an empty strip means no forecast is available. |
| Swelling rings around viewed Components during playback | Retained perturbation sample | Ring size is the sample's relative stored-Q reading; a second faint ring is the baseline when one exists. |

### Pressure, cues, and shell chrome

| What you see | Meaning |
|---|---|
| Colored rim pressing inward from the viewport edge | Active pressure. Rim depth is level; beating means crisis. Blue is Drain, violet Noise, coral Fracture, teal Flood, ochre Interference, grey Drift. |
| Matching local arc on a Component, Route, Current, or boundary | The pressure's actual target. The target is not inferred from the nearest object. |
| Brief pale-cyan burst ring | Non-damaging event cue such as Coupling release, gathering, Port opening, objective completion, or Anchor. |
| Brief coral burst ring | Hard event cue such as a break or collapse. |
| Bottom objective sentence | The one active progression condition. The line is a command, not a label for a nearby object. |
| Top campaign rail | Persistent campaign position | Chapter and objective counts, elapsed campaign time, chapter progress, and overall progress come from the current run. |
| Thin bar under the objective | Authoritative progress for the current objective. Empty can mean the condition has not begun or a continuous hold reset. |
| Why button and expanded paragraph | Full condition explanation. It does not pause the Field and does not consume input. |
| Pressure line with stage, meter, target, and Why | Textual reading of the dominant active pressure. Signal builds to pressure, crisis is the strongest stage, resolution is the exit. |
| Functional criterion rail | Independent evaluation contract: weakest Route, weakest Component margin, leakage ratio, observation window, grace, and hands-off countdown. This is not the campaign objective. |
| Intervention/Impulse count in Still Mode | Available paid causal actions. It is not Q, Route flow, score, health, or time. |
| Render quality control | Presentation density only. Low, Medium, and High must not change simulation results. |

### Sound dictionary

Sound is synthesized locally and carries state changes rather than ambience.
Silence between cues is normal.

| What you hear | Meaning |
|---|---|
| Quiet low two-note rise while E is held | Coupling reach is extending. It should remain soft and continuous rather than retriggering every frame. |
| Brief soft resolving pair during the hold | The first effective Coupling target has locked. Further frames on the same target do not repeat it. |
| Low rounded release body | E was released and a Coupling emission occurred. A shorter/duller version means it reached nothing effective. |
| Warm rising pair after release | Q was gathered into the Form. |
| Longer resolving pair with a high glint | A dormant Port opened. |
| Covered downward triangle | Interference was suppressed. |
| Low pressure onset, beating crisis, settling resolution | A staged pressure entered, reached crisis, or resolved. The crisis is the only deliberately beating cue. |
| Directional depth tone | The Form changed layers. Deeper falls; shallower rises. |
| Two-note Still Mode transition | The Field is entering or leaving its pause ramp. |
| Rising handoff cue | Control moved to another Form. |
| Long steady octave | An Anchor was written. |
| Falling hard cue followed later by its rising inverse | Recoverable collapse, then recovery. |

## Expected causal results

### Steering, transport, and flow

- Steering changes the Form's requested motion. Medium motion can also move the
  Form according to the current Regime and chassis coupling.
- A Supply Stream delivers finite stored usable resource, `Q`, measured in
  Charge Units. It does not push the Form.
- Medium motion can push the Form. It does not deliver `Q`.
- Directed Routes move `Q` algebraically from tail to head when both interfaces
  are open. Internal Route transfer conserves `Q` and respects finite capacity.
- A temporary occupancy overshoot is transport in flight while that Component
  still has an active outgoing Route. An overloaded Component with no active
  relief is a recoverable pattern setback.
- Supply adds `Q`. Upkeep, dissipation, leakage, overload, and assembly loss
  remove it. Typed replacement material is separate from `Q`.

### Coupling Pulse

- Holding `E` expands the true world-space release radius.
- Affected objects lock before release: warm inward motion means stored `Q`
  will move into the Form, a cyan iris means a Port will open, and outward
  ochre chevrons mean reachable Interference will be suppressed.
- Supply delivery is separate: cyan-to-amber filaments run from the Supply
  Stream to each geometric recipient while delivery is possible, and the arc
  around the controlled Form shows its stored-`Q` fraction.
- Releasing gathers one quarter of eligible nearby non-Form Component stock,
  opens reached interfaces, may discharge a Vault reserve, and may reduce
  reachable Supply diversion.
- The preview and release use the same simulation helpers. A release that
  reaches nothing should show an emission but transfer nothing and open no
  interface.
- Pulse costs zero intervention points, but it is external control and is
  recorded as a rescue. It is unavailable in hands-off validation.

### Observation and intervention

- The violet Observation View changes measurement only. It is immediate, free,
  and noncausal.
- The Physical Compartment is causal. Editing it queues a paid intervention,
  changes leakage membership, and enters history when committed.
- Switching between the two tools must not spend intervention points.
- Hover readings should name the actual Component, Route, Supply Stream,
  compartment, View, Form, material, signal, pressure, or intervention under
  the pointer. Decorative atmosphere is not inspectable simulation state.

### Functional criterion

The compact contract rail reports the weakest required Route, lowest required
Component margin, leakage-to-Supply ratio, observation-window progress, failure
grace, and hands-off countdown. A passing run must satisfy the frozen criterion
for its declared window. A temporary violation may consume grace before the
criterion fails.

## Campaign walkthrough

The times in this section are measured from the opening of the named chapter,
not from application launch. They describe authored simulation time. A
continuous pattern timer resets when its pattern fails; a cumulative current
timer pauses outside the band and resumes when the Form returns.

### Chapter 1 of 8: The Pull

**Recognition:** one near-white controlled Form, a bright horizontal Supply
Stream, a surface loop of circular Ports, and one deeper dim Supply Stream.
This is the onboarding chapter and the only chapter that teaches the visual
language one primitive at a time.

1. **Enter the Supply Stream.** Steer into the bright cyan band. The authored
   count is one second. A delivery filament must connect to the Form.
2. **Store 256 CU.** Remain in the band until the Form's amber reservoir arc
   and the objective progress bar reach the threshold. This is a quantity, not
   a timer. Leaving the band stops inflow.
3. **Approach the dormant Port.** Move toward the nearest hollow circle with
   three gate segments. Arrival completes this objective; do not press E yet.
4. **Hold E, then release.** The Coupling reach grows. Wait for a cyan iris and
   tether on the Port, then release E. The Port should become filled/open.
5. **Close one loop.** Find the other dormant Ports on the three-Route ring and
   open them the same way. The objective completes only when all three Routes
   show moving flow ticks on one step.
6. **Keep the loop carrying.** Keep all loop Routes flowing for about 150
   authored seconds. If a Component gets a red overload ring and its outgoing
   Route stops, open the nearby relief Port. A reset to zero is expected when
   the complete pattern fails.
7. **Descend to the deeper current.** Use `]` or a downward wheel gesture, then
   stand in the dim deep band for about 30 cumulative seconds. The deep layer
   supplies more Q but drains every Component on it.
8. **Open that Port, if you like.** This optional offer lasts about 60 seconds.
   Follow the deep band to its far Port, hold E long enough to lock it, and
   release. Ignoring it must not block the chapter.
9. **Hold the current as it moves.** Return to the surface and follow the bright
   band for about 6 minutes 4 seconds of cumulative in-band time. Drift moves
   the band in four visible stages; leaving the old position and reacquiring
   the displaced band is required.

**Timed events:** Interference enters at 1:00 and lasts through its staged
signal/pressure/crisis/resolution cycle. Drift begins around 14:04. Twenty
seconds into the final objective, the deep current turns off. Expected chapter
transition is roughly 25-30 minutes on the scripted paths.

### Chapter 2 of 8: The Edge

**Recognition:** a bright Supply Stream, a central Reserve/Module pair inside a
thick material edge, a separate thin violet View, and outer Ports beyond the
initial compartment.

1. Hold the bright current for about 2 minutes 17 seconds. Compare the Supply
   filaments with what the enclosed Components retain.
2. Set the existing three-Route line running. Both endpoints must be open and
   all three lines must show moving ticks.
3. Open the two outer Ports, north and east, with E. Their Routes become
   additional supply paths into the inside.
4. Keep the compartment route steady for about 4 minutes 33 seconds. The thick
   edge causes leakage at exposed contacts; the violet edge does not.
5. Enter Still Mode and reshape the **thick physical edge**. Select the
   compartment tool, drag a boundary handle across a Component, and press
   Enter. Moving only the violet View cannot complete this objective.
6. Descend and remain in the deep current for about 2 minutes 17 seconds.
7. Optionally open the deep store during its roughly 3 minute 2 second offer.
8. Hold the compartment through the final roughly 7 minute 17 second sequence.
   The bright Supply stops for 50, 80, and 110 seconds while surface drain
   rises. Stored Q, outer Routes, and the physical membership edit must carry
   the inside; changing the View is not a rescue.

**Timed events:** Drain begins almost immediately. A Fracture is scheduled at
about 12:52. During the final hold, Supply/drain changes occur about 0:40,
1:30, 2:10, 3:30, 4:10, and 6:00 after that objective begins.

### Chapter 3 of 8: The Loop

**Recognition:** one bright Supply band, a line of dormant Ports and drawn
Routes, several lower stores, and no authored connection from the Form into the
intake.

1. Follow the bright band for about 3 minutes 2 seconds. The Form is carrying Q
   out of the only source in this chapter.
2. Open the intake Port with E.
3. Open the other three Ports along the run. A sufficiently long Coupling hold
   may open several that are simultaneously locked.
4. Enter Still Mode and draw a Route from the Form Component to the intake.
   Press Enter to spend one intervention and make the preview line causal.
5. Close the circuit and hold it for about 7 minutes 17 seconds. The outfall
   eventually overloads without relief; open a store below it and connect the
   outfall to that store.
6. Optionally open the wide spare store during its roughly 3 minute 2 second
   offer. It does nothing until connected.
7. Keep the supplied circuit valid for about 9 minutes 6 seconds. Leaving the
   band long enough to stop any required Route resets this hold.
8. At the next objective's first step, one Port closes. Reopen that segmented
   gate with E and maintain the run for about 7 minutes 17 seconds.
9. Sustain the circuit under Flood for about 12 minutes 8 seconds. Flood lowers
   the effective threshold of the busiest Component; banked store Q pays for
   the resulting loss.

**Timed events:** Flood begins around 42:13 into the chapter. The final
objective also raises surface drain on its first step. Expected chapter span is
roughly 36-50 minutes.

### Chapter 4 of 8: The Echo

**Recognition:** a bright band feeding a store, a small ring of two dormant
Ports and three Routes, a relief Port beside the near ring Component, and a
separate deep band.

1. Stand in the bright current for about 6 minutes 4 seconds to fill the store.
2. Open the two dark ring Ports with E.
3. Confirm all three ring Routes carry on the same step.
4. Hold the ring for about 7 minutes 17 seconds. The near Component accumulates
   faster than it can pass Q; open the adjacent relief Port if its outgoing
   flow stops.
5. Descend and hold the deeper band for about 9 minutes 6 seconds.
6. Optionally open the deep spare line during its roughly 3 minute 2 second
   offer. Coupling cannot cross layers.
7. Return to the bright current and refill for about 9 minutes 6 seconds.
8. Hold the ring through repeated Echo outages for about 12 minutes 8 seconds.
   During each outage the ring runs on stored Q; moving does not reactivate an
   authored current that is off.

**Timed events:** Noise begins around 11:57. The first ring hold contains one
Supply outage. The final hold turns the bright Supply off/on at approximately
1:00/1:50, 2:50/3:40, and 4:40/5:30 after the objective begins. Expected span
is roughly 44-55 minutes.

### Chapter 5 of 8: The Mesh

**Recognition:** three Forms, only one near-white; northern and southern
patterns; a main intake and spare intake in the bright band; a deeper band and
optional Port.

1. Stand in the bright current for about 3 minutes 2 seconds.
2. In Still Mode, use Handoff to take control of the Form beside the northern
   store. It becomes near-white. Leave Still Mode, hold E, and open the store.
3. Hand off to the Form beside the southern store and open it the same way.
4. Keep the northern pattern valid for about 18 minutes 12 seconds. All four
   named Routes must carry together.
5. Open the spare intake in the bright band.
6. Enter Still Mode and draw one Route into the southern collector. Connecting
   from the main intake and connecting from the spare intake are both valid,
   but they respond differently to later Interference.
7. Descend and hold the deep current for about 1 minute 31 seconds.
8. Optionally open the deep Port during its roughly 3 minute 2 second offer.
9. Hold both patterns at once for about 18 minutes 12 seconds. Interference
   gives one target first claim on Supply; one intake appearing full does not
   prove both patterns are supplied.

**Timed events:** Interference begins around 40:33. Five minutes into the final
objective the spare intake closes and must be accounted for. Expected chapter
span is roughly 36-60 minutes.

### Chapter 6 of 8: The Break

**Recognition:** a bright intake band at the top, two alternative lines below,
a junction/core/outfall run, an off-axis optional store, and a dim current over
the core.

1. Follow the bright current east for about 9 minutes 6 seconds.
2. Open the intake with E.
3. Open both lower line Ports.
4. Use one long Coupling release between the junction, core, and outfall to open
   all three locked Components.
5. In Still Mode, draw one Route from the intake into either line and commit it.
6. Hold the run for about 12 minutes 8 seconds. A dim current activates over the
   core. About 2 minutes 30 seconds into the hold, the junction-to-core Route is
   severed; redirect one end of a line onto the core in Still Mode.
7. Optionally open the distant store during its roughly 3 minute 2 second offer.
8. Keep the repaired run carrying for another about 12 minutes 8 seconds.
9. Hold past Fracture for about 12 minutes 8 seconds. Fracture permanently
   breaks the busiest standing Route. Valid responses include redirecting a
   line, connecting the optional store into the core, or restructuring the
   run around the dim Supply.

**Timed events:** Fracture begins around 45:20. A severed line does not restore
itself at pressure resolution. Expected chapter span is roughly 44-60 minutes.

### Chapter 7 of 8: The Rewrite

**Recognition:** the Form feeds an intake from a bright band; two carriers form
a module into a junction; two dependent Components and a store stand east;
deep spares sit below the run.

1. Hold the bright band for about 9 minutes 6 seconds.
2. Open both dependent Ports with E.
3. Confirm both dependency Routes carry together.
4. Hold the dependency for about 12 minutes 8 seconds. Both dependents fill
   quickly and need outgoing Routes to the store. Draw and commit those Routes
   in Still Mode.
5. Hold the deep band for about 12 minutes 8 seconds. Cross-layer Route reach
   includes a 512-unit penalty per layer.
6. Optionally leave the band to open the deep spare during its roughly 4 minute
   33 second offer.
7. Carry the dependency past Fracture for about 12 minutes 8 seconds. Fracture
   removes the Form-to-intake Route; return to the surface and redraw it before
   banked Q runs out.
8. Rewrite the module while the dependency continues for about 18 minutes 12
   seconds. Surface drain rises immediately and the carrier-to-carrier Route
   breaks at 2:00. The final replacement may require drawing, redirecting,
   cutting, and reshaping within the remaining intervention budget.

**Timed events:** Fracture begins around 45:43. The chapter is intentionally a
compound failure and should not be read as one-button repair. Expected span is
roughly 56-70 minutes.

### Chapter 8 of 8: The Quiet Edge

**Recognition:** an intake, five arms, one hold returning to the intake, an
optional sixth arm, and a deep relief Port. This chapter begins with different
intervention budgets depending on prior campaign play.

1. **Close the spare arm too** is an optional 80-second offer. A six-intervention
   arrival can draw all six required Routes and earn the spare ending mark.
2. **Close the ring at once** is an optional 60-second offer. A five-intervention
   arrival can draw all five arm-to-hold Routes immediately and earn the whole
   ending mark.
3. **Close the ring** requires about 9 minutes 6 seconds of valid pattern. A
   four-intervention arrival uses the deep relief Port for the last arm instead
   of buying a fifth Route.
4. Hold the deep band for about 12 minutes 8 seconds.
5. The deep Supply turns off as the next roughly 12 minute 8 second hold begins.
   The already closed ring must carry what remains.
6. The bright Supply then turns off and surface loss rises. Carry the fully
   closed Field for another about 12 minutes 8 seconds.
7. **Let it run** is the final 32,768-step, roughly 18 minute 12 second hands-off
   window. Release all controls. Noise narrows Route flow during the interval,
   but the ring must retain Q and remain valid through the end.

**Timed events:** Noise begins around 58:53. The expected ending depends on the
optional opening marks: spare, whole, or steady. The campaign ends after the
hands-off window; it must not transition to a ninth chapter.

## Form expectations

| Form | Expected distinction |
|---|---|
| Thread | Fastest steering response; ordinary paid construction within a 448-unit span. |
| Ring | Compact 384-unit construction span and lower loss for its own Form Component only. |
| Relay | Long 1088-unit reach and 64 CU/step newly commissioned Routes, with low reserve capacity. |
| Vault | Slow chassis with an isolated 768 CU reserve and conserving local discharge. |
| Lens | Paid local sensing and a 30-step forecast built only from locally available state. |
| Knot | Finite typed junction blanks and persistent junction upkeep. |
| Wake | Conserving delayed caches of up to 16 CU that release locally after 60 steps. |
| Chorus | Three linked Forms with a 256-unit separation limit and explicit control handoff. |

A Form ability must never create resource, matter, topology, or knowledge from
outside its disclosed contract.

## Laboratory expectations

Open laboratory benches from Still Mode.

| Bench | Expected result |
|---|---|
| Observe | Passive readings from authoritative retained history; no causal mutation. |
| Intervention | A typed plan applied to the live run or a clone with explicit targets, magnitude, duration, and cost. |
| Divergence | Baseline and altered replay from one Anchor using the same recorded control and keyed common randomness. |
| Ensemble | Seeded modeled realizations with individual traces, median, observed range, failure counts, and pass fraction. The band is not a confidence interval. |
| Holdout | A sealed independently identified suite run hands-off; steering, Pulse, rescue, and physical editing are disabled. |
| Archive | Durable local records with scenario, generator, embodied-state, control, evidence, branch, comparison, export, and removal operations. |
| Renewal | Local replacement using finite nearby donor Charge, local junction stock, conductor stock, deficit signals, and assembly loss. |
| Inheritance | Repeated generator-copy assays with retained scenario, generator, control, and embodied provenance. |
| Open Field | A constrained scenario compiler that freezes a complete ScenarioSpec and runs eight descriptive trials against its explicit function criterion. |

## Intended behavior that may look like a bug

- Pointer motion does not steer.
- The opening Field is dark and spatially sparse.
- Entering or leaving Still Mode uses a short visual ramp before the state
  settles.
- The main objective disappears while Still Mode is open.
- Clicking Why, tools, laboratory controls, or other chrome does not Pulse.
- Observation View edits cost nothing and do not alter the replay baseline.
- Physical Compartment edits cost intervention points and do alter causal state.
- Open Field's reference transport baseline is fixed; other Regimes may include
  authored noise, periodic Supply, medium motion, collision, or leakage.
- Hands-off trials intentionally remove steering, Pulse, handoff, and rescue.
- Repeating a run with the same frozen scenario, control, and seed should be
  deterministic. Ensemble members differ because their addressed seeds differ.
- The Quiet Edge finale removes direct control for 32,768 simulation steps.
  The closed ring should spend reserves and finish with retained Charge; the
  three authored continuity paths currently close with 410, 683, and 377 whole
  Charge Units respectively.

## High-risk areas for bug reports

Report these carefully with the exact Regime, Form, seed, and step:

1. **Campaign progression:** The Loop and The Quiet Edge automated gates now
   complete. Report any objective that remains at zero while its named Routes
   are flowing, or any finale ring that reaches zero before the ending.
2. **Long-run timing:** objective windows, grace, periodic Supply, cache release,
   pressure expiry, and hands-off countdowns are sensitive to exact step order.
3. **Renderer parity:** WebGL and Canvas2D should show the same causal objects
   and intervention marks. Bloom and particle density may differ by quality.
4. **Archive durability:** verify records persist across reload, reopen the correct
   state, preserve all three identities, and compare without mutating either
   record.
5. **Mobile layout:** check Atlas, Form selection, Still Mode, every laboratory
   bench, long result tables, and the on-screen keyboard at narrow widths.
6. **Intervention expiry:** Clamp, Scramble, Decoy, Delay, Breach, and Transplant
   must affect only declared targets for their declared duration, then restore
   the correct authored state.
7. **Renewal locality:** no distant or shell-selected repair target, material,
   desired position, or topology may enter autonomous repair.
8. **Depth and collision:** changing layers, medium drag, and same-layer contact
   must not teleport the Form, duplicate resource, or move unrelated Components.

## Minimum human playtest

This protocol does not require finishing the seven-hour campaign in every
session. Record the furthest checkpoint reached so another tester can continue
from an Archive or Anchor.

1. **Selection, 5 minutes:** inspect every implemented Atlas Regime and all
   eight Form contracts. Start Thread in Open Field.
2. **Opening language, first 10 minutes:** confirm pointer motion never steers;
   complete the first five Pull objectives; compare each objective sentence,
   Why text, visual lock, and actual outcome against this guide.
3. **Coupling contrast:** release once with No connection, once on a gather
   target, and once on a dormant Port. Record whether radius, tether, effect
   animation, strip summary, sound, and result all agree.
4. **Depth and pressure:** change depth with both the bracket binding and wheel;
   observe Interference at approximately 1:00 of The Pull and all four pressure
   stages. Confirm the ochre local target and the viewport rim name the same
   pressure.
5. **Still Mode:** switch between View and Physical Compartment without cost;
   queue one Route, redirect, or boundary edit; undo it; queue it again; commit
   it; confirm preview and committed geometry use different registers.
6. **First transition, 25-30 minutes:** complete The Pull. Record the exact
   chapter step and real elapsed time. Confirm the chapter review and Anchor,
   then identify The Edge from its first objective and Field layout.
7. **One mid-campaign chapter:** choose The Loop, Mesh, or Break checkpoint from
   a retained run. Observe one authored closure or cut, one pressure crisis,
   one recoverable setback or stalled pattern, and the repair that advances it.
8. **Laboratory and Archive:** open each available bench; run one operation;
   create, compare, export, reopen, and delete an Archive record without
   mutating the live run unexpectedly.
9. **Finale checkpoint:** from a retained Quiet Edge run, verify the appropriate
   optional opening path, both Supply stops, and at least the opening minute of
   the hands-off countdown. A full finale certification runs the entire 32,768
   neutral-input steps.
10. **Layout pass:** repeat selection, active Field, Coupling, Why, Still Mode,
    and one laboratory result at a narrow viewport. Text may wrap but must not
    overlap the Coupling strip, objective progress, pressure line, or criterion
    rail.

## Bug report template

```text
Build / commit:
Browser and viewport:
Renderer and quality tier:
Regime / Form:
Campaign chapter and position (for example, 3 of 8 - The Loop):
Objective sentence:
Chapter elapsed time and simulation step:
Run id / scenario hash / seed:
Control contract:
Actions taken:
Expected result (cite guide section):
Actual result:
Does replay reproduce it?:
Screenshot, export, Archive record, or console output:
```

Treat visual atmosphere as presentation and object readings as evidence. A bug
report should distinguish a wrong reading, a wrong causal outcome, and a purely
visual mismatch whenever possible.
