# Performance Plan

This document defines the final performance architecture for `hypreact` across native and browser runtimes.

The goal is clean code, not compatibility.

- no compatibility layers to preserve today’s pull-driven refresh path
- no duplicate reload systems with different semantics
- no broad ad hoc caches with unclear invalidation
- no fake symmetry between native and browser execution models
- no “temporary” middle-ground architecture that we intend to replace later

This is the target architecture we should implement directly.

- choose the final model first
- refactor toward it directly
- break internal APIs if they block the cleaner architecture

## Goals

- make the native runtime cheap enough that normal placement queries are mostly cache hits
- make file edits apply to the visible workspace immediately, without needing a user action such as workspace switching
- keep JS, Lua, and Fennel runtime performance models explicit
- use native-only bytecode caches where they are a real execution win
- keep browser UX equally responsive without pretending the browser should share native bytecode internals
- make invalidation rules explicit, narrow, and trustworthy

## Non-Goals

- preserving the current refresh-on-query model as the long-term design
- adding compatibility shims around the existing runtime flow
- rebuilding everything on every edit because it is simpler in the short term
- inventing browser bytecode layers to imitate native caches
- storing native bytecode as a stable, portable, or externally trusted artifact format

## Final Architecture

The final architecture is:

- one long-lived project runtime state per loaded config root
- one explicit dependency graph for authored inputs and derived artifacts
- one invalidation engine that marks only affected artifacts dirty
- one shared evaluation path used by both placement queries and live reload
- one watcher-driven push path for native visible reload
- one source-oriented reactive preview path for the browser
- one native executable cache per runtime, including bytecode where appropriate

The runtime must stop behaving like a near-cold start on every placement query.

## Core Principles

### 1. Source Is Canonical

Authored files are the source of truth.

- `.ts`, `.tsx`, `.js`, `.jsx` are canonical for JS projects
- `.lua` is canonical for Lua-authored projects
- `.fnl` is canonical for Fennel-authored projects

Prepared outputs, compiled outputs, and bytecode are caches.

They are never the canonical source of truth.

### 2. Derived Artifacts Must Be Explicit

Every expensive runtime product must have an explicit artifact identity.

Required artifact categories:

- prepared config artifact
- prepared layout artifact
- stylesheet artifact
- executable runtime artifact

Examples:

- JS layout:
  - prepared JS module graph
  - executable QuickJS module artifact
  - native QuickJS bytecode artifact
- Lua layout:
  - prepared Lua source artifact
  - executable Lua chunk artifact
  - native Lua bytecode artifact when supported by the embedded engine path
- Fennel layout:
  - compiled Lua source artifact
  - executable Lua chunk artifact
  - native Lua bytecode artifact when supported by the embedded engine path

### 3. Invalidation Must Be Dependency-Driven

The runtime must know exactly which authored inputs produced each artifact.

That means:

- no broad cache clears as the normal path
- no implicit “reload everything because something changed” logic
- no hiding dependency relationships inside runtime payload blobs

### 4. Live Reload Must Reuse The Same Evaluation Path

There will be one evaluation path.

It is triggered by:

- a placement query
- a watcher-driven reload event
- a browser editor change

The trigger changes. The evaluation semantics do not.

## Current Problems

Today the runtime still pays too much repeated work in the hot path.

Repeated costs include:

- config refresh work
- layout source reads
- Fennel-to-Lua compilation
- JS runtime setup and module evaluation preparation
- stylesheet reads and CSS parse/analyze work
- full scene rebuild work even when authored inputs are unchanged

The biggest UX problem is separate from raw CPU cost:

- edits are not pushed to screen immediately
- the current visible update usually waits for a later Hyprland-triggered placement query

## Role Of `.hypreact-build`

`.hypreact-build/` remains part of the final architecture, but with a clearer purpose.

It is the native prepared artifact root.

It should contain only runtime-owned prepared artifacts, not vague leftovers from intermediate reload steps.

### Final Meaning Of `.hypreact-build/`

For native runtimes, `.hypreact-build/` is the local derived-artifact store for the current config root.

It holds:

- prepared config artifacts
- prepared JS runtime modules
- copied or generated CSS artifacts when needed
- runtime metadata needed to validate freshness
- native executable caches, including bytecode caches where enabled

It does not replace the in-memory runtime state.

It complements it.

The in-memory runtime state is the active hot path.

`.hypreact-build/` is the persistent local cache backing that state.

## Runtime State Model

The runtime service will own one long-lived project runtime state.

Responsibilities:

- track the loaded config root
- track authored file dependencies
- store artifact records and their validity state
- answer invalidation queries from file changes
- produce updated prepared and executable artifacts
- serve cheap cache hits during placement queries

This state replaces the current coarse flow of:

- refresh config on demand
- clear caches broadly
- reconstruct runtime products again during placement

## Dependency Graph Model

The runtime state must contain an explicit dependency graph.

Graph nodes:

- authored files
- derived artifacts

Required authored file categories:

- config entry file
- config-time imported JS/TS modules
- layout entry files
- layout-time imported JS/TS modules
- layout stylesheets
- global stylesheet

Required derived artifact categories:

- config artifact
- layout artifact by layout name
- stylesheet artifact by path
- executable artifact by runtime entry
- bytecode artifact by runtime entry in native mode

Every derived artifact must store:

- its authored input set
- the invalidation token of each input
- the runtime version token relevant to its format

## Invalidation Tokens

The final model uses content hashes as the canonical invalidation token for runtime artifacts.

Not mtimes.

Mtimes can still be used as a fast prefilter for file watching, but artifact validity is decided by content identity.

This is the correct final model because:

- editor save behavior varies
- mtime-only logic is noisy and brittle
- content identity gives deterministic invalidation

Required token inputs:

- file content hash
- runtime engine version token where relevant
- compiler version token where relevant
- artifact schema/version token

Examples:

- QuickJS bytecode validity depends on:
  - prepared JS content hash
  - QuickJS version token
  - hypreact bytecode schema token
- Fennel compiled-Lua validity depends on:
  - `.fnl` content hash
  - vendored Fennel compiler version token
  - hypreact compile schema token
- Lua bytecode validity depends on:
  - compiled Lua source hash
  - Lua engine version token
  - hypreact bytecode schema token

## Invalidation Rules

Invalidation is narrow and mandatory.

Examples:

- config file changes:
  - invalidate config artifact
  - invalidate any artifact depending on config-selected layout metadata
- JS module imported by config changes:
  - invalidate config artifact only
- layout entry file changes:
  - invalidate that layout artifact
  - invalidate that layout executable artifact
  - invalidate that layout bytecode artifact
- layout stylesheet changes:
  - invalidate stylesheet artifact for that path
  - invalidate the layouts that depend on that stylesheet artifact
- global stylesheet changes:
  - invalidate the global stylesheet artifact
  - invalidate all layouts that include it in scene construction
- unrelated layout changes:
  - do not recompute the active workspace unless it depends on that layout

Broad cache clears are not part of the final design.

## Native Live Reload

Native live reload is part of the final architecture, not an add-on.

The native runtime must watch the config root and push visible updates.

### Final Native Flow

1. Load config root and construct project runtime state.
2. Start a watcher scoped to the config root.
3. Watch:
   - config entry file
   - all discovered config-time dependencies
   - all discovered layout-time dependencies
   - all discovered stylesheet dependencies
4. Debounce events.
5. Re-hash changed files.
6. Invalidate affected artifacts through the dependency graph.
7. Rebuild only the dirty artifacts needed by visible workspaces.
8. Recompute placement for affected active workspaces.
9. Apply placement immediately.

This is the final live-reload model.

No workspace switch is required.

## Native Failure Behavior

During active editing, the runtime must preserve the last known good visible result.

Final rule:

- if a live reload rebuild fails, keep the previous visible placement
- publish diagnostics immediately
- do not tear down the visible layout just because the current file is mid-edit

This is required for good UX.

## CSS Plan

CSS parsing and analysis must be cached as a first-class artifact.

The final CSS artifact contains:

- canonical stylesheet source
- parse result
- analysis diagnostics
- any normalized style representation reusable by scene construction

This artifact is keyed by content hash.

Scene computation is still separate because scene layout also depends on:

- current workspace windows
- selected layout tree
- current monitor/workspace context

So the final split is:

- cache stylesheet processing aggressively
- recompute scene layout only when layout or workspace inputs require it

## JS Plan

### Final Native JS Model

For native JS:

- authored source remains canonical
- `.hypreact-build/` stores prepared JS runtime outputs
- the in-memory runtime state owns executable prepared artifacts
- native QuickJS bytecode is a derived acceleration cache on top of prepared JS artifacts

### Final JS Artifact Chain

The chain is:

- authored JS/TS source
- prepared JS runtime module graph
- executable QuickJS module artifact
- native QuickJS bytecode artifact

### Final Bytecode Decision

We do use native QuickJS bytecode.

We do not use it as the canonical prepared representation.

Prepared JS source artifacts remain canonical because they:

- are easier to debug
- are easier to invalidate deterministically
- are the correct bridge between authored source and executable cache

Bytecode is the native execution acceleration layer only.

### QuickJS Bytecode Rules

- native only
- local cache only
- invalidated by prepared source hash, QuickJS version, or schema version change
- never used in browser
- never treated as portable or trusted external input

## Lua Plan

### Final Native Lua Model

For native Lua:

- authored Lua source remains canonical
- the runtime keeps executable compiled chunk artifacts in memory
- the runtime stores native Lua bytecode artifacts in `.hypreact-build/` when the embedded Lua path exposes clean dump/load support

### Final Lua Artifact Chain

The chain is:

- authored Lua source
- executable compiled Lua chunk artifact
- native Lua bytecode artifact

### Lua Bytecode Decision

We plan for native Lua bytecode as the final acceleration layer.

But the architecture does not depend on bytecode dump/load being present in one exact binding API.

The hard architectural rule is:

- the native Lua runtime must have an executable compiled artifact cache
- if the engine path exposes bytecode dump/load cleanly, persist that executable cache to `.hypreact-build/`
- if not, the executable compiled artifact remains in memory and the architecture stays otherwise unchanged

This is not a middle-ground design.

It is the correct final abstraction boundary:

- canonical source
- executable compiled artifact
- persisted bytecode when the engine exposes it cleanly

The browser never participates in this layer.

## Fennel Plan

Fennel remains authoring-only.

Lua remains the execution runtime.

### Final Native Fennel Model

For native Fennel:

- `.fnl` source is canonical
- Fennel compiles to Lua exactly once per source-content change
- the compiled Lua output is a first-class artifact
- the Lua runtime executes that compiled Lua through the same executable cache layer used by Lua-authored code

### Final Fennel Artifact Chain

The chain is:

- authored Fennel source
- compiled Lua source artifact
- executable compiled Lua chunk artifact
- native Lua bytecode artifact when supported by the embedded Lua path

### Final Fennel Rule

Never compile Fennel during every placement query.

Compilation is an invalidation-time operation only.

### Diagnostics Rule

Fennel diagnostics stay attached to `.fnl` source.

Bytecode and compiled Lua are runtime internals, not user-facing authored identity.

## Browser Plan

The browser keeps the same dependency and invalidation model, but stays source-oriented.

### Final Browser Model

- in-memory project runtime state
- dependency graph
- content-hash invalidation
- immediate reevaluation on editor changes
- no bytecode layers

### Browser Artifact Chains

JS browser chain:

- authored source
- prepared JS runtime graph
- executable browser module evaluation artifact

Lua browser chain:

- authored Lua source
- executable browser Lua evaluation artifact

Fennel browser chain:

- authored Fennel source
- compiled Lua source artifact
- executable browser Lua evaluation artifact

The browser must not attempt to mirror native bytecode persistence.

That would complicate code without improving UX.

## Hyprland Integration

The plugin side must gain one explicit live-reload application path.

Responsibilities:

- accept watcher-driven invalidation notifications from the runtime state
- ask for fresh placement for affected active workspaces
- apply that placement immediately using the existing placement application path

The plugin does not get a second layout engine.

It remains a consumer of the same placement result surface.

## Runtime Payload Rules

`PreparedLayout.runtime_payload` remains runtime-owned.

Core crates do not inspect whether a runtime artifact is backed by:

- source
- compiled source
- executable compiled state
- bytecode

The final requirement is only that runtime-owned artifacts participate in explicit dependency tracking and invalidation.

## Clean Refactor Decisions

These are hard decisions, not options:

- replace broad refresh-on-query behavior with long-lived runtime state
- use content hashes as canonical invalidation tokens
- add native watcher-driven live reload
- make stylesheet processing a first-class cached artifact
- keep prepared source artifacts canonical
- add native QuickJS bytecode acceleration
- add native Lua executable compiled-artifact caching
- persist Lua bytecode when the embedded engine path exposes it cleanly
- keep browser caches source-oriented

## Final Implementation Order

### 1. Replace current runtime service internals with long-lived project runtime state

- artifact registry
- dependency graph
- content-hash invalidation tokens
- no broad cache clear behavior

### 2. Make stylesheet processing a first-class artifact

- content-hash keyed stylesheet cache
- diagnostics and normalized style data cached separately from scene computation

### 3. Make config and layout artifacts dependency-aware

- prepared config artifact
- prepared layout artifact by layout name
- executable artifact identity by runtime entry

### 4. Add native watcher-driven live reload

- file watcher
- debounce
- invalidation routing
- immediate visible workspace reapply
- last-known-good behavior on failures

### 5. Tighten JS prepared artifact ownership under `.hypreact-build`

- prepared JS outputs become first-class runtime artifacts
- placement queries become cache-hit oriented

### 6. Add native QuickJS bytecode artifact generation and loading

- derived from prepared JS artifacts
- invalidated by source hash, engine version, and schema version

### 7. Add native Lua executable compiled-artifact caching

- remove repeated source parsing from the placement hot path

### 8. Add native Lua bytecode persistence when supported cleanly by the embedded Lua path

- same artifact chain
- persisted acceleration layer only

### 9. Add Fennel compiled-Lua artifacts and route them through the same Lua executable cache path

- compile on invalidation only
- never compile on every placement query

### 10. Apply the same dependency and invalidation model to browser runtime state

- no bytecode
- immediate preview reevaluation
- source-oriented caches only

## Open Technical Verification

These are implementation verifications, not architecture questions:

- which exact `rquickjs` API surface is used to serialize and load native QuickJS bytecode in our current version
- which exact `mlua` or embedded Lua 5.4 API surface is used to dump and load Lua chunks in our current build

Those answers determine the implementation details.

They do not change the final plan.

## Summary

The final performance plan is:

- long-lived dependency-aware runtime state
- content-hash invalidation
- first-class prepared and executable artifacts
- watcher-driven native live reload
- immediate visible workspace reapply
- first-class CSS artifact caching
- native QuickJS bytecode acceleration
- native Lua executable artifact caching and bytecode persistence when supported cleanly
- Fennel compiled once per change and then routed through the Lua executable cache path
- browser kept source-oriented but equally reactive

This is the plan we can execute step by step without building temporary architecture we intend to throw away.
