CARGO := $(HOME)/.cargo/bin/cargo
CARGO_HOME ?= $(HOME)/.cargo
WASM_BINDGEN := $(CARGO_HOME)/bin/wasm-bindgen
WASM_BINDGEN_VERSION := 0.2.105

# Every recipe line runs inside the project's shared memory and CPU budget; see tools/run-in-budget.sh.
SHELL := tools/run-in-budget.sh
.SHELLFLAGS := bash -c

.PHONY: install build check test lint serve e2e e2e-slow journeys ci wasm-bindgen art art-drawings art-demo art-screenshots art-themes bench-render bench bench-guests bench-construction bench-player-circuits generate-report-types check-report-types generate-profiles

install:
	npm ci

build: wasm-bindgen art check-report-types
	$(CARGO) build --release --target wasm32-unknown-unknown -p theme-park-sim-wasm
	$(WASM_BINDGEN) target/wasm32-unknown-unknown/release/theme_park_sim_wasm.wasm --target web --out-dir web/src/wasm
	npm run build

# Regenerate the TypeScript wire types after changing a Rust report.
generate-report-types:
	UPDATE_REPORT_TYPES=1 $(CARGO) test -p theme-park-sim-wasm --lib generated_types_are_current

check-report-types:
	$(CARGO) test -p theme-park-sim-wasm --lib generated_types_are_current

generate-profiles:
	UPDATE_SOLVED_PROFILES=1 $(CARGO) test -p theme-park-sim --lib track::profile::tests::solved_profiles_are_current

# The CLI version must match the wasm-bindgen crate version in Cargo.lock.
wasm-bindgen:
	@if ! "$(WASM_BINDGEN)" --version 2>/dev/null | grep -qxF "wasm-bindgen $(WASM_BINDGEN_VERSION)"; then $(CARGO) install wasm-bindgen-cli --version $(WASM_BINDGEN_VERSION) --locked --force; fi

# The sprite atlas is generated from web/art/drawings/ and not committed.
check: build
	$(CARGO) test -p theme-park-sim --lib -- world::tests guests::behavior::tests guests::path_food_rebuild_tests physics::tests track::construction::tests track::construction::flexibility_tests track::construction::walk_tests facilities::tests --skip block_operation_keeps_trains_separate_and_completes_runs
	npm test
	npm run test:art
	npm run typecheck
	PLAYWRIGHT_PORT=$${PLAYWRIGHT_PORT:-4271} npm run e2e -- --grep @fast

test: art
	$(CARGO) test --workspace -- --skip track::construction::walk_tests::seeded_
	$(CARGO) test -p theme-park-sim --features threads -- --skip track::construction::walk_tests::seeded_
	FLEXIBILITY_WALKS=256 $(CARGO) test --release -p theme-park-sim --lib track::construction::walk_tests::seeded_
	npm test
	npm run test:art

lint:
	$(CARGO) fmt --all -- --check
	$(CARGO) clippy --workspace --all-targets -- -D warnings
	$(CARGO) clippy -p theme-park-sim --all-targets --features threads -- -D warnings
	npm run typecheck

serve:
	npm run dev -- --host 127.0.0.1 --port 5173 --strictPort

e2e: build
	npm run e2e

e2e-slow: build
	npm run e2e -- --grep @slow

journeys: build
	npm run e2e -- --project journey --workers=2

bench:
	$(CARGO) run --release -p theme-park-sim --features threads --example tick_bench

bench-construction:
	$(CARGO) run --release -p theme-park-sim --example construction_bench -- catalog
	$(CARGO) run --release -p theme-park-sim --example construction_bench -- choices hybrid
	$(CARGO) run --release -p theme-park-sim --example construction_bench -- choices multi_launch
	$(CARGO) run --release -p theme-park-sim --example construction_bench -- choices wooden
	$(CARGO) run --release -p theme-park-sim --example construction_bench -- elements hybrid
	$(CARGO) run --release -p theme-park-sim --example construction_bench -- elements multi_launch
	$(CARGO) run --release -p theme-park-sim --example construction_bench -- elements wooden
	$(CARGO) run --release -p theme-park-sim --example construction_bench -- hill
	$(CARGO) run --release -p theme-park-sim --example construction_bench -- per-piece

bench-player-circuits:
	$(CARGO) run --release -p theme-park-sim --example player_circuit_preview_bench

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

# Builds the alternative art themes under web/art/themes/ and screenshots them next to print-modern
# in web/art/screenshots/themes/. Open one in the demo with demo.html?theme=<name>.
art-themes: art
	$(NODE_TS) web/art/pipeline/themes.ts

# The same checks as .github/workflows/ci.yaml, except the secret scan and dependency audit.
ci: install build lint test e2e
