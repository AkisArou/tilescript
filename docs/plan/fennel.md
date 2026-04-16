# Fennel Runtime Plan

This document describes how Fennel should fit into `tilescript` as a first-class authoring language.

The goal is clean code, not compatibility.

- no compatibility layers
- no duplicating a second runtime backend when one semantic backend is enough
- no hidden transpilation behavior that makes runtime identity ambiguous
- prefer one explicit language pipeline over adapters and shims

This is the final target plan, not a migration sketch.

- choose the clean model first
- refactor directly toward it
- do not add temporary compatibility APIs just to land partial steps

## Goals

- support Fennel as a first-class authored language alongside JS/TS and Lua
- keep Lua as the execution runtime for both Lua and Fennel
- avoid duplicating native and browser runtime implementations when Fennel can compile to Lua
- make authoring-language identity explicit where tooling and discovery need it
- keep the normalized layout/config model shared after language-specific compilation

## Non-Goals

- adding a separate `RuntimeKind::Fennel`
- implementing a full independent Fennel runtime backend
- preserving JS- or Lua-only assumptions in discovery if they block a cleaner language model
- hiding Fennel compilation in places where diagnostics or tooling need to know the original source language

## Why Fennel Fits

Fennel is the best Lisp-family fit for this repo because it compiles to Lua.

That gives us:

- Lisp-family authoring syntax
- one native execution backend
- one browser execution backend
- one normalized runtime payload shape after compilation

The key architectural point is:

- Fennel is an authoring language
- Lua is the execution runtime

That is much cleaner than adding a completely separate Scheme or Lisp runtime backend.

## Authored Fennel API

The authored API should mirror the Lua DSL exactly, just in Fennel syntax.

Example equivalent of `examples/lua/layouts/master-stack/index.lua`:

```fennel
(local h (require :tilescript))

(fn layout [ctx]
  ((h.workspace {:id "frame"})
    [(h.slot {:id "master"
              :take 1
              :class "master-slot"})

     ((h.when (> (# ctx.windows) 1))
      [((h.group {:class "stack-group"})
        [(h.slot {:id "stack-slot"
                  :class "stack-group__item"})])])]))

layout
```

The exact surface area should stay aligned with Lua:

- `workspace`
- `group`
- `slot`
- `window`
- `when`

If the Lua DSL changes, the Fennel authoring story should change with it rather than forking into a separate API.

## DSL Rules

The DSL rules are inherited from Lua because Fennel compiles to Lua and should target the same helper contract.

- `(h.workspace props)` returns a callable container builder
- `(h.group props)` returns a callable container builder
- `(h.slot props)` creates a leaf node
- `(h.window props)` creates a leaf node
- `(h.when cond)` returns a callable conditional builder
- child lists are flattened automatically
- `nil` children are dropped automatically
- false conditions contribute no children

Fennel should not introduce extra semantic helpers unless they are also justified for Lua.

## Clean Architecture Target

Fennel should be modeled as a frontend to the Lua runtime, not as a new runtime backend.

The clean target is:

- runtime-neutral loading in `crates/config` and `crates/layout-runtime`
- one Lua execution runtime for both Lua-authored and Fennel-authored modules
- one Fennel compilation layer that produces Lua source before runtime evaluation
- one normalized layout/config decoding path after compilation

That means the repo should distinguish between:

- authoring language
- execution runtime

Today `RuntimeKind` already represents execution runtime. That should stay true.

Fennel should not overload runtime identity.

## Core Model Changes

### 1. Keep `RuntimeKind` Runtime-Focused

`RuntimeKind` should continue to describe the execution runtime:

```rust
pub enum RuntimeKind {
    Js,
    Lua,
}
```

Fennel should compile to Lua and execute under `RuntimeKind::Lua`.

Adding `RuntimeKind::Fennel` would incorrectly imply a second execution backend and would duplicate runtime plumbing for no real benefit.

### 2. Add Authoring Language Where Needed

We should add an explicit authored language model where discovery, tooling, examples, or diagnostics need it.

Suggested shape:

```rust
pub enum AuthoringLanguage {
    JavaScript,
    TypeScript,
    Lua,
    Fennel,
}
```

This does not need to be threaded everywhere in core runtime artifacts.

It should appear only where it has real value:

- config discovery
- layout discovery
- source-bundle/tooling manifests
- diagnostics/source maps
- playground/editor state

### 3. Keep Prepared Layout Payload Runtime-Owned

The prepared layout payload for Fennel-authored layouts should still belong to the Lua runtime.

That payload may need to include:

- original Fennel source
- compiled Lua source
- optional source map / line mapping metadata

Core crates should still treat that payload as opaque runtime-owned data.

## Fennel Pipeline

The Fennel pipeline should be explicit.

### Native

- discover `config.fnl` or `layouts/<name>/index.fnl`
- compile Fennel source to Lua source
- pass compiled Lua to the existing Lua runtime pipeline
- decode and validate the resulting config/layout using the shared Lua/runtime-neutral decoders

### Browser

- discover Fennel source bundle entries
- compile Fennel source to Lua in-browser
- pass compiled Lua to the existing browser Lua runtime
- keep diagnostics tied to original Fennel source when possible

### Important Rule

Compilation should happen once per authored source update, not repeatedly during every layout evaluation if the source has not changed.

The clean model is:

- compile authored source
- cache compiled Lua as runtime payload or source-bundle cache data
- evaluate compiled Lua repeatedly as needed

## Suggested Crate Layout

Suggested additions:

- `crates/runtimes/fennel/core`
- `crates/runtimes/fennel/native`
- optionally `crates/runtimes/fennel/browser`

Responsibility split:

- `fennel/core` owns source discovery rules, compile contract types, and shared cache payload formats
- `fennel/native` owns native Fennel compilation and forwards compiled Lua into the Lua native runtime path
- `fennel/browser` owns browser-side Fennel compilation and forwards compiled Lua into the Lua browser runtime path

Important:

- `fennel/*` is a compilation frontend
- `lua/*` remains the execution backend

If that split feels too heavy initially, `fennel/native` and `fennel/browser` can be thin wrappers over `lua/*`, but the conceptual separation should stay explicit.

## Discovery Rules

Fennel should participate in config and layout discovery explicitly.

Recommended entrypoints:

- `config.fnl`
- `layouts/<name>/index.fnl`

Suggested precedence should remain explicit rather than magical.

If multiple config entrypoints exist, the repo should not silently guess based on extension ordering. The chosen config root entrypoint should be resolved intentionally and diagnostics should explain conflicts.

For layouts, mixed language projects should be allowed as long as each layout resolves to a single execution runtime.

Examples:

- JS config + JS layouts + Lua layouts + Fennel layouts
- Lua config + Lua layouts + Fennel layouts
- Fennel config + Lua/Fennel layouts

All Lua and Fennel layouts still execute under the Lua runtime backend.

## Diagnostics and Source Maps

This is the main place where Fennel needs dedicated care.

If compilation errors occur:

- diagnostics must point to `.fnl` source, not generated Lua

If runtime decode/evaluation errors occur after compilation:

- diagnostics should map back to Fennel source when line mapping is available
- otherwise diagnostics should clearly identify that the failure occurred in compiled Lua originating from a `.fnl` file

The minimum clean requirement is:

- preserve original source path in diagnostics
- keep a line mapping structure in runtime payload/cache when feasible

Without this, Fennel support will feel bolted-on even if execution works.

## SDK and Authoring Support

Suggested package layout:

- `packages/sdk/fennel/`

This package should provide:

- Fennel examples
- helper import conventions
- editor support docs
- generated or maintained reference material pointing to the Lua helper contract

Because Fennel targets Lua, we should avoid duplicating the runtime helper implementation itself.

Prefer:

- one real helper contract in Lua
- Fennel authors `require` that helper through standard Lua-compatible interop

## Playground Support

The playground should model Fennel as its own authoring language option if we support it.

That means:

- separate starter files for Fennel
- Monaco syntax highlighting for Fennel if available, otherwise a reasonable fallback
- Fennel source compilation to Lua before preview evaluation
- diagnostics surfaced against `.fnl` files

The browser runtime provider selection should still resolve to the Lua execution backend after Fennel compilation.

Clean mental model:

- editor mode: Fennel
- compile target: Lua
- execution runtime: Lua

## Examples Structure

If Fennel becomes supported, examples should be explicit:

- `examples/js`
- `examples/lua`
- `examples/fennel`

Those starter projects should mirror each other as closely as practical.

The Fennel example should not invent an alternative layout style just to look more Lispy. It should demonstrate the same conceptual DSL as Lua.

## Browser Compiler Strategy

The browser-side Fennel implementation should prefer a real Fennel compiler running in-browser over hand-written translation logic.

Recommended approach:

- use the official Fennel compiler in a browser-compatible Lua environment or a JS-distributed compiler path if available
- compile `.fnl` to Lua source in a worker or isolated module
- cache compiled source by file content hash or model version

We should not implement an in-house Fennel parser or compiler. That would duplicate a mature upstream tool for little value.

## Native Compiler Strategy

For native support, use the upstream Fennel compiler rather than reimplementing it.

Possible implementation shapes:

- embedded Lua runtime invoking the Fennel compiler
- vendored/compiler source distribution invoked through Lua

The key rule is the same as browser support:

- compile to Lua
- then execute via the existing Lua runtime backend

## Recommended Implementation Order

1. Add `docs/plan/fennel.md`
2. Introduce explicit authoring-language metadata where discovery/tooling need it
3. Add native Fennel config/layout discovery
4. Implement native Fennel -> Lua compilation
5. Forward compiled Lua into the existing native Lua runtime
6. Add `examples/fennel`
7. Add docs/README references for Fennel support
8. Add browser Fennel compilation
9. Wire playground authoring mode and preview execution
10. Improve diagnostics/source mapping

## Clean Final State

The clean final state should look like this:

- JS/TS authoring executes under the JS runtime backend
- Lua authoring executes under the Lua runtime backend
- Fennel authoring compiles to Lua and executes under the Lua runtime backend
- normalized layout/config decoding stays shared after language-specific compilation/evaluation
- examples, docs, and playground all treat Fennel as a real authored language, not a hidden alias

That keeps the implementation honest:

- one Lisp-family language option
- no extra backend explosion
- no fake runtime identity
- no duplicated native/browser evaluator stack
