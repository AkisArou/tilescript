# Hyprland

## Basic Setup

Build the plugin:

```sh
make hypr-plugin
make hypr-plugin-snippet
```

`make hypr-plugin-snippet` prints the `plugin` block to paste into your Hyprland config. It does not modify files or load the plugin for you.

`make hypr-plugin` copies the plugin to:

```text
${XDG_DATA_HOME:-$HOME/.local/share}/tilescript/tilescript-hypr.so
```

Example Hyprland config:

```ini
plugin = /absolute/path/to/tilescript-hypr.so

plugin {
  tilescript-hypr {
    config_path = /absolute/path/to/your/tilescript-config/
  }
}
```

`config_path` should point to a config directory.

The plugin looks for `config.ts`, `config.tsx`, `config.js`, `config.jsx`, `config.lua`, or `config.fnl` inside that directory.

If `config_path` is omitted, the plugin uses `~/.config/tilescript`.

If that config root does not exist yet, the plugin bootstraps it from `examples/js/`.

For starter projects, see `examples/js/`, `examples/lua/`, and `examples/fennel/`.

Related keybindings:

```ini
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

## Runtime Status

Use `hyprctl tilescript-hypr` to inspect plugin/runtime state.

It includes:

- current runtime workspace/output/focus state
- whether layouts loaded successfully
- selected layout name
- blocking layout/config errors
- structured CSS diagnostics

Useful commands:

```sh
hyprctl tilescript-hypr
hyprctl tilescript-hypr reload-layouts
hyprctl tilescript-hypr debug-layout-workspace 1
```

## Development

For local plugin development, Hyprland is tracked as a git submodule at `third_party/Hyprland/`.

Bootstrap the submodule:

```sh
make hypr-bootstrap
```

Build Hyprland in debug mode when needed:

```sh
make hypr-build
```

Build the nested-dev plugin against that Hyprland tree:

```sh
make hypr-plugin-dev
```

That copies the dev plugin to:

```text
${XDG_DATA_HOME:-$HOME/.local/share}/tilescript/tilescript-hypr-dev.so
```

Launch nested Hyprland with the repo dev config:

```sh
make hypr-run-dev
```

This builds the dev plugin and launches `third_party/Hyprland/build/Hyprland --config dev/hypr/hyprland.conf`.

If Hyprland is not built yet, run `make hypr-build` first.

`dev/hypr/hyprland.conf` uses:

- `plugin = $XDG_DATA_HOME/tilescript/tilescript-hypr-dev.so`
- `config_path = ../../dev/test`

Reload loop from inside the nested session:

```sh
make hypr-reload
```

Reload loop for your daily-driver Hyprland session:

```sh
make hypr-user-reload
```
