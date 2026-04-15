# Lua Runtime Plan

This document describes the authored Lua API we want and the refactor needed to support Lua as a first-class runtime alongside JS.

The goal is clean code, not compatibility.

- no compatibility layers
- no preserving JS-only naming if it blocks a cleaner model
- no hidden fallback behavior that guesses runtime intent
- prefer a small number of explicit runtime concepts over adapters and shims

This is the final target plan, not a migration sketch.

- choose the clean model first
- refactor directly toward it
- do not add temporary compatibility APIs just to land partial steps

## Goals

- support JS and Lua layouts as first-class runtimes
- make runtime selection explicit in core types
- keep scene, CSS, resize, and compositor logic runtime-agnostic
- make authored layout decoding converge on one normalized layout tree shape
- keep config and runtime loading architecture simple enough that more runtimes can be added later without another major rewrite

## Non-Goals

- preserving current JS-only API names where they are misleading
- supporting every intermediate legacy shape during the refactor
- adding a compatibility abstraction just to avoid renaming a type

## Authored Lua API

The preferred Lua authoring API is a small declarative DSL that reads similarly to JSX.

Example equivalent of `examples/js/layouts/master-stack/index.tsx`:

```lua
local h = require("hypreact")

return function(ctx)
  return h.workspace({ id = "frame" }) {
    h.slot({
      id = "master",
      take = 1,
      class = "master-slot",
    }),

    h.when(#ctx.windows > 1) {
      h.group({ class = "stack-group" }) {
        h.slot({
          id = "stack-slot",
          class = "stack-group__item",
        }),
      },
    },
  }
end
```

## DSL Rules

- `h.workspace(props) { ... }` creates a `workspace` node
- `h.group(props) { ... }` creates a `group` node
- `h.slot(props)` creates a `slot` node
- `h.window(props)` creates a `window` node
- `h.when(condition) { ... }` conditionally contributes children
- child lists are flattened automatically
- `nil` children are dropped automatically
- `h.when(false) { ... }` returns no children
- `h.when(true) { a, b }` may contribute one or more children

## Minimal Lua Helper Contract

The Lua helper module should stay small. It should only provide authoring primitives and child normalization.

Suggested surface:

```lua
h.workspace(props) -> callable container builder
h.group(props) -> callable container builder
h.slot(props) -> leaf node
h.window(props) -> leaf node
h.when(condition) -> callable conditional builder
```

The authoring DSL should normalize to the same runtime-facing node shape regardless of language.

Suggested normalized shape:

```json
{
  "type": "workspace",
  "props": { "id": "frame" },
  "children": [
    {
      "type": "slot",
      "props": { "id": "master", "take": 1, "class": "master-slot" },
      "children": []
    }
  ]
}
```

That means JS JSX and Lua should meet at a common structural representation before Rust validates or resolves layout semantics.

## Clean Architecture Target

The current repo has good low-level runtime seams, but the end-to-end loading path is still JS-centric.

The clean target is:

- runtime-neutral config and layout services in `crates/config` and `crates/layout-runtime`
- one runtime registry responsible for resolving a runtime implementation from authored config or layout metadata
- one normalized prepared layout artifact shape in `crates/core`
- per-runtime authored SDKs and loaders isolated under `crates/runtimes/<runtime>` and `packages/sdk/<runtime>`

## Core Refactor

### 1. Make Runtime Selection Explicit

Today runtime selection is mostly implicit because the top-level service constructs the JS runtime directly.

We should make runtime identity explicit in core data structures.

Suggested additions:

- add `RuntimeKind` enum in a runtime-neutral crate
- add `runtime: RuntimeKind` to `LayoutDefinition`
- consider adding `runtime` to prepared layout metadata too for defensive validation and diagnostics

Suggested shape:

```rust
pub enum RuntimeKind {
    Js,
    Lua,
}
```

and:

```rust
pub struct LayoutDefinition {
    pub name: String,
    pub runtime: RuntimeKind,
    pub directory: String,
    pub module: String,
    pub stylesheet_path: Option<String>,
    pub runtime_cache_payload: Option<serde_json::Value>,
}
```

Without this, mixed JS and Lua layouts in one project remain awkward or implicit.

### 2. Replace JS-Specific Bundle Construction

`crates/layout-runtime` should not import the JS runtime crate directly.

Instead:

- introduce a runtime registry/factory crate or module
- let `LayoutRuntimeService` depend on runtime-neutral construction
- move JS and Lua runtime registration to runtime-specific crates

The current shape hard-codes JS at the top of the stack. That should be removed rather than wrapped.

### 3. Split Config Runtime From Layout Runtime

There are two separate questions:

- what language config files are authored in
- what runtime each layout uses

Those should be modeled independently.

Recommended first clean model:

- keep project config runtime explicit and singular per config root
- allow layout runtime to vary per layout entry

That gives a practical path to:

- JS config with JS and Lua layouts in one project
- Lua config later, if wanted, without entangling the first refactor

### 4. Normalize Runtime Payload Ownership

`PreparedLayout.runtime_payload` is already runtime-neutral enough. Keep that idea.

But the rules should be explicit:

- payload contents are owned only by the selected runtime
- core crates must not inspect runtime-specific payload internals
- prepared payloads should be serializable and cheap to validate

## Runtime Registry

We should add a registry that owns runtime lookup and construction.

Responsibilities:

- map `RuntimeKind` to runtime implementation
- construct config runtime and layout runtime instances
- expose supported runtime info for diagnostics and tooling

Suggested trait split:

- config runtime trait
- prepared layout runtime trait
- source-bundle/browser runtime trait if the playground keeps that architecture

The registry should return runtime implementations through these traits instead of leaking concrete JS or Lua types upward.

## JS Refactor Needed

To support Lua cleanly, current JS names and placement should be cleaned up.

Recommended changes:

- rename JS-specific error variants in `RuntimeError` to runtime-neutral names where possible
- move runtime-neutral concepts out of `crates/runtimes/js/*`
- keep JS graph compilation and QuickJS execution in JS-only crates
- stop letting JS layout discovery define the global project model

Examples of current JS-centric assumptions that should be removed:

- config discovery only probing `config.tsx`, `config.ts`, `config.jsx`, `config.js`
- layout discovery only looking for JS/TSX layout entries
- docs and status surfaces implying authored layouts are inherently JSX

## Lua Runtime Structure

Suggested crate layout:

- `crates/runtimes/lua/core`
- `crates/runtimes/lua/native`
- optionally `crates/runtimes/lua/browser` later if playground support is needed

Suggested responsibility split:

- `lua/core` owns Lua-side decoding contracts, source discovery rules, and shared payload formats
- `lua/native` owns the native embedded Lua engine integration and authored config/layout execution
- `lua/browser` only exists if we later decide the web playground must execute Lua directly in-browser

## Lua Authoring Discovery

We should stop baking JS file names into runtime-neutral discovery.

Instead, discovery should be runtime-aware.

Suggested model:

- config root resolves a config runtime and entrypoint explicitly
- layout discovery can scan for runtime-specific entries by registered runtime rules
- or config can declare layouts explicitly with runtime and module path

Preferred direction for clean code:

- make layout definitions explicit in config
- treat auto-discovery as optional convenience, not the core contract

That avoids tying the project model to any single language's filesystem conventions.

If we keep convenience discovery, it should be runtime-owned and explicit.

Examples:

- JS runtime may discover `layouts/<name>/index.tsx`
- Lua runtime may discover `layouts/<name>/index.lua`

## Decoding And Validation

Rust should continue to own validation of layout semantics.

That means Lua should not invent a second semantic model.

Required flow:

1. Lua authoring DSL builds Lua values
2. Lua runtime converts those values into a normalized layout node representation
3. Rust decodes that normalized representation into `SourceLayoutNode`
4. existing Rust validation and scene logic stays authoritative

This keeps:

- one semantic validator
- one resolver model
- one CSS/scene pipeline
- one resize model

## Playground Strategy

Lua should not force immediate browser support if it complicates the core design.

Preferred order:

1. native Lua runtime support
2. config/runtime-neutral architecture cleanup
3. browser/playground Lua support if still desired

If the playground eventually supports Lua, that should happen through the same normalized contracts, not through a second bespoke layout model.

## SDK Structure

Suggested authored SDK layout:

- `packages/sdk/js`
- `packages/sdk/lua`

Lua SDK contents should be minimal:

- the `hypreact` Lua module implementing `workspace`, `group`, `slot`, `window`, and `when`
- authoring docs and examples
- possibly Lua language-server metadata later if useful

We should avoid trying to fake JS semantics in Lua. The DSL should feel native to Lua while normalizing to the same underlying node shape.

## LuaLS Typing

Lua authoring should be as typed as practical from the start.

We should ship LuaLS annotations for the Lua SDK and examples.

Requirements:

- use EmmyLua/LuaLS annotations understood by Sumneko Lua / LuaLS
- annotate the layout context, node props, node shapes, and helper return types
- make `require("hypreact")` resolve to annotated APIs in example projects
- keep annotations close to the shipped Lua helper module instead of maintaining a separate shadow type layer

Suggested annotated types:

- `Hypreact.LayoutContext`
- `Hypreact.LayoutWindow`
- `Hypreact.WorkspaceProps`
- `Hypreact.GroupProps`
- `Hypreact.SlotProps`
- `Hypreact.WindowProps`
- `Hypreact.LayoutNode`
- `Hypreact.Child`

Suggested helper annotation shape:

```lua
---@class Hypreact.Module
---@field workspace fun(props: Hypreact.WorkspaceProps): Hypreact.ContainerBuilder
---@field group fun(props: Hypreact.GroupProps): Hypreact.ContainerBuilder
---@field slot fun(props: Hypreact.SlotProps): Hypreact.LayoutNode
---@field window fun(props: Hypreact.WindowProps): Hypreact.LayoutNode
---@field when fun(condition: boolean): Hypreact.ConditionalBuilder
```

The template Lua examples should use these annotations so editor support is present immediately.

## File And Type Changes

The following areas should be refactored as part of Lua support:

- `crates/layout-runtime`
  - remove direct dependency on JS runtime construction
  - depend on runtime registry or runtime-neutral factory
- `crates/config/src/model.rs`
  - add explicit runtime metadata to layouts
  - redesign config discovery to be runtime-aware or explicit
- `crates/config/src/runtime.rs`
  - keep generic traits, but revisit names if they still imply the JS-prepared model too strongly
- `crates/core/src/runtime/runtime_error.rs`
  - remove JS-specific naming from shared error variants where practical
- `README.md`, `docs/config.md`, `docs/jsx.md`
  - stop describing the product as only JSX/TSX-authored

## Concrete Implementation Checklist

### Runtime Model

- add `RuntimeKind` to shared runtime/core types
- add `runtime: RuntimeKind` to `LayoutDefinition`
- add `runtime: RuntimeKind` to `SelectedLayout`
- add `runtime: RuntimeKind` to `RuntimeInfo` if that status type remains useful
- update all `PreparedLayout` construction sites to set the selected runtime explicitly

### Runtime Construction

- add a runtime-neutral factory in `crates/layout-runtime`
- remove direct `hypreact_runtime_js_native` construction from `LayoutRuntimeService`
- start with JS registered there
- add Lua registration there once native Lua runtime exists

### JS Cleanup

- thread `RuntimeKind::Js` through all JS config/layout discovery and prepared layout loading
- update tests and fixtures to include explicit runtime metadata
- rename shared JS-specific error text only where it blocks clean multi-runtime messaging

### Lua SDK

- add `packages/sdk/lua`
- ship `hypreact.lua` with the chosen DSL
- ship LuaLS annotations in the SDK module files
- add `.luarc.json` for examples and local fixtures if needed
- add a Lua template example once runtime execution exists

### Lua Runtime

- add `crates/runtimes/lua/core`
- add `crates/runtimes/lua/native`
- choose embedded runtime crate and keep it isolated there
- implement Lua value normalization to the shared node shape
- decode normalized Lua output into `SourceLayoutNode`

### Discovery And Docs

- make config and layout discovery runtime-aware or explicit
- update docs to describe JS and Lua authoring without compatibility wording
- keep browser/playground Lua as a follow-up unless it becomes trivial after the core refactor

## Suggested Execution Order

1. add explicit runtime identity to config and prepared layout metadata
2. introduce a runtime registry and remove JS-specific construction from `crates/layout-runtime`
3. make config and layout discovery runtime-aware or explicit
4. implement Lua native runtime and Lua DSL helper module
5. add Lua examples and docs
6. decide separately whether browser/playground Lua support is worth the added complexity

## Immediate Build Order

The immediate implementation path should be:

1. make runtime identity explicit in shared types
2. remove top-level JS-only runtime construction
3. thread `RuntimeKind::Js` through the current system until it compiles cleanly
4. add Lua SDK with LuaLS annotations and the chosen DSL
5. add native Lua runtime crates and begin wiring them into the registry

That sequence moves the repo toward the final architecture directly instead of creating a temporary mixed model.

## Design Rules To Keep

- Rust owns semantic validation, resolution, resize, and scene generation
- runtime crates own authored-language loading and evaluation
- authored runtimes normalize to one layout tree contract
- runtime selection is explicit, never inferred from whichever service happened to be constructed
- avoid compatibility adapters when a rename or refactor makes the model clearer

## Recommendation

The cleanest first shipped model is:

- explicit config runtime
- explicit per-layout runtime
- JS config remains allowed
- Lua layouts become first-class
- browser Lua support waits until native support and runtime-neutral architecture are solid

That gives the repo a clean multi-runtime foundation without carrying compatibility baggage.
