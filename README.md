# tilescript

`tilescript` is a layout runtime for Wayland compositors.

It lets you define workspace layouts in JSX/TSX, Lua, or Fennel together with CSS, then evaluates those layouts in Rust and hands the resulting placement decisions to a compositor adapter.

The current concrete adapter target is Hyprland.

## Features

- JSX/TSX, Lua, and Fennel layouts
- CSS-driven layout
- runtime layout selection by workspace index, workspace name, and monitor
- flex-inferred resize behavior with configurable resize step and minimum pane size
- Hyprland plugin integration
- CSS language server crates and a VS Code client package

## Examples

<details open>
<summary><code>layouts/master-stack/index.tsx</code></summary>

```tsx
import type { LayoutContext } from "@tilescript/sdk/layout";

import "./index.css";

export default function layout(ctx: LayoutContext) {
  return (
    <workspace>
      <slot take={1} class="master-slot" />

      {ctx.windows.length > 1 ? (
        <group class="stack-group">
          <slot class="stack-slot" />
        </group>
      ) : null}
    </workspace>
  );
}
```

</details>

<details>
<summary><code>layouts/master-stack/index.lua</code></summary>

```lua
local h = require("tilescript")

---@param ctx Tilescript.LayoutContext
return function(ctx)
  return h.workspace() {
    h.slot({
      take = 1,
      class = "master-slot",
    }),

    h.when(#ctx.windows > 1) {
      h.group({ class = "stack-group" }) {
        h.slot({
          class = "stack-slot",
        }),
      },
    },
  }
end
```

</details>

<details>
<summary><code>layouts/master-stack/index.fnl</code></summary>

```fennel
(local h (require "tilescript"))

(fn [ctx]
  ((h.workspace)
   [(h.slot {:take 1
             :class "master-slot"})
    ((h.when (> (# ctx.windows) 1))
     [((h.group {:class "stack-group"})
       [(h.slot {:class "stack-slot"})])])]))
```

</details>

`layouts/master-stack/index.css`

```css
workspace {
  display: flex;
  flex-direction: row;
  gap: 6px;
  padding: 6px;
  width: 100%;
  height: 100%;

  .master-slot {
    flex-basis: 0;
    flex-grow: 3;
    min-width: 0;
    min-height: 0;
  }

  .stack-group {
    display: flex;
    flex-direction: column;
    gap: 6px;
    flex-basis: 0;
    flex-grow: 2;
    min-width: 0;

    .stack-slot {
      flex-basis: 0;
      flex-grow: 1;
      min-height: 0;
    }
  }
}
```

## Config Layout

`tilescript` loads a config root directory.

At minimum, that directory contains:

- a config entry such as `config.ts`, `config.tsx`, `config.js`, `config.jsx`, `config.lua`, or `config.fnl`
- one or more layouts under `layouts/<name>/`
- optional root `index.css` for shared stylesheet rules

Typical project layout:

```text
config.ts
index.css
layouts/
  master-stack/
    index.tsx
    index.css
  primary-stack/
    index.lua
    index.css
```

To use it with Hyprland:

1. Build the plugin with `make hypr-plugin`.
2. Run `make hypr-plugin-snippet` and paste the printed `plugin` block into your Hyprland config.
3. Set `config_path` to your config root directory.
4. Reload Hyprland or reload the plugin after changing layouts or config.

## Docs

- `docs/config.md`
- `docs/hyprland.md`
- `docs/jsx.md`
- `docs/css.md`
- `docs/css-lsp.md`
- `docs/development.md`
- `docs/playground.md`

## Core Dependencies

`tilescript` builds on a small set of foundational libraries:

- `stylo`, `cssparser`, and `selectors` for CSS parsing and selector machinery
- `taffy` for layout computation
- `oxc` and `oxc_resolver` for JS/TS/TSX parsing and module graph analysis
- `rquickjs` for the native JS runtime
- `mlua` for the native Lua runtime
- `leptos` and `leptos_router` for the browser playground UI
- `wasm-bindgen` and `web-sys` for browser/WASM interop
- `monaco-editor`, `monaco-vim`, and `wasmoon` for the playground editor and browser Lua execution
