#!/usr/bin/env sh

set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
HYPRLAND_REPO=${HYPRLAND_REPO:-"$ROOT/third_party/Hyprland"}
HYPRLAND_BUILD_DIR=${HYPRLAND_BUILD_DIR:-"$HYPRLAND_REPO/build"}
HYPR_DEV_CONFIG=${HYPR_DEV_CONFIG:-"$ROOT/dev/hypr/hyprland.conf"}

if [ -n "${HYPRLAND_BIN:-}" ]; then
  hyprland_bin=$HYPRLAND_BIN
elif [ -x "$HYPRLAND_BUILD_DIR/Hyprland" ]; then
  hyprland_bin="$HYPRLAND_BUILD_DIR/Hyprland"
elif [ -x "$HYPRLAND_BUILD_DIR/start/start-hyprland" ]; then
  hyprland_bin="$HYPRLAND_BUILD_DIR/start/start-hyprland"
else
  printf 'could not find a Hyprland binary in %s\n' "$HYPRLAND_BUILD_DIR" >&2
  exit 1
fi

exec env HYPRLAND_NO_CRASHREPORTER=1 "$hyprland_bin" --config "$HYPR_DEV_CONFIG"
