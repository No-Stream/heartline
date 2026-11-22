# Hyper and Giga Elements: Speed Hill, Overbanked Turn, Hammerhead Turnaround (2026-11-22)

Plan for `TASK-172`. The hyper model (`hyper_model` in `crates/sim/src/track/models.rs`, "Horizon Hawk" by "Boulder & Marlin") gets the elements that real B&M-style non-inverting hypers and gigas use and that Sky Harrier could not build: a speed hill, an overbanked turn and a hammerhead-style turnaround. A stall is deliberately left out (see section 1 and the owner questions). This document is investigation and design only; no code was changed.

**Landing order matters.** Three lines of work append to the same positional enums: branch `giga-ride` (not merged; appends `PieceId::GigaSixtyDownToLevel` after `ShapedTurnRight`), the uncommitted `TASK-168` shaped-inversion work in the main checkout (appends six `PieceId`s at that same position, plus `ElementIntent` / `ElementSpec` / `BinaryElementSpec` variants), and this plan. `giga-ride` and the `TASK-168` work already conflict with each other on `PieceId` order, independent of this plan. The order assumed here is `giga-ride`, then `TASK-168`, then the slices below; whichever lands second must move its new `PieceId`s after the other's.

---

## 1. What real B&M hypers and gigas contain

A web research pass (Wikipedia ride and element articles, Coasterpedia, forum threads) gave the list below. Bank angles and g figures for real rides are mostly marketing or anecdotal numbers and should be read as approximate; the research agent could not open RCDB or CoasterForce. Real ride names appear only in this section and must not be used in game content.

| Element | What it is on real rides | Typical figures found |
| --- | --- | --- |
| Camelback hills | The core of the ride: 5 to 8 hills of falling height | 15 to 55 m tall; anecdotal airtime -0.2 to -0.5 g on older rides, up to about -0.9 g on newer ones |
| Hammerhead turn | A climbing 180-degree turnaround at the far end of an out-and-back layout. The track curves away first, crests wide and steeply banked, and comes back down next to the outbound track | 37 to 48 m tall; quoted banks from 55 to 123 degrees |
| Overbanked turn | A tall banked turn of about 180 degrees whose bank passes 90 degrees at the crest; on gigas and at least one hyper, usually soon after the first drop | 48 to 50 m tall on gigas; quoted banks 91 and 115 degrees. Angles above about 120 degrees belong to other manufacturers |
| Speed hill | A low, long hill taken at high speed (one giga names one explicitly, after its second drop) | No published dimensions found |
| High-speed banked turns, S-curve | Ground-level turns at near top speed on gigas | No angles found |
| Helix | One or two, late in the ride, 270 to 540 degrees, descending or rising | - |
| Mid-course brake run, trim brakes | Hypers have a mid-course brake about two thirds through; gigas mostly do not. Trims sit on late hills | - |
| First drop | 65 to 80 degrees on hypers, 80 to 85 degrees on gigas | Top speed 30 to 36 m/s (hyper), 41 to 42.5 m/s (giga) |
| Train | 7 to 9 cars, one row of 4 per car | Matches our 4 to 8 cars |

Published maximum forces are 4.1 to 4.5 g. No B&M hyper or giga has a stall, a zero-g stall or any other inversion; the closest thing is the floaty crest of a tall hammerhead or overbanked turn. Wave turns and trick track are other manufacturers' signatures (one B&M giga has a banked airtime hill that its park labels a wave turn; treated here as an exception and not copied).

**Elements this plan adds to the hyper:** speed hill, overbanked turn (90 and 180 degrees), hammerhead turnaround. **Already present:** camelbacks (shaped hills), level banked turns (shaped turns), brakes, the 60-degree and giga pullouts. **Excluded:** stall, wave turn, any bank above 120 degrees. **Named as follow-ups, not designed here:** helix, 180/270-degree level turns that hold their bank, a steeper first drop.

---

## 2. Audit: what the hyper can do today

All probes below were run with the prebuilt design tool in the `giga-ride` worktree (commit `4382d08`), on Sky Harrier truncated after its first-drop runout: 8 cars, level, 42.1 m/s.

### Speed hill

The model already lists two classic speed hills, `SpeedHill4x64` and `SpeedHill6x80` (`models.rs`, hyper `allowed_pieces`). Their design speeds are 23.9 and 24.9 m/s (`catalog.rs`), so they are useless at 38 to 42 m/s. That is why the designer saw no speed hill.

A shaped hill with a small fixed height is already a speed hill in shape, and it partly works at 42.1 m/s:

| Probe | Result |
| --- | --- |
| `shape:hill:strongFloater:default:auto` | 36.1 m tall, 272 m span, -0.84 g (the automatic size is 40% of the energy height, `hill_height_energy_fraction: 0.4`) |
| `strongFloater` -1.05 g, `fixed-20` | solves: 20.1 m x 240 m, -1.05 g |
| `strongFloater` -1.05 g, `fixed-16` | solves with the target relaxed to -1.00 g: 16.0 m x 228 m, delivered -0.97 g |
| `strongFloater` -1.05 g, `fixed-12` and `fixed-8` | refused, `infeasibleTarget` (headroom +0.003 g and +0.14 g against the required 0.25 g) |
| `strongFloater` -0.85 g, `fixed-12` / `fixed-10` | refused, `infeasibleTarget` |
| `floater` -0.5 g, `fixed-12` | solves: 12.0 m x 220 m |
| `floater` -0.5 g, `fixed-10`; `strongFloater`, `fixed-6` | refused, `invalidGeometry` |
| `ejector` -1.2 g, `fixed-10` | solves: 10.0 m x 184 m, -1.20 g (ejector is outside the hyper's typical feels, so it is judged by the ASTM envelope and shows the amber note) |
| `ejector` -1.2 g, `fixed-8` / `fixed-6` | refused |

Further along the same ride the lowest strong floater that solves at -1.05 g drops with speed: at 37.8 m/s, 14 m (186 m span) solves and 12 m is refused; at 35.2 m/s, 14 m (168 m span) solves at the full target, 12 m solves relaxed to -1.00 g, and 10 m is refused. Across the three speeds the lowest height that delivers the full -1.05 g is 0.22 of the energy height `v^2 / 2g` (20, 16 and 14 m after rounding up to the 2 m grid).

So the gap is narrower than "no piece": at giga speed a low hill solves down to about 16 m for a strong floater and 10 m for an ejector. What is missing is (a) a speed-hill choice with a sensible automatic low height, since `auto` always gives the tall camelback, (b) a feel whose typical target is brief stronger airtime inside the hyper's own force budget, and (c) a typed refusal below the geometric minimum instead of `invalidGeometry`. I did not isolate which force (minimum vertical, maximum vertical or longitudinal) eats the 0.25 g budget on the 8 to 12 m strong floaters, because `probe` does not print per-axis headroom; the first speed-hill step measures it. The likely cause of `invalidGeometry` is the flank search in `hill_profile_for_height` (2 to 75 degrees) meeting the fixed 0.18 rad taper (`SHAPED_HILL_TAPER_ANGLE`), also to be confirmed.

### Why turns stop near 73 degrees

This is physics, not a parameter. A shaped turn (`solve_turn` in `shaped.rs`, geometry in `integrate_shaped_turn_at_bank` in `curves.rs`) is level and coordinated: the bank at each point is the angle at which the sideways force is zero, so the vertical force is `1 / cos(bank)`. The hyper's budget is 4.0 g with 0.25 g headroom, and `1 / cos(74.5 degrees)` is 3.75 g. The probes agree: at 42.1 m/s a 100 m radius gives 71.5 degrees and 3.45 g, 96 m gives 73.1 degrees and 3.67 g, and 92 m is refused. `maximum_bank_degrees: 75` and the `peak_bank < FRAC_PI_2` assertions in `curves.rs` sit behind the same limit. Real ground-level high-speed turns bank about the same, so the level turn is already realistic.

A bank beyond 90 degrees is only possible when the track crests while it turns. With the track accelerating downward faster than gravity, the force a rider feels points toward the inside and below the horizon, and the seat has to roll past vertical to stay under it. An overbanked turn is therefore a hill crest and a turn at once, and it needs a new shaped element, not a wider range on the level turn.

### Other findings

- Each shaped turn is exactly 90 degrees and its bank ramps from level to peak and back (`speed_turn_bank_fraction`), so the two-turn pairs in Sky Harrier pass through a straight, unbanked moment in the middle of every 180-degree turn. A real turnaround holds its bank.
- `HybridOverbankLeft/Right` exist for the hybrid and launch models, but they are small fixed pieces rated 5 to 13 m/s (`Shape::Overbank`, a 120-degree roll painted onto a flat large turn). They are not usable on a hyper and not force-solved.
- `StallLeft/Right` exist on the hybrid and launch models and are inversions (`RolledProfile` with `Twist::Stall`). The hyper has `allowed_inversions: vec![]`, and `tests.rs` asserts it stays empty.
- **An overbank is counted as an inversion today.** A physics sample is `inverted` when `track_sample.up.z < 0.0` (`physics/mod.rs:749`). `count_inversion_passages` and `inverted_seconds` in `physics/testing.rs`, and `active_inverted_seconds` in `physics/features.rs`, use that flag without looking at `piece.inversion`. Any bank past 90 degrees would add an inversion to the ride statistics, break Sky Harrier's `inversions: 0` expectation and feed the time-inverted score. From reading the code, the existing 120-degree `HybridOverbank` pieces have this problem already; I did not run a ride to confirm it. No content ride uses them.
- The hyper's first drop tops out at 60 degrees (`Slope::DownSixty`), against 65 to 85 degrees on real rides.

---

## 3. Design

Three rules apply to every addition. Existing solves must not drift: `hill_pull_g`, `turn_vertical_g`, the flank bounds and the automatic sizes for the existing feels stay as they are, so Horizon Hawk, Daisy Dash and Sky Harrier re-solve to their saved geometry digests. Capability comes from `ShapedProfile` fields, never from model keys in `shaped.rs` (`solver_source_names_no_coaster_model_key` stays green). New geometry uses `crate::math` and solver inputs rounded with `round_element_value`, so native and WebAssembly builds solve the same shape, and each solve stays within 4 previews and the 250 ms confirm budget of `TASK-157`.

### 3.1 Speed hill: a new feel on the shaped hill

This is a parameter and policy change, not a new element kind. `HillFeel::Speed` already exists in the enum (used by the classic pieces), and `ElementSpec::Hill` / `BinaryElementSpec::Hill` store no feel, so nothing is appended to any save enum. The hill geometry, digest and sampling code are reused unchanged.

- **Intent:** `ElementIntent::Hill { feel: HillFeel::Speed, strength_g, size }`. Wire id `shape:hill:speed:<g|default>:<auto|fixed-<m>>`; CLI `place hill feel=speed [g=] [height=]`.
- **Feel:** a low hill taken fast, with a short, stronger pop of airtime. Default target -1.0 g sustained (the owner's "about -1 g"), held to the hyper's own budget: no car below -1.25 g and none above 3.75 g. `Speed` joins the hyper's `typical_hill_feels`, so a default speed hill shows no amber note.
- **Automatic size:** a new profile field `speed_hill_height_energy_fraction`, starting at 0.22 and rounded up to the 2 m grid, against 0.4 for camelbacks. That is the measured lowest height for a full -1.05 g strong floater at 35.2, 37.8 and 42.1 m/s (14, 16 and 20 m). Below 35 m/s it is untested; the matrix in section 4 covers 26 to 44 m/s and the fraction is tuned there. The height is chosen once, analytically, so the solve keeps the same 4-preview budget as other hills; if that height fails the force budget the solver refuses and does not search other heights.
- **Solver gates to open:** `solve_hill_analytically` and `intent_matches_spec` in `shaped.rs` both reject every feel except floater, strong floater and ejector, and `ShapedProfile::default_hill_strength` / `default_hill_strength` have no `Speed` arm. All four need the new feel.
- **Profile fields** (`ShapedProfile`): `default_speed_hill_g` and `speed_hill_height_energy_fraction`. A model with `default_speed_hill_g == 0.0` refuses the feel with `UnsupportedScope`. Give the hyper values now; the other four models can get values in the same slice if their matrices pass, since the code path is shared.
- **First step, before any tuning:** a native test that prints per-car minimum vertical, maximum vertical and longitudinal force for 8, 10, 12 and 16 m hills at 34, 38 and 42 m/s, to find which force blocks the low sizes. If it is the gap between sustained and peak airtime on a short crest, the fix is the lower default height above; if it is the pull-up, the speed feel gets its own `speed_hill_pull_g`. Do not change `hill_pull_g`.
- **Refusal cleanup:** a fixed height below what the flank search can reach returns `InfeasibleTarget` instead of `InvalidGeometry`.
- **UI:** one more palette entry, "Speed hill", and `speed` in the hill feel stepper; strength and height steppers as for other hills. The classic `SpeedHill4x64` / `SpeedHill6x80` pieces stay.

### 3.2 Overbanked turn and hammerhead turnaround: one new shaped kind, the crest turn

Both elements are a climbing turn that crests with the bank past 90 degrees, so both come from one solver. The internal name is **crest turn**; players see "Overbanked turn" (90 or 180 degrees) and "Hammerhead turnaround" (180 degrees, exit beside the entry).

**Geometry: a path integrated from forces.** The existing solvers already work this way in two dimensions: the level turn integrates plan curvature from a bank schedule, the hill integrates vertical curvature from a g schedule. The crest turn does the same in three. Along the heart line, with speed from energy as in the hill solver, the solver prescribes a felt-force schedule `n(s)` (g along the seat normal) and a bank schedule `bank(s)`. Zero sideways force is built in: the seat normal `up` is the unbanked up vector rotated about the tangent `T` by the bank, and the path curvature vector is `(n * g * up - g * (z - (z . T) * T)) / v^2`, where `z` is the world vertical unit vector (the second term is the part of the upward reaction to gravity that lies across the track). Integrating gives position, tangent and bank at every step. Past the point where the downward curvature exceeds gravity's share, the same formula yields a bank over 90 degrees with a positive seat force; nothing special-cases it.

**Symmetric halves.** Only the first half, entry to apex, is integrated. The second half is its mirror image across the vertical plane through the apex that is perpendicular to the apex tangent, as the hill and the `TASK-168` loop mirror about their apex. The exit is then automatically level, unbanked and at entry height, and the exit position is the mirror of the entry: for a 180-degree element it has the same forward coordinate and a sideways offset `W`; for a 90-degree element it is `(R, R)` like a level turn. The train is slightly slower in the second half, so forces there differ a little; the calibration previews measure the real train and judge that.

**Schedules.** The half path has four consecutive phases, each with a length that is a solver knob: an optional flare (hammerhead only: a short opposite bank, so the track curves away from the turn first, which is what brings the exit back beside the entry), a pull-up ramp and hold at the pull-up force, an easing phase in which `n` falls to the apex value and the bank rises to the apex bank, and an apex hold at constant apex force and bank. The pull-up force, apex force and apex bank are inputs. Unknowns are matched to conditions one for one:

| Mode | Conditions | Unknowns |
| --- | --- | --- |
| Overbank, automatic height | apex tangent level; apex heading = half the sweep | pull-up hold length, easing length (apex hold fixed at a short constant) |
| Overbank, fixed height | the two above; apex height | the two above; apex hold length |
| Hammerhead, automatic height and width | the first two; flare fixed at the profile's default angle | pull-up hold length, easing length |
| Hammerhead, fixed width and/or height | the first two; exit offset; apex height | pull-up hold length, easing length, flare length, apex hold length |

Automatic height is therefore the natural height of the solved shape, an output, not a target; in the sketch below it came out at about a third to a half of the energy height. Solve with a fixed number of Newton steps on finite differences from an analytic first guess (deterministic, no data-dependent iteration count), then round the resolved parameters to nine decimals and store them as the recipe. A fixed height or width that the knobs cannot reach is refused with `InfeasibleTarget`.

**Grid.** After mirroring, scale the whole path uniformly by a factor close to 1 so that `W` (180 degrees) or `R` (90 degrees) lands on the 2 m grid (`2 * sqrt(2)` m on diagonal headings), as the level turn scales its knots. Exits: 90 degrees reuses `turn_exit`; 180 degrees gets a `turnaround_exit` (heading + 4, sideways offset in half tiles, same height), modelled on `raven_turn_exit`.

**Intent parameters.**

| Parameter | Values | Notes |
| --- | --- | --- |
| `hand` | left / right | |
| `sweep` | quarter (90 degrees) / half (180 degrees) | Hammerhead is always half |
| `bank_degrees` | steps of 5; proposed range 90 to 120, hyper default 110 | The apex bank. The range offered is whatever the feasibility map of the solver slice shows to solve across the speed range; the profile stores it (`minimum_overbank_degrees`, `maximum_overbank_degrees`) |
| `apex_feel` | `LoopTopFeel` from `TASK-168`: floaty 0.3 g / neutral 1.0 g / pressed in 1.5 g; hyper default neutral | Felt force at the crest. Reusing the enum keeps one "top feel" control in the UI |
| `size` | auto, or fixed height in 2 m steps | Auto is the natural height of the solved shape |
| `width` (hammerhead only) | auto, or fixed exit offset in 2 m steps, minimum 12 m | Auto picks the smallest offset that passes clearance and forces |

**Force targets.** Entry and exit pull at the profile's `crest_turn_pull_g` (start at 3.3 g, like `hill_pull_g`), which puts the sustained positive force in the owner's 3 to 3.5 g band; no car above 3.75 g within the model's typical feel. Apex force within 0.2 g of the chosen feel. Sideways force at most 0.15 g at design speed (`MAXIMUM_FULLY_BANKED_LATERAL_G`) and at most 0.3 g at plus or minus 0.5 m/s. Peak roll rate under `Piece::MAXIMUM_ROLL_RATE_DEGREES_PER_SECOND`. Calibration uses at most 4 previews, with secant updates on the pull-up and apex values against the worst car, as the loop solver does. Above-typical requests are judged by the ASTM envelope with the amber note, as elsewhere.

**Feasibility sketch.** A throwaway integration of this construction (crude schedules, coarse two-parameter search, no train, no losses) at 42 m/s entry with a 3.3 g pull gave, for a 180-degree element: bank 115 degrees and 1.5 g at the apex, about 31 m tall, 84 m between entry and exit, 210 m of track, apex speed 34 m/s; bank 110 degrees and 1.2 g, about 49 m tall, 106 m wide, 264 m of track, apex speed 28 m/s. At 30 m/s entry the 110-degree element was about 23 m tall and 50 m wide. These sizes are in the range of the real rides in section 1. The sketch left a few degrees of residual error in some cases, missed by 14 degrees for a 100-degree bank at 1.5 g, and nearly stalled for an 80-degree bank, so the feasible region of bank against apex feel is not yet known and the 90 to 120 range above is a proposal; mapping it is the first deliverable of the solver slice, and combinations outside it are refused with `InfeasibleTarget`.

**Types and data.**

- `ElementIntent::CrestTurn { hand, sweep, bank_degrees, apex_feel, size }` and `ElementIntent::Hammerhead { hand, bank_degrees, apex_feel, size, width }`, appended after `RavenTurn`.
- One `ElementSpec::CrestTurn` (and matching `HumanReadableElementSpec` / `BinaryElementSpec` variants, appended) for both: design speed, cars, train length, hand, sweep, target and delivered apex g, delivered peak g, apex bank, height, width, the recipe (pull g, apex g, half length, hold end, apex-phase length, flare angle, scale), `geometry_digest`, and the regenerated `CrestTurnGeometry` (knots of heart position, tangent and bank).
- `PieceId::ShapedOverbankLeft/Right` and `ShapedHammerheadLeft/Right`, appended last; names added to the catalog list and to `piece_from_spec`; arms in `is_resolved_shaped_piece` (`models.rs`).
- `Piece::sample` / `raw` / `sample_for_physics` in `curves.rs` get a `CrestTurn` arm that interpolates the knots with cubic Hermite segments, like `speed_turn_frame_with_geometry`, with the rail at `heart - up * HEART_HEIGHT_METRES`.
- `ShapedProfile` fields: `minimum_overbank_degrees`, `maximum_overbank_degrees` (0 means unsupported), `default_overbank_degrees`, `crest_turn_pull_g`, `hammerhead_flare_degrees`. Only the hyper gets values in this plan. The hybrid and launch models could adopt the element later by filling in the fields.
- New solver code goes in `crates/sim/src/track/shaped/crest_turn.rs`, next to `inversions.rs`.

**An overbank is not an inversion.** Keep the per-sample `inverted` flag as it is (it is hashed into samples), and change the three consumers so they count only where the piece declares an inversion: `count_inversion_passages`, `count_inversion_passages_on_sampled_run` and `inverted_seconds` in `physics/testing.rs`, and `active_inverted_seconds` in `physics/features.rs`. `inversion_passages` in `features.rs` already checks `piece.inversion`. Crest-turn pieces have `inversion: None`. This also corrects the existing `HybridOverbank` pieces, so any test pin or score for a circuit that uses them may move and is re-recorded in that slice. The other readers of the flag keep their behaviour: `is_eventful` and the highlight on first inverted sample in `features.rs` may keep treating an overbank crest as an eventful moment, and `inverted_minimum_vertical_g` in `physics/preview.rs` (the lowest seat force while upside down) is a useful number for a crest turn too. The fix relies on every real inversion declaring `piece.inversion`; the catalog sets it for loops, Raven rolls, heartline rolls, zero-g rolls and stalls (`catalog.rs` around lines 728 and 1374 to 1858), but I did not check every inverting piece, so the slice adds a catalog-wide test: every piece with a sample whose `up.z` is negative either declares an inversion or is one of the overbank ids. The new piece ids map to `ElementClass::Overbank` in `element_classes`, so they earn the same variety credit.

### 3.3 Stall: not added

No real B&M hyper or giga has one, it is an inversion, and the hyper's defining rule is no inversions. The hammerhead with `apex_feel = floaty` gives the nearest realistic sensation: a slow, steeply banked crest at about 0.3 g. If the owner still wants a true stall on this model, it would be the classic `Stall` profile solved for speed as a shaped inversion under `TASK-168`'s machinery, and the hyper would need `allowed_inversions` to stop being empty; that is a product decision, listed in the questions.

### 3.4 What each area needs

- **Clearance and supports:** unchanged code path. `append_resolved_piece` runs `validate_piece` on the solved piece. Two new cases need tests: a 180-degree element whose exit runs back beside its own entry (the minimum width of 12 m must pass `validate_piece`), and support columns under track banked past 90 degrees.
- **Places a new piece id shows up:** besides the solver files, `ShapedTurnLeft` is referenced in `models.rs`, `catalog.rs`, `curves.rs`, `crates/sim-wasm/src/construction.rs`, `crates/design-tools/src/commands.rs` and `web/e2e/rides/ride-agreement.ride.spec.ts`. Adding the single classic id `GigaSixtyDownToLevel` on `giga-ride` also touched `track/tests.rs`, `web/src/piece-icons.test.ts`, `web/e2e/piece-catalog.spec.ts` and Horizon Hawk's builder trace digest; check each of those for the new shaped ids.
- **Rated windows:** none. Shaped pieces have `design_speeds: None` and are solved for their entry speed; `rated_windows.generated.rs` does not change.
- **Saves:** append-only. New variants go at the end of `PieceId`, `ElementIntent`, `ElementSpec`'s wire enums and `BinaryElementSpec`. The speed hill appends nothing. Add round-trip tests (ride-file JSON and `TPSAVE`) for each new variant and keep the existing legacy-decode tests green.
- **Wire grammar** (`crates/sim-wasm/src/construction.rs`, `reports.rs`, `web/src/generated/`): `shape:hill:speed:...`, `shape:overbank:<left|right>:<quarter|half>:<bank>:<floaty|neutral|pressedIn>:<auto|fixed-<m>>`, `shape:hammerhead:<left|right>:<bank>:<feel>:<auto|fixed-<m>>:<auto|width-<m>>`. Extend `ConstructionShapeSpecWire` with apex g, peak g, bank, height and width, and regenerate with `make generate-report-types`.
- **Builder UI** (`construction-choices.ts`, `construction-shaped-elements.ts`, `construction.ts`, `candidate-force-readout.ts`): palette entries shown only when the model's profile supports them (a new capability flag in the generated model data, like `allowedInversions`); steppers for bank (5-degree steps), apex feel, height and width (2 m steps); readout of design speed, apex and peak g, bank, height and width.
- **Design tools** (`crates/design-tools/src/commands.rs`): `place hill feel=speed`, `place overbank <left|right> [sweep=90|180] [bank=] [apex=] [height=]`, `place hammerhead <left|right> [bank=] [apex=] [height=] [width=]`; `options shapes`, `probe`, `estimate` and the report's element table; the regeneration script and tables in `docs/designing-rides.md`. Also print per-axis headroom in shaped refusals, which this audit lacked. The finish search (`completion.rs`) keeps placing only level turns.
- **Native and browser agreement:** add parity cases to `shaped_parity_cases_json` (`make check-shaped-turn-arc-parity`) for a speed hill, a 90-degree overbank, a 180-degree overbank and a hammerhead, and cases to `make bench-shaped-elements-cold`. All stepper values above are reachable in the browser.

---

## 4. Tests

Native physics first, in `crates/sim/tests/shaped_elements.rs` and unit tests beside the solver:

1. **Crest-turn offline solver (no simulator):** apex tangent level and apex heading exact within 1e-6; mirrored half joins with continuous position, tangent, curvature and bank; exit lands on the grid for all 8 headings; point-mass force along the path matches the schedules; the feasible region of bank (90 to 120) against apex feel at 26, 34 and 42 m/s is recorded, and every combination outside it returns `InfeasibleTarget`.
2. **Acceptance matrix on the hyper** (4, 6 and 8 cars; 26 to 44 m/s; both hands; cardinal and diagonal entries; default and construction-test physics): delivered apex g within 0.2 g of target; every car within the force budget at design speed and plus or minus 0.5 m/s; sideways force limits above; apex bank equals the request and exceeds 90 degrees when asked; height grows with entry speed at fixed intent; roll rate under the limit; `append_shaped` passes clearance and supports on flat ground.
3. **Not an inversion:** a closed test circuit with a 110-degree overbank reports 0 inversions and 0 inverted seconds, and counts one `ElementClass::Overbank`; a circuit with a real loop still counts 1.
4. **Speed hill:** the per-axis diagnostic above; matrix at 26 to 44 m/s for 4, 6 and 8 cars with delivered airtime within 0.05 g of target and height near the profile fraction; automatic speed hill is lower and shorter than the automatic strong floater at the same speed; too-low fixed heights give `InfeasibleTarget`.
5. **No drift:** `every_content_ride_shaped_intent_resolves_to_its_saved_geometry_digest`, the Horizon Hawk and Sky Harrier force tests and the classic-piece digests stay unchanged.
6. **Typed refusals:** models whose profile lacks the new fields return `UnsupportedScope`.
7. **Saves:** round trips for every new variant; determinism and canonicalisation.

Then `make test-design-tools` (parsing, `options shapes`, probe output, a ride file using each element), `make check-report-types`, `make test-agreement`, web unit tests for the selection ids and steppers, and Playwright cases in `web/e2e/construction.spec.ts` that place each element on the hyper and step its controls. The browser ride-agreement run for Sky Harrier comes once, at the end.

---

## 5. Implementation sequence

Each slice lands on its own with its tests green. "Waits" means it edits files that the uncommitted `TASK-168` work is changing (`shaped.rs`, `models.rs`, `curves.rs`, `catalog.rs`, `construction.rs`, `commands.rs`, the web shaped UI) or appends to the same enums.

| # | Slice | Depends on | Conflict with `TASK-168` work |
| --- | --- | --- | --- |
| 1 | Count inversions only on pieces that declare one (`physics/testing.rs`, `physics/features.rs`) with the tests in item 3 | nothing | Small: `features.rs` has a 14-line change there. Can start now in a worktree |
| 2 | Crest-turn offline solver and geometry type in the new file `shaped/crest_turn.rs`, with the tests in item 1, including the feasibility map | nothing | None while it stays in new files. Can start now |
| 3 | Speed-hill diagnostic test and height/pull investigation on `giga-ride` | `giga-ride` (44 m/s ceiling) | None for the measurement; the fix waits (slice 4) |
| 4 | Speed hill feel end to end: solver policy, profile fields, wire id, CLI, UI entry, guide | `giga-ride` and `TASK-168` landed | Waits |
| 5 | Crest turn in the sim: intent, spec, piece ids, sampling, model gating, saves, acceptance matrix (overbank 90 and 180 degrees) | slices 1 and 2, `TASK-168` landed (also for `LoopTopFeel`) | Waits |
| 6 | Overbank in wire types, design tools, parity and benchmark cases, builder UI and Playwright | slice 5 | Waits |
| 7 | Hammerhead: flare and width control, own intent and piece ids, tools and UI | slices 5 and 6 | Waits |
| 8 | Sky Harrier redesign with an overbanked turn after the first drop, a speed hill, and a hammerhead turnaround; re-record its test pins; `tools/design check` under both physics configurations; one browser ride-agreement run; update `docs/designing-rides.md` | slices 4 to 7 | Waits |

Slices 1, 2 and 3 can run in parallel now. Slices 4 and 5 can run in parallel after `TASK-168` lands, with 5 appending its enum variants first if both are in flight.

**Follow-ups worth their own tickets, not designed here:** level turns of 180 and 270 degrees that hold their bank (removes the unbanked moment in the middle of today's turn pairs; must leave the 90-degree turn's digests untouched); rising and descending helices (needs the vertical-transition controls of `TASK-169`); a steeper first drop for the hyper (75 to 85 degrees); crest turns for the hybrid and launch models, replacing the fixed `HybridOverbank` pieces.

---

## 6. Open points

**Not verified in this investigation:**

- Which force blocks strong-floater hills of 8 to 12 m at 42 m/s, and the exact cause of `invalidGeometry` below about 10 m (slice 3 measures both).
- That `HybridOverbank` pieces add an inversion to ride statistics today (read from the code, not run).
- The feasible region of apex bank against apex feel for the crest turn; the sketch numbers in 3.2 are from a crude throwaway integration.
- The bank angles and g figures for real rides in section 1.
- How `TASK-169` (lateral-rich turn feel) will extend the `shape:turn` id; the new ids here use their own prefixes to stay clear of it.

**Questions for the owner (feel and realism):**

1. **Stall.** Real B&M hypers and gigas have none, and it is an inversion. This plan leaves it out and offers the hammerhead with a floaty crest (about 0.3 g, slow, banked past vertical) in its place. Is that acceptable, or should the hyper get a true stall and stop being a no-inversion model?
2. **Crest feel of the overbank and hammerhead.** Default proposed: about a 110-degree bank with about 1.0 g at the crest, with floaty (0.3 g) and pressed-in (1.5 g) as choices and the bank adjustable in 5-degree steps (90 to 120 degrees if the solver can deliver that range). Real B&M versions are quoted between 91 and 123 degrees. Is 110 degrees and neutral the right default, or should the default be the floaty version?
3. **Speed hill strength.** Default proposed: -1.0 g, brief, on a hill about 20 m tall at giga speed (about half the height of today's automatic camelback). Real B&M airtime is gentler (about -0.5 g, newer rides nearer -0.9 g). Keep -1.0 g as the default, or default to a B&M-like -0.85 g and leave -1.0 g one step away?
4. **Steeper first drop.** Real gigas drop at 80 to 85 degrees and hypers at 65 to 80; ours stops at 60. Should a steeper drop be the next ticket after this plan?
