# Shaped elements: ask for a feel, get the shape

Written 2026-11-19 from read-only research at master 744b458. Nothing was built or run for this plan. It replaces the fixed-size answer to the brake-heavy Horizon Hawk (TASK-126 notes) with a root-cause fix: elements shaped at placement for the real train at its real entry speed, with every classic piece kept as it is.

## 1. The target (owner's words)

> "I think you should basically be able to say, I want this type of element with this type of force or whatever and get what you want. So you'd say something like: I want a left turn with strong laterals or whatever, or I want a fully banked left turn, or ... as we have with the airtime hills, I think is an excellent reference: I want a floater hill, an ejector hill, whatever, and then the game engine should do the work for you to calculate the exact shape of the element to get what you want. And we should still support the standard fixed element shapes since those are important to be able to either just build it like an old school RCT game or to be able to get the train back to the station or whatever. So there should be some flexibility here. But ideally, this stuff should basically just work like you say I want a Raven turn and maybe you have some flexibility over peak force, whether you want it to be 4 G, 4.5 G, 3.8 G, whatever. But basically things should just work and be delightful and get us to great forces. Basically I want to have as close as we can the feel of a custom designed spline-based layout, but with the simplicity and pleasantness of a templatized builder."

> "think about a B&M looper of the various varieties: your loop might be taller or shorter to get the forces right."

A Raven turn is a real named inversion (a half loop entered from the top or bottom, like an Immelmann without the roll), so the intent model covers named inversions with a peak-force choice, not only hills and turns.

## 2. Root cause

Elements are shaped once, at authoring time, for a guessed speed and a generic train, then frozen. The layout must then be bent to arrive at that speed, and brakes are the only tool for that. Rated windows, feel caps, the 1.5 m/s margin and speed-matched variants all compensate for this.

Evidence (paths under the repo root):

- **The solver is offline.** `crates/sim/src/track/profile.rs` has a force-target solver (`ProfileSpec::hill`, `pull`, `crest`, `half_loop`, `full_loop`, `zero_g_hill`; `integrate`; `snap_hill`, `snap_pull`, `snap_half_loop`, `snap_full_loop`), all under `#[cfg(test)]`. Its output is 21 `HILL_CASES` frozen into `hills.generated.rs` (12,880 lines) and `profiles.generated.rs`.
- **It designs for the wrong train.** The solver assumes an 18 m train of 6 or 12 cars at rail speed (`train_mean_heights`). The hyper runs 8 cars over 20 m. `FloaterHill20x128` is designed for -1.0 g at 33 m/s, yet the hyper's worst car reaches -0.9 g at 29.97 m/s (TASK-128, TASK-151).
- **Identity is a fixed enum.** `PieceId` has 196 fieldless variants hashed `as u8`. A new size touches `HILL_CASES`, the generated tables, `PieceId`, the wire map in `crates/sim-wasm/src/construction.rs`, `catalog.rs`, model allow-lists in `models.rs` and the 351-row window table.
- **Windows mix hard limits with taste.** `crates/sim/examples/measure_force_windows.rs` caps floater maxima where airtime leaves "Good" (`cap_maximum_for_hill_feel`, 48 `AirtimeFeel` rows) and keeps hand-set design ±3 m/s for speed turns (`DesignCap`, 44 rows; `catalog.rs:1977`).
- **The margin gate treats every edge as hard.** `assert_design_margins` in `crates/sim/tests/known_circuit_statistics.rs` (and `rated_speed_margin` on branch dt-friction) demands 1.5 m/s inside every edge, while the same test already asserts measured forces 0.25 g inside limits for every car under both physics configurations.
- **Windows are advisory in the game.** They produce notes and labels (`record_rated_speed_band` in `physics/testing.rs`) and the "unavailable" refusal; scoring does not read them. The simulated ride is the truth.
- **Seams for intent-based building already exist:** `construction_select` accepts `hill:<feel>:<h>x<span>:<twist>` (`construction.rs:893`); `Construction` has `selected_hill_variant`; `PieceFamily::Hill(HillFamily { feel, height_metres, span_metres, twist, design_speed })` and `PieceFamily::SpeedTurn { radius, hand, design_speed }` are already spec-shaped; `Piece` is a value with geometry (`Shape::Profile(Arc<SolvedProfile>)`, `Shape::SpeedBank { design_speed }`) and is saved whole; `build_speed_turn(radius, design_speed)` already fits banked turns at runtime in a few milliseconds, limited only by the five-entry `SPEED_TURN_DESIGNS` table; `PreviewCache` (`physics/preview.rs`) re-simulates only the new piece from a snapshot with per-car evaluators; named elements exist as compositions in `construction/elements.rs` (`raven-turn-left` = `HalfLoopUp` + `RavenRollLeftDown`; Immelmann, dive loop, sea serpent).
- **Diagonal placement.** Profile pieces are stretched by √2 in x and z (`curves.rs` `raw()`) while the window table is keyed only by model, id and slopes, so a diagonal hill is a different hill with the same window. Read from code, not tested.

Horizon Hawk needed nine brakes because no hill is shaped for 34.6 m/s, its fixed turns accept only ±3 m/s with a gap at 23 to 25 m/s, and floater caps plus the 1.5 m/s margin force entry at -0.4 to -0.6 g.

## 3. Approaches

| | 1. Denser catalog, honest windows | 2. Shaped elements on grid connectors (recommended) | 3. Free spline or force-vector editor |
|---|---|---|---|
| Idea | More fixed sizes, force-derived windows, feel moved out of window edges, smarter variant selection | Intent (type, feel, optional force) solved at placement for the actual entry speed and train; output is an ordinary `Piece` | Off-grid nodes with force sections |
| Serves the vision | Partly: "nearest size", not "exact shape" | Yes | Yes for feel, no for templated simplicity |
| Complexity | Low per size, but each size is a seven-place edit forever | Medium: promote the solver, a calibration loop, new wire and UI states | Very high: supports, clearance, completion search and blocks all assume connectors |
| Runtime cost | None | One solve per candidate; estimated 40 to 60 ms in WASM (unverified) | High |
| Windows | Table grows; `PieceId` cannot hold a lattice | Table stays for classic pieces only; shaped elements carry a tolerance band | None |
| Player UI | More stepper positions | The hill inspector's steppers, now continuous | New editor |
| Test burden | Table and screenshot churn per addition | Property tests over (model, train, speed, intent) | Everything new |

Recommendation: approach 2, with the honest-window part of approach 1 applied to classic pieces as stage 2. Approach 3 is rejected. A precomputed lattice of geometry is rejected as the main route but kept as a fallback behind the same API if the WASM time budget is missed.

## 4. Design

### 4.1 Element and intent model

New module `crates/sim/src/track/shaped/`.

- `ElementIntent { kind, feel, strength: Option<f64>, size: SizeHint, hand }`: what the player or agent asked for, kept so an element can be refitted. `SizeHint` defaults to Auto for every kind.
- `ElementSpec`: the resolved numbers (design speed, cars, force targets achieved, height, span, radius, bank), stored on the piece and generalising `PieceFamily`.
- `solve(intent, context) -> Result<SolvedElement, SolveRefusal>`, where context is the open-end `TrackState`, the entry speed from the builder's preview, the model and the train.

| Kind | Feel intents | Strength | Size | Existing machinery |
|---|---|---|---|---|
| Airtime hill | speed, floater, strong floater, ejector; outer-bank twist | crest g | sized by the engine; optional override | `ProfileSpec::hill`, `reflect_hill`, `Twist::OuterBank` |
| Turn | fully banked, strong laterals, flat | vertical g or lateral g | sized by the engine; optional override | `build_speed_turn`, `Shape::SpeedBank` |
| Pullout, pull-up, crest, drop | none | peak g | sized by the engine | `ProfileSpec::pull`, `crest`, `snap_pull` |
| Loop, half loop | top feel | bottom g (3.8, 4.0, 4.5) | sized by the engine | `ProfileSpec::full_loop`, `half_loop` |
| Raven turn, Immelmann, dive loop, sea serpent, cobra roll | inside or outside where it exists | peak g | sized by the engine | compositions in `elements.rs`; roll length scaled for roll rate |
| Zero-g roll, stall | none | pull g, hang time | sized by the engine | `zero_g_hill`, `snap_zero_g_hill` |
| Classic pieces | all 196 ids unchanged | none | fixed | `catalog()` |

Models gain design rules (allowed kinds, default and maximum strengths, maximum bank, radius range). These are identity fields restored from the registry on load, like `key` and `cars_per_train`, so model serialisation does not change.

### 4.2 How the solver works

1. **Synthesis.** One pass of `integrate(spec, None, steps)`, a point-mass force schedule. For turns, `build_speed_turn` with a lateral term: centripetal g = (L + sin β) / cos β, where L is the target lateral.
2. **Grid snap** (section 4.3).
3. **Train calibration.** Simulate the model's actual train over the candidate from the builder's cached preview snapshot, plus a virtual run-out so the rear cars clear. Read the worst car's filtered, duration-aware peak, correct the target by the error, and repeat at most three times. This replaces the offline 18 m-train fixed-point iteration, so the readout shows the numbers the test run will.
4. **Robustness.** Hard limits must also pass at design speed ±0.5 m/s (the estimate-versus-test agreement tolerance); otherwise back the target off.
5. **Refusal.** A typed reason with the nearest feasible variant ("would stall: tallest here is 28 m", "4.5 g is over this model's limit").
6. **Validation.** The existing `validate_piece` for clearance, supports and tunnels.

WASM timing is estimated, not measured: synthesis and snapping under 2 ms; each calibration run about one candidate preview (TASK-123 recorded 6 to 10 ms native, roughly double in WASM); three runs give 40 to 60 ms against today's placement p95 of 164 to 191 ms in Chrome (TASK-133). Budget: solve p95 at most 50 ms in WASM, memoised on (intent, speed rounded to 0.1 m/s, connector, model, cars) with debounced stepper clicks. The offline 2048-step, 12-pass solve and the 40-iteration loop snap are too slow for this loop. Stage 1 starts with a one-day bench; if the budget is missed, the fallback is a precomputed table of calibration coefficients behind the same `solve` API.

### 4.3 Staying on the grid and closing circuits

Connectors stay exactly as they are (`TrackState` with 2 m half-tiles, 8 headings, 9 slopes, 4 banks). A shaped element starts and ends on a normal connector, so supports, clearance, blocks and Finish coaster (`completion.rs`) work unchanged.

- **Hills:** level to level at the same z; the apex is interior, so height is free; span snaps to 2 m with at most 2 m of base straight, as `snap_hill` does.
- **Turns:** a quarter turn lands on the grid for any even footprint radius, so radius is near-continuous (the code already does `radius / 2`).
- **Pulls and crests:** both endpoint coordinates snap with start and end inserts (`snap_pull`).
- **Loops:** exit insert, as `snap_full_loop` does.
- **Diagonals:** solve for the true footprint instead of stretching.
- **Closing:** classic pieces and the existing completion search do this; later, a "fit to target" option picks a turn radius or hill span so the end lines up with the station.

### 4.4 What a window means now

| | Classic piece | Shaped element |
|---|---|---|
| Hard envelope (forces with the duration rule, stall and clearance, roll rate) | From the generated table per model; no feel or design caps | Measured as a tolerance band around the design speed |
| Feel band | Speeds where it delivers its named feel (the 2026-11-18 "ejector still rates Great" rule lives here) | True by construction at the design speed; the band says how far speed can drift |
| Builder label | Red outside hard; amber outside feel, with the consequence ("rides as an ejector here") | "Shaped for 122 km/h"; "Refit" if the arriving speed leaves the band |
| Gate for showcase rides | Measured forces only | Measured forces only |

Where the 1.5 m/s margin belongs: force edges get no speed margin, because the 0.25 g margin on measured forces for every car under both physics configurations already does that job (about 1.2 to 1.7 m/s of entry speed at hyper speeds, by arithmetic). Energy and clearance minimums keep a speed margin, because a stall is not a comfort question. Feel is judged by `RideExpect` sustained targets (`TestRun::sustained_force_peaks`), never by a window edge. A train-length change re-solves shaped elements instead of needing per-length tables (TASK-150).

New report lines: mid-course brake runs; share of lift energy removed by trims; shaped elements off their design speed.

### 4.5 Builder and design tool

**Builder** (`web/src/construction.ts`, `construction-choices.ts`, `candidate-force-readout.ts`): the hill inspector becomes the shape inspector with Feel, Strength (g) and Size (Auto by default) steppers; the readout comes from the calibration run; the "For speed" bank position becomes the shaped turn with a radius stepper; the palette shows Classic and Shaped groups; a stale badge with Refit appears when the arriving speed changes; the wire format generalises the `hill:` grammar to `shape:<kind>:...`.

**Design tool** (`crates/design-tools/src/commands.rs`): `place hill feel=floater [g=-1.0] [height=36]`; `place turn left feel=banked [g=3.5 | radius=64]`; `place pullout g=3.5`; `place loop g=4.0`; `place raven-turn left g=4.0`; `options shapes` lists feasible ranges at the open end; `refit [n|all]`; `--matched` stays for classic pieces. The report gains an Elements table (design speed against entry under both configurations, intent against delivered, headroom in g) and the brake budget.

### 4.6 Save format and migration

- **Park saves:** `Piece` already saves its geometry, so a shaped element replays exactly without re-solving. Adding the spec changes the layout, so `TPSAVE` goes from version 3 to 4; older saves are rejected, as with every earlier bump.
- **`PieceId`:** new generic ids are appended at the end, because postcard encodes by index.
- **Ride files** (`track/ride_file.rs`): a `shape` step holds the intent and the resolved spec, so a physics retune does not silently reshape a committed ride; `refit` is explicit. String `place` steps are unchanged.
- **Regenerated assets:** `.snapshot` caches, the rated table (stage 2 only) and the design-tool goldens.
- **Classic geometry never changes,** so existing rides, sessions and builder digests should stay identical in stage 1 (not verified against state hashes).

### 4.7 The engine sizes every element

For every shaped family the player or agent gives the element type, the feel and optionally a force; the engine picks the dimensions from the entry speed. A faster entry gets a taller loop, a taller hill, a wider turn and a longer pullout, with the forces held on target. A size the player sets is an optional override. Fixed sizes exist only as the classic pieces. The catalog already reflects this: `Loop` and `LargeLoop` in `profile.rs` are the same intent (4.2 g bottom, 1.5 g top) solved at 27.5 and 31.3 m/s and frozen.

| Family | Forces that pin the shape | What the engine sizes | How it lands on the grid |
|---|---|---|---|
| Airtime hill | pull g, crest g | height and span | span snaps with up to 2 m of base straight; apex is free |
| Banked or lateral turn | vertical g, lateral g | radius and bank | any even radius lands on the grid |
| Pullout, pull-up, crest | peak g | arc radius and length | start and end inserts (`snap_pull`) |
| Vertical loop | bottom g, top g | height and width | exit insert (`snap_full_loop`) |
| Half loop, Immelmann, dive loop, Raven turn | bottom g, top g | height; roll length | height snaps to 2 m by adjusting the hold angle (`snap_half_loop`) |
| Zero-g roll, stall | pull g, 0 g crest, hang time | hill size and roll length | span insert (`snap_zero_g_hill`) |
| Sea serpent, cobra roll | as their halves | every part | each part as above |

Compositions are solved in order: each part is shaped for the speed the previous part delivers, and the calibration run covers the whole element; a roll half is lengthened for the speed at the top so it stays under the 200 degrees per second roll rule. Elements entered from the top (dive loop, descending half loop) are sized from the pullout force at the exit, working back from the bottom. Half-loop heights are quantised to 2 m with the hold angle absorbing the remainder, which moves the delivered force slightly (estimated 0.1 to 0.2 g, not measured). A cobra roll does not exist yet; it would be a new composition of half loop up, two half corkscrews and half loop down (connector alignment not checked).

## 5. Stages

Each stage merges green on `make check`, `make test-rust`, `make test-web`, `make lint`, feature tests, journeys and `make ride-agreement`.

### Stage 1: shaped airtime hills and banked turns, and a Horizon Hawk the owner can ride

Work: the solver bench; promote the hill half of `profile.rs` out of `#[cfg(test)]`; replace the `SPEED_TURN_DESIGNS` array with a keyed cache; `Construction::append_shaped` with `selected_hill_variant` generalised; wire, inspector and design-tool commands; rebuild Horizon Hawk. Shaped pieces have no rated window in this stage (`design_speeds: None`), so the old margin rule does not apply to them.

Acceptance:
- New `crates/sim/tests/shaped_elements.rs`, for every model, every allowed train length and entry speeds 14 to 36 m/s: worst-car sustained airtime within 0.1 g of the intent; every car at least 0.25 g inside limits at design speed and inside limits at ±0.5 m/s; a turn's lateral at most 0.1 g at design speed and bank at most 75 degrees; exit on a grid connector; typed refusals for stall and infeasible targets; with Auto size and a fixed feel, hill height and turn radius increase with entry speed.
- Save round trip identical; solving twice gives the same hash; solve time recorded natively and in the browser bench.
- `horizon_hawk_is_a_straight_lift_hyper_under_both_physics_configs` asserts at most one mid-course brake run (contiguous brakes not ending at the station), sustained airtime in [-1.2, -0.8], sustained positive in [3.0, 3.5], lateral at most 0.6 g, excitement 6 to 8, at least 6 airtime moments. `content/rides/horizon-hawk.ride.json` gains `sustainedMinVerticalG`.
- Classic builder digests and session replays unchanged.

### Stage 2: honest windows for classic pieces, tolerance bands for shaped ones

Work: split hard from feel in `rated_windows.rs`; move the scan into a library function shared by the generator and the runtime; remove `AirtimeFeel` and the speed-turn `DesignCap` as edges; apply the margin by edge source; consequence labels (TASK-145), Refit and the brake budget report.

Acceptance: a test that no hill or speed-turn row has a feel or design edge; sampled table rows equal the runtime measurement; every changed window listed in a review document; all five catalog rides pass on measured forces with no window margin; readout unit tests; affected screenshots re-recorded, each explained.

### Stage 3: strength control, turn feels and vertical transitions

A first drop or pullout shaped for 3.3 to 3.5 g at any height; "strong laterals" and flat turns. Acceptance: property tests extended; a pullout from 30 to 70 m drops holds the target within 0.15 g.

### Stage 4: named inversions with peak force

Loop, half loops, Immelmann, dive loop, Raven turn, sea serpent, cobra roll (new composition), zero-g roll and stall. Acceptance: loop height increases with entry speed at fixed bottom and top g; bottom and top forces within 0.2 g of target across the feasible speed range for Timber Tumbler and Velocity Viper trains; the 200 degrees per second roll rule holds; no stalls; each named inversion has a typed refusal below its minimum speed; `Loop`, `LargeLoop`, `SteelLoop` and other classic ids keep their geometry and tests.

### Stage 5: rebuild the other showcase rides

Rebuild with intents, fit-to-target closing, trim brakes with a set speed, top hats (TASK-125). Folds in TASK-128, 139, 145, 150 and 151.

## 6. Owner decisions (feel, realism and scope)

1. Feel names and default strengths per model. Today "floater" targets -0.3 g in `select_speed_matched_variant` and the "Good" band ends at -0.9 g; the owner wants about -1 g on the hyper. Proposal: floater -0.5, strong floater -0.9 (the hyper camelback default), ejector -1.2 on steel and -0.8 on wood.
2. Whether the 2026-11-18 ejector rule moves from a window edge to a feel band, so a classic hill ridden past its feel shows an amber note instead of a red "too fast".
3. Peak-force choices per model (3.8, 4.0, 4.5 g), and whether 4.5 g is a brief peak under the current duration rule (limit plus 0.5 g for 0.2 s) or a model's sustained limit rises.
4. Brake realism for showcase rides (at most one mid-course brake run, final brake run exempt), and whether a player's excitement score penalises trims or only reports them (proposal: report only).
5. Refit behaviour when upstream edits change the speed: always ask, or reshape silently when the footprint does not change.
6. Default strength of "strong laterals": 0.6 g (the owner's comfort cap) or up to the model limit.
7. Loop top feel: the catalog loops use 1.5 g at the top (1.0 g on the Briar Blitz steel loop); should shaped loops default to that pressed-in feel or a lighter, floatier top, and should it be a choice next to the bottom force?

## 7. Assumed, not verified

- All solver and calibration timings, and that one-pass synthesis plus at most three calibration runs converges with smooth forces (for loops too; the offline loop snap iterates up to 40 full solves).
- The closed-form estimate that a force-derived speed-turn window is far wider than ±3 m/s (for the 32 m/s turn roughly 19.5 to 35 m/s at 0.6 g lateral and 4.0 g vertical).
- That drag alone brings Horizon Hawk from 34.6 m/s to the mid-20s by the end of a roughly 1,200 m course, so the final brake run may need to be long or elevated.
- That measurement and preview code run on a piece with a new generic id (id-specific matches exist in `rated_piece.rs`; `features.rs` and `scoring.rs` not audited).
- That tall shaped hills route supports (`supports.rs` not read in depth; TASK-126 notes record `NoSupportRoute` failures near tunnels).
- That spline rolls lengthen by moving endpoints (`RolloutLeft` suggests so).
- Branch dt-friction (with `rated_speed_margin`) merges to master first.

## Critical files

- crates/sim/src/track/profile.rs
- crates/sim/src/track/curves.rs
- crates/sim/src/track/construction/mod.rs
- crates/sim/src/physics/rated_piece.rs
- crates/sim-wasm/src/construction.rs

## 8. Decisions (2026-11-19)

Owner:
1. Hill feels by default: floater -0.5 g, strong floater -0.9 g (the default hyper camelback), ejector -1.2 g on steel and -0.8 g on wood, sustained airtime. Strength stays adjustable per hill.
3. Peak force steps on steel: 3.5, 3.8, 4.0, 4.2 and 4.5 g. 4.5 g is a brief peak under the existing duration rule (limit plus 0.5 g for 0.2 s); sustained limits do not change.
4. Brakes: owner's words: "if we are aiming for realism, midcourse brake runs tend to be a bit of a downside at least if they subtract substantial speed so we might actually want a small penalty here. Not something brutal, but a mild penalty when a midcourse brake run takes out a ton of energy." So: a mild excitement penalty that grows with the share of energy removed by mid-course brakes (none for light trims, small for heavy ones), shown in the score breakdown. Showcase rides additionally keep the test cap of at most one mid-course brake run.
7. Loop top feel is a setting next to the bottom force: pressed-in (1.5 g), neutral (1.0 g, the default) or floaty (about 0.3 g).

Lead (technical defaults, reported to the owner):
2. The 2026-11-18 ejector rule moves from a window edge to a feel band in stage 2: a classic hill ridden past its feel gets an amber note naming the consequence, not a red refusal.
5. Refit: reshape silently when the footprint does not change; ask when it would.
6. "Strong laterals" defaults to 0.6 g (the owner's comfort cap), adjustable up to the model limit.

## 9. Correction after the first solver runs (2026-11-19, lead)

The feel defaults in section 8 were chosen without checking them against each model's force limits, and the stage 1 property "every car at least 0.25 g inside limits" then contradicts them on several models. The limits (minimum vertical g) are: wooden twister -1.0, hybrid -1.5, Velocity Viper -1.5, family coaster -0.6, hyper -1.3. An ejector at -1.2 g sustained cannot fit 0.25 g inside the hyper's -1.3 g limit, a wooden ejector at -0.8 g leaves at most 0.2 g, and a family floater at -0.5 g leaves at most 0.1 g, before the rear car's deeper peak is counted.

Rule from now on: the limits and the 0.25 g margin stay exactly as they are, and a feel's target is capped per model. Each feel's sustained target is the section 8 default, made shallower where needed so the worst car's filtered peak stays at least 0.25 g inside the model's limit. The solver aims for the capped target and the property test checks delivery within 0.1 g of the capped target. The builder and the design tool say when a feel is capped ("ejector capped at -0.95 g on this model"). Grid points where no shape passes after a bounded search are recorded as infeasible with the measured best case, and the lead reviews that table before it is accepted.

The classic digest change came from enabling serde_json's `float_roundtrip` feature in three crates, which changes how ride-file floats parse by one unit in the last place. Shaped elements must not depend on it; the feature is removed so classic parsing stays as on master.

## 10. Correction after the first implementation stalled (2026-11-20)

The first implementation (branch `shaped-elements`, about 15 agent-hours) built a 46,845-row, 51.5 MB precomputed table and never went green. An audit found three causes, none of them physics. First, both design laws disagree with the simulator's force formula: `rider_g` measures force along the heartline per unit of rail length, while `build_speed_turn` and the offline hill integrator assume the heart moves at train speed. Banked turns therefore come out over-banked (measured lateral falls monotonically with speed, as predicted), and crests deliver deeper airtime than intended, worst on tight crests. Second, the table stored point-mass shapes: the train-aware solve ran but its result was discarded, and the crest correction stopped after three non-converging steps. Third, the scope (every model, every train length, 14 to 36 m/s) and a 50 ms budget smaller than one full-train preview pushed the design toward a table.

The classic pieces fail the original turn property too: SpeedTurnLarge16/20/24/28 measure 0.105 to 0.181 g lateral at design speed, against the 0.1 g property and the existing 0.35 g classic gate. Classic hills are rated below their own design speed. The thresholds below are anchored to that baseline.

Revised stage 1:
- Scope: hyper, 4 to 8 cars, entry 22 to 36 m/s; floater (-0.5 g), strong floater (-0.85 g), fully banked turn. Below 22 m/s, typed refusal; classic pieces cover low speed. A second model follows once this is green.
- No table. Analytic first guess with the heart/rail factor in both laws (shaped pieces only; classic and the 21 legacy hill cases untouched), then a secant on the real simulator: at most four previews per solve. Browsing shows the analytic shape plus one preview, which doubles as the force readout; confirming the piece runs the remaining previews.
- Hill sizing: pullout about 3.0 g, height 30 to 40 percent of the entry energy height, chosen by a bounded sweep.
- Turn bank capped where the radius would go below the model minimum.
- Acceptance: hill worst-car sustained airtime within 0.10 g of the capped target at design speed (0.15 g under the other physics configuration); strong-floater cap never shallower than -0.75 g from 24 to 36 m/s; every car 0.25 g inside limits at design speed and inside limits at ±0.5 m/s in both configurations; turn lateral at most 0.15 g at design speed and 0.25 g at ±0.5 m/s, bank at most 75 degrees; exit on grid; height and radius non-decreasing with speed; typed refusals; deterministic; save round trip; commit p95 at most 250 ms in Node/WASM; WASM within 150 KB of master; classic digests unchanged.
- Open owner question: the hyper's -1.3 g limit with the 0.25 g margin caps sustained airtime near -0.95 g; the owner's target is about -1 g.
