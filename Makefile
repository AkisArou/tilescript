.PHONY: configure build plugin ffi test fmt lint lsp-check live playground clean

configure:
	cmake -S . -B build

ffi:
	cargo build --release -p tilescript-ffi

plugin: configure
	cmake --build build

build: plugin

test:
	cargo test -p tilescript-scene
	cargo test -p tilescript-layout-runtime
	cargo test -p tilescript-runtime-js-core
	cargo test -p tilescript-runtime-js-native
	cargo test -p tilescript-ffi

live: plugin
	cp build/tilescript-hypr.so build/tilescript-hypr-live.so
	@printf 'load with: hyprctl plugin load %s/build/tilescript-hypr-live.so\n' "$$(pwd)"

playground:
	trunk serve --config apps/tilescript-playground/Trunk.toml --open

clean:
	rm -rf build
