# CSS Refactor Plan

## Goals

- rely on `cssparser` / Stylo for CSS parsing as much as possible
- remove raw-text fallback parsing and any dead legacy `-tilescript-*` support
- keep Tilescript-owned code focused on:
  - selector subset validation
  - property subset validation
  - lowering parsed CSS into Tilescript's compact runtime model
- design parser architecture so modern CSS features like nesting can be added cleanly

## Current Problems

The current parser stack is split across multiple layers:

1. top-level stylesheet parsing with `cssparser::StyleSheetParser`
2. declaration parsing with Stylo `parse_property_declaration_list(...)`
3. a raw-text fallback scanner in `crates/css/src/parsing.rs`
4. a separate grid fallback parser in `crates/css/src/grid.rs`
5. custom tokenization / value compilation in `crates/css/src/tokenizer.rs`, `parse_values.rs`, and `compile.rs`

The main architectural issue is the raw fallback scanner. It manually scans declaration text for top-level `;` and `:` while skipping comments and strings. That is exactly the sort of parsing we should defer to crates rather than maintain ourselves.

## Refactor Direction

### Phase 1: Remove fallback and legacy layers

- remove `fallback_declarations(...)` and its raw scanning helpers
- remove `append_custom_tilescript_declarations(...)`
- remove `parse_grid_fallback_declarations(...)`
- make `parse_stylesheet(...)` rely only on Stylo declaration-list output for declarations

This leaves us with one declaration source of truth.

### Phase 2: Reduce string re-tokenization

Current `stylo_compile.rs` often serializes a Stylo value back to CSS text and then re-tokenizes it through our custom token model before compiling it.

That is still better than raw block scanning, but it is not ideal.

We should progressively replace string round-trips with direct lowering from Stylo value types into Tilescript runtime values for:

- sizing
- spacing
- overflow
- alignment
- flex
- grid

`grid` already mostly follows this direct-lowering approach and should be the model.

### Phase 3: Move stylesheet parsing closer to Stylo rule parsing

We should stop treating authored CSS as a flat custom stylesheet parser if we want modern features cleanly.

The better direction is:

- parse stylesheet rules through Stylo's stylesheet / rule parser stack
- validate the selector subset after parsing
- lower supported style rules into flat `CompiledStyleRule`s
- reject unsupported at-rules explicitly

This becomes especially important for nesting.

### Phase 4: Add nesting from crate-backed parsing

Stylo already has nesting-aware rule parsing and nested style rule structures.

When we adopt that parser path, nesting support should be implemented by:

- parsing nested style rules through Stylo
- flattening nested selectors into flat Tilescript rules
- preserving source locations for diagnostics and LSP

We should not implement nesting with a hand-rolled string transformer.

## Modern CSS Features Worth Designing For

### Good near-term candidates

- CSS nesting
- logical sizing and spacing properties
- shorthand expansion for layout properties
- `place-*` shorthands
- `grid-area`, `grid-template`, and `grid` shorthands

### Reasonable later candidates

- `@media` limited to viewport/layout-relevant queries if Tilescript wants authored responsive layouts
- `@supports` if we want authored progressive enhancement inside the CSS language
- cascade layers if stylesheet composition becomes complex
- `:is()` / `:where()` / `:has()` only if selector power is worth the complexity for authored layouts

### Not goals

- paint / decorative styling
- text styling
- motion / animation
- compatibility code for old parser layers
- any raw fallback parser that reparses authored CSS text manually

## Execution Notes

- prefer deleting code over preserving alternate parser paths
- if a feature cannot be supported through the crate-backed path cleanly, drop it until it can be supported cleanly
- keep the architecture single-path: one parser source of truth, one lowering pipeline
