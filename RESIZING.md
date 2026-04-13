# RESIZING

## Goal

Design a resize system for `hypreact` that is correct for CSS-authored layouts, not a port of classic tiling-WM split logic.

The resize system must:

- work with authored JSX/TSX layouts and CSS-driven scene computation
- respect authored constraints instead of fighting them
- keep resize policy in Rust
- keep the Hyprland plugin as a thin adapter only
- avoid compatibility layers and legacy abstractions
- be stable across reevaluation of dynamic layouts
- degrade cleanly to no-op when a layout or edge is not resizable

This document describes the target design, not an incremental migration plan.

## TODO

- make resize step size configurable instead of hardcoding the current step units used by the plugin/runtime bridge

## Product View

`hypreact` is not a fixed tree tiler with a small number of built-in layout algorithms.

`hypreact` is:

- an authored layout runtime
- a CSS/layout engine
- a scene evaluator
- a small WM model for focus, movement, and layout-aware decisions

Resize therefore cannot be modeled as:

- "always mutate compositor-side split ratios"
- "always resize the nearest visual neighbor"
- "always infer a master/stack or dwindle split from current window rectangles"

Those approaches are too tied to built-in tilers and break down for dynamic authored layouts.

The correct model is:

- Rust identifies the authored partition boundary the user is trying to push
- Rust stores an adjustment against a stable authored partition identity
- authored layout evaluation decides how that adjustment changes the final scene

In short:

- resize changes authored layout inputs
- authored layout inputs produce a new scene
- the plugin only applies the resulting placement

## Core Principles

### 1. Resize is semantic, not purely geometric

Geometry is an observation of the current result.

Resize should not directly mutate geometry and should not try to preserve geometry as the primary source of truth.

Instead, resize should target a semantic partition in the authored layout model.

### 2. Resize must respect authored intent

A layout may include:

- fixed-width sidebars
- fixed-height rows
- min/max-constrained regions
- windows pinned to specific tracks
- decorative or structural wrappers that are not resizeable
- dynamic groups that appear/disappear with window count

The resize system must allow layouts to say:

- this edge is resizable
- this edge is not resizable
- resize along this axis uses these weights
- these branches are fixed or bounded

If the authored layout does not expose a resizable partition, resize should no-op.

### 3. Stable identity is mandatory

Resize state must not be attached to pixels or transient geometry.

Resize state must be attached to stable authored partition identities.

Without stable ids, resize will drift or reset unpredictably when the scene is reevaluated.

### 4. Focus and resize are related but not identical

Focus navigation and resize both depend on scene structure.

They should share the same evaluated scene as input, but they should not be forced into one identical tree.

Recommended split:

- `FocusTree` for navigation
- `PartitionTree` for resize

The focus tree answers:

- which window is adjacent in a direction?

The partition tree answers:

- which partition boundary owns the edge the user is trying to move?
- is that partition adjustable?
- which branches should grow and shrink?

### 5. No legacy compatibility layer

Do not preserve or extend the current split-weight work just because it exists.

If parts of the existing resize code are useful, they should be reintroduced only as part of the final clean model.

The shipped design should look like it was designed for `hypreact` from the start.

## What Resize Means In Hypreact

When the user asks to resize the focused tiled window in a direction, the system should:

1. evaluate the authored layout and scene for the current workspace
2. derive a resize-oriented partition model from that evaluated scene
3. find the most specific authored partition boundary that corresponds to the requested directional resize
4. verify that the partition is resizable and that the focused branch can trade space with a neighbor on that side
5. mutate resize state for that authored partition
6. reevaluate the authored layout with the updated resize state
7. apply the new geometry to Hyprland

No compositor-side resize math should be the source of truth for authored tiled layouts.

## Scene-Derived Partition Model

Resize needs a first-class Rust structure derived from layout evaluation.

Recommended name:

- `PartitionTree`

Each partition node should represent a meaningful spatial split in the authored result.

### Partition node contents

Each partition node should contain at least:

- `partition_id`
  - stable identity used for persistent resize state
- `axis`
  - horizontal or vertical
- `children`
  - ordered child branches
- `rect`
  - resolved visual rectangle of the partition
- `descendant_window_ids`
  - all windows under this partition
- `resizable`
  - whether this partition accepts resize adjustments
- `resize_policy`
  - metadata for how adjustments should be interpreted
- `constraints`
  - optional min/max/fixed metadata for branches

Each child branch should contain at least:

- stable branch identity within the partition
- child rect
- descendant window ids
- branch kind
  - window leaf
  - nested partition
  - possibly structural branch with no direct window leaf

When evaluated slot/content expansion introduces non-semantic wrappers, partition derivation should flatten those wrappers if they do not represent an authored resize boundary of their own. A single authored stack slot that expands into multiple visible windows must still produce one resizable branch per visible window for the enclosing partition.

### Source of the partition tree

The partition tree should be derived from the evaluated authored layout, not from Hyprland state.

The best primary source is the resolved layout/scene structure, because it retains authored grouping and stable node metadata that raw rectangles alone cannot recover.

The partition tree may use geometry from the scene, but its topology should come from authored layout structure wherever possible.

Authored `-hypreact-branch-share` and `-hypreact-branch-min-share` / `-hypreact-branch-max-share` values are semantic share units. The runtime may scale those authored values into a higher-resolution internal representation so resize steps can be fine-grained without requiring noisy CSS values like `12`, `24`, or `36` for ordinary authored layouts.

## Stable Identity Rules

Stable partition identity is the foundation of correct resizing.

`partition_id` should be derived from authored structure, not generated from visual order alone.

Preferred identity sources, in order:

1. explicit authored node ids
2. stable slot/group/workspace ids in the resolved layout tree
3. deterministic structural path within the resolved authored tree, only if stable across reevaluation

Not acceptable as primary ids:

- current pixel rectangles
- current child index alone
- current window order alone
- any id that changes when sibling windows are added/removed unless that instability is intentional and acceptable

If a partition cannot be given a stable identity, it should not support persistent resizing.

### Partition id invariants

A valid `partition_id` must satisfy all of the following:

- it identifies the same authored structural partition across reevaluation when the logical partition still exists
- it does not depend on current pixel geometry
- it does not depend on current focused window
- it does not depend on transient sibling order alone
- it survives ordinary window churn when the authored partition itself still exists

The practical rule is:

- partition identity should come from authored structure first
- branch membership can be dynamic
- partition identity itself should not be dynamic unless the authored partition truly appears or disappears

Examples of good ids:

- `frame`
- `main-columns`
- `dashboard/right-column`

Examples of bad ids:

- `x=800:w=400`
- `child-1`
- `current-right-pane`
- `focused-neighbor-split`

### Branch identity invariants

Branch ids matter too, especially for constraints and future persistence.

A valid branch identity should:

- be stable within its parent partition
- come from authored child identity if available
- otherwise use a deterministic structural position within that authored partition

Branch ids may be less stable than partition ids when the authored branch set itself changes, but they still must not be derived from geometry.

## Authored Resize Contract

Resize must not depend on JSX props, special CSS properties, or other resize-only annotations.

The authored contract should stay aligned with what `hypreact` already is:

- JSX/TSX describes structure
- standard CSS describes layout behavior
- Rust evaluates the resulting structure and style

That means resize semantics should come from:

- stable node identity in authored structure
- standard computed layout style such as `display`, `flex-direction`, `flex-grow`, `flex-shrink`, `flex-basis`, `width`, and `height`
- the resolved scene topology

Not from ad-hoc JSX props that turn the layout tree into a custom WM DSL.

### Recommended contract

The runtime should infer resize semantics from normal authored flex layout semantics:

- flex containers imply candidate partitions
- `flex-direction` implies partition axis
- `flex-grow` implies default resize share
- explicit main-axis sizes and zero-grow branches imply fixed branches
- stable node ids still provide structural identity when available

That keeps resize faithful to authored layout without introducing a second resize DSL.

### Auto-Infer Plan

For the current clean slice, resize should be inferred from standard flex layout only.

Inference rules:

- a node forms a resize partition when it resolves to a flex container with at least two visible descendant branches
- partition axis is inferred from `flex-direction`
  - `row` / `row-reverse` => horizontal partition
  - `column` / `column-reverse` => vertical partition
- partition identity comes from authored node id when present, otherwise deterministic structural identity in the resolved tree
- branch identity comes from authored child id when unique, otherwise leaf window id, otherwise deterministic structural fallback
- default branch shares come from `flex-grow`
  - positive grow factors are scaled into internal share units
  - missing grow falls back to equal-share behavior
- branch fixedness is inferred from ordinary layout behavior
  - explicit main-axis size like `width` on a row partition or `height` on a column partition makes the branch fixed
  - `flex-grow: 0` makes the branch fixed for resize purposes
  - `flex-shrink: 0` remains a strong signal that the branch should not give up space
- a partition is adjustable only when at least two sibling branches under it are not fixed

This gives a concrete first implementation with no resize-specific authored surface area.

### Why auto-infer is the right surface

- node structure remains structural
- layout behavior remains described by normal CSS
- authored layouts do not need WM-specific resize annotations
- resize behavior follows the same information that already drives scene computation

### Scope limits

The current implementation intentionally limits inference to cases we can model correctly:

- flex row and flex column partitions
- stable structural ids from authored nodes when available
- default shares inferred from `flex-grow`
- fixed branches inferred from standard size and flex behavior

The system should not yet try to infer resize semantics for:

- arbitrary grid track resizing
- overlapping absolute-position layouts
- layouts whose visible geometry looks splittable but whose structure does not expose a clear flex partition

Those should remain no-op rather than guessed incorrectly.

### Runtime knobs

The authored config may expose top-level resize behavior knobs for runtime tuning without changing layout structure:

- `resize.step_px`
  - requested pixel resize increment per command
  - this is the equivalent of i3-style configurable resize amount
- `resize.min_branch_size_px`
  - practical minimum inferred branch size on the partition main axis
  - used only to derive inferred minimum share constraints for flex-inferred partitions

These are runtime policy controls, not per-node layout metadata.

### No implicit resize from random geometry

Layouts should not become resizable just because rectangles happen to line up.

The inference source of truth should be resolved authored flex structure, not raw geometry heuristics.

That means:

- geometry is used to apply the result, not to decide semantics
- resize remains deterministic across reevaluation
- unsupported structures fail soft with no candidate

## Resize State Model

Resize state should live in the core model as persistent authored-layout adjustment state.

Recommended shape:

- `layout_adjustments_by_workspace`
  - scoped per workspace
- keyed by `partition_id`
- containing branch sizing data for that partition

A partition adjustment should represent authored intent, not pixels.

Recommended representation:

- normalized weights or fractions per branch

Examples:

- `partition_id -> [2, 1]`
- `partition_id -> [0.25, 0.75]`

The exact numeric form is less important than these invariants:

- easy to increment/decrement by one step
- deterministic serialization
- easy to clamp
- stable across reevaluation
- independent of monitor pixel size

### Recommended choice

Use rational or fixed-point normalized shares in Rust, not compositor pixels.

That gives:

- deterministic state
- DPI independence
- easier min/max clamping
- clean authored consumption

## Resize Resolution Algorithm

When asked to resize the focused window in `left`, `right`, `up`, or `down`:

### Step 1. Evaluate current workspace scene

Obtain the evaluated resolved layout and computed scene for the active workspace.

From that evaluation derive:

- `FocusTree`
- `PartitionTree`

Both should come from the same scene evaluation pass.

### Step 2. Locate the focused leaf

Find the leaf or branch in the partition tree that contains the focused window.

If no such leaf exists:

- no-op

### Step 3. Walk ancestors to find a matching partition

Map requested direction to resize axis:

- `left/right` => horizontal partition
- `up/down` => vertical partition

Walk from the focused leaf upward until finding the nearest ancestor partition where:

- partition axis matches the requested direction
- the focused window belongs to one child branch
- there is an adjacent sibling branch on the requested side
- the partition is marked resizable

This nearest matching ancestor is the candidate resize anchor.

This is the key algorithmic rule.

It gives behavior that feels natural in nested CSS layouts:

- inner partitions resize before outer partitions
- if no inner partition matches, the search climbs outward
- resize respects real authored grouping

### Step 4. Resolve grow and shrink branches

Within the chosen partition:

- identify the branch containing the focused window
- identify the immediate sibling branch on the requested side

Directional mapping:

- `right` or `down`
  - grow the focused branch
  - shrink the adjacent sibling on that side
- `left` or `up`
  - grow the focused branch
  - shrink the adjacent sibling on that side

Equivalently:

- the focused branch grows toward the requested edge
- the neighboring branch on that edge gives up space

If there is no sibling on that side:

- continue climbing to a broader matching ancestor
- if none exists, no-op

### Step 5. Validate constraints

Before mutating state, validate:

- partition is resizable
- target sibling exists
- both branches are adjustable
- shrinking branch is above minimum
- growing branch is below maximum, if any
- fixed branches cannot be changed

If validation fails:

- no-op

### Step 6. Apply one logical resize step

Mutate the chosen partition's branch shares by one logical step.

The user-facing step may be defined in pixels, but the runtime still applies resize in authored-layout share space.

The runtime may source it from top-level config:

- `resize.step_px`

The runtime converts that requested pixel step into share-space against the active partition size and current share total.

The step value should be globally consistent for a running config.

### Step 7. Clamp against inferred practical minimums

For flex-inferred partitions, practical minimum branch size may be configured in pixels:

- `resize.min_branch_size_px`

The runtime converts that floor into share-space using the current partition size and total default branch shares, then clamps shrinking branches against the resulting minimum share.

This keeps runtime state share-based while letting the usability floor be expressed in pane-sized terms.

### Step 7. Reevaluate scene

Reevaluate the authored layout with updated adjustment state.

The authored layout uses the new partition shares to produce a new scene.

### Step 8. Apply placement

The plugin applies the resulting new geometries.

The plugin never decides which partition to resize and never computes the resize shares itself.

## Why This Algorithm Is Correct

This algorithm matches how nested authored layouts behave.

Example:

- workspace split into main column and side column
- side column split into top and bottom
- focused window is bottom-right

If resizing `up`:

- do not resize the outer left/right split
- resize the nearest vertical partition inside the side column

If resizing `left`:

- do not resize the inner top/bottom split
- climb to the outer horizontal partition and resize that

This is the same reason a focus tree works well for directional navigation: nearest structurally-relevant ancestor wins.

But resize needs partition ownership and constraints, so it needs its own tree.

## Fixed And Dynamic CSS Cases

### Fixed sidebar

If a layout intentionally creates a fixed-width sidebar:

- that partition can be marked non-resizable
- resize toward that edge should no-op

This is correct behavior.

The system should not try to outsmart authored intent.

### Dynamic layouts

If a layout changes structure based on window count or matching rules:

- resize state should remain attached to stable partition ids that still exist
- state for vanished partitions should remain inert or be garbage-collected later
- if a partition disappears and later reappears with the same stable id, the adjustment may be reused if that remains coherent

This is another reason stable authored ids matter more than geometry.

### Windows pinned to specific columns

If authored logic places certain windows into specific regions:

- resize still targets the authored partition, not the window role directly
- the authored layout remains free to keep that window in its designated region while honoring partition size changes

## Partition Tree vs Pure Geometry Heuristics

Pure geometry heuristics are tempting because they seem generic.

For example:

- pick the nearest adjacent window
- infer a split from overlapping edges
- resize based on rectangle relationships only

This should not be the primary design.

Why pure geometry fails:

- it cannot distinguish authored wrappers from real partitions
- it cannot distinguish fixed from adjustable regions
- it becomes unstable under dynamic layout changes
- multiple nested partitions can produce the same local geometry but different intended resize behavior
- it does not preserve authored semantics

Pure geometry may still be useful as a later fallback when no authored resize metadata exists, but it should not define the core model.

## Recommended Data Structures

### In `core`

Keep only pure model/state/algorithm data:

- `ResizeDirection`
- persistent workspace-scoped resize state
- partition adjustment state keyed by stable `partition_id`
- pure resize selection and mutation logic

Possible types:

- `PartitionId`
- `PartitionAxis`
- `PartitionNode`
- `PartitionBranch`
- `PartitionTree`
- `PartitionAdjustment`
- `ResizeCandidate`

Recommended shapes:

```rust
pub struct PartitionId(pub String);

pub enum PartitionAxis {
    Horizontal,
    Vertical,
}

pub struct PartitionConstraints {
    pub min_share: Option<u32>,
    pub max_share: Option<u32>,
    pub fixed: bool,
}

pub struct PartitionBranch {
    pub branch_id: String,
    pub rect: LayoutRect,
    pub descendant_window_ids: Vec<WindowId>,
    pub constraints: PartitionConstraints,
}

pub struct PartitionNode {
    pub partition_id: PartitionId,
    pub axis: PartitionAxis,
    pub rect: LayoutRect,
    pub branch_ids: Vec<String>,
    pub branches: Vec<PartitionBranch>,
    pub adjustable: bool,
}

pub struct PartitionTree {
    pub root_partition_ids: Vec<PartitionId>,
    pub partitions: BTreeMap<PartitionId, PartitionNode>,
    pub window_to_partition_path: BTreeMap<WindowId, Vec<PartitionId>>,
}

pub struct PartitionAdjustment {
    pub branch_shares: Vec<u32>,
}

pub struct WorkspaceResizeState {
    pub adjustments_by_partition_id: BTreeMap<PartitionId, PartitionAdjustment>,
}

pub struct ResizeCandidate {
    pub partition_id: PartitionId,
    pub grow_branch_index: usize,
    pub shrink_branch_index: usize,
}
```

Notes:

- `window_to_partition_path` makes nearest-ancestor lookup cheap and deterministic
- `branch_shares` should be fixed-point integer shares, not floats
- branch ids are useful even if the first implementation only resizes adjacent sibling indices

### In `layout-runtime`

Own orchestration and scene-derived structures:

- derive partition tree from evaluated layout/scene
- return resize candidate info and apply adjustment mutations through the model
- expose typed facade functions for plugin consumption

Recommended runtime-facing structures:

```rust
pub struct EvaluatedWorkspaceLayout {
    pub resolved_root: ResolvedLayoutNode,
    pub scene_root: LayoutSnapshotNode,
    pub focus_tree: FocusTree,
    pub partition_tree: PartitionTree,
}
```

### In `hypr-ffi`

Bindings/marshaling only:

- `resize_direction(...) -> changed`
- no resize policy

### In the plugin

Glue only:

- sync focused window/workspace
- call typed Rust resize API
- recalculate workspace immediately on success

## Authoring API Direction

The authored layout runtime should expose resize-aware layout semantics without introducing WM-specific JSX props.

The right direction is:

- structure in JSX/TSX
- resize semantics in CSS-recognized layout properties
- stable ids/classes in authored structure so style can target the right nodes
- runtime state available during layout evaluation so authored layout and style can consume current partition adjustments

This is better than trying to reverse-engineer resize semantics after CSS has already flattened them, and better than polluting JSX with special-purpose resize props.

Ideal authored experience:

- authored layout defines stable structural nodes with ids/classes
- CSS declares which structural groups participate as partitions and how they size children
- runtime receives current partition adjustments
- layout evaluation combines structure, style, and adjustments into a new scene
- scene output naturally reflects resize state

### Proposed engine surface

The exact property names can be decided during implementation, but the engine should support style concepts equivalent to:

- partition axis
- partition adjustability
- branch share default
- branch min share
- branch max share
- branch fixed sizing

These should be normal style concepts understood by the `hypreact` engine, not ad-hoc plugin metadata.

### First implementation scope for style semantics

The first clean vertical slice should intentionally support only the subset we can model correctly:

- flex row partitions
- flex column partitions
- stable partition ids from node `id`
- branch shares from engine-managed resize state

It should explicitly not attempt to fully solve on day one:

- CSS grid track resizing
- arbitrary overlapping absolute-position layouts
- inferred resize semantics for layouts with no structural ids

That is not a compromise in architecture. It is a clean first slice of the final architecture.

### Proposed runtime surface

The layout evaluation context should eventually expose resolved resize adjustments in a way the authored runtime can consume while building the resolved tree.

That likely means:

- current workspace resize state is part of layout evaluation input
- resolved nodes retain enough stable metadata for later partition-tree derivation

The important rule is:

- the authored runtime does not perform WM policy
- it only consumes adjustment state as layout input

## Rust API Sketch

The following API shape is a good target.

### In `core`

```rust
pub enum ResizeDirection {
    Left,
    Right,
    Up,
    Down,
}

pub fn select_resize_candidate(
    partition_tree: &PartitionTree,
    focused_window_id: &WindowId,
    direction: ResizeDirection,
) -> Option<ResizeCandidate>;

pub fn apply_resize_step(
    state: &mut WorkspaceResizeState,
    partition_tree: &PartitionTree,
    candidate: &ResizeCandidate,
    step_units: u32,
) -> bool;

pub fn gc_resize_state(
    state: &mut WorkspaceResizeState,
    partition_tree: &PartitionTree,
);
```

### In `layout-runtime`

```rust
pub fn evaluate_workspace_layout(
    service: &mut LayoutRuntimeService,
    model: &WmModel,
    workspace_id: &WorkspaceId,
) -> Result<EvaluatedWorkspaceLayout, LayoutRuntimeError>;

pub fn resize_direction(
    service: &mut LayoutRuntimeService,
    model: &mut WmModel,
    workspace_id: &WorkspaceId,
    focused_window_id: &WindowId,
    direction: ResizeDirection,
) -> Result<bool, LayoutRuntimeError>;
```

Behavior:

- evaluate the workspace
- derive the partition tree
- select the nearest valid resize candidate
- mutate workspace-scoped resize state
- return whether anything changed

### In `hypr-ffi`

```rust
pub extern "C" fn hypreact_runtime_resize_direction(
    handle: *mut HypreactRuntimeHandle,
    direction: *const c_char,
) -> HypreactStatusResult;
```

### In the plugin

Required flow only:

1. sync focused window and workspace
2. call typed Rust resize entrypoint
3. if changed, recalculate current workspace immediately

No plugin-side resize logic.

## Worked Examples

### Example 1. Master and stack

Structure:

- workspace root
- group `#frame`
- child `#master`
- child `.stack-group`

Style semantics:

- `#frame` is a horizontal adjustable partition
- `#master` and `.stack-group` are its two branches
- initial share is, for example, `3:2`

Behavior:

- focused master + `resize right` grows master and shrinks stack
- focused stack + `resize left` grows stack and shrinks master
- `resize up/down` inside stack does nothing unless `.stack-group` itself forms a vertical adjustable partition with multiple branches

### Example 2. Fixed sidebar

Structure:

- workspace root
- group `#sidebar`
- group `#content`

Style semantics:

- root is horizontal
- `#sidebar` has fixed width
- root partition is not adjustable, or the sidebar branch is fixed

Behavior:

- focused content + `resize left` no-ops
- focused sidebar + `resize right` no-ops

This is correct because the authored layout says the sidebar is fixed.

### Example 3. Nested dashboard

Structure:

- root horizontal partition
- left column
- right column
- right column contains top and bottom groups stacked vertically

Behavior:

- focused bottom-right + `resize up`
  - choose inner vertical partition in right column
- focused bottom-right + `resize left`
  - climb to outer horizontal partition
- focused left column + `resize down`
  - no-op unless the left column itself contains a vertical adjustable partition

### Example 4. Dynamic stack count

Structure:

- master area
- stack area whose children depend on window count

Behavior:

- top-level master/stack partition keeps a stable `partition_id`
- stack-inner partition may appear only when stack has 2+ windows
- if stack-inner partition disappears, its saved adjustment becomes inert
- if it reappears with the same stable id, its saved state may be reused

This gives predictable behavior without tying resize to transient pixel geometry.

## Implementation Phases

The implementation should still happen in phases, but each phase must preserve the final architecture rather than introducing temporary compatibility layers.

### Phase 1. Core partition model

- replace legacy resize state with partition-oriented resize state
- add `PartitionTree`, `PartitionNode`, `PartitionBranch`, `ResizeCandidate`
- store resize state per workspace keyed by `partition_id`

Exit criteria:

- no old split-weight-specific naming remains in the model
- core types reflect the final partition-based design

### Phase 2. Scene-derived partition tree for flex layouts

- derive partitions from resolved scene/layout for flex row and flex column groups
- use stable node ids from authored structure
- ignore unsupported structures cleanly

Exit criteria:

- runtime can produce a `PartitionTree` for current authored test layouts
- unsupported layouts fail soft with no candidate instead of incorrect behavior

### Phase 3. Resize mutation path in Rust

- add `select_resize_candidate`
- add `apply_resize_step`
- add `resize_direction(...)` orchestration in `layout-runtime`

Exit criteria:

- Rust tests show correct nearest-ancestor selection and share mutation

### Phase 4. FFI and plugin integration

- add typed resize entrypoint in `hypr-ffi`
- add thin `hypreact:resizewindow` dispatcher in plugin
- recalculate workspace immediately on successful resize

Exit criteria:

- live tiled resize works on supported flex-based layouts

### Phase 5. Style property support

- parse and evaluate engine-level resize style properties
- support explicit partition behavior beyond pure flex inference
- add min/max/fixed branch semantics

Exit criteria:

- authored CSS can intentionally declare resize semantics

### Phase 6. Broader layout support

- extend partition derivation for grid or other supported layout models where semantics are clear
- avoid broad heuristic fallbacks that weaken correctness

Exit criteria:

- broader support without compromising the partition-based architecture

## Edge Semantics

### What should happen when resizing toward the outside edge?

If the focused branch has no sibling on the requested side in the nearest matching partition:

- keep climbing to broader matching partitions
- if none qualifies, no-op

This avoids arbitrary wraparound or mirror behavior.

### Should resize ever wrap?

No.

Focus may wrap.
Resize should not.

Resize is a mutation of a concrete owned boundary, not a navigation action.

### Should resizing a window directly resize that window?

Only indirectly.

The mutation target is always a partition boundary, not a window rectangle.

The focused window merely selects which partition branch should grow.

## Animation Implications

Because resize produces a normal reevaluated scene and then normal placement updates:

- tiled resize animation should come from Hyprland target updates as with other layout changes
- resize should not rely on warp/no-animation paths except when explicitly intended

This keeps resize behavior consistent with open/close/move relayout behavior.

## Testing Strategy For The Final Design

The final design should be testable mostly in Rust.

### Unit tests

Test pure resize selection logic:

- nearest matching ancestor partition wins
- climbs outward when inner partition does not match axis
- fixed/non-resizable partition returns no candidate
- sibling-side selection is correct
- min/max clamping works

### Runtime tests

Test scene-derived partition tree on real authored layouts:

- master-stack
- nested rows/columns
- fixed sidebar with resizable main area
- dynamic layouts where branch count changes

### Live validation

Validate after clean Hyprland restart:

- resize changes intended region only
- fixed regions stay fixed
- animations are preserved
- repeated resize steps remain stable
- close/open after resize preserves coherent state

## Explicit Non-Goals

This design should not do any of the following:

- reimplement Hyprland master/dwindle resize algorithms inside the plugin
- mutate compositor-side split trees as the source of truth
- infer resize state only from current rectangles
- preserve generic WM compatibility abstractions
- keep legacy resize code just because it already exists
- force all authored layouts to become implicitly resizable

## Final Recommendation

The best resizing model for `hypreact` is:

- explicit authored partitions with stable ids
- a scene-derived `PartitionTree` in Rust
- nearest-matching-ancestor resize selection
- persistent partition adjustments stored in workspace state
- reevaluation of authored layout using those adjustments
- plugin as thin sync + apply adapter only

The authored expression of those partitions should live in structure plus CSS-recognized style semantics, not JSX props.

This is the cleanest design because it matches what `hypreact` actually is:

- not a classic built-in tiler
- but an authored layout engine whose window management behavior must remain faithful to authored structure

## Short Version

Focus should use a focus tree.

Resize should use a partition tree.

Both should be derived from the same evaluated scene.

Resize must target stable authored partitions, not raw window rectangles.
