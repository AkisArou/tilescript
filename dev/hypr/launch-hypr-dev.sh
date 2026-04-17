#!/usr/bin/env sh

set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
HYPRLAND_REPO=${HYPRLAND_REPO:-"$ROOT/third_party/Hyprland"}
HYPRLAND_BUILD_DIR=${HYPRLAND_BUILD_DIR:-"$HYPRLAND_REPO/build"}
HYPR_DEV_CONFIG=${HYPR_DEV_CONFIG:-"$ROOT/dev/hypr/hyprland.conf"}
HYPRLAND_BIN="$HYPRLAND_BUILD_DIR/Hyprland"

exec env HYPRLAND_NO_CRASHREPORTER=1 "$HYPRLAND_BIN" --config "$HYPR_DEV_CONFIG"
