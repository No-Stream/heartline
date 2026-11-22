# Global force limits and shaped elements for every model (2026-11-21)

> Status (2026-11-22): Both Part A (`TASK-166`) and Part B (`TASK-167`) landed on `master` (`bbb856c`), including the five-model `ShapedProfile` rollout, ASTM F2291 envelope judging with amber model-feel notes, two-edge rated windows, all-model finish search, and the Daisy Dash force-headroom redesign. Remaining follow-ups live in `TASK-168` (shaped inversions), `TASK-169` (lateral-rich turns and vertical transitions), `TASK-170` (refresh remaining showcase rides), `TASK-157` (shaped hill confirm latency), and `TASK-158` (shaped inspector layout clipping).

Owner direction, 2026-11-21: player-built coasters are judged by one force-and-duration envelope that follows ASTM F2291 loosely, erring slightly generous. The per-model force limits are "vibe limits" so sample rides feel like the model they are inspired by; they must not punish a player's ride that is fine by the envelope. Shaped elements should exist for every coaster model, built as one modular system.

Decisions taken by the lead on the plan's open questions (owner may override):
1. Generosity: use the ASTM numbers as read, with every ambiguity resolved in the player's favour (per-level durations, 200 ms minimum event, the 1.1 combined multiplier); no extra uniform margin.
2. The builder still says when a force exceeds the model's typical feel: an amber note with no score effect, never red.
3. Lateral and longitudinal "strong" starts at one global 0.75 g instead of half the model limit; "great from" stays per model because it only adds credit.
4. No family-model exception to the envelope; guests with low intensity preference already avoid intense rides.

## Findings on master bc12252

- `ComfortableForceLimits` per model in `crates/sim/src/track/models.rs` feed the force rating bands, the force evaluation (sustained limit plus a 0.5 g / 0.2 s brief allowance, `physics/force_evaluation.rs`), the rated window generator (`examples/measure_force_windows.rs`, `physics/rated_piece.rs`), the finish search (`completion.rs:1157`, model limit minus 0.25 g), the shaped hill strength cap (`shaped.rs:1203`, model limit plus 0.25 g), the design tool's headroom check (`crates/design-tools/src/report.rs`, minus 0.25 g), and web texts (`web/src/score-findings.ts:268`, `web/src/coaster-testing.ts:253`).
- Players are penalised twice: `scoring.rs` has a per-model `ExcessiveForces` term (time in TooMuch and Dangerous bands) and a global `Envelope` term.
- The "vertical force 5.0 g, above the 4.0 g limit" testing note comes from `force_advice` (`physics/testing.rs:1408`) and quotes the envelope, not the model.
- `comfort.rs` has two envelope implementations: the test-only `ComfortTracker` (per-level durations, ignores events under 200 ms) and the production `EnvelopeTracker`, which times everything above the curve plateau (stricter than ASTM).
- The +Gz curve in code (6 g to 1 s, 4 g at 2-4 s, 3 g at 5-11.8 s, 2 g from 12 s) may be stricter than F2291 (recalled as 5 g at 2 s, 4 g at 4 s, 3 g at 11.8 s, 2 g at 40 s); unverified.
- Shaped elements are hyper-only via `model.key == "hyper"` (`shaped.rs:1076`); other hyper-specific parts: 22-36 m/s and 4-8 car constants, `SHAPED_HILL_PULL_G` 3.3, `HILL_HEIGHT_ENERGY_FRACTION` 0.4, the 3.5 g / 0.6 g finish ceilings (`completion.rs:42-44`), the hyper-only test matrix and the Horizon Hawk-only parity check. `shaped_element_rules()` already derives partly from the model, and the builder already shows the shaped palette on every model (`web/src/construction-choices.ts:16`) with a typed refusal.

## Part A: global limits

Rename `ComfortableForceLimits` to `DesignForceTargets` (field `design_force_targets`) so the compiler lists every use. Design targets serve `tools/design check`, ride-file expectations, shaped feel defaults, feel-edge windows and `tools/design finish`. Everything that judges a player's ride uses the envelope.

| Consumer | Today | After |
|---|---|---|
| Score penalty | `ExcessiveForces` (per model) plus `Envelope` | `Envelope` only |
| TooMuch / Dangerous bands | above model limit / 1.3x | envelope exceeded / exceeded by more than 30% |
| Weak..Strong bands | partly scaled by model limit | absolute g thresholds; "great from" per model |
| `forceLimitExceeded`, overlay, ride graph | model limits | envelope |
| Rated window maximum | one edge, model evaluator | hard edge (envelope, energy, clearance, roll rate) and feel edge (design target, airtime feel) |
| Builder "too fast" | any window edge | red past hard edge; amber "stronger than a typical <model>" past feel edge |
| Finish search | design target minus 0.25 g | caller passes the judge: envelope in the builder, design targets in `tools/design finish` |
| `tools/design check`, ride expectations | design target minus 0.25 g | unchanged |

| Work item | Owner | Gate |
|---|---|---|
| ASTM figure check: six duration curves, 1.1 combined multiplier, reversal, push-pull and onset rules against F2291 figures; record points in `docs/ride-score-calibration.md` | research agent | table of standard value against code value, reviewed by the lead |
| Type split, no behaviour change: rename to `DesignForceTargets`; one envelope module (per-level durations, 200 ms minimum) replacing both trackers | Rust sim agent | workspace tests, `make lint`, every digest and golden byte-identical |
| Envelope drives exceed flags; remove `ExcessiveForces`; absolute bands; apply the ASTM corrections | Rust sim agent | `make test-force-rule`, calibration ordering assertion untouched, every changed golden listed before/after in the calibration doc |
| Two-edge rated windows; regenerate | Rust sim agent | `make generate-rated-windows`, `make test-rated-windows`, `make test-design-tools`, lead reviews edge diff |
| Wire and web: flags, red/amber readouts, overlay, ride graph, score-findings text | web agent | `make check`, `make e2e-feature`, `make journeys`, `make ride-agreement` |
| Finish search takes its judge from the caller | Rust sim agent | `make bench-finish`, finish tests, design-tool finish outputs byte-identical |
| Save migration for stored evaluator limits (versioned, no silent default) | Rust sim agent | old-save load test, park determinism tests |

Expected score effect: sample rides that pass their design check barely move; the Arrow looper archetype (the only synthetic one with time in TooMuch) moves to about 4.6 excitement / 6.7 intensity / 4.7 nausea, inside its bands; the enthusiast ordering must still hold in `calibration_tests.rs`. Guests read the ratings, so park determinism hashes and park-report goldens need re-recording.

## Part B: shaped elements for every model

Add a `ShapedProfile` to each model's data (not saved, not hashed): entry-speed and car ranges, offered hill feels and default targets, hill pull g and height fraction, turn vertical target, maximum bank and radius bounds, finish-search ceilings. The solver reads only the profile and the envelope judge; `model_supports_shaped_elements` becomes "has a profile and the train is in range". A source guard asserts `shaped.rs` names no model key. The acceptance tests in `crates/sim/tests/shaped_elements.rs` become one matrix over every profiled model, and `make check-shaped-turn-arc-parity` a per-model case list (one turn and one hill each, native against Node/WASM, byte-equal).

| Model | Needs | Main risk |
|---|---|---|
| Hyper | constants into its profile | every Horizon Hawk digest must be unchanged |
| Velocity Viper (3-6 cars) | explicit turn target (formula gives 3.86 g); speed floor about 20 m/s | radius and headroom misses at 14-19 m/s (TASK-156) |
| Timber Tumbler (4-8 cars) | ejector default -1.2 g; speed floor question | 14 m/s hills failed in TASK-156 |
| Briar Blitz (4-12 cars) | ejector default capped at -0.75 g; bank cap near 60 degrees | nine train lengths; lateral-rich turn feel is a follow-up |
| Daisy Dash (4-10 cars) | floater only, default -0.35 g; speeds about 10-20 m/s | low-speed hills are where the solver is weakest |

Each model rollout delivers its profile, the acceptance matrix under both physics configurations, a parity case, a small browser-agreement ride, palette and stepper bounds from the profile, and its row in `docs/designing-rides.md`.

| Work item | Owner | Gate |
|---|---|---|
| Modularisation, hyper only | one Rust sim agent (solver owner) | Horizon Hawk state hash, trace digest and 18 geometry digests unchanged; classic digest guards; `make check-shaped-turn-arc-parity`; `make bench-shaped-elements` p95 no worse |
| Viper and Tumbler profiles | two parallel agents, data and tests only | per-model matrix, parity case, `make ride-agreement`, `make e2e-feature` |
| Daisy Dash and Briar Blitz profiles | solver owner, one model at a time | as above, plus the modularisation zero-diff gate after any solver change |
| Finish search places shaped turns for every profiled model | Rust sim agent | `make bench-finish`; classic-only results unchanged where no shaped turn is chosen |

An agent that needs a solver change stops and hands it to the solver owner. The first all-model attempt (branch `shaped-elements`) took about 15 agent-hours and never went green, mainly because of scope.

## Ordering

ASTM figure check and the type split first; the shaped modularisation depends on the type split for the envelope judge. Then two parallel streams: envelope judging, two-edge windows, then web/finish/save; and shaped modularisation, Viper and Tumbler, Daisy Dash and Briar Blitz, then the all-model finish search. Only the envelope stream re-records score goldens; the shaped stream may not change an existing ride's digest.

Running branches: rated-margins lands as is (its flat-turn maximum becomes a feel edge, the two-edge windows work adds the hard edge from the envelope's lateral curve); station-arrival should read the envelope's braking limit (both 1.5 g today; the rename catches it); briar-redesign keeps designing to Briar targets minus 0.25 g and merges before the envelope stream re-records goldens.

## Risks

The ASTM check may show the envelope is stricter than the standard in several places, moving every score with envelope exposure. Low-speed shaped hills may not converge, keeping a model's speed floor high. Save compatibility for the stored evaluator. Hill delivery is sensitive at 1e-5 g across native and WASM, so every model needs its own parity case before content uses it. The window table doubles its force edges, with unmeasured regeneration time.
