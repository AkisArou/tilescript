# Hypreact CSS

VS Code client for `hypreact-css-lsp`.

## Activation

The extension only activates in workspaces that contain a Hypreact config entry:

- `config.tsx`
- `config.ts`
- `config.jsx`
- `config.js`

Even in those workspaces, the language server is disabled by default.

## Enabling It Manually

Open your Hypreact config workspace, for example:

- `~/.config/hypreact/`

Then create `.vscode/settings.json` in that workspace and opt in:

```json
{
  "hypreactCss.enable": true,
  "css.validate": false
}
```

This file should live at:

- `~/.config/hypreact/.vscode/settings.json`

Recommended notes:

- `hypreactCss.enable` turns this extension on for the workspace
- `css.validate: false` reduces duplicate diagnostics from VS Code's built-in CSS support

If you want to disable VS Code's built-in CSS language features more aggressively, there is not a reliable workspace settings key for fully turning that built-in extension off.

The practical option is:

1. Open the Extensions view.
2. Find `CSS Language Features`.
3. Choose `Disable (Workspace)`.

For most use cases, `css.validate: false` is the lowest-friction setting to start with.

## Development

The extension currently ships a bundled Linux x64 server binary and otherwise falls back to workspace-built binaries.

Resolution order:

- `server/linux-x64/hypreact-css-lsp`
- `target/debug/hypreact-css-lsp`
- `target/release/hypreact-css-lsp`

You can also set an explicit path with:

- `hypreactCss.server.path`

The extension prefers a bundled platform binary when one is present.

## Packaging

Build a `.vsix` with:

```sh
pnpm --filter hypreact-css-lsp-vscode prepare:linux-x64
pnpm --filter hypreact-css-lsp-vscode package
```

If `assets/hypreact-mark.svg` exists, the icon can be regenerated with `rsvg-convert`.
Otherwise the checked-in `media/icon.png` is reused as-is.

## Other Editors

Neovim and other editors do not need a separate client package in this repo right now.

Build `hypreact-css-lsp` and point your editor's LSP configuration directly at the binary.

For the current recommended manual Neovim setup, see:

- `docs/css-lsp.md`
