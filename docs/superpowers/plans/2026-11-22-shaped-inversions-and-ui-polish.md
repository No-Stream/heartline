# Shaped UI Polish, Confirm Latency, and Shaped Inversions (2026-11-22)

**Status:** Integration and verification in progress. The prior completion claim is superseded by the reassessment recorded in `TASK-168`.

Plan for executing three related shaped-elements cards in one clean arc:
1. **Warm-up A — `TASK-158`:** Fix shaped inspector layout clipping.
2. **Warm-up B — `TASK-157`:** Cut shaped hill confirm time to under 250 ms (cold Node/WASM p95 across all 45 hyper cases) while preserving existing saved ride digests.
3. **Main deliverable — `TASK-168`:** Add shaped inversions with peak-force intent (vertical loops, zero-g rolls, and Raven turns) on top of the per-model `ShapedProfile` architecture.

---

## 1. Warm-up A: Shaped Inspector Layout Clipping (`TASK-158`)

### Root cause
Inspected `web/e2e/construction.spec.ts-snapshots/shaped-hill-inspector-linux.png`, `web/src/construction.ts`, and `web/src/construction.css`:
- `web/src/construction.ts:156` renders `<span class="stepper__unit">g</span>` after the `+` button on `[data-shape-control="strength"]`, while `construction.ts:994` already formats the value inside `[data-shape-value="strength"]` as `−0.5 g`. This puts a duplicate italic `g` outside `+` at the panel edge and misaligns the `+` button relative to the other steppers.
- `web/src/construction.css:490` gives every `.builder__shape-controls .stepper` a 5th `auto` grid column even when `[data-shape-auto]` is absent or `hidden`.
- `web/src/construction.css:478-486` sets `overflow: hidden; white-space: nowrap;` on `.builder__hill-current` with zero trailing padding, slicing off the italic right overhang of Georgia `h` in `Floater hill · 87 km/h`.

### Changes
- Remove `<span class="stepper__unit">g</span>` from `web/src/construction.ts`.
- Give `.builder__shape-controls .stepper` a 4-column grid by default (`4.3rem 1.6rem minmax(0, 1fr) 1.6rem`), switching to the 5-column template only when an inline `Auto` button is visible (`:has([data-shape-auto]:not([hidden]))`).
- Add `padding-inline-end: 0.2rem;` to `.builder__hill-current`.
- Re-record `web/e2e/construction.spec.ts-snapshots/shaped-hill-inspector-linux.png` (and visually inspect the updated PNG).

### Acceptance criteria (`TASK-158`)
- Strength value displays its `g` unit inside the stepper value slot, with no trailing unit outside `+`.
- All shaped inspector steppers align cleanly on their right edges when `Auto` is hidden.
- Summary header (`Floater hill · 87 km/h`) shows its full trailing glyph without clipping.
- Playwright `construction.spec.ts` snapshot test passes and visual inspection confirms clean alignment.

---

## 2. Warm-up B: Cut Shaped Hill Confirm Time Under 250 ms (`TASK-157`)

### Bottlenecks identified
1. **`O(cars²)` nested loop in `minimum_mean_height_for_train` (`crates/sim/src/track/profile.rs:874-903`):** For 8 cars, every integration step calls `SolvedProfile::sample` (binary search + 2-axis quintic Hermite + 1st/2nd/3rd derivatives + trig) 64 times instead of 15 (`2*cars - 1`) z-only interpolations.
2. **Duplicate `piece.sample(local)` in `preview_car_readings` / `rider_g` (`crates/sim/src/physics/mod.rs:731-794`):** `preview_car_readings` samples `piece.sample(local)` and then `rider_g` immediately recomputes it; plus `rider_g` only needs `.heart` from its finite-difference neighbors.
3. **Redundant work in `calibrate_piece` & `PreviewCache::preview_candidate` (`shaped.rs:2159-2198`, `preview.rs:231-360`):**
   - `calibrate_piece` re-runs `preview_cache.preview(prefix)` on every secant iteration instead of once per solve.
   - `preview_candidate` builds `source_circuit` and then clones all pieces again plus a redundant `virtual_tangent_runout` even though `calibrate_piece` already appended straight runout pieces and exits at `source_length`.
   - `calibrate_piece` adds `ceil(train_len / 4) + 2` straight pieces (8 m extra beyond train length) even though the rear car clears the candidate after `train_len`.
4. **Browse vs confirm measurement:** Measure `solve_preview` (browse, 1 preview) vs `solve` / `solve_confirm` separately in the benchmark harness so both latencies are known.

### Strategy (zero-digest-drift first)
1. Optimize `minimum_mean_height_for_train` with a `2*cars - 1` z-only `SolvedProfile::sample_z` sliding sum (exact same floating-point summation order per car, zero geometry drift).
2. Avoid duplicate `piece.sample(local)` in `preview_car_readings` -> `rider_g_with_sample` and add a lightweight heart-only neighbor sample path for `rider_g` (exact same `heart` values, zero physics drift).
3. Verify entry speed once per `solve_with_preview_limit`, avoid building redundant `source_circuit` / unused `virtual_tangent_runout` in `preview_candidate`, and avoid hashing unchanged prefix pieces when possible.
4. Measure with `make bench-shaped-elements-cold` (all 45 hyper cases, 30 cold Node/WASM runs each). Only if worst-case cold p95 still exceeds 250 ms, apply a further algorithmic speedup (e.g. coarse flank-search steps or analytic initial crest correction) and verify any digest impact.

### Acceptance criteria (`TASK-157`)
- `make bench-shaped-elements-cold` passes with worst-case cold Node/WASM solve-and-commit p95 <= 250 ms across all 45 hyper cases (4/6/8 cars, 22–36 m/s, floater, strong floater, turn).
- Browse (`solve_preview`) latency is measured and reported alongside confirm latency.
- Existing `Horizon Hawk` and `Daisy Dash` saved geometry digests (`every_content_ride_shaped_intent_resolves_to_its_saved_geometry_digest`) and `make check-shaped-turn-arc-parity` remain green.

---

## 3. Main Deliverable: Shaped Inversions with Peak-Force Intent (`TASK-168`)

### Scope
Add three named shaped inversions sized to actual entry speed, train configuration, and target peak vertical force, driven purely by `ShapedProfile` + `CoasterModel` metadata (no hardcoded model keys in `shaped.rs`):

1. **Vertical loop (`ElementIntent::Loop`)**
   - Parameters: `hand: Hand` (`Left` / `Right`), `peak_g: f64` (target peak positive vertical g; steps `3.5, 3.8, 4.0, 4.2, 4.5` g), `top_feel: LoopTopFeel` (`Floaty` 0.3 g, `Neutral` 1.0 g [default], `PressedIn` 1.5 g).
   - Models: every profiled model whose `allowed_inversions` contains `Inversion::VerticalLoop` (`velocity_viper`, `hybrid`, and `wooden_twister`).
   - On wooden-track models (`track_material == Material::Wood`, i.e., `wooden_twister`), automatically sets `piece.structure_material = Some(Material::Steel)` so it obeys the steel-structure loop rule and renders steel supports.
   - Geometry: clothoid full loop (`ProfileSpec::full_loop` promoted out of `#[cfg(test)]` with `heart_speed_correction = true`), reflected across apex, exit straight snapped to grid (`2.0 m` cardinal / `2√2 m` diagonal), lateral offset `±2` grid steps (`±4 m` cardinal, `±4√2 m` diagonal) so the loop clears itself. Sampled in true metres (no diagonal vertical stretch!).
   - Metadata: `PieceId::ShapedLoopLeft` / `ShapedLoopRight`, `inversion = Some(Inversion::VerticalLoop)`, `heartlining = Some(Heartlining::NotHeartlined)`.

2. **Zero-g roll (`ElementIntent::ZeroGRoll`)**
   - Parameters: `hand: Hand` (`Left` / `Right`), `peak_g: f64` (entry/exit pullout peak vertical g; steps `3.0, 3.5, 3.8, 4.0, 4.2, 4.5` g, default from profile).
   - Models: every profiled model whose `allowed_inversions` contains `Inversion::HeartlineRoll` (`velocity_viper`, `hybrid`; NOT `wooden_twister`, `family_coaster`, or `hyper`).
   - Geometry: zero-g hill profile (`crest_g = 0.0`, calibrated `pull_g`) with `flank_angle` bisected so the actual train's crest traversal time keeps peak roll rate comfortably below `200°/s` (targeting ≤ 185°/s across all cars and ±0.5 m/s), paired with `Twist::ZeroGRoll { ramp, start, end }` sampled via `rolled_profile_sample` in true metres (no diagonal stretch).
   - Metadata: `PieceId::ShapedZeroGRollLeft` / `ShapedZeroGRollRight`, `inversion = Some(Inversion::HeartlineRoll)`, `heartlining = Some(Heartlining::Heartlined)`.

3. **Raven turn (`ElementIntent::RavenTurn`)**
   - Parameters: `hand: Hand` (`Left` / `Right`), `peak_g: f64` (target peak positive vertical g; steps `3.5, 3.8, 4.0, 4.2, 4.5` g), `top_feel: LoopTopFeel` (`Floaty` 0.3 g, `Neutral` 1.0 g [default], `PressedIn` 1.5 g).
   - Models: every profiled model whose `allowed_inversions` contains both `Inversion::VerticalLoop` and `Inversion::Corkscrew` (`velocity_viper`, `hybrid`; NOT `wooden_twister`, `family_coaster`, or `hyper`).
   - Geometry: single-piece C²-continuous composite:
     - Stage 1: climbing half-loop (`ProfileSpec::half_loop` promoted out of `#[cfg(test)]`, `heart_speed_correction = true`), height snapped to the 2 m vertical grid via `hold` angle refinement in `[95°, 145°]` (with vertical-insert fallback if needed), ending inverted at `(x_apex, 0, z_top)` facing `-forward` with zero curvature and zero roll.
     - Stage 2: offset half-roll back upright (`RollGeometry::spline()` with `ramp = 0.25`, `lateral_offset = ±4` grid half-tiles = `±8 m` cardinal, `roll_dx` sized to actual apex speed so peak roll rate ≤ 185°/s and exit `x = x_apex - roll_dx` lands on the horizontal grid).
     - Both stages join with exact C² continuity (`position`, `first`, `second = 0`, `up = -Z`, `roll = 0` at join), with axial `roll` advancing `0 -> ±π` during the half-roll stage.
   - Metadata: `PieceId::ShapedRavenTurnLeft` / `ShapedRavenTurnRight`, `inversion = Some(Inversion::VerticalLoop)`, `heartlining = Some(Heartlining::NotHeartlined)`.

### Profile data (`ShapedProfile` in `crates/sim/src/track/shaped.rs` & `models.rs`)
Add per-model inversion profile fields to `ShapedProfile` (unhashed, skipped in saves):
- `default_loop_peak_g: f64` (`velocity_viper`: 4.0, `hybrid`: 4.0, `wooden_twister`: 3.5; non-inverting: 0.0 / unused)
- `default_zero_g_roll_peak_g: f64` (`velocity_viper`: 3.8, `hybrid`: 3.8)
- `default_raven_turn_peak_g: f64` (`velocity_viper`: 4.0, `hybrid`: 4.0)
- `minimum_inversion_speed_mps: f64` (`velocity_viper`: 22.0, `hybrid`: 22.0, `wooden_twister`: 22.0; below this -> typed `SolveRefusal::BelowMinimumSpeed`)
- `maximum_inversion_speed_mps: f64` (`velocity_viper`: 36.0, `hybrid`: 36.0, `wooden_twister`: 30.0)
- Model capability gating comes directly from `model.allowed_inversions` (no model-key checks in `shaped.rs`; `solver_source_names_no_coaster_model_key` stays enforced!).

### Vibe vs global ASTM envelope
Consistent with `TASK-166` / `TASK-167`:
- Default `peak_g` sits at least 0.25 g inside `model.design_force_targets.maximum_vertical_g` (`within_vibe = true` -> enforces `REQUIRED_FORCE_HEADROOM_G = 0.25 g` against `design_force_targets` AND `within_limits` against the ASTM envelope).
- Players may step `peak_g` above the model's typical target up to `4.5 g` (e.g. 4.0 / 4.2 / 4.5 g on `wooden_twister`, or 4.5 g on `hybrid`), which is judged by the global ASTM F2291 comfort envelope (`car.within_limits`) and surfaces the amber `exceeds_model_feel` note in the builder UI — never a red refusal while comfortable under ASTM!
- Refusals:
  - Non-inverting model or disallowed inversion type -> `SolveRefusal::UnsupportedScope`
  - Entry speed below `minimum_inversion_speed_mps` -> `SolveRefusal::BelowMinimumSpeed`
  - Stall / insufficient height energy -> `SolveRefusal::Stall`
  - Unachievable peak force, envelope violation, minimum radius violation, or roll rate > 200°/s -> `SolveRefusal::InfeasibleTarget`

### Calibration & accuracy targets
- **Vertical loop & Raven turn:**
  - Analytic first guess uses `bottom_g = peak_g`, `top_g = top_feel.target_g()`, then up to `MAXIMUM_SHAPED_PREVIEWS` (4) simulator calibration runs using secant updates on `bottom_g` to match worst-car peak positive vertical g (`max_vertical_g`) within **0.20 g** of `peak_g` while maintaining positive top g (no unintended inversion drop/stall; within 0.35 g of `top_feel` target at apex where feasible) and full-train comfort (`within_limits`, plus `0.25 g` headroom when within model vibe).
  - For Raven turn, `roll_dx` is verified so every car's peak roll rate on the piece is `<= 200.0°/s` (targeting `<= 185°/s` at design speed).
- **Zero-g roll:**
  - Bisect `flank_angle` so the crest duration gives peak roll rate `<= 185°/s` for the actual train at `entry_speed_mps` (strictly `< 200°/s` at design speed and `+0.5 m/s`), then calibrate `pull_g` with secant updates (<= 4 previews) so worst-car peak positive vertical g matches `peak_g` within **0.20 g**, crest airtime stays near 0 g (`[-0.45, +0.45] g`), and every car passes the force budget.

### Save format, wire types, UI, and design tools
- **Enum evolution (backwards-compatible):**
  - Append `ShapedLoopLeft`, `ShapedLoopRight`, `ShapedZeroGRollLeft`, `ShapedZeroGRollRight`, `ShapedRavenTurnLeft`, `ShapedRavenTurnRight` to the end of `PieceId`.
  - Append `Loop`, `ZeroGRoll`, `RavenTurn` variants to `ElementIntent`, `ElementSpec`, `HumanReadableElementSpec`, and `BinaryElementSpec`.
  - Keep existing `v6` save compatibility and add save round-trip tests for every new inversion variant.
- **Features & scoring (`crates/sim/src/physics/features.rs`):**
  - Map the new shaped inversion `PieceId`s in `element_classes` (`VerticalLoop`, `HeartlineRoll`, `HalfLoop` / `Corkscrew`) so variety bonuses match classic inversions.
  - `inversion_passages` already reads `piece.inversion` and `piece.heartlining` automatically!
- **Wire grammar (`crates/sim-wasm/src/construction.rs`, `reports.rs`, `web/src/generated/`):**
  - `shape:loop:<left|right>:<peak_g>:<floaty|neutral|pressedIn>`
  - `shape:zeroGRoll:<left|right>:<peak_g>`
  - `shape:ravenTurn:<left|right>:<peak_g>:<floaty|neutral|pressedIn>`
  - Extend `ConstructionShapeSpecWire` with `Loop`, `ZeroGRoll`, and `RavenTurn` variants (`designSpeedMps`, `cars`, `hand`, `targetPeakG`, `deliveredPeakG`, `heightM`, `spanM`, `topFeel`/`topG`, `peakRollRateDegS`, etc.) and regenerate TypeScript types via `make generate-report-types`.
- **Builder UI (`web/src/construction-choices.ts`, `construction-shaped-elements.ts`, `construction.ts`, `candidate-force-readout.ts`):**
  - Add a `"Shaped inversions"` subgroup (or `"inversion"` entries under Shaped palette) showing only the inversions supported by the active model (filtered via model capabilities / `allowedInversions`, or cleanly refused on non-inverting models):
    - Vertical loop left / right (on `velocity_viper`, `hybrid`, `wooden_twister`)
    - Zero-g roll left / right (on `velocity_viper`, `hybrid`)
    - Raven turn left / right (on `velocity_viper`, `hybrid`)
  - Shaped inspector shows:
    - **Peak force** stepper (`3.5 / 3.8 / 4.0 / 4.2 / 4.5 g` for loops & Raven turns; `3.0 / 3.5 / 3.8 / 4.0 / 4.2 / 4.5 g` for zero-g rolls)
    - **Top feel** stepper (`Floaty` / `Neutral` / `Pressed in`) for vertical loops & Raven turns
    - Calibration readout: design speed, train cars, target vs calibrated peak g, resolved height × span (and roll rate for zero-g roll / Raven turn), plus amber `exceedsModelFeel` note when above model design targets.
- **Design tools (`crates/design-tools/src/commands.rs`):**
  - `place loop <left|right> [g=<peak_g>] [top=<floaty|neutral|pressed-in>]`
  - `place zero-g-roll <left|right> [g=<peak_g>]`
  - `place raven-turn <left|right> [g=<peak_g>] [top=<floaty|neutral|pressed-in>]`
  - Update `options shapes`, `probe`, `report` (Elements table), and `docs/designing-rides.md`.

---

## 4. Detailed Verification & Test Plan

### Rust unit & integration tests
1. **Acceptance matrix (`crates/sim/tests/shaped_elements.rs`):**
   - Across `velocity_viper` (3 & 6 cars), `hybrid` (4 & 8 cars), and `wooden_twister` (4, 8, 12 cars):
   - Entry speeds across each model's inversion range (e.g. 22–36 m/s for steel, 22–30 m/s for wooden steel-loop), peak forces `3.5 / 3.8 / 4.0 / 4.2 / 4.5 g`, top feels `Floaty / Neutral / PressedIn`, both `Left` and `Right` hands, and both cardinal and diagonal headings.
   - Verify:
     - Delivered peak vertical g within `0.20 g` of target at design speed.
     - Every car passes `within_limits` (ASTM comfort envelope) at design speed and `±0.5 m/s` under both default and construction-test physics configs; and at least `0.25 g` inside `design_force_targets` when within model vibe.
     - Monotonic sizing: at fixed `peak_g` and `top_feel`, loop height and Raven turn height increase with entry speed.
     - Roll-rate invariant: peak roll rate `<= 200.0°/s` for every car on every zero-g roll and Raven turn.
     - Inversion counting & heartlining: full test run records 1 inversion passage with expected `Inversion` and `Heartlining`; `wooden_twister` shaped loop uses `Material::Steel` supports.
     - Clearance & support routing: `Construction::append_shaped` succeeds with valid clearance and supports on flat terrain.
     - Typed refusals: `UnsupportedScope` on `hyper` / `family_coaster` (and zero-g roll / Raven turn on `wooden_twister`), `BelowMinimumSpeed` below `minimum_inversion_speed_mps`, `InfeasibleTarget` for out-of-range targets.
     - Determinism, canonicalization, geometry digest verification, `RideFile` JSON round-trip, and `TPSAVE` v6 binary save round-trip.
     - `solver_source_names_no_coaster_model_key` continues to pass.
     - Classic inversion pieces (`Loop`, `LargeLoop`, `SteelLoop`, `ZeroGRoll*`, `HalfLoop*`, `RavenRoll*`) remain completely unchanged.
2. **Native vs WASM parity (`make check-shaped-turn-arc-parity`):**
   - Extend `shaped_parity_cases_json` with shaped inversion parity cases (Velocity Viper loop, Timber Tumbler zero-g roll, Velocity Viper Raven turn, Briar Blitz steel loop) verified byte-equal between native and Node/WASM.
3. **Design tools & WASM bridge tests:**
   - `make test-design-tools`, `make check-report-types`, `crates/sim-wasm` construction & save tests.
4. **Web unit & Playwright E2E tests:**
   - `web/src/construction-shaped-elements.test.ts`, `web/src/candidate-force-readout.test.ts`, `web/src/construction-choices.test.ts`.
   - Playwright E2E tests in `web/e2e/construction.spec.ts`: build and place shaped vertical loop, zero-g roll, and Raven turn on `velocity_viper` / `hybrid` / `wooden_twister`, adjust peak-force and top-feel steppers, verify amber feel note when pushing above model targets, verify updated `shaped-hill-inspector-linux.png` snapshot.
5. **Full repo gates:**
   - `make lint`
   - `make test-rust`
   - `make test-web`
   - `make check-report-types`
   - `make check-shaped-turn-arc-parity`
   - `make bench-shaped-elements-cold` (all 45 cases <= 250 ms p95)
   - `make test-agreement`
   - `make ride-agreement` (Horizon Hawk & Daisy Dash unchanged)
   - Playwright `@fast`, `--project feature`, `--project journey`.
