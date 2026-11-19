# Agent design tools: a headless builder, an estimate-versus-test check and a park description file (TASK-149)

> **For agentic workers:** Work items are in section 12, in order, each with an acceptance check, a targeted gate, a time cap and a suggested model. Section 11 says which in-flight branch each item waits for; do not start an item before its branch has merged. Run only the named targeted suites while working; `make ci` runs once, in the close-out item. Heavy commands go through make targets or `tools/run-in-budget.sh`; never two broad gates at once, and browser runs need a free `PLAYWRIGHT_PORT`. Tolerances and timings marked "starting value" are replaced by measured numbers, which are written back here.

**Owner input (2026-11-18):** tools that "pay continued dividends". Agents should design and QA coasters and parks properly and easily, with the same builder core as the game, and eventually each coaster model gets one realistic, good showcase coaster in the park. Ride feel targets: steel pullouts 3 to 3.5 g, wood about 2.5 g, airtime about −1 g, brief peaks fine, banked turns that turn sideways force into positive force. The owner wants loud failures and no fallback code paths.

**Goal:** an agent can design a ride from a terminal, seeing at every step what the builder panel offers and estimates, test it, and read a report, in seconds per action and a few hundred tokens per reply. Every saved ride is checked automatically: the estimate the game shows equals the estimate the tool showed, and both agree with the measured test run within a stated tolerance. A park is described in one file that the game loads as a starting park, with a report on ride scores, force targets and guest outcomes.

**Tickets:** TASK-149 (this work), TASK-146 (estimate disagrees with the test run), TASK-144 (longitudinal force on a pullout), TASK-145 (floater readout), TASK-139 (Horizon Hawk airtime), TASK-140 (demo layout), TASK-109 (fuller starter park). **Neighbours in flight:** `force-rule` (TASK-126), `builder-speed-2` (TASK-133), `builder-qol` (TASK-142, TASK-143), `demo-ride` (TASK-140 with TASK-146 QA).

## 1. What the code says (facts that shape the design)

1. **The game's builder bridge already runs natively.** `crates/sim-wasm` builds as `cdylib` and `rlib`, and `crates/sim-wasm/examples/construction_update_bench.rs` drives `WasmSimulation` natively through the same calls as `web/src/worker.ts` (`construction_select`, `construction_append`, `construction_force_preview_json`, `construction_candidate_force_preview_json`, `construction_speed_match_json`, and so on). The headless builder therefore links `theme-park-sim-wasm` and calls the bridge itself. Nothing has to move out of `sim-wasm` to make this possible.
2. **A small part of the builder's behaviour lives in TypeScript.** `worker.ts` decides the order of bridge calls: it takes the end speed from the force preview (`constructionEndSpeed`), passes it to `construction_choices_with_preview`, caches it (`constructionEndSpeedCache`), and on a non-exact selection swaps the piece for the speed-matched recommendation (`speedMatchedSelection`). It also scales station coordinates (`construction_start(x * 2, y * 2, z / 2, heading * 2)`) and drives the finish search in time slices. This is about thirty lines, and it is the only place where a native tool could drift from the game. The update bench already carries one hand-written copy of it.
3. **The game shows several estimates, computed in different ways.** The placed-track overlay and the candidate readout both come from `physics::preview_open_track`: one front-car reading, 50 ms steps, open track, train starting at rest in the middle of the station, then `correct_sample_forces` recomputes forces with interpolated acceleration (`physics/preview.rs`). The hill inspector uses a separate closed-form prediction (`track::predicted_hill_forces`). The rated window comes from `effective_rated_speeds`.
4. **The test run measures something different from what the estimate predicts.** `construction_test` and `Park::run_ride_test` step the train at 200 Hz on the closed circuit, starting at distance 0 at 2 m/s, and record every car. The report's statistics and force advice are the most extreme values across cars; only the track ribbon is the front car (`ConstructionTestRunRecorder::finish_with_front_car`). So the estimate (front car) and the report (worst car) differ by design today, on top of any numerical difference. This is probably a large share of TASK-146, and the agreement check has to report the two effects separately.
5. **One agreement test exists, for one ride.** `timber_preview_agrees_with_closed_test_run` in `crates/sim/tests/player_circuits.rs` compares the preview with a front-car run on Timber Tumbler under default physics (0.5 m/s and 0.3 g per sample). The game's test uses a 5 m/s lift and a 2 m/s start, which that test does not use.
6. **A test report already keeps every car's samples.** `TestRun.rating_inputs` holds one entry per car with its per-metre samples (`average_car_ratings` collects them). Per-piece worst-car peaks can be read from an existing report with a small accessor and no save-format change. I read this from `testing.rs:432`; the first work item that uses it confirms it.
7. **A park holds one player coaster.** `Park` has a single `construction`, a single `coaster_open`, and `COASTER_PLACE_ID = u32::MAX`; the demo coaster is a separate hard-coded `DemoCoaster`. `docs/reviews/2026-11-03-simulation-code.md` calls several coasters "a product decision". A park with one showcase coaster per model cannot be loaded until that changes (section 14, question 1).
8. **Existing ride formats.** `web/e2e/player/circuits.txt` holds five circuits as piece ids with `+chain` and `-chain`; ten code files and the Makefile read it. Session files (`theme-park-session` version 1) are journals of worker commands with tick stamps and state-hash checkpoints, replayed by `replaySessionIntoWorker`. Park saves are `TPSAVE\x03` plus a checksum and a postcard payload of the whole `WasmSimulation`; older versions are rejected and there is no migration, so a committed save breaks at every format change.
9. **The existing native instrument is separate code.** `crates/sim/examples/circuit_report.rs` builds with `Construction::append` on a flat world and measures worst-car raw readings with its own loop. It does not go through the bridge and does not show the builder's estimates, which is why agents never saw them.

## 2. Design in one page

```
crates/sim                 Construction, preview_open_track, Train, recorders, scoring
                           + track/ride_file.rs        ride file types (plain serde data)
                           + physics piece peaks       per-piece front-car and worst-car peaks from a TestRun
crates/sim-wasm            WasmSimulation (the bridge the game worker calls)
                           + builder_session.rs        the one place that orders bridge calls for a builder action
                           + park_description.rs       builds a WasmSimulation from a park description
crates/design-tools (new)  binary heartline-design: ride commands, ride report, agreement check, park build and report
web/src/worker.ts          calls the same select and estimate functions (after the switch in work item 5)
web/e2e/rides/             one generic browser spec that replays any ride file through the real builder UI
content/rides, content/parks   ride files and park descriptions (source of truth, committed)
```

Decisions, with reasons:

- **Native Rust on the `sim-wasm` rlib, not Node on the wasm build.** It needs no `make build`, runs about twice as fast as wasm, and is the route the update bench already proves. The cost is fact 2: the TypeScript ordering logic. It is handled in two steps. First `builder_session.rs` reproduces that ordering in Rust, and the browser agreement spec (layer 2) fails if the game and the tool ever show different numbers. Then, once `builder-speed-2` and `builder-qol` have merged, the worker calls the same Rust functions and the TypeScript copies are deleted, so drift is no longer possible.
- **The saved ride is a small JSON ride file, not a session and not a park save.** A session ties a ride to tick stamps and state hashes that change with every physics change; a park save is binary and breaks at every format bump. The ride file lists builder steps with exact piece ids, so it survives physics changes, diffs cleanly, and can be placed at a different station position. The tool exports a park save (for the Park menu's import) and a session (for `gameTest.replaySession`) from it on request.
- **The ride file is the session state.** Each tool call replays the file's steps through the bridge, applies the new commands, prints the result and writes the file back. Agents call tools from separate shell commands and cannot keep a process with open pipes alive between them, so a long-lived JSON-lines server would be awkward to use. Replay costs about 30 to 40 ms per piece natively (TASK-133's numbers), so roughly 3 s for a 70-piece ride; several commands fit in one call. If measured replay exceeds 4 s, add a snapshot next to the file; not before.
- **Commands are short text lines and replies are compact text tables; `--json` gives the full wire-shaped data.** The stated problem with browser driving is token cost. `place EjectorHill16x96` and a ten-line table cost far less than JSON both ways. The browser spec and the tests use `--json`, whose shapes are the bridge's own wire types.
- **A new crate for the binary.** `crates/design-tools` (package `theme-park-design-tools`) depends on `theme-park-sim-wasm` and `theme-park-sim`. A `[[bin]]` inside `sim-wasm` would also be built for `wasm32`, and examples cannot be shared with tests without `#[path]` includes (as `tests/circuit_report.rs` does today).

## 3. Layer 1: ride file, builder session and command line

### 3a. Ride file

`content/rides/<name>.ride.json`. Types live in `crates/sim/src/track/ride_file.rs` as plain serde structs (`deny_unknown_fields`), so sim tests, the session and the TypeScript specs read the same file. Station coordinates are the units of the worker's `construction-start` action (tiles, height steps, heading name).

```json
{
  "format": "heartline-ride",
  "version": 1,
  "name": "Horizon Hawk showcase",
  "model": "hyper",
  "station": { "x": 241, "y": 249, "z": 2, "heading": "north" },
  "carsPerTrain": 8,
  "trains": 1,
  "steps": [
    { "chain": true },
    { "place": "LevelToSteepUp", "note": "lift" },
    { "place": "SteepUp" },
    { "chain": false },
    { "place": "WideLevelToSixtyDown", "note": "first drop" },
    { "place": "FloaterHill24x160" },
    { "place": "LaunchLong", "launchSpeed": 31.3 },
    { "place": "element:immelmann-left" },
    { "close": true }
  ],
  "expect": {
    "excitement": [6.0, 8.0],
    "sustainedMaxVerticalG": [3.0, 3.5],
    "sustainedMinVerticalG": [-1.2, -0.8],
    "maxLateralG": 0.6,
    "inversions": 0,
    "airtimeMomentsAtLeast": 4
  }
}
```

A step is one of: `place` (an exact piece id or `element:<id>`, optional `launchSpeed`, optional `note`), `chain`, `close`, or `blockSections`. Each installed editable block records its boundary and brake start in metres and its brake piece indices; the step also records capacity and search completion metadata. For example: `{ "blockSections": [{ "boundaryMetres": 42.5, "brakeStartMetres": 30.0, "brakePieceIndices": [4, 5] }], "trainCapacity": 1, "capacitySearchComplete": true, "placementSearchComplete": true }`. Speed-matched choices, finish-search results, and block layouts are stored as the results they resolved to, so replay never runs a search. `expect` holds the acceptance targets of section 5c; unknown keys fail the parse. The piece ids and block values in the example are illustrative; real files use catalog ids, and an unknown id fails with the list of ids the builder offers at that point.

### 3b. Builder session (`crates/sim-wasm/src/builder_session.rs`)

`BuilderSession` owns a `WasmSimulation` and exposes one method per builder action. Each method makes the bridge calls in the worker's order and returns typed results. To return typed results, the five estimate functions in `construction.rs` (`construction_force_preview_json`, `construction_candidate_force_preview_json`, `construction_choices_with_preview`, `construction_speed_match_json`, `construction_elements`) are each split into a function that returns the wire struct and a `_json` wrapper that serialises it; behaviour does not change. The session does not compute any estimate itself.

| Session action | Bridge calls, in order | Mirrors in the game |
|---|---|---|
| start | `construction_select("model:<key>")`, `construction_start` with the worker's coordinate scaling | station tool |
| estimates | force preview, end speed (last sample when the status is `reachedEnd`), choices with that speed, elements, hill options, speed turns, candidate preview, speed match | `sendConstruction(true)` |
| select | speed match with the cached end speed, then `construction_select` with the recommended id unless exact | `construction-select` |
| place | `construction_finish_invalidate`, `construction_append`, estimates | `construction-append` |
| chain, launch speed, undo, delete, cars per train, train count, colours, price | the one bridge call each | the matching worker action |
| finish | `construction_finish_search_start`, `construction_finish_search_advance` until done, `construction_finish_preview`, `construction_finish_confirm` | Finish coaster |
| close | `construction_close` | Close circuit |
| test | `ride_test("player")`, `ride_info_json("player")` | the ride window's Test button |
| auto blocks | `construction_auto_blocks`, `construction_advance_block_analysis` until complete | Operate tab |

Every refusal is an error carrying the bridge's message (`construction_message`, `construction_preview_reason`, `construction_finish_reason`); the session never skips a step or substitutes a piece. `WasmSimulation::save_state` and `load_state` return `Result<_, JsValue>`, and I believe `JsValue` construction panics off wasm; the first work item checks this and, if so, splits each into a native function returning `Result<_, String>` plus a thin wasm wrapper.

### 3c. Commands (`tools/design`, a wrapper that builds the release binary inside the budget and runs it)

`tools/design ride <file> <command> [<command> ...]`, or commands on standard input, one per line. The file is written back only if every command succeeded; on the first failure the tool prints the error and the state at that point and exits non-zero.

| Command | What it does | Reply |
|---|---|---|
| `new --model <key> --station x,y,z,heading [--cars n]` | creates the ride file and places the station | state |
| `state` | nothing | piece count, open end (position, heading, slope, bank, height above ground, offset to the station entry), chain, predicted end speed, can close, can finish and why not, can test |
| `choices [group]` | lists what the builder offers at the open end | per choice: id, name, group, refusal reason, would tunnel, rated window, margin of the predicted entry speed inside the window, whether it is the speed-matched recommendation |
| `estimate <id> [<id> ...]` | selects each piece as the palette would and reads the candidate preview; restores the selection | per piece: entry speed, rated window and margin, lowest and highest vertical g, lateral g, longitudinal g, each with its rating (weak, good, great, too much, dangerous), preview status; for hills also the inspector's prediction |
| `place <id> [--matched] [--launch-speed v] [--note text]` | selects (exactly, or with the speed match the palette applies) and places | the placed id, the estimate over the placed piece, the new state |
| `chain on` / `chain off`, `launch-speed <piece> <v>`, `cars <n>`, `trains <n>`, `auto-blocks` | the matching builder control | state |
| `undo [n]`, `truncate <step>` | undoes placements and removes their steps | state |
| `finish`, `close` | finish search and confirm, or close when the ends meet | pieces added, or the game's reason |
| `test` | runs the test and prints the report | scores, score breakdown, notes, statistics, and the per-piece table below |
| `report [--samples]` | replays and tests without changing the file | as `test`; `--samples` adds per-metre speed and forces |
| `agreement` | the native estimate-versus-measured check (section 4b) | the agreement table and verdict |
| `map` | nothing | a coarse top-down character map of the footprint and a height profile, for orientation |
| `export --save <path>` / `export --session <path>` | writes a park save or a replayable session | path written |

Per-piece table printed by `test` and `report` (one row per piece; this replaces the `circuit_report` table and adds the estimate columns; the numbers below are illustrative):

```
#   piece               entry m/s  window      margin  est v+ / v-    meas front v+ / v-   meas worst v+ / v-   lat   long  band   notes
12  FloaterHill24x160   33.1       27.0-34.5   +1.4    3.6 / -0.9     3.6 / -0.9           3.7 / -1.1           0.1   0.4   great
13  SpeedTurnLarge32Left 32.4      29.0-35.0   +2.6    3.3 / 1.0      3.3 / 1.0            3.5 / 0.9            0.1   0.2   great
```

Other entry points: `tools/design check <ride files>` (replay, test, `expect` targets, native agreement; exit code for gates), `tools/design park build <park file> --save <path>`, `tools/design park report <park file>`.

## 4. Layer 2: the estimate-versus-test agreement check

### 4a. What is compared

For every placed piece, with the ride's own cars per train:

- **Tool estimate at placement:** the candidate preview at the moment the piece was selected at the open end (entry speed, lowest and highest vertical g, largest lateral and longitudinal g). This is what a player reads before placing (TASK-142).
- **Tool estimate on the finished track:** the same quantities over that piece from the placed-track preview of the complete track, which is what the coloured overlay shows.
- **Game estimate:** both of the above as the browser's worker sent them, and as the panel renders them.
- **Measured, front car** and **measured, worst car:** peaks over the piece from the test run's per-car samples.

Three checks, each reported per piece and per axis:

1. **Tool equals game.** The numbers the worker sent equal the tool's within 1e-6 (m/s and g). They come from the same Rust code compiled natively and to wasm, so only math-library rounding can differ; anything larger is drift and fails. The panel text must equal the game's own formatter applied to the worker's numbers (`candidateForceReadout`, `hillReadout`), which checks the display without re-implementing its formatting.
2. **Finished-track estimate versus the test.** Gate both the front-car comparison and the worst-car comparison. Entry speed must be within 0.5 m/s and each force peak within 0.2 g.
3. **Placement estimate versus measured worst car.** Report force differences above 0.3 g or a milder estimated rating band, but do not fail the gate. Continue the candidate over a virtual level straight until the whole train clears the selected piece. Classify each failure as downstream track when it arises while any car is on a later piece, and otherwise as estimate error; investigate and report any estimate errors. The candidate readout labels the estimate provisional until the following piece exists.

"Same filtering" follows the code: today both sides are unfiltered; after `force-rule` merges both sides are the filtered per-metre samples (its plan runs the same evaluator at 50 ms in the preview and 5 ms in the test), and the tolerance in check 2 absorbs the step difference or is re-measured.

**2026-11-19 decision (dt-estimate):** Check 1 uses 1e-6; the finished-track gates use 0.5 m/s and 0.2 g; the informational placement comparison retains 0.3 g and the rating-band rule. The same-start open-versus-closed demo comparison measured zero sampled difference and its separate longitudinal bound was restored from 0.4 to 0.3 g.

The six native ride reports pass the finished-track gates. The largest entry-speed difference was 0.401 m/s; reported finished-track force peaks matched the front and worst-car measurements in this run. Placement failure messages: Briar Blitz 1 downstream / 0 estimate errors; Browser agreement fast circuit 0 / 0; Daisy Dash 0 / 0; Horizon Hawk 0 / 0; Timber Tumbler 0 / 1; Velocity Viper 1 / 0. Timber's estimate error is the steep-to-level virtual run-out changing train-averaged longitudinal acceleration while cars straddle the join; its placement estimate is 0.656 g versus 0.304 g measured. Velocity's brake estimates 0.330 g versus 0.646 g with a car on the following launch.

### 4b. Native check (fast, in Rust tests)

`crates/design-tools/src/agreement.rs` replays a ride through `BuilderSession`, records the placement-time estimate at each step, runs the test, and builds the table with checks 2 and 3. Per-piece measured peaks come from a new accessor in `crates/sim` that groups each car's samples by `piece_index` (fact 6). `crates/design-tools/tests/rides.rs` runs it for every file in `content/rides/`. It generalises `timber_preview_agrees_with_closed_test_run` to every ride and to the game's test configuration, and that test is deleted when this one gates.

### 4c. Browser check (one generic spec for any ride)

`web/e2e/rides/ride-agreement.ride.spec.ts`, in a new Playwright project `rides` (one worker, like journeys), has one test per file in `content/rides/`. For each ride it starts a new park, places the station, sets cars per train, and places every step through the visible controls with `placeCircuit` from `web/e2e/player/place-circuit.ts` (extended for elements, launch speeds and the ride file reader). After each selection and each placement it reads a new test hook, `window.gameTest.constructionEstimates()` (the latest construction state's selected piece, force preview, candidate preview, speed match and selected-hill estimate), and compares it with the tool's `--json` output for the same step (check 1). After the last step it closes the circuit, runs the test through the UI, and applies checks 2 and 3 to `gameTest.testingReport()` plus the tool's measured table, and the ride's `expect` targets. It saves a screenshot of the finished ride for the designer to look at. The expected values come from running `tools/design check --json` in the spec's setup, so nothing is committed that can go stale.

A ride that uses a piece the helper cannot select through the UI fails with "Piece X is not offered by the palette". That is the intended signal: either the helper needs the new control, or the builder really does not offer the piece.

## 5. Layer 3: park description, park report and showcase rides

### 5a. Park description

`content/parks/<name>.park.json`, built by `crates/sim-wasm/src/park_description.rs` into a `WasmSimulation` by calling the same bridge functions the worker calls for each tool (`terrain_apply`, `path_build`, `place_facility_at`, `set_facility_price`, `place_scenery`, and `BuilderSession` for coasters). Any refused entry fails the whole load with the entry's index and the game's message.

```json
{
  "format": "heartline-park",
  "version": 1,
  "name": "Horizon Hawk showcase park",
  "terrain": { "preset": "flat", "edits": [] },
  "entrance": { "at": [218, 246, 8], "facing": "south" },
  "paths": [
    { "kind": "path", "style": 0, "route": [[218, 250, 8], [218, 256, 8], [240, 256, 8]] },
    { "kind": "queue", "style": 0, "route": [[240, 255, 8], [240, 251, 8]] }
  ],
  "facilities": [
    { "kind": "food_stall", "at": [220, 254, 8], "rotation": 2, "price": 4 },
    { "kind": "restroom", "at": [226, 254, 8], "rotation": 2 }
  ],
  "scenery": [],
  "coasters": [
    { "ride": "content/rides/horizon-hawk.ride.json", "station": { "x": 241, "y": 249, "z": 2, "heading": "north" }, "open": true, "price": 6 }
  ]
}
```

Each entry carries exactly the parameters of its bridge function; the implementing agent reads those signatures (and how a queue path is tied to a ride) before fixing field names. `coasters` is a list, and the loader fails with "this build supports one player coaster per park" when it has more than one entry, until the simulation supports several (question 1). The game loads a description with `?park=<name>`: `main.ts` fetches `parks/<name>.park.json` (bundled from `content/parks/` with its ride files inlined at build time) and the worker's `init` passes it to a new constructor `WasmSimulation::from_park_description(seed, guest_count, json)`. Session export is switched off for described parks in the same way as after loading a save, because the session format only names the `new` and `demo` presets. The existing `new` and `demo` presets do not change in this plan.

### 5b. Park report

`tools/design park report <file> [--guests 2000] [--minutes 20] [--seed 42]` builds the park, tests every coaster, opens them, simulates the given time natively with guests, and prints:

- per coaster: scores, `expect` verdicts, the worst piece per axis, the agreement verdict, trains and throughput estimate;
- guests: rides taken per ride and per hour against the throughput estimate, queue lengths at the end, the share of guests who rode each coaster, guest ratings of each coaster, spending per guest and revenue per facility, the share of guests with an unmet need, and guests who could not reach a destination;
- park facts: path and queue tile counts, facilities by kind, anything that failed to place.

The aggregates that do not exist yet are added as one `ParkOutcomes` struct in `crates/sim/src/guests/` so the game can show them later; the work item starts by listing which already exist. I have not verified which guest aggregates exist beyond revenue, riders and per-guest details.

### 5c. Showcase rides and their acceptance targets

One ride per model. It is the model's reference ride everywhere: the fixture the Rust tests read, the ride the browser spec replays, and the ride in the model's showcase park. There is no second set of "test circuits". The current five circuits are the starting points.

| Model (key) | Excitement band | Sustained highest vertical g | Sustained lowest vertical g | Largest lateral g |
|---|---|---|---|---|
| Timber Tumbler (`hybrid`) | 8 to 10 | 3.0 to 3.5 | −1.2 to −0.8 | 0.6 |
| Velocity Viper (`velocity_viper`) | 6 to 9 | 3.0 to 3.5 | −1.2 to −0.8 | 0.6 |
| Horizon Hawk (`hyper`) | 6 to 8 | 3.0 to 3.5 | −1.2 to −0.8 | 0.6 |
| Briar Blitz (`wooden_twister`) | 5 to 7 | 2.3 to 2.8 | −0.9 to −0.6 (its limit is −1.0) | 0.6 |
| Daisy Dash (`family_coaster`) | 2.5 to 4.5 | question 2 | question 2 | 0.5 |

"Sustained" means held for at least 0.5 s (positive) or 0.3 s (airtime) by any car, computed from the report's per-car samples; shorter peaks are judged only by the model's limits, which matches "brief peaks fine". The 0.5 s and 0.3 s are my choice, to be adjusted if they misclassify a real element. Every ride must also: complete a lap without rollback, stay inside the model's force limits on every car, carry no rated-speed note, pass the three agreement checks, and be seen by the owner once (question 3). Window margins are reported but not gated, because TASK-139 wants hills entered near the top of their windows. Starling Spiral (TASK-148) gets its ride when the model lands.

## 6. How an agent uses it

`docs/designing-rides.md`, one page, linked from `docs/status.md` and the README's test section. It is a plain document so Codex and Claude agents both find it; no skill wrapper is needed. Its content:

1. Start: `tools/design ride content/rides/<name>.ride.json new --model <key> --station x,y,z,heading`.
2. Build in a loop: `state`, then `choices <group>`, then `estimate` on two or three candidates, then `place`. Read the estimate that `place` returns; `undo` if it is wrong. Put several commands in one call.
3. Size the lift or launch for the first element's window, shed speed with a climb or a short brake run, and bring the train home slowly (the three rules from the good-circuits plan).
4. `finish` or `close`, then `test`. Read the per-piece table: a row where the estimate and the worst-car measurement disagree is a tool bug to report, not something to design around.
5. To change an early piece, edit the step in the file and run `report`; the replay stops at the first step that no longer fits and prints the state there.
6. `tools/design check <file>` must pass. Then run the browser spec once for the ride and look at its screenshot.
7. Do not edit `expect` to make a ride pass. If a target looks wrong, say so and ask.
8. For the owner to see it: `export --save`, then Park menu, Import.

## 7. Gates and make targets

- `make design-tools`: builds the release binary (also done on demand by `tools/design`).
- `make ride-check [RIDE=<file>]`: `tools/design check` on one or all rides. It also runs as the `rides` integration test under `cargo test --workspace`, so `make test` and `make ci` include it. It joins `make check` if all rides take under 60 s (measured in work item 3).
- `make ride-agreement [RIDE=<name>]`: `npm run e2e -- --project rides`, one worker. `make e2e` and therefore `make ci` include it because they run every project.
- `make park-report PARK=<file>`.
- `make circuit-report` is removed in work item 7.

All of these are make recipes, so they run through `tools/run-in-budget.sh` like every other target. The browser spec costs about two to three minutes per ride (estimate from the Timber journey's 94 s of construction); work item 7 removes the full-circuit placements from the five model journeys so the total browser time stays about the same.

## 8. What this replaces or absorbs

| Existing | Becomes |
|---|---|
| `web/e2e/player/circuits.txt` and `circuits.ts` | `content/rides/*.ride.json` and a `readRide` helper; the ten readers and the Makefile switch in work item 7 |
| `crates/sim/examples/circuit_report.rs`, `crates/sim/tests/circuit_report.rs`, `make circuit-report` | `tools/design ride <file> report`; its `@turn` and `@hill` directives become `place --matched`; its tests are ported, then the example is deleted |
| The hand-written worker sequence in `construction_update_bench.rs` | calls `BuilderSession` (after `builder-speed-2` merges, because TASK-133 compares the bench's digests with a baseline) |
| `speedMatchedSelection`, the end-speed cache and the call ordering in `worker.ts` | the same functions in `builder_session.rs`, called by the worker (work item 5) |
| `place-circuit.ts` | kept as the one UI placement driver; gains elements, launch speeds and cars per train |
| `window.gameTest` | one new hook, `constructionEstimates()`; nothing removed |
| Score-band, force-limit and feature assertions in `tests/known_circuit_statistics.rs` and the five model journeys | the ride's `expect` block, checked natively and in the browser spec; the Rust file keeps only what `expect` cannot say (both physics configurations), the journeys keep their model-specific interactions on a short prefix |
| `timber_preview_agrees_with_closed_test_run` | the native agreement check on every ride |
| Session files, `record-sessions.mjs`, `rehash-sessions.mjs` | unchanged; they test whole-worker determinism over ticks, which ride files do not |

## 9. Findings from the demo-ride redesign (to be filled in)

The agent on branch `demo-ride` is rebuilding the demo coaster through the real builder UI and will report builder friction and a per-piece estimate-versus-test table. When it reports, record here:

- **Its estimate-versus-test table**, and beside it the table `tools/design ride <demo ride> agreement` prints for the same track. The two must match; a difference is a bug in this tool.
- **Which causes it found for the disagreement** (candidates from the code: front car against worst car; 50 ms steps with interpolated acceleration; open track against closed circuit; start at rest mid-station against 2 m/s at distance 0). These decide the fix in work item 6.
- **Builder friction**, each item marked as (a) a builder change for a ticket, (b) information the `choices`, `estimate` or `state` replies should carry, or (c) nothing to do.
- **How long and how many tokens the UI route took**, as the baseline the tool is compared with in work item 10.

The native agreement report was run on all five catalog rides on 2026-11-18. Each row lists placement, finished-track and measured-front entry speeds in m/s, followed by minimum and maximum vertical, absolute lateral and absolute longitudinal force peaks in g. The worst column is the per-axis envelope across cars. Station index 0 is omitted because it is not a placed piece. Failures are attributed to check 2 (finished estimate versus front car) or check 3 (placement estimate versus the worst-car envelope). Missing values are shown as an em dash. The demo-ride comparison requested above remains pending the other branch.

### briar-blitz

```text
piece check2 check3 entry[placement/finished/test m/s] placement[min/max/lat/long g] finished[min/max/lat/long g] front[min/max/lat/long g] worst[min/max/lat/long g]
1 LevelToGentleUp          Passed Passed 2.079/2.079/2.001 0.898/1.105/0.000/0.463 0.898/1.105/0.000/0.463 0.898/1.105/0.000/0.463 0.898/1.105/0.000/0.463
2 GentleUp                 Passed Passed 5.000/5.000/5.000 0.894/0.894/0.000/0.447 0.894/0.894/0.000/0.447 0.894/0.894/0.000/0.447 0.894/0.894/0.000/0.447
3 GentleUp                 Passed Passed 5.000/5.000/5.000 0.894/0.894/0.000/0.447 0.894/0.894/0.000/0.447 0.894/0.894/0.000/0.447 0.894/0.894/0.000/0.447
4 TurnSmallStandalone      Passed Passed 5.000/5.000/5.000 0.881/0.929/0.179/0.451 0.881/0.929/0.179/0.451 0.881/0.929/0.179/0.451 0.881/0.929/0.179/0.451
5 GentleUpToLevel          Passed Passed 5.000/5.000/5.000 0.830/0.992/0.000/0.463 0.830/0.992/0.000/0.463 0.830/0.992/0.000/0.463 0.830/0.992/0.000/0.463
6 Straight                 Passed Passed 5.000/5.000/5.000 1.000/1.000/0.000/0.000 1.000/1.000/0.000/0.000 1.000/1.000/0.000/0.000 1.000/1.000/0.000/0.000
7 Straight                 Passed Passed 5.000/5.000/5.000 1.000/1.000/0.000/0.000 1.000/1.000/0.000/0.000 1.000/1.000/0.000/0.000 1.000/1.000/0.000/0.208
8 LevelToGentleDown        Passed Failed 5.000/5.000/5.000 0.813/1.000/0.000/0.312 0.813/1.000/0.000/0.312 0.813/1.000/0.000/0.308 0.416/1.000/0.000/0.370
9 BankLeft                 Passed Passed 6.999/6.999/7.133 0.633/0.894/0.897/0.206 0.633/0.894/0.897/0.206 0.633/0.894/0.897/0.202 0.598/0.894/1.150/0.202
10 BankedTurnStandalone     Passed Failed 13.585/13.585/13.752 0.633/2.127/1.322/0.475 0.633/2.127/1.322/0.475 0.634/2.127/1.322/0.476 0.634/2.665/2.041/0.690
11 UnbankLeft               Passed Failed 20.476/20.476/20.658 0.491/0.893/1.410/0.038 0.491/0.893/1.410/0.038 0.491/0.893/1.410/0.037 0.401/0.893/1.659/0.191
12 GentleDownToLevel        Passed Failed 23.874/23.874/23.882 0.895/4.100/0.000/1.205 0.895/4.100/0.000/1.205 0.895/4.100/0.007/1.192 0.895/4.405/0.013/1.192
13 Straight                 Passed Passed 26.330/26.330/26.359 1.000/1.000/0.000/0.234 1.000/1.000/0.000/0.234 1.000/1.000/0.000/0.219 1.000/1.000/0.000/0.219
14 Straight                 Passed Passed 26.732/26.732/26.731 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010
15 Straight                 Passed Passed 26.673/26.673/26.672 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010
16 Straight                 Passed Passed 26.614/26.614/26.613 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010
17 Straight                 Passed Passed 26.555/26.555/26.554 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010
18 Straight                 Passed Passed 26.496/26.496/26.494 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010
19 Straight                 Passed Passed 26.437/26.437/26.435 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.055
20 TopHat                   Passed Passed 26.377/26.377/26.376 -0.392/3.322/0.000/0.624 -0.392/3.322/0.000/0.624 -0.392/3.322/0.000/0.610 -0.392/3.322/0.000/0.613
21 Straight                 Passed Passed 25.874/25.874/25.879 1.000/1.000/0.000/0.051 1.000/1.000/0.000/0.051 1.000/1.000/0.000/0.043 1.000/1.000/0.000/0.043
22 Straight                 Passed Passed 25.889/25.889/25.888 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010
23 Straight                 Passed Passed 25.829/25.829/25.829 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010
24 Straight                 Passed Passed 25.770/25.770/25.770 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010
25 Straight                 Passed Passed 25.711/25.711/25.711 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010
26 Straight                 Passed Passed 25.652/25.652/25.651 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010
27 Straight                 Passed Passed 25.593/25.593/25.592 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010
28 Straight                 Passed Passed 25.534/25.534/25.533 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010
29 Straight                 Passed Passed 25.475/25.475/25.474 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010
30 Straight                 Passed Passed 25.416/25.416/25.415 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010
31 TightTwisterLeft         Passed Passed 25.357/25.357/25.356 1.000/1.000/7.203/0.010 1.000/1.000/7.203/0.010 1.000/1.000/7.203/0.010 1.000/1.000/7.203/0.010
32 TightTwisterRight        Passed Passed 25.283/25.283/25.282 1.000/1.000/7.156/0.010 1.000/1.000/7.156/0.010 1.000/1.000/7.156/0.010 1.000/1.000/7.156/0.010
33 TightTwisterLeft         Passed Passed 25.210/25.210/25.208 1.000/1.000/7.107/0.010 1.000/1.000/7.107/0.010 1.000/1.000/7.107/0.010 1.000/1.000/7.107/0.010
34 TightTwisterLeft         Passed Passed 25.136/25.136/25.134 1.000/1.000/7.070/0.009 1.000/1.000/7.070/0.009 1.000/1.000/7.070/0.009 1.000/1.000/7.070/0.009
35 Straight                 Passed Passed 25.063/25.063/25.060 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009
36 Straight                 Passed Passed 25.004/25.004/25.001 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009
37 Straight                 Passed Passed 24.945/24.945/24.942 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009
38 Straight                 Passed Passed 24.885/24.885/24.883 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009
39 Straight                 Passed Passed 24.826/24.826/24.824 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009
40 Straight                 Passed Passed 24.767/24.767/24.764 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009
41 Straight                 Passed Passed 24.708/24.708/24.705 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009
42 Straight                 Passed Passed 24.649/24.649/24.646 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009
43 Straight                 Passed Passed 24.589/24.589/24.587 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009
44 Straight                 Passed Passed 24.530/24.530/24.527 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009
45 Straight                 Passed Passed 24.471/24.471/24.468 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009
46 Straight                 Passed Passed 24.412/24.412/24.409 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009
47 Straight                 Passed Passed 24.352/24.352/24.350 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009
48 Straight                 Passed Passed 24.293/24.293/24.290 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009
49 Straight                 Passed Passed 24.234/24.234/24.231 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009
50 Straight                 Passed Passed 24.174/24.174/24.172 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009
51 Straight                 Passed Passed 24.115/24.115/24.112 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009
52 Straight                 Passed Passed 24.056/24.056/24.053 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009
53 Straight                 Passed Passed 23.996/23.996/23.994 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009
54 Straight                 Passed Passed 23.937/23.937/23.934 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009
55 Straight                 Passed Passed 23.878/23.878/23.875 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009
56 TurnLargeRightStandalone Passed Passed 23.818/23.818/23.815 1.000/1.000/2.376/0.009 1.000/1.000/2.376/0.009 1.000/1.000/2.376/0.010 1.000/1.000/2.376/0.010
57 Straight                 Passed Passed 23.621/23.621/23.619 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009
58 TurnSmallStandalone      Passed Passed 23.562/23.562/23.559 1.000/1.000/4.657/0.009 1.000/1.000/4.657/0.009 1.000/1.000/4.657/0.009 1.000/1.000/4.657/0.009
8 PlacementEstimateVsWorstCar: minimum vertical g (estimate 0.813 g differs from measurement 0.416 g by 0.397 g, over the 0.3 g tolerance)
10 PlacementEstimateVsWorstCar: maximum vertical g (estimate 2.127 g differs from measurement 2.665 g by 0.539 g, over the 0.3 g tolerance)
10 PlacementEstimateVsWorstCar: maximum vertical g (estimated rating Good is milder than measured rating Great)
10 PlacementEstimateVsWorstCar: lateral g (estimate 1.322 g differs from measurement 2.041 g by 0.718 g, over the 0.3 g tolerance)
10 PlacementEstimateVsWorstCar: lateral g (estimated rating Strong is milder than measured rating Dangerous)
11 PlacementEstimateVsWorstCar: lateral g (estimated rating Strong is milder than measured rating TooMuch)
12 PlacementEstimateVsWorstCar: maximum vertical g (estimate 4.100 g differs from measurement 4.405 g by 0.305 g, over the 0.3 g tolerance)
Agreement: fail
```

### daisy-dash

```text
piece check2 check3 entry[placement/finished/test m/s] placement[min/max/lat/long g] finished[min/max/lat/long g] front[min/max/lat/long g] worst[min/max/lat/long g]
1 LevelToGentleUp          Passed Passed 2.079/2.079/2.001 0.898/1.105/0.000/0.463 0.898/1.105/0.000/0.463 0.898/1.105/0.000/0.463 0.898/1.105/0.000/0.463
2 TurnSmallStandalone      Passed Passed 5.000/5.000/5.000 0.881/0.929/0.180/0.451 0.881/0.929/0.180/0.451 0.881/0.929/0.180/0.451 0.881/0.929/0.180/0.451
3 GentleUpToLevel          Passed Passed 5.000/5.000/5.000 0.830/1.000/0.000/0.463 0.830/1.000/0.000/0.463 0.830/1.000/0.000/0.463 0.830/1.000/0.000/0.463
4 LevelToGentleDown        Passed Failed 5.000/5.000/5.000 0.815/0.991/0.000/0.283 0.815/0.991/0.000/0.283 0.815/0.991/0.000/0.279 0.487/0.991/0.000/0.324
5 OneTileGentleDown        Passed Passed 7.215/7.215/7.285 0.894/0.894/0.000/0.164 0.894/0.894/0.000/0.164 0.894/0.894/0.000/0.159 0.894/0.894/0.000/0.159
6 GentleDownToLevel        Passed Passed 9.049/9.049/9.350 0.905/1.763/0.000/0.498 0.905/1.763/0.000/0.498 0.905/1.763/0.000/0.490 0.905/2.000/0.000/0.490
7 FamilyHill4x40           Passed Passed 14.143/14.143/14.261 -0.237/2.113/0.000/0.900 -0.237/2.113/0.000/0.900 -0.237/2.114/0.000/0.887 -0.237/2.116/0.000/0.887
8 LevelToGentleDown        Passed Failed 13.921/13.921/14.024 -0.163/0.941/0.000/0.477 -0.163/0.941/0.000/0.477 -0.163/0.941/0.000/0.468 -0.479/0.941/0.000/0.572
9 OneTileGentleDown        Passed Passed 15.436/15.436/15.452 0.894/0.894/0.000/0.174 0.894/0.894/0.000/0.174 0.894/0.894/0.000/0.164 0.894/0.894/0.000/0.164
10 GentleDownToLevel        Passed Passed 16.370/16.370/16.516 0.919/2.644/0.000/0.745 0.919/2.644/0.000/0.745 0.919/2.644/0.000/0.733 0.919/2.895/0.000/0.733
11 SpeedTurnLarge20Left     Passed Passed 19.634/19.634/19.713 1.000/2.253/0.149/0.362 1.000/2.253/0.149/0.362 1.000/2.253/0.150/0.363 1.000/2.253/0.152/0.363
12 LevelToGentleUp          Passed Passed 19.733/19.733/19.736 0.907/2.849/0.000/0.625 0.907/2.849/0.000/0.625 0.907/2.849/0.000/0.614 0.903/2.849/0.000/0.740
13 OneTileGentleUp          Passed Passed 18.926/18.926/18.834 0.894/0.894/0.000/0.147 0.894/0.894/0.000/0.147 0.894/0.894/0.000/0.135 0.894/0.894/0.000/0.135
14 GentleUpToLevel          Passed Passed 18.090/18.090/18.062 -0.386/0.967/0.000/0.571 -0.386/0.967/0.000/0.571 -0.386/0.967/0.000/0.561 -0.386/0.971/0.000/0.561
15 OneTileStraight          Passed Passed 14.311/14.311/14.254 1.000/1.000/0.000/0.198 1.000/1.000/0.000/0.198 1.000/1.000/0.000/0.189 1.000/1.000/0.000/0.189
16 FamilyHill4x40           Passed Failed 13.910/13.910/13.882 -0.056/1.988/0.000/0.739 -0.056/1.988/0.000/0.739 -0.056/1.988/0.000/0.728 -0.056/1.988/0.000/0.816
17 FamilyHill4x40           Passed Failed 12.951/12.951/12.991 -0.024/1.945/0.000/0.745 -0.024/1.945/0.000/0.745 -0.024/1.946/0.000/0.734 -0.024/1.948/0.000/0.785
18 TerrainRise              Passed Passed 12.755/12.755/12.778 -0.030/1.935/0.000/0.700 -0.030/1.935/0.000/0.700 -0.030/1.935/0.000/0.691 -0.030/1.942/0.000/0.691
19 LevelToGentleDown        Passed Failed 12.398/12.398/12.294 0.218/0.944/0.000/0.403 0.218/0.944/0.000/0.403 0.218/0.944/0.000/0.395 -0.091/0.949/0.000/0.475
20 GentleDownToLevel        Passed Passed 12.887/12.887/12.928 0.896/2.120/0.000/0.608 0.896/2.120/0.000/0.608 0.896/2.120/0.000/0.597 0.896/2.359/0.000/0.597
21 SpeedTurnLarge16Left     Passed Passed 16.583/16.583/16.635 1.000/1.703/0.040/0.189 1.000/1.703/0.040/0.189 1.000/1.703/0.040/0.190 1.000/1.703/0.042/0.310
22 Brakes                   Passed Passed 16.717/16.717/16.619 1.000/1.000/0.000/0.313 1.000/1.000/0.000/0.313 1.000/1.000/0.000/0.313 1.000/1.000/0.000/0.313
23 FamilyHill4x40           Passed Passed 13.609/13.609/13.559 -0.036/1.966/0.000/0.836 -0.036/1.966/0.000/0.836 -0.033/1.964/0.000/0.824 -0.033/1.964/0.000/0.824
24 OneTileStraight          Passed Passed 12.770/12.770/12.839 1.000/1.000/0.000/0.174 1.000/1.000/0.000/0.174 1.000/1.000/0.000/0.167 1.000/1.000/0.000/0.167
25 OneTileStraight          Passed Passed 13.180/13.180/13.206 1.000/1.000/0.000/0.084 1.000/1.000/0.000/0.084 1.000/1.000/0.000/0.078 1.000/1.000/0.000/0.078
26 Straight                 Passed Passed 13.329/13.329/13.321 1.000/1.000/0.000/0.013 1.000/1.000/0.000/0.013 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.125
27 TerrainFall              Passed Failed 13.275/13.275/13.258 -0.112/2.003/0.000/0.701 -0.112/2.003/0.000/0.701 -0.109/2.000/0.000/0.695 -0.270/2.169/0.000/0.744
28 TerrainFall              Passed Passed 14.046/14.046/14.068 -0.313/2.186/0.000/0.767 -0.313/2.186/0.000/0.767 -0.311/2.184/0.000/0.765 -0.505/2.322/0.000/0.892
29 SpeedTurnLarge16Left     Passed Passed 15.309/15.309/15.313 1.000/1.546/0.135/0.157 1.000/1.546/0.135/0.157 1.000/1.544/0.136/0.158 1.000/1.544/0.137/0.158
30 TerrainRise              Passed Passed 15.535/15.535/15.525 -0.441/2.308/0.000/0.856 -0.441/2.308/0.000/0.856 -0.439/2.306/0.000/0.849 -0.439/2.306/0.000/0.849
31 Straight                 Passed Passed 14.744/14.744/14.705 1.000/1.000/0.000/0.140 1.000/1.000/0.000/0.140 1.000/1.000/0.000/0.139 1.000/1.000/0.000/0.139
32 Straight                 Passed Passed 14.113/14.113/14.098 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.177
33 FamilyHill4x40           Passed Passed 14.039/14.039/14.024 -0.112/2.031/0.000/0.784 -0.112/2.031/0.000/0.784 -0.109/2.029/0.000/0.771 -0.109/2.029/0.000/0.831
34 TerrainFall              Passed Failed 13.258/13.258/13.265 -0.176/2.092/0.000/0.723 -0.176/2.092/0.000/0.723 -0.173/2.089/0.000/0.721 -0.357/2.222/0.000/0.828
35 Straight                 Passed Passed 14.594/14.594/14.660 1.000/1.000/0.000/0.125 1.000/1.000/0.000/0.125 1.000/1.000/0.000/0.124 1.000/1.000/0.000/0.313
36 Brakes                   Passed Passed 15.010/15.010/14.852 1.000/1.000/0.000/0.313 1.000/1.000/0.000/0.313 1.000/1.000/0.000/0.313 1.000/1.000/0.000/0.313
37 Brakes                   Passed Passed 11.305/11.305/11.073 1.000/1.000/0.000/0.312 1.000/1.000/0.000/0.312 1.000/1.000/0.000/0.312 1.000/1.000/0.000/0.312
4 PlacementEstimateVsWorstCar: minimum vertical g (estimate 0.815 g differs from measurement 0.487 g by 0.328 g, over the 0.3 g tolerance)
8 PlacementEstimateVsWorstCar: minimum vertical g (estimate -0.163 g differs from measurement -0.479 g by 0.316 g, over the 0.3 g tolerance)
8 PlacementEstimateVsWorstCar: minimum vertical g (estimated rating Weak is milder than measured rating Good)
16 PlacementEstimateVsWorstCar: longitudinal g (estimated rating Fine is milder than measured rating Strong)
17 PlacementEstimateVsWorstCar: longitudinal g (estimated rating Fine is milder than measured rating Strong)
19 PlacementEstimateVsWorstCar: minimum vertical g (estimate 0.218 g differs from measurement -0.091 g by 0.309 g, over the 0.3 g tolerance)
27 PlacementEstimateVsWorstCar: minimum vertical g (estimated rating Weak is milder than measured rating Good)
34 PlacementEstimateVsWorstCar: minimum vertical g (estimated rating Weak is milder than measured rating Good)
34 PlacementEstimateVsWorstCar: longitudinal g (estimated rating Fine is milder than measured rating Strong)
Agreement: fail
```

### horizon-hawk

```text
piece check2 check3 entry[placement/finished/test m/s] placement[min/max/lat/long g] finished[min/max/lat/long g] front[min/max/lat/long g] worst[min/max/lat/long g]
1 LevelToGentleUp          Passed Passed 2.079/2.034/2.003 0.898/1.105/0.000/0.463 0.898/1.105/0.000/0.463 0.898/1.105/0.000/0.463 0.898/1.105/0.000/0.463
2 GentleToSteepUp          Passed Passed 5.000/5.000/5.000 0.708/0.929/0.000/0.712 0.708/0.929/0.000/0.712 0.708/0.929/0.000/0.712 0.708/0.929/0.000/0.712
3 SteepUp                  Passed Passed 5.000/5.000/5.000 0.707/0.707/0.000/0.707 0.707/0.707/0.000/0.707 0.707/0.707/0.000/0.707 0.707/0.707/0.000/0.707
4 SteepUp                  Passed Passed 5.000/5.000/5.000 0.707/0.707/0.000/0.707 0.707/0.707/0.000/0.707 0.707/0.707/0.000/0.707 0.707/0.707/0.000/0.707
5 SteepToGentleUp          Passed Passed 5.000/5.000/5.000 0.691/0.894/0.000/0.712 0.691/0.894/0.000/0.712 0.691/0.894/0.000/0.712 0.691/0.894/0.000/0.712
6 GentleUpToLevel          Passed Passed 5.000/5.000/5.000 0.830/1.000/0.000/0.463 0.830/1.000/0.000/0.463 0.830/1.000/0.000/0.463 0.816/1.000/0.000/0.469
7 WideLevelToSixtyDown     Passed Failed 5.000/5.000/5.000 0.435/0.982/0.000/0.541 0.435/0.982/0.000/0.541 0.435/0.982/0.000/0.532 -0.803/0.982/0.000/1.195
8 WideSixtyDownToLevel     Passed Failed 17.222/17.222/17.624 0.500/2.970/0.000/0.254 0.500/2.971/0.000/0.254 0.500/2.971/0.000/0.240 0.500/3.162/0.000/0.323
9 Brakes                   Passed Passed 34.552/34.579/34.616 1.000/1.000/0.000/0.323 1.000/1.000/0.000/0.323 1.000/1.000/0.000/0.323 1.000/1.000/0.000/0.323
10 FloaterHill24x160        Passed Passed 33.201/33.200/33.126 -0.676/3.197/0.000/0.455 -0.676/3.197/0.000/0.455 -0.665/3.187/0.000/0.436 -0.665/3.187/0.000/0.455
11 SpeedTurnLarge32Left     Passed Passed 32.437/32.437/32.368 1.000/3.152/0.072/0.343 1.000/3.152/0.072/0.343 1.000/3.139/0.076/0.338 1.000/3.139/0.077/0.338
12 Brakes                   Passed Passed 31.832/31.860/31.768 1.000/1.000/0.000/0.320 1.000/1.000/0.000/0.320 1.000/1.000/0.000/0.321 1.000/1.000/0.000/0.321
13 Brakes                   Passed Passed 30.227/30.256/30.155 1.000/1.000/0.000/0.321 1.000/1.000/0.000/0.321 1.000/1.000/0.000/0.320 1.000/1.000/0.000/0.320
14 FloaterHill20x128        Passed Passed 28.626/28.626/28.519 -0.604/3.074/0.000/0.534 -0.604/3.074/0.000/0.534 -0.587/3.059/0.000/0.519 -0.587/3.059/0.000/0.541
15 FloaterHill20x128        Passed Passed 27.989/27.989/27.887 -0.512/2.995/0.000/0.530 -0.512/2.995/0.000/0.529 -0.496/2.981/0.000/0.515 -0.496/2.981/0.000/0.521
16 SpeedTurnLarge28Left     Passed Passed 27.435/27.435/27.340 1.000/2.979/0.152/0.403 1.000/2.979/0.152/0.402 1.000/2.959/0.158/0.399 1.000/2.959/0.159/0.399
17 Brakes                   Passed Passed 27.059/26.955/26.921 1.000/1.000/0.000/0.317 1.000/1.000/0.000/0.317 1.000/1.000/0.000/0.317 1.000/1.000/0.000/0.317
18 FloaterHill16x112        Passed Passed 25.254/25.116/25.105 -0.036/3.036/0.000/0.689 -0.019/3.012/0.000/0.683 -0.020/3.013/0.000/0.673 -0.020/3.013/0.000/0.680
19 FloaterHill12x96         Passed Passed 24.572/24.421/24.449 -0.122/3.114/0.000/0.796 -0.105/3.089/0.000/0.789 -0.105/3.090/0.000/0.780 -0.105/3.090/0.000/0.782
20 FloaterHill12x96         Passed Passed 24.081/23.930/23.963 -0.076/3.043/0.000/0.778 -0.059/3.018/0.000/0.771 -0.059/3.019/0.000/0.762 -0.059/3.019/0.000/0.763
21 FloaterHill12x96         Passed Passed 23.669/23.519/23.550 -0.030/2.975/0.000/0.761 -0.014/2.951/0.000/0.754 -0.014/2.952/0.000/0.746 -0.014/2.952/0.000/0.746
22 Straight                 Passed Passed 23.258/23.109/23.139 1.000/1.000/0.000/0.145 1.000/1.000/0.000/0.145 1.000/1.000/0.000/0.135 1.000/1.000/0.000/0.317
23 Brakes                   Passed Passed 23.447/23.289/23.292 1.000/1.000/0.000/0.317 1.000/1.000/0.000/0.317 1.000/1.000/0.000/0.317 1.000/1.000/0.000/0.317
24 SpeedTurnLarge20Left     Passed Passed 21.328/21.172/21.114 1.000/2.512/0.049/0.376 1.000/2.482/0.061/0.370 1.000/2.471/0.066/0.370 1.000/2.471/0.068/0.370
25 FloaterHill6x64          Passed Passed 20.979/20.831/20.782 -0.023/2.845/0.000/1.013 -0.007/2.820/0.000/1.001 0.002/2.810/0.000/0.989 0.002/2.810/0.000/0.989
26 FloaterHill6x64          Passed Passed 20.256/20.106/20.078 0.008/2.779/0.000/0.983 0.024/2.754/0.000/0.972 0.030/2.744/0.000/0.956 0.030/2.748/0.000/0.970
27 FloaterHill6x64          Passed Passed 19.971/19.820/19.832 0.039/2.731/0.000/1.012 0.055/2.706/0.000/1.001 0.062/2.696/0.000/0.985 0.062/2.700/0.000/0.985
28 FloaterHill8x64          Passed Passed 19.686/19.534/19.505 -0.198/3.040/0.000/1.152 -0.178/3.010/0.000/1.140 -0.170/2.999/0.000/1.123 -0.170/3.001/0.000/1.123
29 FloaterHill8x64          Passed Passed 19.276/19.123/19.137 -0.159/2.981/0.000/1.113 -0.138/2.951/0.000/1.100 -0.131/2.940/0.000/1.082 -0.131/2.944/0.000/1.211
30 Brakes                   Passed Passed 18.985/18.720/18.731 1.000/1.000/0.000/0.313 1.000/1.000/0.000/0.311 1.000/1.000/0.000/0.313 1.000/1.000/0.000/0.313
31 SpeedTurnLarge16Left     Passed Passed 17.077/16.785/16.753 1.000/1.710/0.043/0.178 1.000/1.671/0.047/0.171 1.000/1.667/0.050/0.171 1.000/1.667/0.052/0.309
32 Brakes                   Passed Passed 16.612/16.405/16.373 1.000/1.000/0.000/0.312 1.000/1.000/0.000/0.312 1.000/1.000/0.000/0.312 1.000/1.000/0.000/0.312
33 TerrainFall              Passed Failed 13.485/13.324/13.183 -0.140/2.014/0.000/0.728 -0.116/1.993/0.000/0.717 -0.097/1.976/0.000/0.704 -0.234/2.050/0.000/0.721
34 OneTileStraight          Passed Passed 13.896/13.896/13.786 1.000/1.000/0.000/0.093 1.000/1.000/0.000/0.093 1.000/1.000/0.000/0.093 1.000/1.000/0.000/0.204
7 PlacementEstimateVsWorstCar: minimum vertical g (estimate 0.435 g differs from measurement -0.803 g by 1.237 g, over the 0.3 g tolerance)
7 PlacementEstimateVsWorstCar: minimum vertical g (estimated rating Weak is milder than measured rating Good)
7 PlacementEstimateVsWorstCar: longitudinal g (estimate 0.541 g differs from measurement 1.195 g by 0.654 g, over the 0.3 g tolerance)
7 PlacementEstimateVsWorstCar: longitudinal g (estimated rating Fine is milder than measured rating Strong)
8 PlacementEstimateVsWorstCar: maximum vertical g (estimated rating Good is milder than measured rating Great)
33 PlacementEstimateVsWorstCar: minimum vertical g (estimated rating Weak is milder than measured rating Good)
Agreement: fail
```

### timber-tumbler

```text
piece check2 check3 entry[placement/finished/test m/s] placement[min/max/lat/long g] finished[min/max/lat/long g] front[min/max/lat/long g] worst[min/max/lat/long g]
1 LevelToGentleUp          Passed Passed 2.079/2.079/2.001 0.898/1.105/0.000/0.463 0.898/1.105/0.000/0.463 0.898/1.105/0.000/0.463 0.898/1.105/0.000/0.463
2 GentleToSteepUp          Passed Passed 5.000/5.000/5.000 0.708/0.929/0.000/0.712 0.708/0.929/0.000/0.712 0.708/0.929/0.000/0.712 0.708/0.929/0.000/0.712
3 SteepToGentleUp          Passed Passed 5.000/5.000/5.000 0.691/0.894/0.000/0.712 0.691/0.894/0.000/0.712 0.691/0.894/0.000/0.712 0.691/0.894/0.000/0.712
4 GentleUpToLevel          Passed Passed 5.000/5.000/5.000 0.830/0.994/0.000/0.463 0.830/0.994/0.000/0.463 0.830/0.994/0.000/0.463 0.830/0.994/0.000/0.463
5 LevelToGentleDown        Passed Failed 5.000/5.000/5.000 0.820/0.999/0.000/0.294 0.820/0.999/0.000/0.294 0.820/0.999/0.000/0.290 0.460/0.999/0.000/0.340
6 GentleToSteepDown        Passed Failed 6.989/6.989/7.160 0.406/0.892/0.000/0.222 0.406/0.892/0.000/0.222 0.406/0.892/0.000/0.218 -0.124/0.892/0.000/0.306
7 SteepToGentleDown        Passed Failed 15.244/15.244/15.378 0.708/2.052/0.000/0.473 0.708/2.052/0.000/0.473 0.708/2.052/0.000/0.466 0.708/2.545/0.000/0.466
8 GentleDownToLevel        Passed Passed 22.048/22.048/22.096 0.898/3.734/0.000/1.081 0.898/3.734/0.000/1.081 0.899/3.735/0.000/1.067 0.899/4.018/0.000/1.067
9 EjectorHill16x96         Passed Passed 24.815/24.815/24.854 -0.687/3.294/0.000/0.740 -0.687/3.294/0.000/0.740 -0.686/3.294/0.000/0.726 -0.686/3.294/0.000/0.726
10 EjectorHill16x96         Passed Passed 24.608/24.608/24.615 -0.587/3.208/0.000/0.726 -0.587/3.208/0.000/0.726 -0.587/3.208/0.000/0.715 -0.587/3.208/0.000/0.721
11 ZeroGRollFastLeft        Passed Passed 24.127/24.127/24.155 -0.369/3.326/0.467/1.093 -0.369/3.326/0.467/1.093 -0.369/3.326/0.468/1.083 -0.391/3.326/0.469/1.114
12 ZeroGRollFastRight       Passed Passed 23.522/23.522/23.548 -0.400/3.236/0.482/1.125 -0.400/3.236/0.482/1.125 -0.400/3.236/0.483/1.113 -0.424/3.236/0.483/1.113
13 SpeedTurnLarge24Left     Passed Passed 23.088/23.088/23.091 1.000/3.128/0.230/0.639 1.000/3.128/0.230/0.639 1.000/3.128/0.231/0.641 1.000/3.128/0.231/0.643
14 EjectorHill12x80OuterLeft Passed Passed 23.036/23.036/23.043 -0.272/3.163/0.365/0.826 -0.272/3.163/0.365/0.826 -0.272/3.163/0.365/0.815 -0.272/3.163/0.365/0.821
15 StallLeft                Passed Passed 22.466/22.466/22.496 -0.607/3.031/0.729/0.891 -0.607/3.031/0.729/0.891 -0.607/3.031/0.730/0.881 -0.607/3.031/0.730/1.143
16 Brakes                   Passed Passed 21.850/21.850/21.933 1.000/1.000/0.000/0.317 1.000/1.000/0.000/0.317 1.000/1.000/0.000/0.317 1.000/1.000/0.000/0.317
17 SpeedTurnLarge20Left     Passed Passed 19.774/19.774/19.859 1.000/2.196/0.178/0.324 1.000/2.196/0.178/0.324 1.000/2.219/0.169/0.329 1.000/2.219/0.170/0.329
18 FloaterHill12x96         Passed Passed 19.377/19.377/19.504 0.441/2.295/0.000/0.547 0.441/2.295/0.000/0.547 0.430/2.311/0.000/0.542 0.430/2.311/0.000/0.553
19 WaveTurnLeft             Passed Passed 18.757/18.757/18.926 1.066/1.498/0.971/0.310 1.066/1.498/0.971/0.310 1.067/1.505/0.971/0.310 1.065/1.505/0.972/0.310
20 FloaterHill12x80         Passed Passed 18.517/18.517/18.661 0.244/2.574/0.000/0.748 0.244/2.574/0.000/0.748 0.229/2.595/0.000/0.744 0.229/2.595/0.000/0.744
21 Straight                 Passed Passed 17.963/17.963/18.162 1.000/1.000/0.000/0.154 1.000/1.000/0.000/0.275 1.000/1.000/0.000/0.144 1.000/1.000/0.000/0.316
22 Brakes                   Passed Passed 18.130/18.130/18.241 1.000/1.000/0.000/0.315 1.000/1.000/0.000/0.315 1.000/1.000/0.000/0.316 1.000/1.000/0.000/0.316
23 Brakes                   Passed Passed 15.177/15.177/15.295 1.000/1.000/0.000/0.314 1.000/1.000/0.000/0.314 1.000/1.000/0.000/0.314 1.000/1.000/0.000/0.314
24 Straight                 Passed Passed 11.656/11.656/11.875 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007
25 TurnLargeRightStandalone Passed Passed 11.562/11.562/11.782 1.000/1.000/0.552/0.007 1.000/1.000/0.552/0.007 1.000/1.000/0.574/0.007 1.000/1.000/0.574/0.007
26 Straight                 Passed Passed 11.250/11.250/11.473 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007
27 Straight                 Passed Passed 11.155/11.155/11.379 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007
28 Straight                 Passed Passed 11.060/11.060/11.285 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007
29 Straight                 Passed Passed 10.964/10.964/11.190 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007
30 Straight                 Passed Passed 10.868/10.868/11.095 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007
31 Straight                 Passed Passed 10.772/10.772/10.999 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007
32 Straight                 Passed Passed 10.675/10.675/10.903 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007
33 Straight                 Passed Passed 10.577/10.577/10.807 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007
34 Straight                 Passed Passed 10.479/10.479/10.710 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007
35 Straight                 Passed Passed 10.381/10.381/10.613 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007
36 Straight                 Passed Passed 10.282/10.282/10.515 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007
37 Straight                 Passed Passed 10.182/10.182/10.417 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007
38 Straight                 Passed Passed 10.082/10.082/10.318 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007
39 Straight                 Passed Passed 9.982/9.982/10.219 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006
40 Straight                 Passed Passed 9.881/9.881/10.119 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006
41 Straight                 Passed Passed 9.779/9.779/10.018 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006
42 Straight                 Passed Passed 9.677/9.677/9.918 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006
43 Straight                 Passed Passed 9.574/9.574/9.816 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006
44 Straight                 Passed Passed 9.470/9.470/9.714 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006
45 Straight                 Passed Passed 9.366/9.366/9.611 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006
46 OneTileStraight          Passed Passed 9.261/9.261/9.508 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006
47 HalfTileStraight         Passed Passed 9.235/9.235/9.482 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006
48 HalfTileStraight         Passed Passed 9.222/9.222/9.469 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006
49 Straight                 Passed Passed 9.208/9.208/9.456 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006
50 OneTileStraight          Passed Passed 9.102/9.102/9.352 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006
51 TurnSmallStandalone      Passed Passed 9.076/9.076/9.326 1.000/1.000/0.684/0.006 1.000/1.000/0.684/0.006 1.000/1.000/0.724/0.006 1.000/1.000/0.724/0.006
52 Straight                 Passed Passed 8.898/8.898/9.154 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006
53 OneTileStraight          Passed Passed 8.790/8.790/9.048 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006
54 OneTileStraight          Passed Passed 8.762/8.762/9.021 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006
55 TurnSmallStandalone      Passed Passed 8.735/8.735/8.994 1.000/1.000/0.633/0.006 1.000/1.000/0.633/0.006 1.000/1.000/0.672/0.006 1.000/1.000/0.672/0.006
5 PlacementEstimateVsWorstCar: minimum vertical g (estimate 0.820 g differs from measurement 0.460 g by 0.360 g, over the 0.3 g tolerance)
6 PlacementEstimateVsWorstCar: minimum vertical g (estimate 0.406 g differs from measurement -0.124 g by 0.530 g, over the 0.3 g tolerance)
7 PlacementEstimateVsWorstCar: maximum vertical g (estimate 2.052 g differs from measurement 2.545 g by 0.493 g, over the 0.3 g tolerance)
Agreement: fail
```

### velocity-viper

```text
piece check2 check3 entry[placement/finished/test m/s] placement[min/max/lat/long g] finished[min/max/lat/long g] front[min/max/lat/long g] worst[min/max/lat/long g]
1 LaunchLong               Passed Passed 2.221/2.221/2.003 1.000/1.000/0.000/0.571 1.000/1.000/0.000/0.571 1.000/1.000/0.000/0.571 1.000/1.000/0.000/0.571
2 LaunchLong               Passed Passed 19.187/19.187/19.050 1.000/1.000/0.000/0.571 1.000/1.000/0.000/0.571 1.000/1.000/0.000/0.571 1.000/1.000/0.000/0.571
3 Launch                   Passed Passed 26.996/26.996/26.861 1.000/1.000/0.000/0.571 1.000/1.000/0.000/0.571 1.000/1.000/0.000/0.571 1.000/1.000/0.000/0.571
4 LaunchedTopHat           Passed Passed 29.995/29.995/30.000 -0.221/3.224/0.000/0.430 -0.221/3.224/0.000/0.430 -0.221/3.225/0.000/0.411 -0.221/3.225/0.000/0.451
5 Brakes                   Passed Passed 29.135/29.135/29.155 1.000/1.000/0.000/0.322 1.000/1.000/0.000/0.322 1.000/1.000/0.000/0.322 1.000/1.000/0.000/0.322
6 Brakes                   Passed Passed 27.383/27.383/27.386 1.000/1.000/0.000/0.322 1.000/1.000/0.000/0.322 1.000/1.000/0.000/0.321 1.000/1.000/0.000/0.321
7 Brakes                   Passed Passed 25.484/25.484/25.482 1.000/1.000/0.000/0.320 1.000/1.000/0.000/0.320 1.000/1.000/0.000/0.320 1.000/1.000/0.000/0.320
8 SpeedTurnLarge24Left     Passed Passed 23.528/23.528/23.467 1.000/3.156/0.218/0.679 1.000/3.156/0.218/0.679 1.000/3.142/0.223/0.673 1.000/3.142/0.227/0.673
9 EjectorHill16x96         Passed Passed 23.149/23.149/23.100 -0.312/2.941/0.000/0.652 -0.312/2.941/0.000/0.652 -0.302/2.932/0.000/0.638 -0.302/2.932/0.000/0.656
10 Brakes                   Passed Passed 22.471/22.471/22.476 1.000/1.000/0.000/0.317 1.000/1.000/0.000/0.317 1.000/1.000/0.000/0.317 1.000/1.000/0.000/0.317
11 ZeroGRollFastRight       Passed Passed 20.424/20.424/20.381 -0.637/2.648/0.645/0.843 -0.637/2.648/0.645/0.843 -0.640/2.641/0.648/0.833 -0.665/2.641/0.649/0.833
12 SpeedTurnLarge20Left     Passed Passed 19.634/19.634/19.609 1.000/2.249/0.155/0.324 1.000/2.249/0.155/0.324 1.000/2.241/0.159/0.324 1.000/2.241/0.161/0.324
13 FloaterHill12x96         Passed Passed 19.651/19.651/19.617 0.413/2.333/0.000/0.562 0.413/2.333/0.000/0.562 0.416/2.327/0.000/0.553 0.416/2.327/0.000/0.564
14 WaveTurnLeft             Passed Passed 19.030/19.030/19.028 1.116/1.512/0.971/0.302 1.116/1.512/0.971/0.302 1.115/1.510/0.971/0.299 1.089/1.511/0.972/0.318
15 Brakes                   Passed Passed 18.749/18.749/18.691 1.000/1.000/0.000/0.315 1.000/1.000/0.000/0.315 1.000/1.000/0.000/0.315 1.000/1.000/0.000/0.571
16 LaunchShort              Passed Passed 16.379/16.379/16.302 1.000/1.000/0.000/0.571 1.000/1.000/0.000/0.571 1.000/1.000/0.000/0.571 1.000/1.000/0.000/0.571
17 EjectorHill12x64         Passed Passed 18.720/18.720/18.692 -0.453/3.000/0.000/0.960 -0.453/3.000/0.000/0.960 -0.440/2.988/0.000/0.947 -0.440/2.988/0.000/1.197
18 Brakes                   Passed Passed 17.851/17.851/17.824 1.000/1.000/0.000/0.314 1.000/1.000/0.000/0.314 1.000/1.000/0.000/0.314 1.000/1.000/0.000/0.314
19 SpeedTurnLarge16Left     Passed Passed 15.556/15.556/15.447 1.000/1.507/0.165/0.152 1.000/1.507/0.165/0.152 1.000/1.499/0.171/0.151 1.000/1.499/0.172/0.309
20 Brakes                   Passed Passed 15.095/15.095/14.965 1.000/1.000/0.000/0.313 1.000/1.000/0.000/0.313 1.000/1.000/0.000/0.313 1.000/1.000/0.000/0.313
21 TurnSmallRightStandalone Passed Passed 11.571/11.571/11.466 1.000/1.000/1.110/0.007 1.000/1.000/1.110/0.007 1.000/1.000/1.097/0.007 1.000/1.000/1.097/0.007
22 TurnSmallRightStandalone Passed Passed 11.382/11.382/11.313 1.000/1.000/1.081/0.007 1.000/1.000/1.081/0.007 1.000/1.000/1.068/0.007 1.000/1.000/1.068/0.007
23 Straight                 Passed Passed 11.225/11.225/11.153 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007
24 Straight                 Passed Passed 11.130/11.130/11.057 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007
25 Straight                 Passed Passed 11.035/11.035/10.962 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007
26 Straight                 Passed Passed 10.939/10.939/10.866 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007
27 Straight                 Passed Passed 10.843/10.843/10.769 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007
28 Straight                 Passed Passed 10.746/10.746/10.672 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007
29 Straight                 Passed Passed 10.649/10.649/10.574 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007
30 Straight                 Passed Passed 10.551/10.551/10.476 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007
31 Straight                 Passed Passed 10.453/10.453/10.378 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007
32 Straight                 Passed Passed 10.355/10.355/10.279 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006
33 Straight                 Passed Passed 10.256/10.256/10.180 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006
34 Straight                 Passed Passed 10.156/10.156/10.080 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006
35 Straight                 Passed Passed 10.056/10.056/9.979 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006
36 Straight                 Passed Passed 9.955/9.955/9.878 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006
37 Straight                 Passed Passed 9.854/9.854/9.776 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006
38 Straight                 Passed Passed 9.752/9.752/9.674 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006
39 TurnLargeStandalone      Passed Passed 9.650/9.650/9.571 1.000/1.000/0.383/0.006 1.000/1.000/0.383/0.006 1.000/1.000/0.377/0.006 1.000/1.000/0.377/0.006
40 TurnLargeStandalone      Passed Passed 9.306/9.306/9.225 1.000/1.000/0.355/0.006 1.000/1.000/0.355/0.006 1.000/1.000/0.349/0.006 1.000/1.000/0.349/0.006
41 OffsetLeft               Passed Passed 8.954/8.954/8.871 1.000/1.000/0.209/0.006 1.000/1.000/0.209/0.006 1.000/1.000/0.206/0.006 1.000/1.000/0.206/0.006
42 OneTileStraight          Passed Passed 8.791/8.791/8.708 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006
43 OneTileStraight          Passed Passed 8.764/8.764/8.681 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006
Agreement: pass
```

## 10. Test strategy (tests first in every item)

- **Ride file:** parse and round-trip; unknown keys, unknown model and unknown piece ids fail with a message naming the step.
- **Session equals the worker's sequence:** for the three bench circuits, the outputs of `BuilderSession` per placement hash to the same digest as the current hand-written sequence in `construction_update_bench.rs` (`--digest`). Replay is deterministic: two replays of a ride give the same `state_hash`. `undo` after `place` restores the previous track and construction state, excluding the monotonic piece-id counter.
- **Test paths agree:** `ride_test("player")` gives the same report as the animated `construction_test` stepped by ticks, on one ride (add this test if the ride-window work did not).
- **Commands:** golden-text tests for `choices`, `estimate`, `place` and `test` on a short fixed ride; a refused placement exits non-zero, prints the game's reason and leaves the file untouched.
- **Agreement logic:** unit tests on synthetic tables (equal values pass; a 0.3 g shift fails check 2; a milder estimated band fails check 3), and one end-to-end test on a short straight-and-hill ride.
- **Browser:** the generic spec on a ten-piece ride tagged `@fast`, so the fast tier covers the mechanism; the full rides run in the `rides` project.
- **Park description:** a description that rebuilds the `new` preset has the same `world_state_hash` as `WasmSimulation::new(seed, 0, "new")`; a refused facility fails with its index; two coasters fail with the one-coaster message; a described park loads in the browser with `?park=` and guests board the coaster (one spec).
- **Park report:** deterministic for a seed; golden numbers for a tiny park.

## 11. Sequencing against the in-flight branches

| Branch | What it touches that matters here | Consequence |
|---|---|---|
| `builder-qol` | `sim-wasm/src/construction.rs` (adds `samples` to the candidate wire), `main.ts` heavily, `construction.ts` | Work item 1 touches the same wire struct: a conflict of a few lines for the lead. Work items 4 and 8 touch `main.ts` and wait for its merge. |
| `builder-speed-2` | `worker.ts`, `construction-protocol.ts`, `construction/mod.rs`, `elements.rs`, `supports.rs`; proves itself with the bench digests | Work item 5 (worker switch, bench on `BuilderSession`) waits for its merge. Work items 1 to 3 add files and do not touch these. |
| `force-rule` | the recorder, `TestSample` (filtered values), the preview, rated windows, the save version, `known_circuit_statistics.rs`, readouts | Work items 6, 7 and 11 wait for its merge. Work items 1 to 5 read estimates and reports only through the bridge's types, so they keep compiling; tolerances are re-measured after the merge. |
| `demo-ride` | the demo circuit, default-park screenshots; reports findings for section 9 | The demo becomes a ride file in work item 7, after it merges. Work item 6 uses its findings. |
| `longitudinal-diag` | one ignored diagnostic test | Merged or dropped with work item 6, as TASK-144 says. |

Work items 1 to 3 start now on branch `design-tools` and add new files, except for the function split in `construction.rs` and a three-line accessor in `physics/testing.rs`.

## 12. Work items, in order

Suggested models follow the routing rule: well-specified and checkable goes to Codex (Terra at medium effort for work with many moving parts, Luna at xhigh for routine work); taste goes to Opus. Stop and report after two failed fix attempts on the same failure. Cargo packages: `theme-park-sim`, `theme-park-sim-wasm`, `theme-park-design-tools`.

**1. Ride file and builder session (Terra, medium; cap 4 h; start now).** `track/ride_file.rs`; the typed-function split in `construction.rs`; `builder_session.rs` with every action of section 3b; the `save_state` and `load_state` native split if needed. Tests of section 10 for the ride file and the session, written first. Acceptance: the digest test matches the bench's hand-written sequence on the three bench circuits; replay is deterministic; undo after place restores the previous track and construction state, excluding the monotonic piece-id counter; `make check-report-types` still passes (the split must not change generated types). Gate: `cargo test -p theme-park-sim-wasm`, `cargo test -p theme-park-sim --lib track::ride_file`.

**2. Command-line tool and ride report (Terra, medium; cap 4 h).** `crates/design-tools`, `tools/design`, `make design-tools`; every command of section 3c; the per-piece peaks accessor in `crates/sim`; the report table; `export --save` and `export --session`; the five current circuits written as ride files (new files; a temporary test asserts each equals its `circuits.txt` section until work item 7 deletes the text file). Acceptance: golden-text tests pass; a 70-piece ride replays and answers `state` in under 4 s in release (write the measured time here); an exported save loads in a native `load_state` test and an exported session passes `parseReplaySession` in a Vitest. Gate: `cargo test -p theme-park-design-tools`, `npm test -- session-replay`.

Measured under the shared budget on 70 half-tile straights: cold replay and snapshot write 4.29 s; a subsequent matching snapshot and `state` 0.17 s, with identical text. Cold replay remains above the 4 s target. The snapshot is keyed by the exact ride bytes and executable fingerprint and preserves builder replay metadata.

**3. Native agreement table (Terra, medium; cap 3 h).** `agreement.rs`, the `agreement` command, the synthetic tests. Run it on the five rides and write the full table into TASK-146 and section 9. Acceptance: the unit tests pass; the table exists for all five rides with every disagreement attributed to check 2 or check 3; `tools/design check` on all five takes a measured time written here. The check is not a gate yet: `check` reports agreement failures but exits zero on them until work item 6, and says so in its output. Measured release `tools/design check content/rides/*.ride.json`: 33.02 s, exit 0; all expectations passed. No test is ignored or disabled.

**4. Browser agreement spec (Terra, medium; cap 4 h; after `builder-qol` merges).** The `constructionEstimates()` hook; `readRide`; `placeCircuit` extensions; the `rides` Playwright project; `make ride-agreement`; the ten-piece `@fast` ride. Check 1 gates from this item on. Acceptance: the spec passes check 1 on all five rides (run one ride at a time while developing); a deliberate one-line change to the session's select rule makes it fail, shown once and reverted. Gate: `npm run e2e -- --project rides`, `npm test`, `npm run typecheck`.

**5. One code path for the worker and the tool (Terra, medium; cap 3 h; after `builder-speed-2` merges).** Export the session's select and estimates functions through `wasm_bindgen`; `worker.ts` calls them; delete `speedMatchedSelection`'s use in the worker and the TypeScript end-speed cache; the update bench calls `BuilderSession`. Acceptance: bench digests equal the pre-change digests; `make bench-builder-updates` p95 within 5% of before; the feature project and journeys pass. Gate: `npm run e2e -- --project=feature`, `make journeys`, the `rides` project (broad gates, one at a time).

**6. Make the estimate agree with the test and switch the gate on (Terra, medium; TASK-146, TASK-144).** The preview starts as the test starts and reports the train's worst car. The placement preview uses a virtual run-out and is provisional. Gate native/browser parity and both finished-track comparisons; report and classify placement disagreements under the rule in section 4a. Record tolerances and measurements, delete `timber_preview_agrees_with_closed_test_run`, and merge or drop the `longitudinal-diag` test. Report `make bench-builder-updates` against TASK-133's numbers.

**7. Move the fixtures to ride files and retire the old instrument (Terra, medium; cap 4 h; after `demo-ride` and `force-rule` merge).** The ten `circuits.txt` readers read ride files; the demo circuit becomes `content/rides/demo.ride.json` read by `demo_circuit()`; `circuit_report` and its make target are deleted after its test cases are ported; score, limit and feature assertions move into `expect` and their duplicates are deleted (list every deleted assertion in the commit message); the five model journeys keep their model-specific steps on a short prefix. Acceptance: `rg circuits.txt` finds nothing outside old plan documents; `make test`, the feature project, `make journeys` and the `rides` project pass.

**8. Park description and loader (Terra, medium; cap 4 h; after `builder-qol` merges; can run beside items 4 to 7).** `park_description.rs`, `from_park_description`, `?park=<name>` loading, bundling of `content/parks`, `tools/design park build`. Tests of section 10. Acceptance: the `new`-preset equivalence test passes; a one-coaster park loads in the browser and guests ride. Gate: `cargo test -p theme-park-sim-wasm`, one feature spec.

**9. Park report (Terra, medium; cap 4 h).** `ParkOutcomes`, `tools/design park report`, `make park-report`. Acceptance: deterministic golden test; the report for a one-coaster park shows riders per hour within the throughput estimate and names any guest who could not reach a ride.

**10. Agent guide and a trial design (Opus, high; cap 4 h; after item 3. Opus because whether the workflow is easy and the ride is good cannot be checked by a test).** Write `docs/designing-rides.md`. Then, using only that page and the tool, rework Horizon Hawk toward −1 g camelbacks with fewer trims (TASK-139). Report friction, time and tokens against the `demo-ride` baseline. Acceptance: the ride passes `check` (agreement reported, not yet gating if item 6 is not done); every friction item is fixed or filed. If `force-rule` has not merged, the ride is re-checked after it merges.

**11. Showcase rides for the other models (Opus, high; one agent per model in parallel, cap 3 h each; after items 6 and 7).** Timber Tumbler, Velocity Viper, Briar Blitz, Daisy Dash and the demo (TASK-140, from the `demo-ride` result), each to its row in section 5c, each with a one-coaster showcase park. The native tool is light, so the agents run in parallel; the browser spec runs one ride at a time. Acceptance: `check` and the browser spec pass for each; the owner has seen each (question 3).

**12. Close-out (Luna, xhigh; cap 3 h).** `docs/status.md`, TASK-149, TASK-146, TASK-144, TASK-139, TASK-140 and TASK-109 notes, the follow-ups of section 15 filed, the measured numbers written into this plan, `make ci`, fix what is red, handoff.

Order: 1, 2, 3 now. 4 and 8 when `builder-qol` merges; 5 when `builder-speed-2` merges; 10 after 3. 6 and 7 when `force-rule` has merged and `demo-ride` has reported. 9 after 8. 11 after 6 and 7. 12 last. About 45 agent hours.

## 13. Risks

- **The tool and the game diverge before work item 5.** Until the worker calls the session's functions, the ordering logic exists twice. Check 1 in the browser spec is the guard, and it gates from work item 4.
- **Native and wasm numbers differ by more than rounding.** Trigonometric functions come from different math libraries. If a difference exceeds 1e-6, find which function causes it before widening the tolerance.
- **Replay time grows with ride length.** Stateless calls replay the whole ride. The snapshot is the remedy if the 4 s target is missed.
- **The agreement gate would be red today.** The owner's playtest shows a disagreement exists. The check therefore lands as a report (item 3) and becomes a gate together with the fix (item 6). No test is disabled, and the known disagreements are written into TASK-146.
- **Making the estimate cover every car slows the builder.** TASK-133 wants a placement update under 100 ms p95 in the browser, and the previews already take 30 to 40 ms natively. Work item 6 measures before committing.
- **`force-rule` changes sample values and windows after rides were designed.** Rides designed before it merges are re-checked after; the remedy is a size swap, as in the good-circuits plan.
- **The UI helper cannot select every piece.** `placeCircuit` knows palette pieces, hills, zero-g rolls and speed turns. Elements and launch speeds are added in work item 4; a ride that uses something else fails loudly and the helper is extended then.
- **Agents design to the score.** The targets are checkable, but "realistic and good" is not. The owner's look at each ride (question 3) is the control.
- **One player coaster per park** limits layer 3 to one-coaster parks (question 1).

## 14. Open questions for the owner

1. **Several coasters in one park.** The simulation holds one player coaster, so "one showcase coaster per model in the park" needs a larger change first (coaster state, guests' ride choice, the builder's "which coaster am I editing", saves). My recommendation: this plan delivers each model's showcase ride with its own small park, and several coasters per park gets its own plan right after, which then assembles the full starter park (TASK-109). The alternative is to do that change first and delay the showcase rides. Which do you prefer?
2. **Targets for the family and wooden rides.** You gave steel and wood numbers. For Daisy Dash I would aim for pullouts of 2 to 2.5 g and airtime of −0.3 to −0.5 g (its limits are 3.0 and −0.6). For Briar Blitz, pullouts of about 2.5 g and airtime of −0.6 to −0.9 g (its limit is −1.0). Do these match the feel you want?
3. **Your look at each showcase ride.** Do you want to ride each one (import the exported park, about two minutes each) before it becomes the model's reference, or is passing the targets enough, with your review afterwards in normal play?

## 15. Follow-ups filed, not done here

- Several coasters per park (question 1), then the full starter park (TASK-109) and the `demo` preset as a park description file, with `demo_world.rs` and the hard-coded facilities removed.
- A rendered image of a ride (plan view and height profile with force colours) from the native tool, if the character map proves too coarse.
- Sessions recorded on described parks (the session format names only `new` and `demo`).
- A long-lived server mode for the tool (for example behind MCP) if replay time becomes the bottleneck.
- TASK-145 (floater readout wording) once `force-rule` merges; the `estimate` reply should print the same explanation the readout gains.
