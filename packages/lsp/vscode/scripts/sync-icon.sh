#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(dirname "$0")/../../../.."
SRC="$ROOT_DIR/assets/hypreact-mark.svg"
DST="$ROOT_DIR/packages/lsp/vscode/media/icon.png"

if [[ ! -f "$SRC" ]]; then
  if [[ -f "$DST" ]]; then
    printf 'sync-icon: %s not found, keeping existing %s\n' "$SRC" "$DST"
    exit 0
  fi

  printf 'sync-icon: %s not found and %s is missing\n' "$SRC" "$DST" >&2
  exit 1
fi

rsvg-convert -w 256 -h 256 "$SRC" -o "$DST"
