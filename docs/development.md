# Development

## Repo Structure

- `crates/core` - shared WM model, resize, focus, and query logic
- `crates/css` - CSS parser, compiler, metadata, and analysis
- `crates/scene` - style matching and layout/scene computation
- `crates/runtimes/js/core` - shared JS graph, compile, payload, and loader logic
- `crates/runtimes/js/native` - native QuickJS authored config and layout runtime
- `crates/layout-runtime` - end-to-end workspace evaluation and placement logic
- `crates/hypr-ffi` - Rust bridge exposed to the Hyprland plugin
- `apps/hypreact-playground` - browser playground modeled on `spiders-wm-www` preview/editor/system flow
- `plugin/hyprland/src/plugin.cpp` - Hyprland-side plugin adapter
- `packages/sdk/js` - authored config/layout SDK surface
- `packages/lsp/vscode` - VS Code client for `hypreact-css-lsp`
- `dev/test-config` - local authored config fixture
- `examples/js` - JavaScript/TypeScript starter config
- `examples/lua` - Lua starter config
- `docs/plan/lua.md` - Lua runtime and authoring plan
- `docs/plan/fennel.md` - Fennel authoring/runtime plan
- `docs/plan/performance.md` - runtime caching, bytecode, and live reload plan

## Common Commands

Recommended entrypoints:

```sh
make plugin
make playground
make test
make live
```

Rust:

```sh
cargo test -p hypreact-scene
cargo test -p hypreact-layout-runtime
cargo test -p hypreact-runtime-js-core
cargo test -p hypreact-runtime-js-native
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

Playground:

```sh
make playground
```

`apps/hypreact-playground` intentionally follows the `spiders-wm-www` shape for the applicable browser features:

- preview route
- editor route with workspace file tree and live buffers
- system route with state/diagnostics

It does not carry over old `spiders-wm-www` pieces that do not apply in `hypreact` yet, such as the CLI route and the Monaco/browser IPC stack.

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

`dev/test-config/` is the main local authored-config fixture.

Use it for:

- runtime-js tests
- layout-runtime tests
- local Hyprland plugin validation

`examples/js/` is the default starter project skeleton used by plugin bootstrap.

`examples/lua/` mirrors the same starter layout in Lua form.

For external config roots, the Hyprland plugin now manages a local SDK mirror under:

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
- `docs/jsx.md`
- `docs/css.md`
- `docs/css-lsp.md`
- `docs/development.md`
- `docs/plan/resizing.md`
- `docs/plan/animations.md`
- `docs/plan/lua.md`
- `docs/plan/fennel.md`
- `docs/plan/performance.md`
