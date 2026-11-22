# Several coasters in one park (TASK-171)

Goal: a park holds any number of coasters. Each coaster has its own track, trains, test runs, ride statistics and ratings, queue and station, ticket price and open or closed state, and guests choose between them and ride each one. The player can start a new coaster while others exist and can select, edit, test, open, close or demolish any of them. This is the prerequisite for the richer starter park (TASK-109), and the last two slices move the starter park out of hardcoded Rust into a park description file.

All file and line references are to master at commit a1ed934 (2026-11-22). Line numbers will drift, so the audit also names the symbol.

## What exists

The simulation already holds two coasters, but in two unrelated forms. The first is the "demo coaster" (`DemoCoaster`, `crates/sim/src/park.rs:76`): a fixed circuit loaded from `content/rides/demo.ride.json` with one train that runs laps forever. Guests cannot ride it, it has no station access, no price and no block sections, and its track never changes. The second is the player's coaster, which is the `construction: Option<Construction>` field plus about thirty flat fields on `Park` (`park.rs:89-125`): the test train, the test report, open state, price, live operation, riders, guest-facing facility, ratings and all block-section state. Starting a new coaster with `construction_start` (`crates/sim-wasm/src/construction.rs:960`) overwrites the existing one, and there is no command to demolish a coaster.

The guest layer is already close to ready for many coasters. A coaster reaches guests as an ordinary `Facility` with a place id (`coaster_facility`, `crates/sim/src/facilities.rs:2006`). Guests keep a set of externally driven rides (`external_rides: BTreeSet<u32>`, `crates/sim/src/guests/behavior.rs:63`) and key riders by facility id and train id (`coaster_riders: BTreeMap<(u32, u64), Vec<GuestId>>`, `behavior.rs:64`). Ride choice in `crates/sim/src/guests/decisions.rs:253-290` scores every known place by intensity match, favourite ride type and queue, with nothing specific to one coaster. Ride outcomes are also keyed by facility id (`crates/sim/src/guests/outcomes.rs:24`). So the work is in the `Park` type, the WASM bridge, the web client and the tools, and very little of it is in guest behaviour.

Saves are a postcard encoding of the whole `WasmSimulation` struct behind a `TPSAVE\x06` header and a checksum (`crates/sim-wasm/src/save.rs:8-10, 141-154`). Postcard is positional and has no field names, so the field order of `WasmSimulation` and `Park` is the save format. Version 4 and 5 saves load through the same struct with decode contexts that reject the parts that changed (`save.rs:172-192`). The existing tests fake an old save by rewriting the version byte of a current save (`crates/sim-wasm/src/save_tests.rs:430-431`); that stops working the moment the struct layout changes. The state hash is separate from the save bytes: `Park::state_hash` (`park.rs:212-326`) writes each field explicitly under the tag `Park-v2`.

## Audit of the one-coaster assumption

Depth is how much work the place needs: "mechanical" means a rename or a loop over coasters with no design decision, "moderate" means new logic in one module, "deep" means a design change that other modules depend on.

### Simulation crate (crates/sim)

| Where | What assumes one coaster | Depth |
|---|---|---|
| `park.rs:76-128` `DemoCoaster`, `Park` | One optional demo coaster, one optional construction and about thirty flat per-coaster fields (`construction_train`, `construction_report`, `demo_report`, `coaster_open`, `coaster_price`, `coaster_operation`, `coaster_riders`, `coaster_facility`, `coaster_ratings`, every `block_*` field, `desired_train_count`, `support_render_cache`) | Deep. This is the data model change. |
| `park.rs:142-159` `Park::new` | Takes one circuit and one train for the demo coaster | Mechanical |
| `park.rs:203-209` `set_coaster_price`, `coaster_income` | One price; income read from the single `COASTER_PLACE_ID` | Mechanical |
| `park.rs:212-326` `state_hash` | Writes the demo coaster and each flat field once | Moderate (new hash layout) |
| `park/tick.rs:20` `COASTER_PLACE_ID = u32::MAX` | One fixed place id for "the coaster" | Deep (id scheme) |
| `park/tick.rs:112-281` `tick_with` | Adds one coaster facility (123-139), sets one external ride (140-144), steps the demo train (150-154), boards and dispatches one operation (155-198), steps one builder test train (199-253) | Moderate. Becomes a loop over coasters in id order. |
| `park/tick.rs:320-341` `proximity_supports`, `tick.rs:441-470` `refresh_construction_supports` | One support cache keyed by piece id; piece ids are counted per construction and collide across coasters | Moderate |
| `park/tick.rs:343-418` `close_live_coaster`, `recover_live_trains`, `finish_live_train` | Operate on the single operation; near-miss scoring passes only the demo coaster as "other track" (387-389) | Moderate |
| `park/block_analysis.rs:49-543` | About fifteen `construction_*` block and operate methods on `Park` (`construction_set_open` at 109, `install_block_layout` at 202, `construction_advance_block_analysis` at 236) read the flat fields | Mechanical once the methods take a coaster id |
| `park/ride_test.rs:14-18, 89-156` `RideKey { Demo, Player }` | Two hardcoded rides; the instant test duels only against the demo train | Moderate |
| `park/commands.rs:76, 209, 452, 461, 538` | Facility, scenery and terrain edits check collisions and support routes against the demo coaster only; `facility_openings` (111-117) adds one station's openings | Mechanical (loop over coasters) |
| `park/commands.rs:285-370` tunnels, `park.rs:133-136` `TunnelOwner { generation, piece_id }` | A "generation" number stands for the current construction; activating a new generation restores every tunnel of older generations (346-352), which is how the previous coaster's tunnels disappear | Moderate. The generation becomes the coaster id and restoring happens on demolish. |
| `facilities.rs:396-430` `validate_facility_path_access` | Takes one `Option<&Construction>` when counting queue owners | Mechanical |
| `facilities.rs:2006-2040` `coaster_facility` | Already takes the place id as a parameter. `ride_type` is the constant 2 for every coaster and the default ratings are (50, 30). | Small; see guest selection below |
| `guests/behavior.rs`, `guests/park.rs:147-213` | Already keyed by facility id and train id | None |
| `physics/mod.rs:153-222` `demo_circuit`, `demo_world.rs` | The starter park's coaster and terrain as Rust code | Removed by the starter-park slices |

### WASM bridge (crates/sim-wasm)

| Where | What assumes one coaster | Depth |
|---|---|---|
| `lib.rs:80-107` `WasmSimulation` | Builder state for one coaster: `construction_model`, `selected`, `selected_shape`, `selected_element`, `construction_undo_groups`, the finish-search fields, two support render caches, `recovered_station` | Deep (which coaster is being edited) |
| `lib.rs:225, 259-264` | `demo_coaster()` accessor; new track treats only the demo coaster as an obstacle | Mechanical |
| `lib.rs:313-330` `encode_construction_geometry` | Encodes steel as `-(piece id + 1)` and wood as `+1_000_000` in render records; no coaster id | Moderate (render protocol) |
| `world.rs:7-140` `WasmSimulation::new` | The `is_demo_park` branch builds terrain, six facilities and the demo coaster in code | Removed by the starter-park slices |
| `construction.rs` (3,510 lines) | About 230 references to `self.construction` and 25 to the flat per-coaster fields. `construction_start` (960-1027) replaces the coaster; `construction_test` (2543); `ride_info_json` and `ride_test` (2580-2632) take the strings "demo" or "player"; `construction_riders` (2076); train poses and states (2698-2770) | Mechanical in volume, but it is the largest diff and the main merge-conflict risk |
| `blocks.rs:5-90`, `builder_session.rs` (2,254 lines) | Block commands and the headless builder session act on "the construction". `BuilderSession` owns a whole simulation and its snapshot (`TPBUILDER\x02`, line 24) embeds save bytes. | Mechanical if the bridge keeps one "coaster being edited" |
| `guests.rs:16-90` | `coaster_geometry`, `coaster_rails`, `coaster_ties`, `coaster_supports`, `coaster_colors`, `coaster_train_layout`, `coaster_train_poses`, `coaster_train_states` export the demo coaster only | Moderate (render protocol) |
| `save.rs:8-10, 141-217` | Version 6 header; decodes `WasmSimulation` directly | Deep (new version and migration) |
| `park_description.rs:205-209, 241-264` | Rejects a second coaster; one `coaster_should_open` flag | Mechanical once the bridge can create a second coaster |
| `reports.rs:171-180` `RideInfoReport` | `ride` is the string "demo" or "player" | Mechanical |
| `tests/world.rs` (2,183 lines), `tests/construction.rs` (2,344 lines), `save_tests.rs` | About 30 uses of `demo_coaster()` and direct reads of the flat fields | Mechanical |

### Web client (web/src)

| Where | What assumes one coaster | Depth |
|---|---|---|
| `protocol.ts:17, 28, 36-37` | `park: "new" | "demo"`; ride actions carry `RideKey`; the world message has one set of `coasterGeometry/Rails/Ties/Supports/Colors/Train`; the snapshot has `trainPoses` (demo) and `constructionTrainPoses`, `constructionTrainStates`, `constructionRiders` (player) | Deep (protocol) |
| `construction-protocol.ts` | No construction action names a coaster; `ConstructionState` describes the one construction including its placed rails, price, income, open state and block report | Moderate |
| `worker.ts:42-47, 115-123, 147, 247-259` | `coasterGeometrySent` flag sends the demo track once; one `selectedRide`; one set of train arrays per snapshot | Moderate |
| `main.ts:150, 168, 211, 899-905, 1026-1027, 1249` | One `construction` state, one `selectedRide`, `demoCrests` and `constructionCrests`, `demoTrain` and `constructionTrain` passed to sound | Moderate |
| `compose-scene.ts:26-30, 107-112, 128, 158, 176-179` | Draws "the coaster" and "the construction" as two fixed layers | Moderate |
| `ride-window/picking.ts:4-11`, `render/track/geometry.ts:28` | A pick id with the prefix `construction-` means the player's coaster, otherwise the demo coaster | Moderate |
| `ride-window/protocol.ts:3`, `generated/RideInfoReport.ts` | `RideKey = "demo" | "player"` | Mechanical |
| `audio/mapping.ts:28, 80, 90-97` | Two fixed coasters; the voice key is `trainIndex * 2 + coaster` | Mechanical |
| `construction.ts:516, 537`, `station-tool.ts:41` | The builder panel is the one coaster's panel; the ride-info button sends "player"; placing a station always starts (and so replaces) the coaster | Deep (selection model) |
| `game-test.ts:83-98` | Test hooks `riders()`, `testingReport()`, `constructionTrainPoses()`, `trainStates(): { coaster, construction }` | Mechanical |
| `session-recording.ts:18`, `session-replay.ts:20, 98` | Recorded sessions (format version 1) allow only the parks "new" and "demo" and their construction commands carry no coaster id | Kept working by the addressing design below |
| `save-storage.ts`, `autosave.ts`, `park-saves.ts` | Treat Rust save bytes as opaque | None |

### Design tools and tests

| Where | What assumes one coaster | Depth |
|---|---|---|
| `crates/design-tools/src/park_report.rs:57-63` | Rejects more than one coaster | Mechanical |
| `park_report.rs:77-285, 776-790, 832-901` | Tests, opens and reports one coaster; uses `COASTER_PLACE_ID` for names, revenue, queue lengths and ride outcomes; writes `facilities_by_kind["coaster"] = 1` | Moderate (loop and per-coaster sections) |
| `crates/design-tools/src/commands.rs:338-346` `park build` | Rejects more than one coaster when bundling ride files into a park description | Mechanical |
| `commands.rs` ride commands | One `BuilderSession` per ride file | None |
| `crates/design-tools/tests/goldens/horizon_hawk_park_report.txt` | Golden text for a one-coaster park | Re-record if the report layout changes |
| `web/e2e/ride-window.spec.ts`, `web/src/ride-window/picking.test.ts`, `web/src/audio/mapping.test.ts`, `web/src/compose-scene.test.ts`, `web/src/world-snapshot.test.ts`, `web/src/guest-panel/worker.test.ts` | Use "demo" and "player" or the two fixed train arrays | Mechanical |
| `web/e2e/player/driver.ts:308-348` | `placeStation`, `testRide`, `openRide` drive the one builder panel | Small additions (select a coaster first) |
| `web/e2e/sessions/*.json`, `crates/sim-wasm/src/guest_benchmark.rs:131` | Recorded state-hash checkpoints and one hardcoded hash | Re-record when the hash layout changes |
| 18 snapshot directories with 147 PNG baselines under `web/e2e` | The page defaults to the demo park (`main.ts:832`), so many baselines show the starter park | Re-record only in the starter-park content slice |

## Target design

### Data model

One `Coaster` struct in a new `crates/sim/src/park/coaster.rs` replaces both `DemoCoaster` and the flat fields. `Park` holds `coasters: BTreeMap<CoasterId, Coaster>` and `next_coaster_id: u32`. `CoasterId` is a `u32` newtype that is assigned once when the station is placed and is never reused after a demolish, so saved references, guest memories and recorded commands can never point at a different coaster. The map is a `BTreeMap` so every loop runs in id order, which the deterministic tick needs.

`Coaster` owns: a display name, the `Construction` (always present, because a coaster exists from the moment its station is placed), the builder test train with its own test message, outcome, slow-step counter, recorder and report, the open flag, price, live `BlockOperation`, riders per train, the guest-facing `Facility`, the remembered front-car ratings, all block-section state, the desired train count and the support render cache. `Park` keeps what is shared: the world, facilities, guests, the physics remainder, the tunnel bookkeeping and one builder message for refused commands, because `construction_message` is also written when no coaster exists (an invalid heading or a refused station, `construction.rs:972, 993-1008`). Today the tick uses that one message string as the test train's state (`tick.rs:205-226`), so the several-coasters slice splits it into the per-coaster test message and the park-level builder message. `TunnelOwner` keeps its two-number layout, with the first number now meaning the coaster id; tunnels are restored when that coaster is demolished or a piece is deleted, never because another coaster was started.

Alternatives considered: (a) keep the player's coaster in flat fields and add a `Vec` of "extra" coasters, which is less churn now but leaves two code paths forever; (b) a `Vec<Coaster>` addressed by index, which makes ids shift when a coaster is demolished and would corrupt guest memories and recorded sessions. The keyed map with stable ids is the recommendation.

### Place ids for guests

Each open coaster appears to guests as a facility with place id `u32::MAX - coaster_id`. Building ids count up from zero (`facilities.rs:273-274`) and coaster ids count up from zero, so the two ranges cannot meet in practice, and the code asserts it. The first coaster in a park therefore keeps today's `COASTER_PLACE_ID`, so migrated saves keep their guests' memories, income and outcome records without rewriting any guest state. The alternative, allocating coaster place ids from the building counter, gives one id space but renumbers buildings in every existing park and recorded session for no gain.

### How commands address a coaster

Two options were weighed. The first puts a coaster id on every command: about 90 `construction_*` bridge methods, every construction action in the worker protocol, the builder session, the design tools and the recorded-session format would all change, and version 1 recorded sessions would need a migration. The second keeps one "coaster being edited" in the bridge and is the recommendation.

With the recommended option, `WasmSimulation` gains `editing: Option<CoasterId>`. Every existing `construction_*` method keeps its name and signature and acts on that coaster. In the finished design `construction_start` creates a new coaster and makes it the one being edited, instead of replacing the old one; the slices get there in two steps so the browser never holds a coaster it cannot draw. New methods: `coaster_edit(id)`, `coaster_stop_editing()`, `coaster_demolish(id)`, `coaster_rename(id, name)` and `coasters_json()` (id, name, model, open, price, income, queue length, whether it is complete). Commands that make sense without entering the builder take an id: `ride_info_json(id)`, `ride_test(id)`, `coaster_set_open(id, open)` and `coaster_set_price(id, price)`. Inside the simulation crate there is no "edited" notion: the `Park` methods in `block_analysis.rs` and `ride_test.rs` take a `CoasterId` and the bridge passes `editing`.

This keeps recorded sessions, ride files, `BuilderSession` and `tools/design ride` working unchanged, because each of them builds one coaster at a time. The builder's per-edit state (selected piece, selected shape, undo groups, finish search, preview caches) stays single; switching the edited coaster clears it the same way loading a save does today (`save.rs:211-215`). The undo groups move into `Coaster` so undo still works after switching away and back. Block analysis work goes to the edited coaster first and then to any other coaster with an unfinished search, in id order.

### Saves

The new layout is save version 7. The current `WasmSimulation` and `Park` field lists are copied into a frozen `save_v6.rs` as `WasmSimulationV6` and `ParkV6` (reusing the unchanged inner types such as `World`, `Construction` and `ParkGuests`), and version 4, 5 and 6 saves decode into those with the existing decode contexts and are then converted. Conversion moves the flat fields into one `Coaster` with id 0 when a construction exists; old saves are loaded, never refused for being single-coaster. Before the layout changes, one real version 6 save recorded from master is committed as a test fixture, because the byte-flipping trick in `save_tests.rs:430` cannot produce an old-layout save afterwards. The fixture should be a new park with a built, tested and open coaster with riders on a train; its size must be checked first, and if it is more than a few hundred kilobytes it should be recorded with few guests or stored compressed (memory rule: no large generated files in history).

The rule for later slices is the same: a slice that changes the saved layout bumps the version and freezes the previous field lists of `WasmSimulation`, `Park` and `Coaster` (about 60 lines each). Three bumps are expected: version 7 for the grouping slice, version 8 for the several-coasters slice (name, per-coaster test message, tunnel owners keyed by coaster id) and version 9 when the special demo coaster is retired and `demo_coaster` leaves `Park`. Conversion details that are easy to miss: stored tunnel owners carry the old generation number and must be rewritten to the coaster's id; the player's coaster gets id 0 so it keeps place id `u32::MAX`; and a retired demo coaster gets the next id after it. Version 4 and 5 test saves are made by rewriting the version byte of the version 6 fixture, as the tests do today with a current save. The `TPBUILDER\x02` builder snapshot embeds save bytes and is a cache for the design tools, so it gets a new header version and old snapshots are discarded and rebuilt, not migrated.

### Determinism and the state hash

The tick steps coasters in id order. Near-miss and duelling inputs for a coaster are the positions of every other coaster's trains, taken at the end of the previous physics step so the result does not depend on which coaster is stepped first. The hash tag becomes `Park-v3`: it writes the coaster count, then each coaster's id and fields in id order. The edited-coaster id is bridge state and is not hashed, the same as the selected piece today.

The first slice deliberately keeps the hash unchanged. While the map can hold at most one built coaster, `state_hash` can write the same sequence as `Park-v2` from that single entry (and the same defaults when the map is empty), so every recorded session checkpoint, the guest benchmark hash and the determinism tests pass without re-recording and prove the regrouping changed no behaviour. Three fields are not reset by `construction_start` today (`construction.rs:1012-1025` and `clear_block_layout`, `block_analysis.rs:185-200`, reset everything else): `coaster_price`, `coaster_ratings` and `construction_slow_steps`. In the first slice the replacing coaster inherits those three values so the hash stays identical; from the several-coasters slice on, a new coaster starts with defaults. The hash layout then changes once, in the slice that allows a second coaster.

### How guests choose between coasters

No new mechanism is needed. Each tick the park adds one `Facility` per open coaster with that coaster's place id, price, height limit, seat capacity and its own remembered intensity and nausea, and passes all their ids to `set_external_rides`. Guests then discover, compare, queue for and remember each coaster separately through the existing knowledge and choice code. Boarding and finishing a train already take the facility and train id.

Two limits are worth knowing but do not block this work. Every coaster reports `ride_type: 2`, so a guest's favourite-ride-type preference cannot tell a family coaster from a giga; and the choice score uses intensity and nausea but not excitement, so two coasters of similar intensity differ only by queue and distance. Giving coaster models different ride types and letting excitement into the score is a follow-up ticket, to be decided together with the starter park because it changes how crowds spread.

### Selection model in the interface

Recommended: the player selects a coaster by clicking it in the park. Clicking any coaster's track, supports or train opens the ride window for that coaster (this exists today for two fixed coasters). The ride window gains the name, open or closed switch, price, a Test button (exists), an Edit button that opens the builder panel on that coaster, and Demolish with a confirmation. The coaster tool, opened with nothing being edited, shows a short list of the park's coasters (name, model, open or closed) with a "New coaster" entry that leads to the existing model picker and station placement. Closing the builder panel stops editing; the coaster stays in the park.

The alternative is a dropdown at the top of the builder panel as the only way to switch. It is less work but hides the park's coasters inside a build tool and gives no way to open, close or price a ride without entering build mode, so the plan does not recommend it. The list in the coaster tool covers the same need for coasters that are off screen.

### Rendering, sound and the worker protocol

The world message carries a list of coasters, each with id, colours, train layout, rails, ties, supports, sampled geometry and a track revision number; the worker resends a coaster's entry only when its revision changes, replacing the single `coasterGeometrySent` flag. Each snapshot carries, per coaster, train poses, train states and riders. The construction-state message keeps describing only the coaster being edited (choices, preview, estimates, block report), so the builder panel code changes little. Pick ids become `track-<coaster id>-<piece id>` (and the same for supports and trains) in place of the `construction-` prefix. Sound keys a train by coaster id and train index, and still plays the four most audible trains.

## Implementation sequence

Each slice lands on master on its own with its tests green. Tests are written first in each slice: Rust integration tests, then Vitest, then browser tests.

### Slice: group the per-coaster fields into a Coaster type (no behaviour change)

Record the version 6 save fixture from master first and add a failing test that loads it. Then introduce `CoasterId`, `Coaster` and `Park::coasters`, move the flat fields, give the `Park` methods in `block_analysis.rs`, `ride_test.rs` and `tick.rs` a coaster id parameter, add `editing` to the bridge, bump saves to version 7 with the frozen version 6 structs, and bump the builder snapshot header. `construction_start` still replaces the existing coaster in this slice (the new coaster inherits the price, ratings and slow-step counter, as described under the state hash), `construction_message` and the tunnel generation stay as they are, and the demo coaster keeps its own type. Tests: the fixture and a version 5 and version 4 save load and keep running; save and reload round trips; every existing state-hash golden passes unchanged. Files: `crates/sim/src/park.rs`, `park/*.rs`, most of `crates/sim-wasm/src`. No web change. Nothing is re-recorded. The `content/rides/*.ride.json.snapshot` builder caches are untracked and rebuild themselves after the header bump. This slice touches `sim-wasm/src/construction.rs` on about 250 lines and should be done in one sitting and merged quickly.

### Slice: the simulation and bridge hold several coasters

Add `coaster_start`, which always adds a new coaster and edits it. `construction_start` keeps today's behaviour for now: if a coaster is being edited it demolishes that coaster and starts a new one. The web client can only draw the demo coaster and the edited coaster until the rendering slice, so the browser must not be able to create a second coaster yet; the interface slice switches the browser to the adding path and removes the replacing branch. Add edit, stop editing, demolish, rename, the coaster list, and the id-taking operate, price, info and test methods. Loop over coasters in the tick, in facility, scenery, terrain and path-access checks, in near-miss scoring and duelling, and in new-track obstacle checks. Switch to place ids `u32::MAX - id`, per-coaster support caches, per-coaster test messages and tunnel ownership by coaster id; saves go to version 8. Remove the limit in `park_description.rs:205` and open each described coaster according to its own flag. The existing demo-coaster exports and the "demo"/"player" ride keys stay for now, with "player" meaning the edited coaster, so the web client keeps working unchanged.

Rust integration tests, written first: a park with two coasters of different models, both open with queues, where guests ride both and each coaster records its own riders, income and ratings; closing one leaves the other running; demolishing one restores its tunnels, sends its queue away and leaves the other untouched; a second coaster cannot be built through the first; the same seed gives the same hash twice and a save taken mid-ride reloads and continues to the same hash; a two-coaster park description loads. This slice changes the hash layout to `Park-v3`, so it re-records the checkpoints in `web/e2e/sessions/*.json` (with `web/e2e/rehash-sessions.mjs`), and the hash at `guest_benchmark.rs:131`. The tests in `crates/sim/src/park/hash_tests.rs` and `guests/park_determinism_tests.rs` compare two runs with each other and hold no recorded constants. Files: same Rust files as the first slice, so it must follow it.

### Slice: design tools report every coaster

`tools/design park build` bundles every listed ride file, and the park report tests, opens and reports each coaster with its own section, revenue, queue length, riders per hour and unreachable-guest counts. Tests: a two-coaster park fixture in `crates/design-tools/tests/park_report.rs` and `park_commands.rs`. The golden `horizon_hawk_park_report.txt` is re-recorded only if the one-coaster layout changes; aim to keep it identical. Files: `crates/design-tools/src/park_report.rs` and `commands.rs` only, so this can run in parallel with the web slices after the bridge slice lands.

### Slice: worker protocol, rendering and sound per coaster

Replace the fixed demo and construction arrays with the per-coaster lists described above in `guests.rs` (bridge exports), `protocol.ts`, `worker.ts`, `main.ts`, `compose-scene.ts`, `render/track`, `ride-window/picking.ts`, `audio/mapping.ts` and `game-test.ts`. Ride keys become coaster ids in `RideInfoReport` and the ride actions; the demo coaster is exposed through the same list with a reserved id until it is retired. Vitest first: picking returns the right coaster id, the scene draws two built coasters with their own colours and trains, sound keeps trains of two coasters apart, the worker resends only the changed coaster's track. The picture for existing parks must not change, so all existing screenshot baselines pass without re-recording, and that is the main check of this slice. Files overlap with the interface slice (`main.ts`, `worker.ts`), so the two run in sequence.

### Slice: select, edit, start and demolish coasters in the interface

Make `construction_start` always add a coaster (remove the replacing branch left by the bridge slice), then build the selection model: ride window controls, the coaster list in the coaster tool, New coaster, Edit, Demolish, rename. Add driver helpers in `web/e2e/player/driver.ts` to select a coaster by name. Vitest for the ride window and the list. Browser journey, written first: in a new park build Daisy Dash and a second small coaster from ride files, connect a queue and exit to each, open both, and wait until guests have ridden both; then edit the first coaster while the second keeps running, test it and reopen it; then demolish the second and check the first still runs. New screenshot baselines are added for the coaster list and the extended ride window; `ride-window.spec.ts` baselines are re-recorded because the window gains controls.

### Slice: the starter park loads from a park file that reproduces today's park

Extend the park description format (version 2) with what today's demo park needs and version 1 lacks: a terrain preset or terrain data equal to `demo_world()`, and a coaster without queue and exit paths that stays closed. Write `content/parks/starter.park.json` for the current park, make the "demo" preset load it, convert the demo coaster into an ordinary `Coaster` built from `content/rides/demo.ride.json`, and delete `DemoCoaster`, `demo_report`, `RideKey`, the `is_demo_park` branch in `world.rs`, `demo_world.rs` and the demo-only bridge exports. `demo_circuit()` stays as a test helper because about twenty physics tests use it. Saves go to version 9; an older save with a demo coaster gets it back as a closed ordinary coaster rebuilt from the ride file, with its train at the station.

This slice changes behaviour in one visible way that needs the owner's answer (first question below): today the demo train runs laps with nobody on it, and an ordinary coaster only dispatches a train that has riders. Tests that rely on the always-moving demo train (`sim-wasm/src/tests/world.rs:1960-2019`, the train and scream audio specs, `smoothness.spec.ts`, crest confetti) move to an open coaster with guests or to a builder test lap. Measure the page load time of the park file before choosing how to ship it: every coaster is replayed through the builder on load, and each shaped element needs a solve whose cost has not been measured for a whole park. If loading takes more than about a second, generate a save from the park file at build time with `tools/design park build` and have the preset load that; the generated save is a build output and is not committed. The default-park screenshot baselines that show the coaster's train are re-recorded here. The three recorded sessions all use the new park, so they are not affected.

### Slice: the new starter park content

Replace the content of `starter.park.json` with the richer park: the giga on the Horizon Hawk model, Daisy Dash, flat rides, themed areas and scenery, with every coaster open and connected. The reworked Timber Tumbler and Velocity Viper are added to the file later, after the inversion work, as the owner asked. This slice is content only, and it re-records every screenshot baseline that shows the default park. How many of the 147 baselines that is has not been counted: many specs use fixture pages or `park=new`, but the page's default park is the demo park, so count them at the start of that slice. Tests that need a small, stable scene should move to `park=new` or a small dedicated park file before this slice, so later content edits do not keep invalidating baselines.

### Order and parallel work

The first two slices are strictly in order and touch the same Rust files. After the bridge slice, the design-tools slice can run in parallel with the two web slices, which are in order with each other. The starter-park mechanism slice needs both web slices. The content slice is last and also waits for the giga ride design.

## Likely merge conflicts with other sessions

The force-optimised inversion work (plan `2026-11-22-shaped-inversions-and-ui-polish.md`, TASK-168) edits `crates/sim/src/track/shaped.rs`, `track/profile.rs`, `physics/mod.rs`, `physics/features.rs`, `crates/sim-wasm/src/construction.rs`, `crates/design-tools/src/commands.rs`, `web/src/construction.ts` and `construction.css`. This plan stays out of `track/` and `physics/` except for removing demo code at the end, so the real collisions are `sim-wasm/src/construction.rs` (the first slice rewrites every `self.construction` access), `design-tools/src/commands.rs` and `web/src/construction.ts`. The first slice should be merged right after the inversion session's next merge to master, and that session told to rebase onto it; the edits are a mechanical rename and are easy to redo on a conflict.

Branches with unmerged commits against master that touch the same files (some may be stale after squash merges and should be checked before worrying): `circuit-finish`, `envelope-split` and `blitz-top-hat` (`sim-wasm/src/construction.rs`), `hawk-shaped` (`save_tests.rs`, `design-tools/src/commands.rs`), `rated-margins` (`builder_session.rs`), `fast-agreement` (`web/src/worker-construction.ts`), `inversion-forces` (`web/src/main.ts`, `web/e2e/sessions/timber-tumbler-built.json`), `station-arrival` (`physics/mod.rs`). The `slow-tests-design` worktree has uncommitted edits to `design-tools/src/commands.rs` and `tests/park_report.rs`, and `slow-tests-sim-lib` to `physics/block_operation.rs`. The main checkout itself has another session's uncommitted inversion edits at the time of writing (`physics/mod.rs`, `physics/preview.rs`, `track/profile.rs`, `track/shaped.rs`, `web/src/construction.ts`, `construction.css`). Recorded session files conflict with any other session that re-records them; re-run `rehash-sessions.mjs` after merging instead of resolving by hand. Files under `web/src/generated` are tracked and regenerated by the report types test, so regenerate after each merge.

## Risks

- The only recorded state hashes in the repository are the three session files and `guest_benchmark.rs:131`. The other matches for "hash" and "digest" in `crates/design-tools/tests` and `web/e2e/park-description.spec.ts` compare two runs or check shaped-geometry digests, which this work does not change.

- The first slice is a large mechanical diff in files other sessions edit daily. Keeping the hash unchanged is what makes it safe to land fast; if the hash cannot be kept identical for some session, stop and find out why before re-recording.
- Old saves: the frozen version 6 structs only freeze the top level. If another session changes the layout of an inner type (`Construction`, `TestRun`, `BlockLayout`), old saves break the same way they would today; the real fixture test will catch it.
- Frame cost grows with the number of coasters: each has trains to step, supports to route and possibly a block search. The two-coaster journey should record tick time, and the starter park with four coasters should be measured before the content slice.
- Guest knowledge of a demolished coaster's place id has not been traced in this investigation. Removing a building takes the same route (the facility simply stops being listed), so it probably works, but the demolish test must cover a guest who was walking to the demolished coaster.
- Not verified here: the size of a version 6 save, and the page load time of a park file with several shaped coasters.

## Questions for the owner

1. Should a coaster with no riders run its trains? Today the demo train always laps, and a built coaster dispatches only trains with riders, so in an empty park the converted starter coaster would stand in its station. Recommendation: keep "dispatch only with riders" for now and add a classic three-state control later (closed, testing with empty trains running continuously, open), since that also gives the player a way to watch a ride run.
2. When an old save with the demo coaster is loaded after it becomes an ordinary coaster, it will be closed with its train in the station until the player connects paths and opens it. Is that acceptable, or should migrated saves keep it visibly running?
3. Should coasters of different models count as different ride types for guest preferences, and should excitement affect which coaster a guest picks? This changes how crowds split between a family coaster and a giga, so it is a feel decision; the plan leaves it as a follow-up ticket.
4. Should open, close, price and demolish be in the ride window, with the builder panel used only for track, trains, colours and block sections? The plan assumes yes.
