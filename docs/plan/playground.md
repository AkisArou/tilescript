# Playground Plan

## Goal

Port the useful parts of `spiders-wm-www` into `tilescript` as `tilescript-playground` without dragging over obsolete architecture or reintroducing dead abstractions.

The target is not a full one-shot port. The target is a staged migration that gives `tilescript`:

1. a reusable host/action boundary shared by multiple frontends
2. a source-bundle playground runtime for browser-side preview and editing
3. a browser app shell that can evolve into the old preview/editor/system/cli workflow

## What existed in `spiders-wm`

The old system had two important seams.

### 1. Host boundary in `spiders-wm-runtime`

Relevant file:

- `crates/wm-runtime/src/host.rs`

It defined:

- `WmHost`
- `dispatch_wm_command(...)`

This boundary converted `WmCommand` into host-side effects like:

- activate workspace
- focus window
- toggle floating/fullscreen
- reload config
- set/cycle layout
- close focused window

That made it possible for the preview app to reuse command semantics without directly depending on the compositor plugin implementation.

### 2. Browser playground runtime in `spiders-wm-www`

Relevant files:

- `apps/spiders-wm-www/src/session.rs`
- `apps/spiders-wm-www/src/layout_runtime.rs`
- `apps/spiders-wm-www/src/workspace.rs`
- `apps/spiders-wm-www/js/src/monaco-host.ts`
- `apps/spiders-wm-www/js/src/xterm-host.ts`

Important characteristics:

- browser-side source bundle evaluation
- editor buffers projected into an in-memory config/layout workspace
- preview session state for windows/workspaces/focus
- Monaco and xterm hosts as browser integrations, not core runtime concerns

## What `tilescript` has now

### Present today

- `tilescript_core::command::WmCommand`
- `tilescript_core::host::{HostAction, dispatch_wm_command(...)}`
- `tilescript_core::query::state_snapshot_for_model(...)`
- `tilescript_config::runtime::SourceBundleRuntimeBundle`
- `tilescript_config::build_source_bundle_authoring_layout_service(...)`
- `tilescript_layout_runtime` for authored/prepared layout evaluation
- `tilescript_runtime_js` for JS authored/prepared runtime handling, currently collapsed into one crate
- Hyprland plugin integration through `tilescript-ffi`

### Missing today

- no browser app crate/package
- no browser-side source-bundle runtime provider crate in this repo
- no editor/CLI/browser host integrations
- no split between shared JS graph/compiler code and native/browser runtime backends

## Key conclusion

We should keep the idea behind `WmHost`, but not restore the old trait or invent a new heavy shared playground crate up front.

Why:

- `tilescript` already moved the reusable host-action boundary into `tilescript-core`
- the old `WmHostEffect` shape was tied to the old runtime layering
- the real missing seam is not host dispatch anymore, it is the runtime split that old `spiders-wm` had and `tilescript` currently does not
- the current repo already wants frontend-neutral logic in core/runtime crates, with frontend-specific execution in:
  - Hyprland plugin
  - future browser playground

So the correct abstraction to restore is:

- shared command dispatch to host actions
- shared JS runtime/compiler core reused by native and browser backends

Not necessarily:

- the exact old `WmHost` trait
- a new `crates/playground-core` crate before we know what truly needs sharing

## Proposed architecture

### Stage 1. Shared host actions in `tilescript-core`

This is already done.

The shared boundary now lives in:

- `tilescript_core::host`

and `crates/ffi/src/action.rs` is now just a thin adapter layer.

This gives us the old benefit of `WmHost` without coupling it to a specific runtime or app.

### Stage 2. Split `tilescript-runtime-js` into core/native/browser crates

This should follow the old `spiders-wm` structure because the split maps cleanly to the code we already have.

Target crate shape:

- `crates/runtimes/js/core`
- `crates/runtimes/js/native`
- `crates/runtimes/js/browser`

The current single `crates/runtimes/js` crate already contains all three categories mixed together:

- shared compiler / graph / payload / loader logic
- native QuickJS execution and authored-config rebuild logic
- future browser-facing source-bundle execution concerns

Proposed ownership after the split:

- `crates/runtimes/js/core`
  - `compile.rs`
  - `graph.rs`
  - `loader.rs`
  - `module_graph.rs`
  - `layout_value.rs`
  - `payload.rs`
  - virtual SDK module plumbing
- `crates/runtimes/js/native`
  - `runtime.rs`
  - `module_graph_runtime.rs`
  - `authored.rs`
  - native `build_default_runtime(...)`
  - native `RuntimeBundle` provider helpers
- `crates/runtimes/js/browser`
  - browser `SourceBundleRuntimeBundle` provider
  - source-bundle config decode / layout evaluation in WASM context
  - no Monaco/xterm/editor concerns

Why do this before porting the app:

- it avoids duplicating runtime logic in the playground app
- it keeps plugin and browser frontends thin
- it matches the user's direction that host/runtime logic belongs in shared crates, not in app glue

### Stage 3. Reuse existing source-bundle runtime boundaries, not a new layout abstraction

Use what already exists in `tilescript_config::runtime`:

- `SourceBundle`
- `SourceBundleRuntimeBundle`
- `build_source_bundle_authoring_layout_service(...)`

Do not invent a second layout runtime abstraction.

The old `spiders-wm-www/src/session.rs` ideas should be ported selectively, but the default destination should be the app unless a smaller shared seam clearly appears.

That means:

- in-memory editor buffers can start in the app
- preview session state can start in the app
- preview-state mutation from `HostAction` can start in the app

Only extract a shared crate later if both browser and another frontend need the same state machine.

### Stage 4. Add browser runtime/provider support

`spiders-wm-www` used browser-specific JS runtime providers that do not currently exist in `tilescript`.

That support should live in:

- `crates/runtimes/js/browser`

Responsibilities:

- evaluate source bundles in WASM/browser context
- expose a `SourceBundleRuntimeBundle` implementation for web
- stay runtime-focused only

It should not know about Monaco, xterm, file trees, session state, or UI widgets.

### Stage 5. Add `tilescript-playground` app shell

Only after Stages 1 to 3.

Suggested location:

- `apps/tilescript-playground`

It should start small:

- source bundle fixture
- layout preview canvas/view
- command application
- diagnostics display

Initial scope should not include the full old app immediately.

### Stage 6. Port editor and terminal hosts as separate browser integrations

Port only after the preview/runtime path is working.

Pieces worth porting:

- Monaco host
- CSS LSP worker integration
- xterm host
- file tree/editor buffers

These should remain app-layer integrations, not core/runtime abstractions.

## What not to port directly

### Do not restore `WmHostEffect`

The current repo already has a simpler and more direct action model.

Port the concept, not the exact old type.

### Do not move browser host code into core/runtime crates

Keep these separate:

- core command/action model
- runtime/source-bundle execution
- browser UI hosts

### Do not add `crates/playground-core` preemptively

The current repo already has the important shared seam in `tilescript-core`, and the next missing seam is the JS runtime split.

If preview session state only serves `apps/tilescript-playground`, keep it there until reuse pressure is real.

### Do not start with full Monaco/xterm port

First prove:

- source bundle evaluation
- preview state
- host-action application

Then port UI pieces.

## Implementation order

### Phase A. Shared host actions

1. add `tilescript_core::host`
2. move `HostAction` and `dispatch_wm_command(...)` there
3. update `tilescript-ffi` to consume the shared module

Status: complete.

### Phase B. JS runtime split

1. create `crates/runtimes/js/core`
2. move shared compile/graph/payload/loader code there
3. create `crates/runtimes/js/native`
4. move QuickJS runtime and authored/prepared config logic there
5. leave a compatibility-free workspace layout; update downstream deps immediately

### Phase C. Browser runtime support

1. add `crates/runtimes/js/browser`
2. make source-bundle evaluation work in browser/WASM context
3. plug it into `tilescript_config::runtime::SourceBundleRuntimeBundle`

### Phase D. App shell

1. add `apps/tilescript-playground`
2. start with source bundle fixture + preview-only workflow
3. keep session state local to the app initially
4. add diagnostics and command entry

### Phase E. Editor/CLI integrations

1. Monaco host
2. CSS LSP worker
3. xterm host
4. file tree/download/export helpers

## First implementation slice

The first slice to implement now should be Phase B.

Why:

- Phase A is already done
- the runtime split is the next structural change that prevents app-layer duplication
- it creates the right home for later browser runtime code
- it still avoids choosing the browser UI stack too early

Concrete first change:

- add `crates/runtimes/js/core` and move the shared non-QuickJS modules there first
- make `tilescript-runtime-js` become the native backend crate or rename/split it cleanly in one pass
- keep imports direct and simple instead of introducing temporary wrapper layers

## Success criteria

We should consider the groundwork successful when:

1. Hyprland plugin and future playground can consume the same host action model
2. browser playground preview can evaluate a source bundle using shared JS runtime core plus a browser runtime backend
3. editor/terminal/browser-specific integrations remain outside core runtime crates
4. no legacy `spiders-wm` runtime layering is copied over mechanically
5. no duplicate host/runtime logic is reintroduced in `tilescript-playground`
