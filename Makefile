.PHONY: configure build plugin ffi test fmt lint lsp-check live playground clean

configure:
	cmake -S . -B build

ffi:
	cargo build --release -p hypreact-hypr-ffi

plugin: configure
	cmake --build build

build: plugin

test:
	cargo test -p hypreact-scene
	cargo test -p hypreact-layout-runtime
	cargo test -p hypreact-runtime-js-core
	cargo test -p hypreact-runtime-js-native
	cargo test -p hypreact-hypr-ffi

live: plugin
	cp build/hypreact.so build/hypreact-live.so
	@printf 'load with: hyprctl plugin load %s/build/hypreact-live.so\n' "$$(pwd)"

playground:
	trunk serve --config apps/hypreact-playground/Trunk.toml --open

clean:
	rm -rf build
