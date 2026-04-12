# HYPREACT

## Goal

Create a new repo focused only on authored layouts for Hyprland.

`hypreact` should contain only:

- authored layout config loading from `config.ts` / `config.tsx`
- CSS parsing and scene/layout computation
- minimal WM model and navigation logic needed to evaluate layouts and directional actions
- a Rust FFI bridge for the Hyprland plugin
- the Hyprland plugin itself

It should not contain:

- custom compositor implementations
- Wayland compositor app
- X11 compositor app
- web preview/runtime app
- custom CLI app
- IPC/browser/native app stacks that only existed for the old multi-platform product

## Product Shape

`hypreact` is a Hyprland layout engine, not a general-purpose window manager.

The repo should provide:

- a Rust workspace for layout evaluation and host-model logic
- a Hyprland plugin that loads authored config and applies computed placement

Architecture choice:

- Option 1: C++ Hyprland plugin + Rust layout engine
- keep Rust for CSS, scene, authored config loading, JS runtime, layout evaluation, and layout-aware decisions
- keep C++ only at the compositor edge and for Hyprland object integration
- keep the ABI small and Hyprland-specific

Locked product decisions:

- no compatibility-layer architecture should remain in the finished product
- do not preserve multi-host abstractions just because they existed in `spiders-wm`
- remove generic host protocols (`WmSignal`, `WmEvent`, `WmHostEffect`, generic query transport) once replaced by explicit `hypreact`/Hyprland-specific surfaces
- remove `SpiderPlatform` entirely rather than narrowing it
- remove seat abstractions entirely
- keep move and resize commands only where they are real `hypreact`/Hyprland behaviors
- keep shell matching support only to distinguish Wayland vs Xwayland windows
- remove output transform modeling unless it becomes concretely necessary

The repo should not provide:

- a standalone `hypreact` daemon
- a standalone `hypreact` CLI
- alternate compositor frontends

## Keep vs Drop

Keep, in some form:

- `crates/core`
- `crates/css`
- `crates/scene`
- `crates/config`
- `crates/runtimes/js`
- `crates/layout-runtime`
- `crates/hypr-ffi`
- Hyprland plugin sources in `src/` and `include/`

Drop entirely from the new repo:

- `apps/spiders-wm`
- `apps/spiders-wm-x`
- `apps/spiders-wm-www`
- `crates/cli`
- `crates/cli/core`
- `crates/ipc`
- `crates/ipc/browser`
- `crates/ipc/core`
- `crates/ipc/native`
- `crates/css-lsp/stdio`
- `crates/css-lsp/web`
- `crates/fonts/native`
- `crates/logging`

Likely drop or shrink heavily:

- `crates/css-lsp/core`
  - not required for the initial `hypreact` runtime/plugin goal
  - can be migrated later only if editor tooling is still wanted

## Minimal Dependency Slice

Current dependency shape suggests this minimal Rust slice:

- `hypreact-core`
  - based on `spiders-core`
  - keep shared types, snapshots, commands, focus/navigation, layout node types, WM model
- `hypreact-css`
  - based on `spiders-css`
- `hypreact-scene`
  - based on `spiders-scene`
- `hypreact-config`
  - based on `spiders-config`
  - only keep authored layout config and scene request building
- `hypreact-runtime-js`
  - based on the merged JavaScript runtime from `spiders-runtime-js-core` and `spiders-runtime-js-native`
- `hypreact-layout-runtime`
  - based on `spiders-layout-runtime`
- `hypreact-hypr-ffi`
  - based on `spiders-wm-runtime-ffi`
  - should become Hyprland-specific
- Hyprland plugin sources
  - based on `apps/spiders-wm-hypr`

## What To Copy First

Copy first, with rename but minimal behavior changes:

1. `crates/core`
2. `crates/css`
3. `crates/scene`
4. `crates/config`
5. `crates/runtimes/js`
6. `crates/layout-runtime`
7. `crates/hypr-ffi`
8. Hyprland plugin sources

This gives a compilable first vertical slice.

## Rust Cleanup

This section covers only the crates that belong in `hypreact`.

### `core`

Keep:

- IDs and shared layout node types
- state snapshots
- WM model
- focus tree and directional navigation
- resize/layout-adjustment state
- runtime contract types needed by the JS/authored layout engine

Clean up:

- remove `SpiderPlatform` entirely
- remove seat abstractions entirely
  - delete `SeatId`, `SeatModel`, and any seat-specific state/signal plumbing
- remove generic host transport types from `core`
  - delete `WmSignal`
  - delete `WmEvent`
  - delete `WmHostEffect`
  - delete generic query request/response transport types
  - replace them with direct typed Hyprland FFI entrypoints and direct state snapshot helpers
- trim `WmCommand` to the Hyprland-relevant subset
  - keep: `Spawn`, `ReloadConfig`, `SetLayout`, `CycleLayout`, `ViewWorkspace`, `ActivateWorkspace`, `SelectWorkspace`, `SelectNextWorkspace`, `SelectPreviousWorkspace`, `ToggleFloating`, `ToggleFullscreen`, `AssignFocusedWindowToWorkspace`, `ToggleAssignFocusedWindowToWorkspace`, `FocusWindow`, `FocusDirection`, `SwapDirection`, `MoveDirection`, `ResizeDirection`, `CloseFocusedWindow`
  - drop: `Quit`, `ToggleViewWorkspace`, `AssignWorkspace`, `FocusMonitorLeft`, `FocusMonitorRight`, `SendMonitorLeft`, `SendMonitorRight`, `SetFloatingWindowGeometry`
- simplify shell modeling
  - keep only what `hypreact` needs for matching: Wayland vs Xwayland
  - remove broader shell taxonomy
- remove output transform modeling
- remove or ignore stale architecture docs in the crate root during migration
  - `TITLEBAR.md`, `FONTS.md`, `RESULT.md`, `TEMP.md`, `RUNTIME.md` should not be treated as product requirements for `hypreact`

### `css`

Keep almost entirely.

Notes:

- this crate is already close to the desired scope
- no major product-surface cleanup is required here beyond renaming and normal dependency cleanup

### `scene`

Keep almost entirely.

Notes:

- this is core layout/styling infrastructure and should remain in Rust
- cleanup should focus on naming and dead tests only, not behavioral simplification unless intentional

### `config`

Keep:

- layout definitions
- layout selection
- authored config path discovery and service APIs
- scene request construction

Clean up:

- config is now layout-centric
  - keep authored `layouts`
  - workspace context now comes from live Hyprland state, not authored config
  - discovered layout modules and runtime cache payloads remain part of the Rust config model
  - `workspaces`, `options`, `inputs`, `rules`, `bindings`, `autostart`, and `autostart_once` are removed from the supported Hypreact config surface
- trim `crates/config/src/virtual/api.js`
  - keep only runtime stubs that matter to authored layout/config evaluation
  - remove no-op APIs for old WM-only commands that `hypreact` will never support

### `runtime-js`

Keep:

- module graph building
- compile pipeline for authored config/layout apps
- runtime graph payload encoding/decoding
- QuickJS integration
- authored config evaluation
- prepared runtime loading
- runtime graph evaluation for layout entrypoints

Clean up:

- remove `SpiderPlatform` plumbing and `*_for_platform` APIs entirely
- removed `platformMatch` and platform runtime globals from the JS SDK/runtime surface
- remove config/bindings-specific assumptions if they are only there for old WM product ergonomics
- verify whether preview-bundle helpers are still needed; if not, drop them
- `HypreactConfig` now replaces `SpiderWMConfig` in the public SDK/test-config surface
- merged the old core/native split into a single crate
- simplify config decode
  - completed: bindings and other old WM config sections are no longer part of the typed Hypreact config surface
  - decode only the layout-centric config fields that still matter in `hypreact`
  - `layouts.per_workspace` is indexed by live workspace ordering from current state
- remove Xorg/Web conditional behavior and tests
- public SDK/API imports now use `@hypreact/sdk`

### `layout-runtime`

Keep:

- config discovery from authored config path
- config load/reload
- workspace evaluation
- scene evaluation
- ordered window IDs and geometry extraction

Clean up:

- remove platform arguments from `LayoutRuntimeService::new`
- make the service construct the single native/Hyprland JS runtime directly
- cache paths now use `.hypreact-build`
- current live use is current-workspace/current-output evaluation only
- keep watching for broader helper APIs that no longer have a concrete product use

### `hypr-ffi`

Keep:

- opaque runtime handle
- host sync API for outputs/workspaces/windows/focus
- authored config load/reload
- query placement
- focus-direction
- swap-direction

Clean up:

- this crate should become a Hyprland-specific bridge, not a generic WM bridge
- removed `parse_bindings_source`
- removed generic `handle_signal`, generic host-sync JSON, and generic `query` entrypoints
- removed `PreviewRenderAction` from the FFI command response model
- removed `SpiderPlatform::Wayland` usage and construct the only supported runtime directly
- trimmed naming and payload shape to the data the plugin actually needs
- use explicit typed APIs for:
  - output upsert/remove
  - workspace activation
  - focused window selection
  - window upsert/remove
  - command dispatch and returned host actions
  - placement and directional candidates
  - state/status inspection used by the plugin
- do not preserve JSON message-bus style APIs just for compatibility

### Hyprland plugin

Keep:

- config path option
- runtime sync
- algorithm registration
- layout-aware placement
- focus/swap integration
- dispatcher wrapping where useful

Clean up:

- rename away from `spiderswm`
- keep query/debug surfaces only if they help real plugin debugging
- remove any compatibility scaffolding that existed only for the older broader product
- keep plugin logic at the Hyprland adapter layer only; layout rules and placement decisions must come from Rust

## What To Trim During Migration

### `core`

Keep:

- snapshot/model types
- command/effect/event/signal types used by FFI/plugin
- focus and directional navigation
- layout node and scene-facing shared types

Trim if unused by `hypreact`:

- old compositor-facing abstractions that only mattered to custom WM backends

### `config`

Keep:

- authored config loading
- layout selection
- authored layout service interfaces
- scene request construction

Drop if unused:

- config options only meant for old standalone WM product surface
- binding/autostart semantics if they are no longer consumed anywhere

Note:

- it is fine for `config.ts` to still expose some old fields temporarily during migration
- but `hypreact` should only treat layout-related parts as supported product surface

### `hypr-ffi`

Keep behavior that matters to Hyprland:

- reset/sync host model
- load/reload authored config
- query runtime/layout status
- query placement
- focus-direction
- swap-direction

Drop behavior not needed in the new product:

- generic/non-Hyprland legacy naming
- `parse_bindings_source`
- any preview/browser-oriented response semantics
- anything only there for old app shells

Potential rename:

- `spiders-wm-runtime-ffi` -> `hypreact-hypr-ffi`
  - completed

### Hyprland plugin

Keep:

- config path option
- runtime sync
- algorithm registration
- layout-aware placement
- focus/swap integration
- dispatcher wrapping where useful

Drop or simplify:

- generic `spiderswm` naming
  - completed for the live plugin/runtime path
- old compatibility/debug layers once migration settles

Potential rename:

- plugin binary: `hypreact-hypr.so`
- config namespace: `plugin:hypreact-hypr`
- hyprctl command: `hypreact`

## New Workspace Layout

Suggested initial structure:

```text
hypreact/
  Cargo.toml
  HYPREACT.md
  crates/
    core/
    css/
    scene/
    config/
    runtimes/js/
    layout-runtime/
    hypr-ffi/
  src/
  include/
  test_config/
```

## Naming Migration

Rename during migration, but do it in a controlled order.

Recommended order:

1. copy first with old names so it builds quickly
2. get the new repo compiling and plugin loading
3. rename crates, CMake targets, headers, config namespace, SDK imports, cache paths, and hyprctl command from `spiders` to `hypreact`

This is safer than renaming everything before the first successful build.

## Migration Phases

### Phase 1: Create a Buildable Skeleton

- initialize Cargo workspace in `/home/akisarou/projects/hypreact`
- copy the minimal crate/app set
- copy the Hyprland plugin CMake project
- make it build under old names first if that is fastest

Exit criteria:

- Rust workspace builds
- Hyprland plugin builds

### Phase 2: Preserve Current Functionality

- load authored `config.ts`
- evaluate layout for current workspace
- expose typed placement/candidate/status APIs through FFI
- register `spiders` or temporary equivalent tiled algorithm
- verify placement/focus/swap still work in Hyprland

Exit criteria:

- current plugin behavior is reproduced in the new repo

### Phase 3: Prune Old Product Surface

- remove copied code not referenced by the new repo
- remove web/browser/X11/CLI assumptions
- simplify manifests and dependency graph

Exit criteria:

- no dead legacy app stacks in `hypreact`
- dependency graph is clearly Hyprland-layout focused

### Phase 4: Rename to `hypreact`

- rename crate/package/library/plugin symbols
- rename config namespace and hyprctl command
- update test config and docs

Exit criteria:

- user-facing naming is `hypreact`
- no `spiders-wm` branding remains except historical comments if any

## Copy Checklist

Copy:

- source files for the kept crates/apps
- relevant tests for kept crates
- plugin headers in `apps/spiders-wm-hypr/include`
- `test_config/` for live validation
- `.clangd` only if still useful for plugin development

Do not copy by default:

- old app-specific docs
- browser assets
- wasm/web scaffolding
- old generated cache/build outputs
  - removed stale checked-in `.spiders-wm-build` fixture output
- unrelated config/editor files unless they help current development directly

## Immediate Recommendation

Start `hypreact` by copying these first:

- `crates/core`
- `crates/css`
- `crates/scene`
- `crates/config`
- `crates/runtimes/js/core`
- `crates/runtimes/js/native`
- `crates/layout-runtime`
- `crates/hypr-ffi`
- Hyprland plugin sources
- `test_config`

Completed major cleanup tasks:

- remove `SpiderPlatform` across the kept crates and JS runtime
- remove seat abstractions and generic host transport types from `core`
- replace signal/query-era FFI with typed Hyprland-specific APIs
- trim old WM product-surface commands/options from config and JS decode

Next cleanup tasks:

- keep shrinking the JS/config surface away from legacy WM-only no-op APIs
- completed: removed the old event/query no-op bus from the public authored-config API surface
- rename internal `spiders_*` crate/module identifiers where the churn is justified
- delete or archive stale scratch architecture docs under `crates/core/src/*.md`

Evaluation at end:

- keep `layouts.per_workspace` index-based for now
- optionally revisit name-based mapping at the very end if index-ordering becomes a real product problem

## Non-Goals

- preserving feature parity with the old standalone compositor apps
- preserving multi-backend abstraction for its own sake
- keeping generic IPC/CLI surfaces that Hyprland users do not need
- migrating everything before proving the minimal Hyprland-only slice works

## Success Criteria

`hypreact` is successful when:

- the repo is clearly smaller and easier to understand than `spiders-wm`
- the only supported runtime target is the Hyprland plugin
- authored JSX/TSX layouts and CSS still work
- Hyprland placement/focus/swap/resize work through the FFI/plugin path
- there is no standalone compositor or CLI product left in scope

## Focus Parity Plan

Goal:

- match `spiders-wm` directional focus behavior in `hypreact`
- keep all focus business logic in Rust
- keep FFI and the Hyprland plugin as thin adapters
- do not reintroduce generic WM/runtime compatibility layers

Observed state:

- `hypreact` already has the same core `FocusTree`, remembered-scope memory, focus-loss fallback, and directional candidate selector as `spiders-wm`
- the main remaining gap is runtime integration, not the core selector algorithm
- `spiders-wm` keeps `model.focus_tree` aligned with the rendered scene/snapshot, while `hypreact` still leans too heavily on one-off geometry candidates during dispatcher queries

Plan:

1. Persist scene-derived focus structure in Rust
   - make `layout-runtime` return the workspace `FocusTree` derived from the evaluated scene geometry
   - make the `hypr-ffi` path update `model.focus_tree` with that tree for the active workspace before directional focus selection
   - continue pruning remembered focus memory through `core::wm`

2. Make directional focus use one shared Rust path
   - keep directional selection in `core::navigation`
   - expose only typed candidate/query entrypoints from `hypr-ffi`
   - avoid any branch- or memory-related logic in the plugin

3. Preserve workspace-local focus memory semantics
   - ensure remembered scope focus is updated only from workspace-local rendered trees
   - make sure switching workspaces keeps each workspace’s focus memory coherent through the shared model

4. Align focus-loss behavior with `spiders-wm`
   - validate close/unmap fallback behavior on active workspace
   - keep same-scope remembered fallback ahead of workspace-wide fallback
   - add missing tests only where behavior is currently unverified in `hypreact`

5. Align move-direction behavior with focus-direction behavior
   - reuse the same directional candidate source for `movewindow`
   - keep the actual order mutation in `core::wm`
   - keep the plugin responsible only for syncing Hyprland state and triggering recalculation

6. Validate parity with authored layouts that stress remembered focus
   - master-stack with multiple stack children
   - nested vertical/horizontal groups
   - wrap behavior at branch edges
   - focus recovery after close/unmap

Execution order:

- first: persist scene-derived `FocusTree` into the active Rust model
- second: add targeted parity tests around remembered directional focus and focus-loss fallback
- third: audit `movewindow` to use the same directional candidate source cleanly

Non-goals for this plan:

- reintroducing seats/hovered/interacted abstractions from `spiders-wm`
- adding a generic WM runtime layer
- moving focus policy into FFI or the C++ plugin
