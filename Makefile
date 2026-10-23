# Convenience targets. Requires the Dioxus CLI (`dx`, 0.7.x) and the
# wasm32-unknown-unknown target (`rustup target add wasm32-unknown-unknown`).

APP := crates/app

.PHONY: web desktop serve-web run-desktop test lint renders scenes bench calibrate stabs rnd-rate fit clean

## Serve the web build with hot reload on http://127.0.0.1:8080
serve-web:
	cd $(APP) && dx serve --web --release

## Run the native Linux app (PulseAudio / PipeWire-pulse)
run-desktop:
	cd $(APP) && dx serve --desktop --release

## Release bundles: target/dx/stabkussion/release/{web/public,linux/app}
web:
	cd $(APP) && dx build --web --release

desktop:
	cd $(APP) && dx build --desktop --release

test:
	cargo test --release -p stabkussion-dsp -p stabkussion

lint:
	cargo fmt --all --check
	cargo clippy --all-targets -p stabkussion-dsp -p stabkussion-worklet -- -D warnings
	cargo clippy --all-targets -p stabkussion --features desktop -- -D warnings
	cargo clippy -p stabkussion --features web --target wasm32-unknown-unknown -- -D warnings

## Render every factory preset to WAV files in ./renders
renders:
	cargo run --release -p stabkussion-dsp --example render -- renders

## Render every cross-modulation scene to WAV files in ./renders
scenes:
	cargo run --release -p stabkussion-dsp --example scenes -- renders

bench:
	cargo run --release -p stabkussion-dsp --example bench

## Loudness survey of every engine across pitch/click/macro settings
calibrate:
	cargo run --release -p stabkussion-dsp --example calibrate

## How stab-like are RND hits? Features of 600 rolls vs the reference stabs
REFS ?= examples
stabs:
	cargo run --release -p stabkussion-dsp --example stabs -- rnd $(REFS) 600

## How stab-like is RND, per engine, against the built-in reference features (no WAVs needed)
rnd-rate:
	cargo run --release -p stabkussion-dsp --example rnd_rate -- 2000

## Fit a patch to every reference stab (slow: minutes per stab, all cores)
fit:
	cargo run --release -p stabkussion-dsp --example fit -- $(REFS) fits

clean:
	cargo clean
