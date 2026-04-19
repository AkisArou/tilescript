# CSS

`tilescript` uses a `tilescript`-specific CSS subset for layout selection and layout geometry.

The language source of truth lives in `crates/css` (`tilescript-css`).

Unsupported selectors and properties fail clearly. They are not silently ignored.

## Ownership

`tilescript-css` owns:

- supported selector metadata
- supported property metadata
- parsing and compilation
- structured analysis and diagnostics

`tilescript-scene` consumes compiled stylesheet data for layout matching and geometry.

`tilescript` compositor adapters consume the resulting resolved layout tree.

## Pipeline

1. A selected config or layout app provides authored CSS.
2. `tilescript-css` parses and compiles the stylesheet.
3. `tilescript-scene` and related runtime code match selectors against the resolved layout tree.
4. Layout properties determine geometry.
5. The compositor adapter consumes the resulting layout output.

## Selector Targets

Supported selector targets:

- `workspace`
- `group`
- `window`

Invalid selector targets:

- `slot`

`slot` is not a valid CSS selector target and produces an error.

## Supported Selectors

Supported selectors:

- `workspace`
- `group`
- `window`
- `#id`
- `.class`
- exact-match window metadata selectors, including:
- `window[app_id="..."]`
- `window[title="..."]`
- `window[class="..."]`
- `window[instance="..."]`
- `window[role="..."]`
- `window[shell="..."]`
- `window[window_type="..."]`

Supported pseudo-classes:

- `:focused`
- `:floating`
- `:fullscreen`
- `:urgent`
- `:closing`
- `:enter-from-left`
- `:enter-from-right`
- `:exit-to-left`
- `:exit-to-right`

Notes:

- runtime window state class aliases like `.focused`, `.floating`, `.fullscreen`, and `.urgent` are still supported as runtime state classes
- selector matching is structural and metadata-based

## Properties

The canonical supported property list lives in `crates/css/src/language.rs`.

The groups below summarize the supported surface.

### Layout And Sizing

- `display`
- `box-sizing`
- `aspect-ratio`
- `position`
- `inset`
- `top`
- `right`
- `bottom`
- `left`
- `overflow`
- `overflow-x`
- `overflow-y`
- `width`
- `height`
- `min-width`
- `min-height`
- `max-width`
- `max-height`

### Flexbox And Alignment

- `flex-direction`
- `flex-wrap`
- `flex-grow`
- `flex-shrink`
- `flex-basis`
- `align-items`
- `align-self`
- `justify-items`
- `justify-self`
- `align-content`
- `justify-content`
- `gap`
- `row-gap`
- `column-gap`

### Grid

- `grid-template-rows`
- `grid-template-columns`
- `grid-auto-rows`
- `grid-auto-columns`
- `grid-auto-flow`
- `grid-template-areas`
- `grid-row`
- `grid-column`
- `grid-row-start`
- `grid-row-end`
- `grid-column-start`
- `grid-column-end`

Named grid lines, named spans, and `repeat(...)` are supported.

### Box Model

- `padding`
- `padding-top`
- `padding-right`
- `padding-bottom`
- `padding-left`
- `margin`
- `margin-top`
- `margin-right`
- `margin-bottom`
- `margin-left`

## Analysis And Diagnostics

`tilescript-css` exposes structured analysis for authored CSS.

That includes:

- structured diagnostics with ranges
- rule symbols

Current analysis diagnostics include:

- invalid syntax
- unsupported selectors
- unsupported properties
- unsupported values
- inapplicable properties
- unsupported selector attribute keys

## Example

```css
workspace {
  display: grid;
  grid-template-columns: 2fr 1fr;
  gap: 12px;
  padding: 12px;
}

#main {
  min-width: 0;
}

.stack {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

window {
  min-width: 0;
  overflow: hidden;
}
```

If the CSS surface grows later, it should grow with more layout primitives such as additional grid, sizing, spacing, alignment, overflow, and placement controls rather than presentation styling.

## Missing Layout Properties

Reference checked against `taffy 0.9.2` from `Cargo.lock`, specifically the public `taffy::style::Style` fields and `taffy::style::grid` types.

| Property | Why we want it | Taffy 0.9.2 backing | Notes |
| --- | --- | --- | --- |
| `inline-size` | logical size authoring | alias to `Style::size.width` | safe as a horizontal-writing-mode alias to `width` |
| `block-size` | logical size authoring | alias to `Style::size.height` | safe as a horizontal-writing-mode alias to `height` |
| `min-inline-size` | logical min constraint | alias to `Style::min_size.width` | horizontal-writing-mode alias |
| `min-block-size` | logical min constraint | alias to `Style::min_size.height` | horizontal-writing-mode alias |
| `max-inline-size` | logical max constraint | alias to `Style::max_size.width` | horizontal-writing-mode alias |
| `max-block-size` | logical max constraint | alias to `Style::max_size.height` | horizontal-writing-mode alias |
| `padding-inline` | logical inline spacing | alias to `Style::padding.left` and `Style::padding.right` | horizontal-writing-mode alias |
| `padding-inline-start` | logical inline start spacing | alias to `Style::padding.left` | horizontal-writing-mode alias |
| `padding-inline-end` | logical inline end spacing | alias to `Style::padding.right` | horizontal-writing-mode alias |
| `padding-block` | logical block spacing | alias to `Style::padding.top` and `Style::padding.bottom` | horizontal-writing-mode alias |
| `padding-block-start` | logical block start spacing | alias to `Style::padding.top` | horizontal-writing-mode alias |
| `padding-block-end` | logical block end spacing | alias to `Style::padding.bottom` | horizontal-writing-mode alias |
| `margin-inline` | logical inline spacing | alias to `Style::margin.left` and `Style::margin.right` | horizontal-writing-mode alias |
| `margin-inline-start` | logical inline start spacing | alias to `Style::margin.left` | horizontal-writing-mode alias |
| `margin-inline-end` | logical inline end spacing | alias to `Style::margin.right` | horizontal-writing-mode alias |
| `margin-block` | logical block spacing | alias to `Style::margin.top` and `Style::margin.bottom` | horizontal-writing-mode alias |
| `margin-block-start` | logical block start spacing | alias to `Style::margin.top` | horizontal-writing-mode alias |
| `margin-block-end` | logical block end spacing | alias to `Style::margin.bottom` | horizontal-writing-mode alias |
| `order` | explicit sibling ordering in flex/grid | no public `Style` field | implement in Tilescript by stable-sorting children before building the Taffy tree |
| `flex` | common flex item shorthand | expands to `Style::flex_grow`, `Style::flex_shrink`, `Style::flex_basis` | shorthand only |
| `flex-flow` | common flex container shorthand | expands to `Style::flex_direction` and `Style::flex_wrap` | shorthand only |
| `place-items` | concise grid/flex alignment authoring | expands to `Style::align_items` and `Style::justify_items` | shorthand only |
| `place-self` | concise item alignment authoring | expands to `Style::align_self` and `Style::justify_self` | shorthand only |
| `place-content` | concise content alignment authoring | expands to `Style::align_content` and `Style::justify_content` | shorthand only |
| `grid-template` | compact explicit grid authoring | expands to `Style::grid_template_rows`, `Style::grid_template_columns`, `Style::grid_template_areas`, `Style::grid_template_row_names`, and `Style::grid_template_column_names` | shorthand only |
| `grid-area` | compact item placement authoring | expands to `Style::grid_row` and `Style::grid_column` via `taffy::style::grid::GridPlacement` | shorthand only |
| `grid` | compact full grid authoring | expands to template fields plus `Style::grid_auto_flow`, `Style::grid_auto_rows`, and `Style::grid_auto_columns` | shorthand only |
| `subgrid` | modern nested grid composition | no public `Style` support in `taffy 0.9.2` | not currently feasible without engine work or a Taffy upgrade that exposes it |

Priority order for future layout-only expansion:

1. logical spacing and sizing aliases
2. `order`, `flex`, and `flex-flow`
3. `place-*`, `grid-area`, and `grid-template`
4. `grid`
5. `subgrid` only if the engine story becomes real

For editor and project-aware behavior, see `docs/css-lsp.md`.
