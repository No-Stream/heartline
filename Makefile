CARGO := $(HOME)/.cargo/bin/cargo
CARGO_HOME ?= $(HOME)/.cargo
WASM_BINDGEN := $(CARGO_HOME)/bin/wasm-bindgen
WASM_BINDGEN_VERSION := 0.2.105
PARK ?= content/parks/horizon-hawk.park.json

# Every recipe line runs inside the project's shared memory and CPU budget; see tools/run-in-budget.sh.
SHELL := tools/run-in-budget.sh
.SHELLFLAGS := bash -c

.PHONY: install build check test test-rust test-rated-windows test-force-rule test-construction-cache test-inversion-compile test-threads test-web test-design-json-harness test-canvas-sizing test-flexibility lint serve e2e e2e-feature e2e-journey e2e-smoothness e2e-slow e2e-feature-docker e2e-feature-docker-update-snapshots e2e-journey-docker e2e-journey-docker-update-snapshots journeys ci wasm-bindgen art art-drawings art-demo art-screenshots art-themes bench-render bench bench-guests bench-construction bench-builder-updates bench-builder-updates-browser bench-finish bench-player-circuits bench-player-force-preview bench-shaped-elements bench-shaped-elements-cold check-shaped-turn-arc-parity generate-report-types check-report-types generate-profiles generate-rated-windows ride-agreement design-tools agreement-wasm test-agreement test-design-tools park-report

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

design-tools:
	$(CARGO) build --release -p theme-park-design-tools

# The browser's release WASM build without the art and web bundle, for tools/design agree.
agreement-wasm: wasm-bindgen
	$(CARGO) build --release --target wasm32-unknown-unknown -p theme-park-sim-wasm
	$(WASM_BINDGEN) target/wasm32-unknown-unknown/release/theme_park_sim_wasm.wasm --target web --out-dir web/src/wasm

park-report: design-tools
	target/release/heartline-design park report "$(PARK)"

# The fast browser-path agreement check's own regression tests.
test-agreement: design-tools agreement-wasm
	npx vitest run web/e2e/rides/fast-agreement.test.ts

test-design-tools:
	$(CARGO) test -p theme-park-design-tools

generate-profiles:
	UPDATE_SOLVED_PROFILES=1 $(CARGO) test -p theme-park-sim --lib track::profile::tests::solved_profiles_are_current

generate-rated-windows:
	UPDATE_RATED_WINDOWS=1 $(CARGO) run --release -p theme-park-sim --example measure_force_windows

# The CLI version must match the wasm-bindgen crate version in Cargo.lock.
wasm-bindgen:
	@if ! "$(WASM_BINDGEN)" --version 2>/dev/null | grep -qxF "wasm-bindgen $(WASM_BINDGEN_VERSION)"; then $(CARGO) install wasm-bindgen-cli --version $(WASM_BINDGEN_VERSION) --locked --force; fi

# The sprite atlas is generated from web/art/drawings/ and not committed.
check: build design-tools
	$(CARGO) test -p theme-park-sim --lib -- world::tests guests::behavior::tests guests::path_food_rebuild_tests physics::tests track::construction::tests track::construction::flexibility_tests track::construction::walk_tests facilities::tests --skip block_operation_keeps_trains_separate_and_completes_runs
	npm test
	npm run test:art
	npm run typecheck
	npx vitest run web/e2e/rides/fast-agreement.test.ts
	PLAYWRIGHT_PORT=$${PLAYWRIGHT_PORT:-4271} npm run e2e -- --grep @fast

test: test-rust test-threads test-flexibility test-web

# CI shards the workspace by overriding this, e.g. RUST_TEST_SELECTION="-p theme-park-sim".
RUST_TEST_SELECTION ?= --workspace

test-rust:
	$(CARGO) test $(RUST_TEST_SELECTION) --no-fail-fast -- --skip track::construction::walk_tests::seeded_

test-rated-windows:
	$(CARGO) test -p theme-park-sim --example measure_force_windows

test-force-rule:
	$(CARGO) test -p theme-park-sim --lib physics::force_evaluation::tests
	$(CARGO) test -p theme-park-sim --test rated_element_physics

test-construction-cache:
	$(CARGO) test -p theme-park-sim --lib track::construction::cache_tests

test-inversion-compile:
	$(CARGO) test -p theme-park-sim --test inversion_physics --no-run

test-threads:
	$(CARGO) test -p theme-park-sim --features threads -- --skip track::construction::walk_tests::seeded_

test-flexibility:
	FLEXIBILITY_WALKS=256 $(CARGO) test --release -p theme-park-sim --lib track::construction::walk_tests::seeded_

test-web: art design-tools
	npm test
	npm run test:art

test-design-json-harness:
	npm run test:design-json

test-canvas-sizing:
	npm run test:canvas-sizing

lint:
	$(CARGO) fmt --all -- --check
	$(CARGO) clippy --workspace --all-targets -- -D clippy::disallowed_methods -D warnings
	$(CARGO) clippy -p theme-park-sim --all-targets --features threads -- -D clippy::disallowed_methods -D warnings
	npm run typecheck

serve:
	npm run dev -- --host 127.0.0.1 --port 5173 --strictPort

e2e: build
	npm run e2e

ride-agreement: build design-tools
	RIDE="$(RIDE)" npm run e2e -- --project rides

e2e-design-session: build design-tools
	npm run e2e -- --grep 'replays a design tool explicit block layout through the real worker'

e2e-feature: build
	npm run e2e -- --project=feature

e2e-feature-docker: build design-tools
	THEME_PARK_PLAYWRIGHT_DOCKER=1 npm run e2e -- --project=feature

e2e-feature-docker-update-snapshots: build design-tools
	THEME_PARK_PLAYWRIGHT_DOCKER=1 npm run e2e -- --project=feature --update-snapshots

e2e-journey-docker: build design-tools
	THEME_PARK_PLAYWRIGHT_DOCKER=1 npm run e2e -- --project=journey

e2e-journey-docker-update-snapshots: build design-tools
	THEME_PARK_PLAYWRIGHT_DOCKER=1 npm run e2e -- --project=journey --update-snapshots

e2e-journey: build
	npm run e2e -- --project=journey

e2e-smoothness: build
	npm run e2e -- --project=smoothness

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

# Times every worker call made while placing the journey circuits; ARGS=--digest hashes the outputs instead.
bench-builder-updates:
	$(CARGO) run --release -p theme-park-sim-wasm --example construction_update_bench -- $(ARGS)

bench-builder-updates-browser: wasm-bindgen
	$(CARGO) build --release --target wasm32-unknown-unknown -p theme-park-sim-wasm
	$(WASM_BINDGEN) target/wasm32-unknown-unknown/release/theme_park_sim_wasm.wasm --target web --out-dir web/src/wasm
	flock /tmp/theme-park-sim-browser-tests.lock node web/construction-bench/placements.mjs

# Times the finish search alone on Horizon Hawk without its last 2 to 6 ride steps.
bench-finish:
	$(CARGO) run --release -p theme-park-sim-wasm --example finish_search_bench -- horizon-hawk 2 3 4 5 6

bench-player-circuits:
	$(CARGO) run --release -p theme-park-sim --example player_circuit_preview_bench

bench-player-force-preview:
	$(CARGO) run --release -p theme-park-sim-wasm --example player_circuit_force_preview_bench -- --append-only

bench-shaped-elements: wasm-bindgen
	$(CARGO) build --release -p theme-park-sim-wasm --features shaped-benchmark --example shaped_solver_bench
	$(CARGO) build --release --target wasm32-unknown-unknown -p theme-park-sim-wasm --features shaped-benchmark
	$(WASM_BINDGEN) target/wasm32-unknown-unknown/release/theme_park_sim_wasm.wasm --target nodejs --out-dir /tmp/theme-park-sim-shaped-bench
	node tools/bench-shaped-elements.mjs --run /tmp/theme-park-sim-shaped-bench/theme_park_sim_wasm.js target/release/examples/shaped_solver_bench $(SHAPED_BENCH_CASES)

# Native and Node/WASM must agree on Horizon Hawk's first shaped turn arc table and its first solved shaped hill.
check-shaped-turn-arc-parity: wasm-bindgen
	$(CARGO) build -p theme-park-sim-wasm --features shaped-benchmark --example shaped_solver_bench
	$(CARGO) build --target wasm32-unknown-unknown -p theme-park-sim-wasm --features shaped-benchmark
	$(WASM_BINDGEN) target/wasm32-unknown-unknown/debug/theme_park_sim_wasm.wasm --target nodejs --out-dir /tmp/theme-park-sim-shaped-arc-parity
	node tools/bench-shaped-elements.mjs --check-horizon-hawk-arc-parity /tmp/theme-park-sim-shaped-arc-parity/theme_park_sim_wasm.js target/debug/examples/shaped_solver_bench

# Covers 45 hyper cases; the cold Node/WASM solve-and-append p95 budget is 250 ms.
bench-shaped-elements-cold: wasm-bindgen
	$(CARGO) build --release --target wasm32-unknown-unknown -p theme-park-sim-wasm --features shaped-benchmark
	$(WASM_BINDGEN) target/wasm32-unknown-unknown/release/theme_park_sim_wasm.wasm --target nodejs --out-dir /tmp/theme-park-sim-shaped-bench
	node tools/bench-shaped-elements.mjs --cold-wasm /tmp/theme-park-sim-shaped-bench/theme_park_sim_wasm.js

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
