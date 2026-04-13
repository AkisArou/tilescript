# ANIMATIONS

## Goal

Define a compositor-delegated animation model for `hypreact` where:

- authors describe animation intent in CSS
- the compositor executes the animation timing and rendering
- `hypreact` does not own a long-lived motion engine or animation timeline state
- the authored surface stays as close to standard CSS as possible
- backend-specific behavior remains an adapter concern

This document describes the target design and the constraints discovered from the current Hyprland integration.

## Product View

`hypreact` should not become a second compositor animation runtime.

The intended split is:

- CSS is the authored animation surface
- Rust interprets authored animation intent into backend-capable motion descriptors
- the compositor executes the animation

That keeps `hypreact` closer to a layout-and-scene spec than a bespoke animation engine.

## Non-Goals

The initial compositor-delegated animation design should not try to provide full spec-compliant CSS animation execution.

Specifically, `hypreact` should not initially own:

- keyframe timeline scheduling
- per-element animation progress tracking
- fill mode behavior over time
- iteration counters
- alternate / reverse playback state
- compositor-frame-by-frame sampling

If the compositor is the executor, those semantics are only available if the compositor itself exposes them.

## What Hyprland Provides

Hyprland animations are configured in an `animations {}` block.

The core forms are:

```ini
bezier = name, x0, y0, x1, y1
animation = name, onoff, speed, curve[, style]
```

Conceptually:

- `bezier` defines a named easing curve
- `animation` enables a compositor animation category
- `speed` controls how fast that category animates
- `curve` selects the easing curve
- `style` selects a compositor-specific preset, such as `fade` or `popin 87%`

This is not a CSS animation engine.

It is a compositor policy table for categories like:

- windows
- windows entering
- windows exiting
- workspace transitions
- layer-shell surfaces entering/exiting
- borders
- fades

## What A Layer Is In Hyprland

In Hyprland, `layers` refers to layer-shell surfaces, not ordinary tiled windows.

Typical examples:

- bars
- launchers
- notifications
- overlays
- lock screens
- wallpapers
- panels from programs like `waybar`, `rofi-wayland`, `wlogout`, etc.

These come from the Wayland layer-shell protocol and are placed above or below normal windows depending on layer and anchor settings.

So:

- `windows*` animations affect normal application windows
- `layers*` animations affect layer-shell surfaces

## What Hypreact Already Has

`hypreact` already supports a meaningful subset of CSS motion syntax in parsing and scene computation:

- `@keyframes`
- `animation-*`
- `transition-*`
- cubic bezier and related easing parsing

There is also scene-side runtime support for animatable properties such as:

- `opacity`
- `transform`

However, the current Hyprland integration only consumes final geometry placements, not compositor-executed motion descriptors.

## Constraint: Hyprland Is Not A CSS Animation Runtime

Because Hyprland executes category-based animations rather than CSS timelines:

- CSS transitions map reasonably well
- CSS keyframes do not map cleanly

Examples of CSS semantics that do not have a clean Hyprland equivalent:

- arbitrary intermediate keyframe stops
- `animation-iteration-count`
- `animation-direction`
- `animation-fill-mode`
- multiple named animations with independent timelines
- property-specific keyframe interpolation beyond compositor presets

Therefore, if Hyprland remains the executor, `hypreact` cannot honestly claim full CSS animation compliance.

## The Right Initial Scope

The first backend-delegated animation model should focus on compositor-translatable CSS intent.

That means:

- standard CSS syntax where possible
- transitions first
- keyframes deferred unless the backend can truly execute them
- backend adapters free to reject unsupported authored motion

## Recommended Authored Surface

For compositor-delegated motion, the clean authored surface should be based primarily on standard transition semantics.

Examples:

```css
:root {
  --ease-out-quint: cubic-bezier(0.23, 1, 0.32, 1);
  --almost-linear: cubic-bezier(0.5, 0.5, 0.75, 1);
}

window {
  transition-property: transform, opacity;
  transition-duration: 280ms;
  transition-timing-function: var(--ease-out-quint);
}

window:enter {
  opacity: 0;
  transform: scale(0.87);
}

window:exit {
  opacity: 0;
  transform: scale(0.87);
  transition-duration: 90ms;
  transition-timing-function: linear;
}

workspace {
  transition-property: opacity;
  transition-duration: 120ms;
  transition-timing-function: var(--almost-linear);
}

workspace:enter,
workspace:exit {
  opacity: 0;
}
```

This gives authors a CSS-native way to describe:

- duration
- easing
- opacity fades
- transform-driven presets like popin

## Proposed Selector Model

The first compositor-delegated model should expose lifecycle and compositor-role states through selectors, while keeping the animated properties themselves standard CSS.

Recommended initial selectors:

- `window`
- `workspace`
- `layer`

Recommended initial pseudo-classes:

- `:enter`
- `:exit`
- `:move`
- `:focus`
- `:unfocus`

Notes:

- `:enter` means the compositor is introducing a new visible object
- `:exit` means the compositor is removing a visible object
- `:move` means geometry is changing due to retile, workspace change, or other compositor placement updates
- `:focus` / `:unfocus` apply to compositor focus changes

These pseudo-classes are not CSS-standard, but they only identify compositor lifecycle states. The animation properties applied inside them should remain standard CSS.

Examples:

```css
window {
  transition-property: transform, opacity;
  transition-duration: 240ms;
  transition-timing-function: cubic-bezier(0.23, 1, 0.32, 1);
}

window:enter {
  opacity: 0;
  transform: scale(0.87);
}

window:exit {
  opacity: 0;
  transform: scale(0.87);
  transition-duration: 90ms;
  transition-timing-function: linear;
}

workspace:enter,
workspace:exit {
  opacity: 0;
}

layer:enter,
layer:exit {
  opacity: 0;
}
```

## First Supported CSS Subset For Hyprland

If Hyprland remains the executor, the supported CSS subset should be intentionally small and explicit.

### Supported selectors

- `window`
- `workspace`
- `layer`
- the pseudo-classes listed above

### Supported properties

- `transition-property`
- `transition-duration`
- `transition-timing-function`
- `opacity`
- `transform`

### Supported transform subset

The first transform subset should be limited to forms that can map to known compositor presets:

- `scale(...)`
- possibly later `translateX(...)` / `translateY(...)` if a backend exposes compatible slide styles

### Unsupported in the first Hyprland-backed version

- `@keyframes`
- `animation-name`
- `animation-duration`
- `animation-delay`
- `animation-iteration-count`
- `animation-direction`
- `animation-fill-mode`
- `animation-play-state`
- arbitrary transform stacks with no compositor preset equivalent
- property animation beyond compositor-supported categories

### Behavior of unsupported declarations

Recommended behavior:

- reject or warn at backend-translation time
- do not silently pretend that Hyprland can execute them as real CSS animations

## CSS To Hyprland Mapping Table

The mapping should be capability-driven and intentionally approximate.

| CSS intent | Hyprland category/style | Notes |
| --- | --- | --- |
| `window` + transition on `transform`/`opacity` | `windows` | General window motion / retile category |
| `window:enter` + `opacity: 0` | `fadeIn` | Pure enter fade |
| `window:exit` + `opacity: 0` | `fadeOut` | Pure exit fade |
| `window:enter` + `transform: scale(0.87)` | `windowsIn ..., popin 87%` | Strong mapping from scale to popin |
| `window:exit` + `transform: scale(0.87)` | `windowsOut ..., popin 87%` | Strong mapping from scale to popin |
| `workspace` transition on `opacity` | `workspaces` | General workspace transition category |
| `workspace:enter` + `opacity: 0` | `workspacesIn ..., fade` | Enter fade preset |
| `workspace:exit` + `opacity: 0` | `workspacesOut ..., fade` | Exit fade preset |
| `layer` transition on `opacity` | `layers` | General layer-shell motion |
| `layer:enter` + `opacity: 0` | `layersIn ..., fade` or `fadeLayersIn` | Backend chooses best category |
| `layer:exit` + `opacity: 0` | `layersOut ..., fade` or `fadeLayersOut` | Backend chooses best category |
| `transition-timing-function` | `bezier` + category curve reference | Curves map cleanly |
| `transition-duration` | category speed conversion | Needs explicit conversion policy |

## Example Mapping Of A Real Hyprland Config

Given a Hyprland config like:

```ini
bezier = easeOutQuint, 0.23, 1, 0.32, 1
bezier = almostLinear, 0.5, 0.5, 0.75, 1

animation = windows,    1, 4.79, easeOutQuint
animation = windowsIn,  1, 4.1,  easeOutQuint, popin 87%
animation = windowsOut, 1, 1.49, linear,       popin 87%
animation = fadeIn,     1, 1.73, almostLinear
animation = fadeOut,    1, 1.46, almostLinear
animation = workspaces, 1, 1.94, almostLinear, fade
```

The closest CSS-authored equivalent would be:

```css
:root {
  --ease-out-quint: cubic-bezier(0.23, 1, 0.32, 1);
  --almost-linear: cubic-bezier(0.5, 0.5, 0.75, 1);
}

window {
  transition-property: transform, opacity;
  transition-duration: 280ms;
  transition-timing-function: var(--ease-out-quint);
}

window:enter {
  opacity: 0;
  transform: scale(0.87);
}

window:exit {
  opacity: 0;
  transform: scale(0.87);
  transition-duration: 90ms;
  transition-timing-function: linear;
}

workspace {
  transition-property: opacity;
  transition-duration: 120ms;
  transition-timing-function: var(--almost-linear);
}

workspace:enter,
workspace:exit {
  opacity: 0;
}
```

This is not literal CSS runtime execution. It is CSS-authored motion intent mapped into Hyprland categories and presets.

## Proposed Translation Rules

The Hyprland backend should use deterministic translation rules.

Examples:

- if `transition-timing-function` is a cubic bezier, register or reuse a Hyprland `bezier`
- if `window:enter` sets `opacity: 0` and `transform: scale(N)`, prefer `windowsIn` with `popin N`
- if `window:enter` sets only `opacity: 0`, prefer `fadeIn`
- if `workspace:enter` or `workspace:exit` sets `opacity: 0`, prefer workspace `fade`
- if both a general category and a specialized category are implied, specialized category wins

## Proposed Backend Validation Rules

The Hyprland backend should surface explicit diagnostics for unsupported authored motion.

Examples:

- `@keyframes` present for a Hyprland-delegated target -> unsupported
- `animation-name` used on `window` / `workspace` / `layer` -> unsupported in delegated mode
- `transform: rotate(...)` -> unsupported if no Hyprland preset exists
- multiple transition properties with conflicting preset mappings -> unsupported or partially applied with warning

## First Implementation Recommendation

The first Hyprland-backed animation implementation should only attempt:

- window enter/exit
- workspace enter/exit
- layer enter/exit
- fade presets
- popin presets
- duration/easing translation

It should explicitly defer:

- true CSS keyframes
- animation iteration semantics
- border animation parity
- zoomFactor parity
- generic arbitrary transform animation

## Implementation Roadmap

The first end-to-end implementation should be split into three layers:

- selector and authored-surface support
- backend-neutral motion descriptor generation
- Hyprland adapter translation

## 1. Selector And Parser Changes

### Goal

Allow authored CSS to express compositor lifecycle states without inventing non-CSS animation properties.

### Required parser work

- extend selector support to recognize compositor lifecycle pseudo-classes:
  - `:enter`
  - `:exit`
  - `:move`
  - `:focus`
  - `:unfocus`
- allow these pseudo-classes on compositor-facing semantic node types:
  - `window`
  - `workspace`
  - `layer`

### Required style-tree work

- preserve enough semantic node identity to know whether a styled node corresponds to:
  - a normal window
  - a workspace surface
  - a layer-shell surface
- allow style matching to see transient compositor lifecycle flags supplied by the adapter/runtime evaluation request

### Important constraint

These pseudo-classes should only affect selector matching.

They should not imply that `hypreact` owns timeline state.

The backend will use them only to choose which transition profile applies to a compositor event.

## 2. Backend-Neutral Motion Descriptor

### Goal

After selector matching and computed-style resolution, `hypreact` should produce a small normalized motion descriptor that backends can either support or reject.

### Initial descriptor shape

The first descriptor can be intentionally small.

Conceptually:

```text
targetKind: window | workspace | layer
eventKind: enter | exit | move | focus | unfocus
durationMs: number
timingFunction: easing
effect:
  - fade
  - popin(scale)
  - none
```

### How descriptor extraction should work

- read standard computed CSS declarations from the matched base selector and lifecycle selector
- inspect:
  - `transition-duration`
  - `transition-timing-function`
  - `opacity`
  - `transform`
- infer a backend-neutral effect:
  - `opacity: 0` on enter/exit -> `fade`
  - `transform: scale(N)` on enter/exit -> `popin(scale = N)`
  - absence of recognizable effect -> `none`

### Important rule

Descriptor generation should be deterministic and conservative.

If authored CSS uses unsupported motion semantics, the descriptor layer should surface a structured unsupported result instead of inventing behavior.

## 3. Hyprland Adapter Work

### Goal

Translate normalized motion descriptors into Hyprland animation categories, curves, and styles.

### Minimal adapter responsibilities

- map cubic bezier curves to Hyprland `bezier = ...` definitions
- convert `durationMs` into a Hyprland speed value with one explicit conversion policy
- choose the correct Hyprland animation category:
  - `window + enter` -> `windowsIn` or `fadeIn`
  - `window + exit` -> `windowsOut` or `fadeOut`
  - `window + move` -> `windows`
  - `workspace + enter/exit` -> `workspacesIn` / `workspacesOut`
  - `layer + enter/exit` -> `layersIn` / `layersOut` or `fadeLayers*`
- choose compositor style when needed:
  - `fade`
  - `popin <percent>`

### Important limitation

The adapter should not try to synthesize arbitrary keyframes.

If the descriptor is not expressible as a Hyprland category + curve + style combination, it should be rejected for the Hyprland backend.

## 4. Request / Evaluation Changes

### Goal

Give the style pipeline enough event context to match lifecycle selectors for the current compositor operation.

### Minimal request additions

The scene or motion-evaluation request layer should be able to carry transient context like:

- this window is entering
- this window is exiting
- this workspace is entering
- this workspace is exiting
- this window is moving due to retile
- this window just gained or lost focus

This is not timeline state.

It is just event classification for selector matching.

## 5. Diagnostics

### Goal

Make backend limitations explicit.

### Recommended diagnostics

- unsupported pseudo-class target
- unsupported transform function
- unsupported `animation-*` usage in delegated Hyprland mode
- conflicting motion declarations that cannot map to one Hyprland category/style
- unsupported multiple-transition combinations

Diagnostics should point to authored CSS locations when possible.

## 6. First Vertical Slice

The first implementation slice should be deliberately narrow.

### Supported

- `window:enter`
- `window:exit`
- `workspace:enter`
- `workspace:exit`
- `layer:enter`
- `layer:exit`
- `transition-duration`
- `transition-timing-function`
- `opacity: 0`
- `transform: scale(...)`

### Not yet supported

- `:move`
- `:focus` / `:unfocus`
- multiple concurrent effects
- arbitrary transforms
- `@keyframes`
- all `animation-*`

### Why this slice

It covers the most visible Hyprland categories:

- `windowsIn`
- `windowsOut`
- `fadeIn`
- `fadeOut`
- `workspacesIn`
- `workspacesOut`
- `layersIn`
- `layersOut`

without forcing `hypreact` to own a motion engine.

## 7. Suggested File-Level Work Split

Likely implementation areas in this repo:

- CSS selector/parser support
  - `crates/css/...`
- computed-style / selector matching with lifecycle context
  - `crates/scene/src/style_tree.rs`
  - `crates/scene/src/pipeline/mod.rs`
- backend-neutral motion descriptor extraction
  - likely a new small module near `crates/scene` or `crates/layout-runtime`
- Hyprland adapter translation and application
  - `crates/hypr-ffi/...`
  - `src/plugin.cpp`

## 8. Definition Of Done For The First Slice

The first slice should be considered done when:

- authored CSS can express enter/exit motion with standard transition syntax
- selector matching can see lifecycle pseudo-classes
- `hypreact` produces normalized motion descriptors for supported cases
- Hyprland receives equivalent category/curve/style settings from those descriptors
- unsupported authored motion is diagnosed explicitly
- no long-lived timeline or progress state is stored in `hypreact`

## Proposed Hyprland Mapping

Examples of plausible backend mappings:

- `transition-timing-function` -> Hyprland curve
- `transition-duration` -> Hyprland speed conversion
- `window:enter` with `opacity: 0` -> `fadeIn`
- `window:exit` with `opacity: 0` -> `fadeOut`
- `window:enter` with `transform: scale(0.87)` -> `windowsIn ... popin 87%`
- `window:exit` with `transform: scale(0.87)` -> `windowsOut ... popin 87%`
- `workspace:enter` / `workspace:exit` opacity fade -> `workspacesIn` / `workspacesOut` with `fade`
- layer-shell selectors -> `layers*` / `fadeLayers*`

This is a mapping layer, not literal CSS execution.

## Honest Compatibility Line

The backend contract should be explicit:

- CSS syntax is the authored source format
- only backend-supported subsets are guaranteed to execute
- unsupported motion should either:
  - be ignored with diagnostics
  - or fail validation for that backend

For Hyprland specifically:

- transitions can likely be mapped
- keyframes should not be promised unless Hyprland can truly execute them without `hypreact` owning motion state

## Backend Model

Long term, `hypreact` should expose a backend-neutral animation descriptor layer.

For example, a backend adapter may receive normalized motion intent like:

- target kind: `window`, `workspace`, `layer`
- lifecycle: `enter`, `exit`, `move`, `retile`, `focus`
- duration
- easing
- optional preset shape:
  - fade
  - popin(scale)
  - slide(offset)

The Hyprland backend can then translate that into Hyprland animation categories and styles.

Other compositors can support a different subset.

## Recommendation

The first implementation should:

- focus on CSS-authored transitions only
- support standard timing/easing syntax
- support a narrow set of backend-translatable visual intents
- avoid claiming keyframe support for Hyprland execution

That keeps the design honest while still moving animation authoring out of `hyprland.conf` and into layout-local CSS.

## Open Questions

- What selector/state model should express compositor lifecycle events such as `window:enter` or `workspace:exit`?
- Should unsupported CSS animation features be hard errors or soft diagnostics per backend?
- Should backend presets like Hyprland `popin 87%` be inferred from standard CSS transforms, or exposed through a separate adapter-only preset layer?
- Should `hypreact` support both:
  - compositor-delegated transitions
  - and a future fully-owned motion engine for backends that need true CSS keyframes?
