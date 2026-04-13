#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(dirname "$0")/../../../.."
EXT_DIR="$ROOT_DIR/packages/lsp/vscode"
BIN_DIR="$EXT_DIR/server/linux-x64"

cargo build -p hypreact-css-lsp --release --manifest-path "$ROOT_DIR/Cargo.toml"
mkdir -p "$BIN_DIR"
command cp -f "$ROOT_DIR/target/release/hypreact-css-lsp" "$BIN_DIR/hypreact-css-lsp"
chmod +x "$BIN_DIR/hypreact-css-lsp"

pnpm --dir "$EXT_DIR" run sync:icon
