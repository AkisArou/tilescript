#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(dirname "$0")/../../../.."
EXT_DIR="$ROOT_DIR/packages/lsp/vscode"
BIN_DIR="$EXT_DIR/server/linux-x64"

cargo build -p tilescript-css-lsp --release --manifest-path "$ROOT_DIR/Cargo.toml"
mkdir -p "$BIN_DIR"
command cp -f "$ROOT_DIR/target/release/tilescript-css-lsp" "$BIN_DIR/tilescript-css-lsp"
chmod +x "$BIN_DIR/tilescript-css-lsp"

pnpm --dir "$EXT_DIR" run sync:icon
