# Simulation code review, 2026-11-03

Scope: `crates/sim` (lib.rs, guests, world, facilities, scenery, demo_world) and `crates/sim-wasm`. Track, physics and block internals are covered only where the bridge uses them.

## Summary

The simulation crate is in reasonable shape. Its modules have typed errors, deterministic collections (BTreeMap almost everywhere), and walking is a proper partitioned phase. The main problem is that much of the game now lives in the WebAssembly bridge. `WasmSimulation` has about 40 fields and owns the park tick, coaster operation, block analysis scheduling, support caching and every cross-entity placement rule. None of this can be tested or benchmarked natively without the bridge, it cannot run on threads, and it would be hard to save. The bridge is also where most duplication and stringly typed error handling sit. Most fixes are mechanical and can land one at a time.

## Findings, most useful first

### 1. Park rules and orchestration live in the bridge

`WasmSimulation` (`crates/sim-wasm/src/lib.rs:47-88`) owns the world, facilities, guests, a demo train, the player coaster and its block analysis. `tick` (`lib.rs:932-1084`) runs guests, the demo train, dispatches and finishes coaster trains, and detects test-run outcomes. The rules that span more than one entity are all bridge code: terrain under buildings (`lib.rs:495-506`), terrain against track and supports (`lib.rs:540-658`), scenery against facilities and track (`lib.rs:684-742`), facilities against track (`lib.rs:827-843`), paths against facilities (`lib.rs:2425-2457`). The incremental support cache (`lib.rs:5969-6061`) and `ThroughputEstimator` (`lib.rs:3011-3077`) are simulation algorithms too.

The worst effect is on testing. `crates/sim/src/guests/park_determinism_tests.rs:51-127` copies the bridge's tick by hand (the `200`/`30` physics remainder and the seat-to-car report mapping in `lib.rs:2334-2348`), so the thread-count determinism test checks a copy of the tick, not the one the game runs.

Proposal: add `crates/sim/src/park.rs` with a `Park` that owns `World`, `ParkFacilities`, `ParkGuests` and coasters. It would expose `tick_with(executor)` and commands such as `place_facility`, `apply_terrain` and `place_scenery` that return typed `Result`s. The bridge then only decodes arguments and encodes results, and the determinism test calls `Park::tick_with`. Size: large (about 2,000 lines moved), but it can be done in stages (see the order below).

### 2. Coaster state is about 25 loose fields with implicit invariants

The block fields (`lib.rs:69-81`), construction fields (`60-64`, `84-87`) and coaster fields (`65-68`) form a state machine that the code only enforces through `expect` ("open coaster has an operation", `lib.rs:982`; "validation has an installed layout", `1407`, `2242`) and by hand-written reset lists that must stay in sync (`clear_block_layout` `2382-2396`, `construction_start` `1240-1251`, `construction_append` `1920-1926`, `construction_delete` `1950-1953`). Three booleans (`block_baseline_validation`, `block_apply_best_requested`, `block_layout_is_automatic`) encode which search is pending. The test-run outcome is detected by comparing `construction_message` with literal strings (`lib.rs:1035-1039`, `1053`).

Proposal: a `Coaster { track, blocks: BlockAnalysis, mode }` with `mode` an enum `Editing | Testing(TestRunState) | Operating(OperationState)`, and a `BlockAnalysis` holding `installed`, `pending: Option<PendingSearch>` (`Placement`, `Manual`, `Baseline`) and the throughput estimate. A `TestOutcome` enum maps to player text only in the bridge. Move `ThroughputEstimator`, the fingerprint helpers (`2924-2953`) and `block_layout_change_pending` (`2233-2258`) into `track::blocks`. Size: medium.

### 3. Error handling flattens typed errors into strings and bools

Each domain has a `*_message: String` field (`lib.rs:51-54`, `63`, `76`) set as a side effect, while commands return `bool` and the worker calls a second getter. Typed errors are shown to players through `Debug` (`format!("{error:?}")` at `733`, `854`, `875`). Some getters change state: `construction_supports(&mut self)` writes `construction_message` (`2115`), and `construction_placed_supports` is called inside `tick` (`1061`) and `finish_live_train` (`2318`) only to fill the cache that `proximity_supports` then reads with `expect` (`2277`). There are also silent fallbacks: `path_preview` invents metadata with `unwrap_or` (`323`), while `set_world_surface` panics on bad JS input (`922`).

Proposal: once the `Park` commands from finding 1 return typed errors, add one `ParkError` to wire code and message table in the bridge, and have exports return `Result<_, JsError>` or a `{code, message}` value. Drop the message fields. Make the support cache an explicit `refresh_supports(&mut self)` step and keep getters `&self`. Size: medium, and it can be done one domain at a time.

### 4. Four wire formats with no shared schema

The bridge uses flat typed arrays whose strides appear only in doc comments, plus a hidden encoding of material in the piece-ID slot: steel is `-(id + 1)`, wood is `+1_000_000`, and `stride != 7` stands in for "supports" (`lib.rs:2531-2550`, `5956`, `6049`). It also uses tab-separated strings (`1304`, `1331`), JSON built by hand with `format!` (`771-777`, `889-911`, `1602-1776`, `2028-2061`, `2569-2606`), and `Debug` output used as IDs (`1277`, `1287`). Piece groups are derived by substring matches on `Debug` names (`2608-2627`). `json_string` exists twice (`lib.rs:2629`, `crates/sim/src/guests/details.rs:536`). The two sides have already drifted: the TypeScript `BlockSection` (`web/src/construction-protocol.ts:21-29`) lacks `poweredHoldStartMetres` and `poweredHoldPieceIndices`, and `BlockReport` lacks `source` and `placementWorkCompleted`.

Proposal: use `serde` with `serde-wasm-bindgen` for reports and snapshots that are not hot (block report, test report, facilities, scenery, choices, piece shapes), with `#[serde(rename_all)]` on enums in place of `Debug`. Add explicit `wire_name()` and `from_wire_name()` where strings are needed, and send material as its own field. Keep typed arrays for per-frame geometry, with stride constants in one module. Generating TypeScript types (for example with `tsify`) would catch drift at compile time. The tradeoff is a somewhat larger wasm binary. Size: medium.

### 5. Duplicated logic

- The chain of `construction.pieces()` with `disconnected_segments()` appears 11 times in the bridge (`lib.rs:573`, `591`, `721`, `830`, `1327`, `1960`, `2088`, `2129`, `2161`, `2519`, ...). Add `Construction::all_pieces()` and `all_entries()`.
- Footprint containment is written five times (`facilities.rs:112-117`, `180-184`, `254-256`; `lib.rs:497-502`, `2473-2486`) and facility height (12 or 4) twice (`facilities.rs:164-170`, `lib.rs:2869-2874`). Add `Building::contains_tile` and `Building::clearance_box`.
- Facts about each facility kind are scattered: default price (`facilities.rs:217-223`), service values (`295-303`), minimum height (`310-314`), ride access offsets (`88-95`, `204-205`), and wire names twice in the bridge (`lib.rs:798-805`, `892-896`). Put them in one `FacilityKind::spec()` table.
- Wire codes are repeated: path slope inline (`lib.rs:248-254`), in `path_slope_code` (`2730`) and in `World::state_hash` (`world/mod.rs:636-642`); rotation and direction at `lib.rs:811-820` and `897-898`; scenery names as two hand-written tables (`2815-2858`).
- The nearest-exit search is copied in `ParkGuests::finish_coaster_train` and `close_coaster` (`guests/park.rs:118-138`, `145-164`). The `lift_speed: 5.0` physics config appears three times (`lib.rs:168`, `2014`, `2909`). FNV hashing is implemented four times (`lib.rs`, `world/mod.rs`, `guests/walking.rs`, `physics/mod.rs`).

Each item is small.

### 6. The placeholder scaffold in `crates/sim/src/lib.rs`

`Agent`, `Simulation` and `Phase` (`lib.rs:12-213`) are the original demo. Only `examples/tick_bench.rs:7` and their own tests use them, and the threaded determinism test at `lib.rs:305-326` tests this placeholder rather than guests. `Executor` lives in the same file. The aliases at `lib.rs:104` (`guest_behavior`, `park_guests`, ...) give every type two paths, and `facilities.rs` uses both. Proposal: move `Executor` into `executor.rs`, delete the placeholder, port `tick_bench` to `ParkGuests` and drop the aliases. Size: small.

### 7. Demo setup is split, and the player coaster has construction names

The demo world is built in three places: terrain, paths and scenery in `demo_world.rs`, facilities in `WasmSimulation::new` (`lib.rs:103-149`), and a hard-coded demo coaster that runs every tick (`958-976`). That coaster has its own exports (`1088-1138`) and its own support renderer, which hard-codes `hybrid_model` (`6081`). Operating the player's coaster goes through `construction_*` calls (`construction_set_open`, `construction_riders`), and `COASTER_PLACE_ID = u32::MAX` (`97`) limits the park to one player coaster. One latent bug: `place_facility_at` checks construction pieces against the demo train envelope (`839`) rather than `construction.model().train`. It does no harm today because all three models share an 18 by 2 by 2.5 m envelope. Proposal: `demo_park()` returns a complete `Park`, the demo coaster becomes an ordinary coaster, and operating calls are renamed `coaster_*`. Size: medium. Supporting several coasters is a product decision.

### 8. Determinism and threading coverage is narrower than the design describes

The design spec says each tick is made of phases that write per-partition output. In practice only walking movement goes through the executor (`guests/walking.rs:422`). `BehaviorSimulation::tick_with_exiting` (`guests/behavior.rs:442` onwards) is a long serial loop that changes state in place, and `ParkGuests::tick` fixes `SequentialExecutor` (`park.rs:220`). This is deterministic because it is serial, but it is not yet parallel. The bridge `state_hash` (`lib.rs:2221`) covers walking only, not behaviours, world, facilities or trains. `support_render_cache` is a `HashMap`, and its iteration order feeds the support list in `proximity_supports` (`2271-2281`). The proximity scan appears to use the list only through lookups, but a `BTreeMap` would settle the question. Proposal: `Park::state_hash` covering everything, then split the behaviour tick into observe, decide and resolve phases when threading matters. Size: small for the hash, large for the phases.

### 9. Test organisation

About 3,500 lines of `sim-wasm/src/lib.rs` are tests (`3080-6597`, nine modules). `world_tests` (`3178-5121`) is mostly block-section tests. The sim crate mixes three conventions: inline `mod tests` (`facilities.rs:620-1517`, followed by more public functions at `1825-1908`), sibling files through `#[path]` (`park.rs:1206`), and `dir/tests.rs`. Proposal: pick `dir/tests.rs`, and split the bridge into `lib.rs` (exports), `wire.rs`, `render.rs` and `tests/`. Most bridge tests move into sim with finding 1. Six exports are never called from the web code: `world_state_hash`, `state_hash`, `construction_can_append`, `construction_preview_valid`, `construction_preview_reason` and `construction_set_block_sections`. Some of them are used only by Rust tests. The tick's magic numbers (appeal `100`, physics remainder `200`/`30` in units of 1/6000 s) should come from `TIME_STEP` and a tick-rate constant.

## Proposed refactor order

Each step should land on its own with `make test` passing.

1. Delete the placeholder simulation, move `Executor`, drop the module aliases, and port `tick_bench`.
2. Add the small helpers: `Construction::all_pieces` and `all_entries`, `Building` rectangle and clearance box, `FacilityKind::spec`, enum wire-name functions, and a shared FNV hasher. Replace the copies.
3. Split `sim-wasm/src/lib.rs` into files and move its tests out, with no behaviour change.
4. Move block analysis (`BlockAnalysis` and `ThroughputEstimator`) into `track::blocks` with enum state, and move its tests with it.
5. Introduce `sim::park::Park` with `tick_with` and `state_hash`, delegate the bridge tick to it, and rewrite the determinism test against `Park::tick_with`.
6. Move the terrain, scenery, facility and path commands into `Park` with typed errors, add the single error mapping in the bridge, and remove the message fields.
7. Move structured reports to `serde` and generate TypeScript types.
8. Later, when threads are needed: split the behaviour tick into phases.
