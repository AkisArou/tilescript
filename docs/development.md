# Development

## Repo Structure

- `crates/core` - shared WM model, resize, focus, and query logic
- `crates/css` - CSS parser, compiler, metadata, and analysis
- `crates/scene` - style matching and layout/scene computation
- `crates/runtimes/js` - authored config and layout JS/TSX pipeline
- `crates/layout-runtime` - end-to-end workspace evaluation and placement logic
- `crates/hypr-ffi` - Rust bridge exposed to the Hyprland plugin
- `src/plugin.cpp` - Hyprland-side plugin adapter
- `packages/sdk/js` - authored config/layout SDK surface
- `packages/lsp/vscode` - VS Code client for `hypreact-css-lsp`
- `test_config` - local authored config fixture
- `template` - minimal starter config

## Common Commands

Recommended entrypoints:

```sh
make plugin
make test
make live
```

Rust:

```sh
cargo test -p hypreact-scene
cargo test -p hypreact-layout-runtime
cargo test -p hypreact-runtime-js
cargo build --release -p hypreact-hypr-ffi
```

Plugin:

```sh
cmake -S . -B build
cmake --build build
```

`cmake --build build` now builds the Rust release FFI staticlib automatically before linking `build/hypreact.so`.

JS tooling:

```sh
pnpm install
pnpm fmt
pnpm lint
pnpm --filter hypreact-css-lsp-vscode run check
```

## Hyprland Development Loop

Typical plugin loop:

```sh
make plugin
cp build/hypreact.so build/hypreact-live.so
hyprctl plugin load /absolute/path/to/build/hypreact-live.so
```

Use a fresh `.so` filename when testing repeatedly to avoid stale deleted plugin mappings.

Useful runtime inspection commands:

```sh
hyprctl hypreact
hyprctl hypreact reload-layouts
hyprctl hypreact debug-layout-workspace 1
```

`hyprctl hypreact` returns plugin/runtime status, layout load state, errors, and structured CSS diagnostics.

## Authoring Fixtures

`test_config/` is the main local authored-config fixture.

Use it for:

- runtime-js tests
- layout-runtime tests
- local Hyprland plugin validation

`template/` is the starter project skeleton.

For external config roots, the Hyprland plugin now manages a local SDK mirror under:

- `.sdk/`
- `.sdk/tsconfig.json`
- `.sdk/package.json`
- `.sdk/src/*.d.ts`
- `.sdk/src/*.js`

The plugin bootstraps missing config roots from `template/` and refreshes that managed SDK support when it resolves and loads the config root.

## Style And Tooling

- Rust formatting: `rustfmt.toml`
- JS/TS formatting: `.oxfmtrc.json`
- linting: `.oxlintrc.json`

## Docs

- `README.md`
- `docs/config.md`
- `docs/jsx.md`
- `docs/css.md`
- `docs/css-lsp.md`
- `docs/development.md`
- `docs/plan/resizing.md`
- `docs/plan/animations.md`
