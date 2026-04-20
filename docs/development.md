# Development

## Repo Structure

- `crates/core` - shared WM model, resize, focus, and query logic
- `crates/css` - CSS parser, compiler, metadata, and analysis
- `crates/scene` - style matching and layout/scene computation
- `crates/runtimes/js/core` - shared JS graph, compile, payload, and loader logic
- `crates/runtimes/js/native` - native QuickJS config and layout runtime
- `crates/layout-runtime` - end-to-end workspace evaluation and placement logic
- `crates/ffi` - Rust bridge exposed to the Hyprland plugin
- `apps/tilescript-playground` - browser playground modeled
- `plugin/hyprland/src/plugin.cpp` - Hyprland-side plugin adapter
- `packages/sdk/js` - config/layout SDK surface
- `packages/lsp/vscode` - VS Code client for `tilescript-css-lsp`
- `dev/test-config` - local config fixture
- `examples/js` - JavaScript/TypeScript starter config
- `examples/lua` - Lua starter config
- `examples/fennel` - Fennel starter config

## Common Commands

Recommended entrypoints:

```sh
make hypr-plugin
make playground
make test
make live
```

Rust:

```sh
cargo test -p tilescript-scene
cargo test -p tilescript-layout-runtime
cargo test -p tilescript-runtime-js-core
cargo test -p tilescript-runtime-js-native
cargo build --release -p tilescript-ffi
```

Plugin:

```sh
cmake -S . -B build
cmake --build build
```

`cmake --build build` builds the Rust release FFI staticlib before linking `build/tilescript-hypr.so`.

JS tooling:

```sh
pnpm install
pnpm fmt
pnpm lint
pnpm --filter tilescript-css-lsp-vscode run check
```

Playground:

```sh
make playground
```

`apps/tilescript-playground`

- preview route
- editor route with workspace file tree and live buffers
- system route with state/diagnostics

## Hyprland Development Loop

There are two Hyprland plugin workflows:

- daily-driver plugin build in `build/tilescript-hypr.so`
- nested debug plugin build in `build-hypr-dev/tilescript-hypr-dev.so`

Hyprland is tracked as a git submodule at `third_party/Hyprland/`.

One-time prerequisites:

```sh
make hypr-bootstrap
make hypr-build
```

Build the nested-debug plugin against that exact Hyprland tree:

```sh
make hypr-plugin-dev
```

That copies the dev plugin to `${XDG_DATA_HOME:-$HOME/.local/share}/tilescript/tilescript-hypr-dev.so`.

Launch a nested debug Hyprland session with the repo config:

```sh
make hypr-run-dev
```

That target:

- configures `tilescript` directly against `third_party/Hyprland/src` and `third_party/Hyprland/build`
- launches `third_party/Hyprland/build/Hyprland --config dev/hypr/hyprland.conf`

If the Hyprland binary is missing, `hypr-run-dev` tells you to run `make hypr-build` first.

If the plugin was built against a different Hyprland revision than the running compositor, plugin init fails with a clear hash-mismatch error instead of relying on undefined ABI behavior.

`dev/hypr/hyprland.conf` uses:

- `plugin = $XDG_DATA_HOME/tilescript/tilescript-hypr-dev.so`
- `config_path = ../../dev/test`

For a daily-driver Hyprland session, build the normal plugin and print the corresponding config snippet:

```sh
make hypr-plugin
make hypr-plugin-snippet
```

`make hypr-plugin-snippet` prints the exact `plugin` block to paste into your normal Hyprland config. It does not write a config file or load the plugin automatically.

Daily-driver loading uses `${XDG_DATA_HOME:-$HOME/.local/share}/tilescript/tilescript-hypr.so`, while the nested debug session uses `${XDG_DATA_HOME:-$HOME/.local/share}/tilescript/tilescript-hypr-dev.so`.

Reload loop from inside the nested session:

```sh
make hypr-reload
```

That rebuilds the nested plugin, copies it to `${XDG_DATA_HOME:-$HOME/.local/share}/tilescript/tilescript-hypr-dev.so`, and prints the unload/load command.

Reload loop for your daily-driver Hyprland session:

```sh
make hypr-user-reload
```

That rebuilds the normal plugin, copies it to `${XDG_DATA_HOME:-$HOME/.local/share}/tilescript/tilescript-hypr.so`, and prints the unload/load command for the daily-driver instance.

Useful runtime inspection commands:

```sh
hyprctl tilescript-hypr
hyprctl tilescript-hypr reload-layouts
hyprctl tilescript-hypr debug-layout-workspace 1
```

`hyprctl tilescript-hypr` returns plugin/runtime status, layout load state, errors, and structured CSS diagnostics.

## Authoring Fixtures

`dev/test-config/` is the main local config fixture.

Use it for:

- runtime-js tests
- layout-runtime tests
- local Hyprland plugin validation

`examples/js/` is the default starter project skeleton used by plugin bootstrap.

`examples/lua/` mirrors the same starter layout in Lua form.

For external config roots, the Hyprland plugin manages a local SDK mirror under:

- `.sdk/`
- `.sdk/tsconfig.json`
- `.sdk/package.json`
- `.sdk/src/*.d.ts`
- `.sdk/src/*.js`

The plugin bootstraps missing config roots from `examples/js/` and refreshes that managed SDK support when it resolves and loads the config root.

## Style And Tooling

- Rust formatting: `rustfmt.toml`
- JS/TS formatting: `.oxfmtrc.json`
- linting: `.oxlintrc.json`

## Docs

- `README.md`
- `docs/config.md`
- `docs/hyprland.md`
- `docs/jsx.md`
- `docs/css.md`
- `docs/css-lsp.md`
- `docs/development.md`
- `docs/playground.md`
