CARGO := $(HOME)/.cargo/bin/cargo
CARGO_HOME ?= $(HOME)/.cargo
WASM_BINDGEN := $(CARGO_HOME)/bin/wasm-bindgen
WASM_BINDGEN_VERSION := 0.2.105

.PHONY: install build test lint serve e2e ci wasm-bindgen art art-drawings art-demo art-screenshots art-themes bench-render bench bench-guests

install:
	npm ci

build: wasm-bindgen art
	$(CARGO) build --release --target wasm32-unknown-unknown -p theme-park-sim-wasm
	$(WASM_BINDGEN) target/wasm32-unknown-unknown/release/theme_park_sim_wasm.wasm --target web --out-dir web/src/wasm
	npm run build

# The CLI version must match the wasm-bindgen crate version in Cargo.lock.
wasm-bindgen:
	@if ! "$(WASM_BINDGEN)" --version 2>/dev/null | grep -qxF "wasm-bindgen $(WASM_BINDGEN_VERSION)"; then $(CARGO) install wasm-bindgen-cli --version $(WASM_BINDGEN_VERSION) --locked --force; fi

# The sprite atlas is generated from web/art/drawings/ and not committed.
test: art
	$(CARGO) test --workspace
	$(CARGO) test -p theme-park-sim --features threads
	npm test
	npm run test:art

lint:
	$(CARGO) fmt --all -- --check
	$(CARGO) clippy --workspace --all-targets -- -D warnings
	$(CARGO) clippy -p theme-park-sim --all-targets --features threads -- -D warnings
	npm run typecheck

serve:
	npm run dev -- --host 127.0.0.1

e2e: build
	npm run e2e

bench:
	$(CARGO) run --release -p theme-park-sim --features threads --example tick_bench

bench-guests: wasm-bindgen
	$(CARGO) run --release -p theme-park-sim --features threads --example guest_bench
	$(CARGO) build --release --target wasm32-unknown-unknown -p theme-park-sim-wasm
	$(WASM_BINDGEN) target/wasm32-unknown-unknown/release/theme_park_sim_wasm.wasm --target web --out-dir web/src/wasm
	node web/guest-bench/run.mjs

bench-render:
	node web/bench/run.mjs

NODE_TS := node --experimental-strip-types --disable-warning=ExperimentalWarning

# Rasterizes web/art/drawings/ into the sprite atlas. Hand-replaced SVGs are picked up as they are.
art:
	$(NODE_TS) web/art/pipeline/build-atlas.ts

# Regenerates the parametric SVGs, overwriting any drawing with the same path.
art-drawings:
	$(NODE_TS) web/art/generators/generate.ts

art-demo:
	npx vite web/art --host 127.0.0.1 --port 4180 --strictPort  # then open http://127.0.0.1:4180/demo.html

art-screenshots:
	$(NODE_TS) web/art/pipeline/screenshots.ts

# Builds the alternative art themes under web/art/themes/ and screenshots them next to candy
# in web/art/screenshots/themes/. Open one in the demo with demo.html?theme=<name>.
art-themes: art
	$(NODE_TS) web/art/pipeline/themes.ts

# The same checks as .github/workflows/ci.yaml, except the secret scan and dependency audit.
ci: install build lint test e2e
