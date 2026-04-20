# tilescript

`tilescript` is a layout runtime designed for Wayland compositors and tiling window managers.

It lets you define workspace layouts in JSX/TSX, Lua, or Fennel together with CSS, then evaluates those layouts in Rust and hands the resulting placement decisions to a window manager or compositor adapter.

> Current adapter target: `Hyprland`

> Playground: <https://akisarou.github.io/tilescript/>

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

If `config_path` is omitted, the default config root is `~/.config/tilescript`.

At minimum, that directory contains:

- a config entry such as `config.{ts, tsx, js, jsx, lua, fnl}`.
- one or more layouts under `layouts/<name>/`
- optional root `index.css` for shared stylesheet rules

For starter layouts and config skeletons, see **`examples/`**.

Typical project layout:

```text
config.ts
index.css
layouts/
  master-stack/
    index.tsx
    index.css
  dwindle/
    index.lua
    index.css
```

## Hyprland

To use `tilescript` with Hyprland:

1. Build the plugin with `make hypr-plugin`.
2. Run `make hypr-plugin-snippet` and paste the printed `plugin` block into your Hyprland config.
3. Set `config_path` to your config root directory.
4. Reload Hyprland or reload the plugin after changing layouts or config.

Example Hyprland config:

```ini
plugin = /absolute/path/to/tilescript-hypr.so

plugin {
  tilescript-hypr {
    config_path = /absolute/path/to/your/tilescript-config/
  }
}


general {
    # disable gaps. those should be better configured via CSS
    gaps_in = 0
    gaps_out = 0

    layout = tilescript
}
```

Related keybindings:

```ini
$mainMod = ALT

bind = $mainMod, h, tilescript-hypr, focus, l
bind = $mainMod, j, tilescript-hypr, focus, d
bind = $mainMod, k, tilescript-hypr, focus, u
bind = $mainMod, l, tilescript-hypr, focus, r

bind = $mainMod CTRL, h, tilescript-hypr, resize, l
bind = $mainMod CTRL, j, tilescript-hypr, resize, d
bind = $mainMod CTRL, k, tilescript-hypr, resize, u
bind = $mainMod CTRL, l, tilescript-hypr, resize, r

bind = $mainMod SHIFT, h, tilescript-hypr, move, l
bind = $mainMod SHIFT, j, tilescript-hypr, move, d
bind = $mainMod SHIFT, k, tilescript-hypr, move, u
bind = $mainMod SHIFT, l, tilescript-hypr, move, r

bind = $mainMod, 1, tilescript-hypr, workspace, 1
bind = $mainMod, 2, tilescript-hypr, workspace, 2
bind = $mainMod, 3, tilescript-hypr, workspace, 3
bind = $mainMod, 4, tilescript-hypr, workspace, 4
bind = $mainMod, 5, tilescript-hypr, workspace, 5
bind = $mainMod, 6, tilescript-hypr, workspace, 6
bind = $mainMod, 7, tilescript-hypr, workspace, 7
bind = $mainMod, 8, tilescript-hypr, workspace, 8
bind = $mainMod, 9, tilescript-hypr, workspace, 9
bind = $mainMod, 0, tilescript-hypr, workspace, 10

bind = $mainMod SHIFT, 1, tilescript-hypr, movetoworkspace, 1
bind = $mainMod SHIFT, 2, tilescript-hypr, movetoworkspace, 2
bind = $mainMod SHIFT, 3, tilescript-hypr, movetoworkspace, 3
bind = $mainMod SHIFT, 4, tilescript-hypr, movetoworkspace, 4
bind = $mainMod SHIFT, 5, tilescript-hypr, movetoworkspace, 5
bind = $mainMod SHIFT, 6, tilescript-hypr, movetoworkspace, 6
bind = $mainMod SHIFT, 7, tilescript-hypr, movetoworkspace, 7
bind = $mainMod SHIFT, 8, tilescript-hypr, movetoworkspace, 8
bind = $mainMod SHIFT, 9, tilescript-hypr, movetoworkspace, 9
bind = $mainMod SHIFT, 0, tilescript-hypr, movetoworkspace, 10

bind = $mainMod, q, tilescript-hypr, closewindow
bind = $mainMod, f, tilescript-hypr, fullscreen
```

## Docs

- [`docs/config.md`](docs/config.md)
- [`docs/hyprland.md`](docs/hyprland.md)
- [`docs/jsx.md`](docs/jsx.md)
- [`docs/css.md`](docs/css.md)
- [`docs/css-lsp.md`](docs/css-lsp.md)
- [`docs/development.md`](docs/development.md)
- [`docs/playground.md`](docs/playground.md)

## Core Dependencies

`tilescript` builds on a small set of foundational libraries:

- [`stylo`](https://crates.io/crates/stylo), [`cssparser`](https://crates.io/crates/cssparser), and [`selectors`](https://crates.io/crates/selectors) for CSS parsing and selector machinery
- [`taffy`](https://crates.io/crates/taffy) for layout computation
- [`oxc`](https://crates.io/crates/oxc) and [`oxc_resolver`](https://crates.io/crates/oxc_resolver) for JS/TS/TSX parsing and module graph analysis
- [`rquickjs`](https://crates.io/crates/rquickjs) for the native JS runtime
- [`mlua`](https://crates.io/crates/mlua) for the native Lua runtime
- [`leptos`](https://crates.io/crates/leptos) and [`leptos_router`](https://crates.io/crates/leptos_router) for the browser playground UI
- [`wasm-bindgen`](https://crates.io/crates/wasm-bindgen) and [`web-sys`](https://crates.io/crates/web-sys) for browser/WASM interop
- [`monaco-editor`](https://www.npmjs.com/package/monaco-editor), [`monaco-vim`](https://www.npmjs.com/package/monaco-vim), and [`wasmoon`](https://www.npmjs.com/package/wasmoon) for the playground editor and browser Lua execution
