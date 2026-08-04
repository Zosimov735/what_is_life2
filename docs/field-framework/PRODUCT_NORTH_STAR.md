# What Is Life 2 — Product North Star and Scope Correction

Status: canonical product guidance for future implementation agents  
Established: 2026-08-04

## Purpose

This document exists to prevent architectural drift.

What Is Life 2 is an ambitious engineering simulation, but its success depends on a simple player fantasy:

> The player is an omniscient engineer who creates autonomous life-like systems, releases them, observes their behavior, understands failure, and improves their designs.

The project must not become:

- a scientific simulation without a game;
- a database interface disguised as gameplay;
- a debugging tool where the player reads logs instead of understanding systems;
- a complexity showcase where technical depth replaces meaningful interaction.

The architecture may be rigorous. The player experience must remain intuitive, powerful, and satisfying.

---

# Core player fantasy

The player is not:

- a pilot;
- a character inside the system;
- a manager issuing constant commands;
- a programmer writing software.

The player is:

- an architect;
- an engineer;
- a scientist;
- a designer of artificial organisms.

The emotional experience should be:

> "I created something. It failed. I understood why. I changed one thing. It became better."

The machine itself is the object of attachment.

The player should care about:

- their designs;
- their experiments;
- their discoveries;
- their successful organisms;
- their failed prototypes.

---

# The core gameplay loop

All future implementation should support this loop:

```
Observe system
      ↓
Understand constraints
      ↓
Design organization and local rules
      ↓
Release autonomous system
      ↓
Observe emergent behavior
      ↓
Diagnose failure
      ↓
Make one deliberate revision
      ↓
Run again
      ↓
Create a better organism
```

Every feature should strengthen this loop.

If a feature does not improve:

- creation,
- observation,
- diagnosis,
- revision,
- comparison,

it should be questioned.

---

# The minimum viable game

The project should not attempt to prove the entire vision immediately.

The first successful version only needs:

## One environment

A single artificial biological system.

## A few components

Example:

- Collector
- Relay
- Reservoir
- Receiver

## Simple local policies

Example:

```
IF energy low
THEN seek Supply

IF receiver low
THEN open Route

IF storage high
THEN conserve
```

## One meaningful failure

The player should experience:

"The system failed because of my design."

Not:

"The simulation ended."

## One successful revision

The player changes something and sees:

"My intervention improved the organism."

This proves the game.

---

# The first player experience

A successful first 30 minutes should look like:

## Phase 1 — Creation

The player receives a simple functional goal.

They inspect:

- components;
- resources;
- connections;
- available rules.

They create their first machine.

---

## Phase 2 — Failure

The machine fails.

The failure must answer:

"What happened?"

Example:

```
Receiver lost service.

Cause:
Relay did not open Route.

Reason:
Charge threshold rule was never reached.
```

The player should feel curiosity, not frustration.

---

## Phase 3 — Revision

The player changes one thing:

- a policy rule;
- a route;
- a component;
- an initial condition.

They run again.

---

## Phase 4 — Emergence

The machine works without intervention.

The player experiences:

"I built a system that knows what to do."

This is the core reward.

---

# What must remain

## Local automation

Keep.

The player creates simple local rules.

Complex behavior emerges from interactions.

The player should never need to manually operate every action.

---

## Engineering memory

Keep.

This is a defining feature.

Failed designs should become useful knowledge.

A player's history should look like:

```
Prototype A

Failure:
Receiver starvation

Revision:
Added storage buffer

Result:
Stable service

Prototype B
```

The game should preserve the player's engineering journey.

---

## Qualification

Keep.

But maintain the correct framing.

The player should feel:

"My creation is entering a trial."

Not:

"I am running a benchmark."

The backend may be rigorous.

The experience must remain meaningful.

---

# What should be delayed

The following systems are valuable but should not block proving the core loop:

## Advanced contracts

Delay:

- Balance;
- Interference;
- Closure;
- Renewal;
- Transplant;
- Holdout.

They expand the game after the foundation works.

---

## Deep archival systems

Do not build a perfect historical archive before the player cares about their creations.

Initial needs:

- save;
- duplicate;
- compare;
- revisit.

Advanced provenance can follow.

---

## Excessive scientific infrastructure

Avoid exposing unnecessary complexity.

Players should not think about:

- schema versions;
- hashes;
- migration states;
- protocol identifiers.

Those protect the game.

They are not the game.

---

# Engineering principle

The project should have two layers.

## Internal layer

Can be:

- deterministic;
- reproducible;
- rigorously validated;
- scientifically grounded.

## Player layer

Must be:

- understandable;
- beautiful;
- empowering;
- emotionally engaging.

The player should feel like a genius engineer.

They should not feel like a database administrator.

---

# The danger to avoid

The most likely failure mode is:

> Building a perfect simulation of artificial life that nobody enjoys operating.

Complexity is not automatically depth.

Depth comes from:

- meaningful choices;
- understandable consequences;
- satisfying improvement.

A simple system that creates emergence is more valuable than a complex system that requires documentation to understand.

---

# Development priority hierarchy

Future agents should prioritize in this order:

## 1. Fun loop

Can a player create, fail, understand, revise, and succeed?

## 2. Causal clarity

Can the player understand why something happened?

## 3. Emergence

Do simple rules create interesting behavior?

## 4. Memory

Do designs and experiments matter over time?

## 5. Scientific depth

Do advanced systems expand the experience?

Never reverse this order.

---

# Final product test

Before expanding scope, ask:

Can a new player say:

> "I made this organism. I understand why it failed. I changed it. Now it works."

If yes:

Continue expanding.

If no:

Do not add more systems.

Improve the core loop.

---

# Relationship to the existing roadmap

The current Automation and Contract Authority document remains the implementation authority.

This document is the product authority.

The roadmap answers:

"How do we build the system?"

This document answers:

"Why are we building it?"

The ultimate goal is not a complete simulation.

The goal is a compelling experience of designing autonomous life-like systems.