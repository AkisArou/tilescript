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

1. A selected config or layout app provides CSS.
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
- CSS nesting is supported for nested style rules inside a qualified rule block
- nesting uses the same Tilescript selector front-end, including Tilescript-specific pseudo-classes
- nested selectors currently serialize canonically through the `selectors` crate, so flattened internal forms may include `:is(...)`

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
- `inline-size`
- `height`
- `block-size`
- `min-width`
- `min-inline-size`
- `min-height`
- `min-block-size`
- `max-width`
- `max-inline-size`
- `max-height`
- `max-block-size`

### Flexbox And Alignment

- `flex-direction`
- `flex-wrap`
- `flex-flow`
- `flex`
- `flex-grow`
- `flex-shrink`
- `flex-basis`
- `align-items`
- `place-items`
- `align-self`
- `place-self`
- `justify-items`
- `justify-self`
- `align-content`
- `place-content`
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
- `grid-template`
- `grid-template-areas`
- `grid`
- `grid-area`
- `grid-row`
- `grid-column`
- `grid-row-start`
- `grid-row-end`
- `grid-column-start`
- `grid-column-end`

Named grid lines, named spans, and `repeat(...)` are supported.

### Box Model

- `padding`
- `padding-inline`
- `padding-inline-start`
- `padding-inline-end`
- `padding-block`
- `padding-block-start`
- `padding-block-end`
- `padding-top`
- `padding-right`
- `padding-bottom`
- `padding-left`
- `margin`
- `margin-inline`
- `margin-inline-start`
- `margin-inline-end`
- `margin-block`
- `margin-block-start`
- `margin-block-end`
- `margin-top`
- `margin-right`
- `margin-bottom`
- `margin-left`

## Analysis And Diagnostics

`tilescript-css` exposes structured analysis for project CSS.

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

  > .focused {
    display: grid;
  }
}
```

If the CSS surface grows later, it should grow with more layout primitives such as additional grid, sizing, spacing, alignment, overflow, and placement controls rather than presentation styling.

## Missing Layout Properties

Reference checked against `taffy 0.9.2` from `Cargo.lock`, specifically the public `taffy::style::Style` fields and `taffy::style::grid` types.

| Property  | Why we want it                 | Taffy 0.9.2 backing                        | Notes                                                                         |
| --------- | ------------------------------ | ------------------------------------------ | ----------------------------------------------------------------------------- |
| `subgrid` | modern nested grid composition | no public `Style` support in `taffy 0.9.2` | not currently feasible without engine work or a Taffy upgrade that exposes it |

Priority order for future layout-only expansion:

1. `subgrid` only if the engine story becomes real

For editor and project-aware behavior, see `docs/css-lsp.md`.
