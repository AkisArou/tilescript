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

For editor and project-aware behavior, see `docs/css-lsp.md`.
