# CSS LSP

This document describes the architecture and project model for `tilescript-css-lsp`.

## Purpose

`tilescript-css-lsp` provides editor features for the `tilescript` CSS language and its project-aware integration with authored TSX layouts.

It is CSS-focused.

It is not a replacement for TypeScript or TSX language tooling.

## Ownership

`tilescript-css` owns:

- CSS language metadata
- parser and compilation logic
- editor-agnostic analysis primitives
- source-ranged diagnostics and symbols that also apply outside the LSP

`tilescript-css-lsp` owns:

- LSP transport and protocol shaping
- document lifecycle
- project discovery and app scoping
- TSX selector indexing
- project-aware completion, diagnostics, hover, definition, references, rename, code actions, and workspace symbols

## Project Model

The LSP follows the same app graph model as the runtime JS pipeline.

It reuses discovery and graph building from `tilescript-runtime-js-core`.

The relevant scopes are:

- config app
- layout apps

### Config app

The config app is rooted at the nearest discovered `config.tsx`, `config.ts`, `config.jsx`, or `config.js`.

Its CSS surface is the root `index.css` alongside the config entry.

This is the global CSS scope.

### Layout app

Each layout app is rooted at a layout entry like `layouts/<name>/index.tsx`.

Its scope includes:

- the layout entry module
- imported authored TS and TSX modules in that app graph
- the layout `index.css`
- imported CSS in that app graph

This is the layout-local CSS scope.

Sibling layouts are isolated from each other.

## Selector Boundary Rules

Selector-aware features use app scope boundaries.

That means:

- root `index.css` sees selectors authored in the config app scope
- `layouts/master-stack/index.css` sees selectors authored in the `master-stack` app scope
- shared components imported by that layout are included in scope
- non-imported files outside the app graph are not included
- sibling layout selectors do not leak into each other

## TSX Indexing

The LSP indexes authored selector data from TSX using `oxc`.

It extracts selector-relevant metadata from:

- `workspace`
- `group`
- `window`
- `slot`

It indexes:

- `id`
- `class`
- exact source ranges for static selector definitions

## Implemented Feature Surface

`tilescript-css-lsp` provides:

- diagnostics from `tilescript-css` shared analysis
- project-aware diagnostics for unknown selector ids and classes
- context-aware completion for CSS language constructs
- project-aware selector completion for known ids and classes
- hover for properties, pseudos, attribute keys, and project-backed selectors
- document symbols
- workspace symbols for project-backed ids and classes
- definition for selector ids/classes
- references for selector ids/classes
- rename for selector ids/classes across CSS and TSX within scope
- quick-fix code actions for unknown selector ids/classes

## VS Code Workspace Setup

The VS Code extension client is opt-in.

Create `.vscode/settings.json` in your `tilescript` config workspace with:

```json
{
  "tilescriptCss.enable": true,
  "css.validate": false
}
```

## Neovim Setup

Build the server with:

```sh
cargo build -p tilescript-css-lsp --release
```

Then point your LSP client at:

- `target/release/tilescript-css-lsp`

Use root markers like:

- `config.ts`
- `config.tsx`
- `config.js`
- `config.jsx`
- `.git`
