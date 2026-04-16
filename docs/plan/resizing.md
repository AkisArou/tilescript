# Resizing Plan

This document describes the current resize model in `tilescript` and the constraints it should keep.

## Current Model

- resize behavior is inferred from authored flex layout structure
- Rust owns resize candidate selection, share adjustment, and clamping
- the Hyprland plugin only forwards resize commands and applies the reevaluated result
- unsupported layout structures no-op instead of guessing

When the user resizes a tiled window, the runtime:

1. evaluates the current workspace layout and scene
2. derives a `PartitionTree` from the resolved scene
3. finds the nearest matching partition for the requested direction
4. adjusts internal share state for that partition
5. reevaluates the layout
6. applies the new geometry through the compositor adapter

## What Counts As A Partition

The current implementation only infers resize partitions from standard flex layout semantics.

A node is a candidate partition when:

- it resolves to a flex container
- it has at least two visible branches
- its `flex-direction` determines a meaningful main axis

Axis mapping:

- `row` and `row-reverse` -> horizontal partition
- `column` and `column-reverse` -> vertical partition

Branch sizing signals:

- positive `flex-grow` contributes to default branch share
- explicit main-axis size can make a branch effectively fixed
- `flex-grow: 0` is treated as fixed for resize purposes
- `flex-shrink: 0` is a strong signal that the branch should not give up space

## Stable Identity

Persistent resize state is keyed by authored structure, not by geometry.

Preferred identity sources are:

1. explicit authored node ids
2. stable structural ids from the resolved layout tree
3. deterministic structural fallback paths when the structure is still stable enough

Resize state must not be based on:

- current pixel rectangles
- current focus
- transient window order alone

If a partition does not have a stable identity, persistent resize should not depend on it.

## Runtime Knobs

The current authored config supports top-level resize tuning:

- `resize.stepPx`
- `resize.minBranchSizePx`

`resize.stepPx` is the requested pixel delta per resize command.

The runtime converts that delta into internal share-space for the active partition.

`resize.minBranchSizePx` is a practical minimum branch size on the partition main axis.

The runtime converts that floor into share-space and clamps shrinking branches against it.

## What Resize Does Not Use

The current model does not use:

- resize-specific JSX props
- resize-specific CSS properties
- compositor-side split ratios as the source of truth
- raw geometry heuristics as the primary semantic signal

That constraint is intentional. Resize semantics should come from authored structure plus normal layout style.

## Scope Limits

Supported well today:

- flex row partitions
- flex column partitions
- nested partition selection by nearest matching ancestor
- persistent resize state keyed by stable authored structure

Out of scope for the current model:

- arbitrary grid track resizing
- overlapping absolute-position layouts
- structures that only look splittable from geometry but do not expose a clear flex partition

Those should remain no-op rather than behave unpredictably.

## Design Rules To Keep

- keep resize policy in Rust
- keep the plugin thin
- prefer no-op over guessed behavior
- keep authored surface area minimal
- avoid introducing a second resize DSL

## Future Work

Reasonable follow-up work within the same design:

- improve inferred fixed/min/max handling where standard layout semantics are clear
- improve diagnostics for layouts that cannot produce a resize candidate
- extend partition inference only where authored structure gives a clear semantic boundary
