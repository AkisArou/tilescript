.PHONY: configure configure-hypr-dev hypr-bootstrap hypr-plugin-snippet build hypr-plugin hypr-plugin-dev ffi test fmt lint lsp-check live hypr-user-reload hypr-run-dev hypr-reload playground clean

PLUGIN_BUILD_DIR ?= build
PLUGIN_OUTPUT_NAME ?= tilescript-hypr
PLUGIN_PATH ?= $(CURDIR)/$(PLUGIN_BUILD_DIR)/$(PLUGIN_OUTPUT_NAME).so
PLUGIN_LIVE_PATH ?= $(CURDIR)/$(PLUGIN_BUILD_DIR)/$(PLUGIN_OUTPUT_NAME)-live.so

HYPR_DEV_BUILD_DIR ?= build-hypr-dev
HYPR_DEV_OUTPUT_NAME ?= tilescript-hypr-dev
HYPR_DEV_PLUGIN_PATH ?= $(CURDIR)/$(HYPR_DEV_BUILD_DIR)/$(HYPR_DEV_OUTPUT_NAME).so
HYPR_DEV_PLUGIN_LIVE_PATH ?= $(CURDIR)/$(HYPR_DEV_BUILD_DIR)/$(HYPR_DEV_OUTPUT_NAME)-live.so

HYPRLAND_REPO ?= $(CURDIR)/third_party/Hyprland
HYPRLAND_BUILD_DIR ?= $(HYPRLAND_REPO)/build
TILESCRIPT_CONFIG_ROOT ?= $(CURDIR)/dev/test

configure:
	cmake -S . -B "$(PLUGIN_BUILD_DIR)" -DTILESCRIPT_HYPR_OUTPUT_NAME="$(PLUGIN_OUTPUT_NAME)"

configure-hypr-dev:
	cmake -S . -B "$(HYPR_DEV_BUILD_DIR)" -DTILESCRIPT_HYPR_OUTPUT_NAME="$(HYPR_DEV_OUTPUT_NAME)" -DHYPRLAND_SOURCE_DIR="$(HYPRLAND_REPO)" -DHYPRLAND_PKG_CONFIG_DIR="$(HYPRLAND_BUILD_DIR)"

hypr-bootstrap:
	git submodule update --init --recursive
	$(MAKE) -C "$(HYPRLAND_REPO)" debug

hypr-plugin-snippet: hypr-plugin
	@printf 'plugin = %s\n\nplugin {\n  tilescript-hypr {\n    config_path = %s\n  }\n}\n' "$(PLUGIN_PATH)" "$(TILESCRIPT_CONFIG_ROOT)"

ffi:
	cargo build --release -p tilescript-ffi

hypr-plugin: configure
	cmake --build "$(PLUGIN_BUILD_DIR)"

hypr-plugin-dev: configure-hypr-dev
	cmake --build "$(HYPR_DEV_BUILD_DIR)"

build: hypr-plugin

test:
	cargo test -p tilescript-scene
	cargo test -p tilescript-layout-runtime
	cargo test -p tilescript-runtime-js-core
	cargo test -p tilescript-runtime-js-native
	cargo test -p tilescript-ffi

live: hypr-user-reload

hypr-user-reload: hypr-plugin
	cp "$(PLUGIN_PATH)" "$(PLUGIN_LIVE_PATH)"
	@printf 'reload with: hyprctl plugin unload %s && hyprctl plugin load %s\n' "$(PLUGIN_LIVE_PATH)" "$(PLUGIN_LIVE_PATH)"

hypr-run-dev: hypr-bootstrap hypr-plugin-dev
	HYPRLAND_REPO="$(HYPRLAND_REPO)" HYPRLAND_BUILD_DIR="$(HYPRLAND_BUILD_DIR)" HYPR_DEV_CONFIG="$(CURDIR)/dev/hypr/hyprland.conf" ./dev/hypr/launch-hypr-dev.sh

hypr-reload: hypr-plugin-dev
	cp "$(HYPR_DEV_PLUGIN_PATH)" "$(HYPR_DEV_PLUGIN_LIVE_PATH)"
	@printf 'reload with: hyprctl plugin unload %s && hyprctl plugin load %s\n' "$(HYPR_DEV_PLUGIN_LIVE_PATH)" "$(HYPR_DEV_PLUGIN_LIVE_PATH)"

playground:
	mkdir -p apps/tilescript-playground/js/dist
	trunk serve --config apps/tilescript-playground/Trunk.toml --open

clean:
	rm -rf build
