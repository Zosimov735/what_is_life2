# What Is Life 2 — Platform and Delivery

Status: binding platform contract
Date established: 2026-08-02

## Decision

What Is Life 2 is developed remotely and delivered from one static
application architecture:

```text
React + Pixi/Canvas interface
            |
            v
dedicated module Web Worker
            |
            v
deterministic Rust/WASM core
```

Development uses Vite's local server. A production web build is a static
`app/dist` directory and requires no application server. The first macOS
package will use Tauri 2 as a thin static host around that same directory. It
will not run a localhost server and will not initially move simulation work
through Tauri IPC.

Tauri's official frontend contract explicitly supports static HTML,
JavaScript, and WASM in its webview and recommends Vite for SPA applications:
[Tauri frontend configuration](https://v2.tauri.app/start/frontend/) and
[Tauri with Vite](https://v2.tauri.app/start/frontend/vite/).

## Delivery modes

| Mode | Host | Core execution | Network requirement |
|---|---|---|---|
| Remote development | Vite development server | WASM in module Worker | Development assets only |
| Static web release | Any static-file host | WASM in module Worker | None after assets load |
| Installed macOS game | Tauri asset protocol in WKWebView | WASM in module Worker | None for play |

## Private Azure test host

The initial shared test environment is a small, reproducible Ubuntu VM in
Azure. It serves a versioned copy of `app/dist` through nginx bound only to
`127.0.0.1:8080`. Testers connect through an SSH tunnel from an explicitly
allowlisted management address; the Azure network security group exposes no
public HTTP or HTTPS rule. Infrastructure source and operating instructions
live under `infra/azure/`.

The host stores no canonical source, authored content, database, or required
build toolchain. Codex builds and validates locally, uploads a new release
directory, verifies it, and atomically changes a `current` symlink. Prior
release directories provide the application rollback path. A later public
delivery decision may replace the VM with a managed static host without
changing the Worker/WASM application architecture.

The production artifact is not designed to be opened directly with `file://`.
Module Workers, root-relative assets, and WASM loading use the static host or
the desktop shell's asset protocol.

## Why the core remains WASM first

The current Worker already owns scheduling and exchanges compact transferable
frames with the renderer. Keeping that boundary gives browser and macOS builds
one simulation implementation, one save format, and one deterministic replay
path. Moving the core into native Tauri commands now would add an IPC protocol
and a second runtime path before profiling has identified a problem.

A native Rust host may be considered only after a representative laptop trace
shows the WASM core is the dominant frame-time or energy cost, and a native
prototype demonstrates a material improvement without changing deterministic
results, scheduler semantics, or the hot-frame budget.

## Laptop efficiency contract

- The installed game serves static local assets; it does not ship or start a
  production web server.
- Environmental density is cached or precomposed. Causal objects remain live.
- Effective device-pixel ratio is capped by quality tier.
- Active Field targets 60 rendered frames/s over the 30-step/s simulation;
  Atlas targets 30 frames/s and stops animating while idle or hidden.
- Internal render area remains near or below 2.6 million pixels.
- No full-screen fluid solver, unbounded particle system, or more than four
  full-screen compositing passes.
- Ensemble and Holdout jobs remain off the live rendering path.

These are budgets, not remotely certified Mac results. Frame time, memory,
thermals, and energy must be measured on a macOS runner and the eventual target
laptop.

## Toolchain and clean checkout

The repository pins its default Node, npm, Rust, WASM target, and `wasm-pack`
versions. Generated content, WASM bindings, Rust targets, and frontend bundles
remain ignored; they are reproducible outputs rather than source.

After Node and `rustup` are installed, the intended path is:

```bash
nvm use
npm ci
npm run doctor
npm run dev
```

`npm run dev` prepares authored content and rebuilds development WASM only when
the generated package is absent or older than its Rust inputs.

## macOS release boundary

The Tauri scaffold is added only after the static artifact and representative
Worker/WASM run are verified. The first wrapper milestone must confirm module
Worker loading, WASM initialization, WebGL with Canvas2D fallback, audio startup,
save persistence, and input behavior in WKWebView.

Local unsigned or ad-hoc packages are development artifacts. Distribution to
another Mac requires a Developer ID signature, hardened runtime, secure
timestamp, notarization, and stapling. Those steps require Apple credentials
and a macOS build environment; they never belong in an untrusted Linux secret store.
See [Tauri macOS code signing](https://v2.tauri.app/distribute/sign/macos/) and
[Apple's notarization requirements](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution).

No signing credential, certificate, application-specific password, or
notarization key is committed to this repository.
