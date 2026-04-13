# CSS

`hypreact` uses a `hypreact`-specific CSS subset for layout selection, window styling,
and compositor-managed presentation.

The language source of truth lives in `crates/css` (`hypreact-css`).

Unsupported selectors and properties fail clearly. They are not silently ignored.

## Ownership

`hypreact-css` owns:

- supported selector metadata
- supported property metadata
- parsing and compilation
- structured analysis and diagnostics

`hypreact-scene` consumes compiled stylesheet data for layout matching and geometry.

`hypreact` compositor adapters consume the resulting layout and compositor-relevant style data.

## Pipeline

1. A selected config or layout app provides authored CSS.
2. `hypreact-css` parses and compiles the stylesheet.
3. `hypreact-scene` and related runtime code match selectors against the resolved layout tree.
4. Layout properties determine geometry.
5. The compositor adapter consumes compositor-backed presentation details such as borders and motion.

## Selector Targets

Supported selector targets:

- `workspace`
- `group`
- `window`

Invalid selector targets:

- `slot`

`slot` is not a valid CSS selector target and produces an error.

## Supported Selectors

Supported now:

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

The groups below summarize the currently supported surface.

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

### Box Model And Borders

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
- `border-width`
- `border-top-width`
- `border-right-width`
- `border-bottom-width`
- `border-left-width`
- `border-color`
- `border-top-color`
- `border-right-color`
- `border-bottom-color`
- `border-left-color`
- `border-style`
- `border-top-style`
- `border-right-style`
- `border-bottom-style`
- `border-left-style`

Runtime notes:

- border properties are partial because compositor consumption is selective
- `border-width` on `window` nodes can drive compositor-drawn border width where supported
- `border-style` can suppress compositor border edges whose style is `none`
- `border-color` is consumed for compositor borders where supported

### Window Presentation

- `appearance` partial
- `background` partial
- `background-color` partial
- `color` partial
- `opacity` partial
- `border-radius` partial
- `box-shadow` partial
- `backdrop-filter` planned
- `transform` partial

Notes:

- `appearance` currently accepts `auto` and `none`
- `appearance` is a window decoration-policy hint
- `opacity` currently affects compositor-managed presentation rather than arbitrary client content opacity
- `transform` is typed and sampled, but runtime visual support is still partial

### Motion

Supported motion properties:

- `animation`
- `animation-name`
- `animation-duration`
- `animation-timing-function`
- `animation-delay`
- `animation-iteration-count`
- `animation-direction`
- `animation-fill-mode`
- `animation-play-state`
- `transition`
- `transition-property`
- `transition-duration`
- `transition-timing-function`
- `transition-delay`
- `transition-behavior` partial
- `@keyframes`

Runtime notes:

- motion values are parsed into typed data, not kept as raw CSS strings
- `transition-behavior` is currently accepted for compatibility but ignored by runtime compilation
- compositor-delegated animation support is intentionally narrower than the parsed CSS surface
- see `docs/plan/animations.md` for the current delegated-animation design

## Analysis And Diagnostics

`hypreact-css` exposes structured analysis for authored CSS.

That includes:

- structured diagnostics with ranges
- rule and `@keyframes` symbols
- `animation-name` references

Current analysis diagnostics include:

- invalid syntax
- unsupported selectors
- unsupported properties
- unsupported values
- inapplicable properties
- unknown `animation-name`
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
  border-width: 2px;
}
```

For editor and project-aware behavior, see `docs/css-lsp.md`.
