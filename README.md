# hypreact

`hypreact` is an authored layout runtime for Wayland compositors.

It lets you define workspace layouts in JSX/TSX and CSS, then evaluates those layouts in Rust and hands the resulting placement decisions to a compositor adapter.

The current concrete adapter target is Hyprland.

## Features

- JSX/TSX-authored layouts
- CSS-driven layout and presentation semantics
- runtime layout selection by workspace index, workspace name, and monitor
- flex-inferred resize behavior with configurable resize step and minimum pane size
- Rust-owned layout evaluation and placement logic
- Hyprland plugin integration
- CSS language server crates and a VS Code client package

## Example

`layouts/master-stack/index.tsx`

```tsx
import type { LayoutContext } from "@hypreact/sdk/layout";

import "./index.css";

export default function layout(ctx: LayoutContext) {
  return (
    <workspace id="frame">
      <slot id="master" take={1} class="master-slot" />

      {ctx.windows.length > 1 ? (
        <group class="stack-group">
          <slot id="stack-slot" class="stack-group__item" />
        </group>
      ) : null}
    </workspace>
  );
}
```

`layouts/master-stack/index.css`

```css
#frame {
  display: flex;
  flex-direction: row;
  gap: 6px;
  padding: 6px;
  width: 100%;
  height: 100%;
}

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
}

.stack-group__item {
  flex-basis: 0;
  flex-grow: 1;
  min-height: 0;
}
```

## Usage

1. Create a `config.ts`.
2. Add one or more layouts under `layouts/<name>/index.tsx`.
3. Add sibling layout CSS in `layouts/<name>/index.css`.
4. Load the plugin from `/home/akisarou/projects/hypreact/build/hypreact.so`.
5. Point the Hyprland plugin at your config directory.
6. Reload layouts or reload the plugin after changes.

When the plugin resolves your config root, it bootstraps missing files from `examples/template/` and syncs editor support files into `.sdk/` under that root.

Example Hyprland config:

```ini
plugin = /home/akisarou/projects/hypreact/build/hypreact.so

plugin {
  hypreact {
    config_path = /home/akisarou/projects/hypreact/dev/test-config
  }
}
```

`config_path` should point to a config directory.

The plugin looks for `config.ts`, `config.tsx`, `config.js`, or `config.jsx` inside that directory.

If `config_path` is omitted, the plugin uses `~/.config/hypreact`.

If that config root does not exist yet, the plugin bootstraps it from `examples/template/`.

For editor support, your config `tsconfig.json` should extend `./.sdk/tsconfig.json`.

For a starter project, see `examples/template/`.

## Runtime Status

Use `hyprctl hypreact` to inspect plugin/runtime state.

It includes:

- current runtime workspace/output/focus state
- whether layouts loaded successfully
- selected layout name
- blocking layout/config errors
- structured CSS diagnostics

Useful commands:

```sh
hyprctl hypreact
hyprctl hypreact reload-layouts
hyprctl hypreact debug-layout-workspace 1
```

## Docs

- `docs/config.md`
- `docs/jsx.md`
- `docs/css.md`
- `docs/css-lsp.md`
- `docs/development.md`
- `docs/plan/resizing.md`
- `docs/plan/animations.md`
