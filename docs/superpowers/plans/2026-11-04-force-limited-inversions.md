# Force-limited inversions and drops

> **For agentic workers:** Work items are in the last section, in order, each with its test. Do them with superpowers:subagent-driven-development or superpowers:executing-plans. Do not skip the first milestone (the half loop end to end); it proves the method before the other pieces are converted.

**Goal:** Every inversion, drop and 60° transition in the catalog keeps a full train inside its model's rider force limits across a stated entry speed range, lands exactly on grid connectors, and is produced by one general method instead of hand-tuned constants.

**Branch:** `inversion-forces` (work-in-progress commit `04f3efc` on top of master). Code under `crates/sim/src/track/` (`curves.rs`, `catalog.rs`, `half_loop.rs`, `models.rs`) and `crates/sim/src/physics/`. Tests in `crates/sim/tests/inversion_physics.rs`, `crates/sim/tests/top_hat_circuit.rs`, `crates/sim/src/physics/tests.rs`, `crates/sim/src/track/tests.rs`.

## Why the two earlier attempts failed

Three separate errors stack up, and the earlier attempts only addressed the first.

1. **Shapes were sums of a spline and a circle.** `Piece::raw` in `curves.rs` builds a quintic Hermite from entry to exit and adds a circular offset for `Loop`, `Corkscrew` and `Heartline`. The curvature of that sum is not controlled anywhere, so the master half loop reached 19.3 g. The WIP `half_loop.rs` fixed this for one piece by integrating a constant-force curve, but it hit its grid target with three hand-found constants, which does not generalise.
2. **Forces were designed for a point mass but measured on a train.** The physics integrates one speed for the whole train from the mean slope of all cars (`Train::gravity_acceleration`). When the front car is at the top of a 32 m half loop, the train's mean height is 10–15 m lower, so the front car is 15–25 % faster than a point mass would be, and its force is that much higher. That is why the WIP half loop measures 5.68 g with a train while the point-sampled test `half_loop_forces_across_entry_speeds` passes at 4.4 g.
3. **Forces are computed on the rail, not at the rider.** `rider_g` differentiates the track centreline. A heartline roll is defined by the rider's heart staying still while the rail orbits it, so on the rail it reads as a 1 m radius helix: 4 g lateral at 32 m/s. The same error makes the 16 m barrel-roll drop read tens of g, and it is why the half corkscrews show 2.9 g lateral.

Each of these gets one decision below: design curvature directly from a force target (1), design against the worst car's speed in a full train (2), and measure and design at the heart (3).

## Decisions

### 1. Heart height and where forces are measured

Rider forces are evaluated at the heart, 1.2 m above the rail along the rider's up vector (`HEART_HEIGHT_METRES = 1.2` in `curves.rs`). `TrackSample` gains a `heart: Vec3` field. For all existing shapes it is `position + up * 1.2`; for the new roll shapes it is the designed heart path. `rider_g` in `physics/mod.rs` replaces its finite difference of `tangent` with a central second difference of `heart` over track distance: with `H(s)` the heart and `v`, `a` the train speed and acceleration along the track, the felt acceleration is `H''(s) v² + H'(s) a + g ẑ`, projected on `(up, up × tangent, tangent)`. Use a step of 0.02 m, clamped to the piece as today.

Blast radius: every force reading in the game shifts by a few percent (loops read slightly higher because the heart circle is smaller than the rail circle; turns are unchanged because the offset is vertical). There are no golden state hashes, and the ratings tests in `physics/testing.rs` mostly use synthetic readings, so this is a tolerance check, not a rewrite.

### 2. Force limits per model

Limits are **peak** values: the maximum over all cars and all time steps of the heart reading, with the full train simulated. Sustained load is not a design criterion. The ride report already handles duration: it notes a contiguous run of at least 8 m above 3 g (`HIGH_FORCE_*` in `physics/testing.rs`) and scores nausea per sample above 3.5 g. Every real loop produces that 8 m note; leave the report as it is and do not try to design around it.

| Model | Inspiration | Vertical max | Vertical min | Lateral | Longitudinal |
|---|---|---|---|---|---|
| Briar Blitz | GCI wooden twisters (Wodan, Thunderhead, Mystic Timbers) | 4.0 | −1.0 | 1.5 | 1.5 |
| Timber Tumbler | RMC I-Box hybrids (Steel Vengeance, Iron Gwazi, Twisted Timbers) | 4.5 | −1.5 | 1.5 | 1.5 |
| Velocity Viper | Intamin and Mack multi-launch (Velocicoaster, Taron, Maverick) | 5.0 | −1.5 | 1.5 | 1.5 |

Changes from the WIP: Timber Tumbler's vertical max drops from 5.0 to 4.5 (RMC rides peak around 4.2 g on accelerometer rides; 5 g is launched-steel territory), and longitudinal rises from 1.0 to 1.5 on all three. Longitudinal must rise because the physics reports `g (t_car − t̄_train)` on a vertical section, which reaches 0.9 g for the front car of a long train with no design lever to reduce it; 1.5 matches the report's own `HIGH_FORCE_LONGITUDINAL_G`. Wood at 4.0/−1.0 is conservative and matches the lower restraint standard of wooden trains. These remain game design targets, in the spirit of the existing doc comment on `ComfortableForceLimits`; ASTM F2291 allows more for very short durations, which is why peak-only checking at these values is reasonable.

Design targets are the model limit minus a margin: vertical `G_design = max − 0.3`, negative `G_design = min + 0.5`. The margin covers interpolation error, the heart frame being measured in the rail frame, and the small longitudinal coupling.

A piece has one geometry. Where two models allow it, it is designed for the most restrictive one (the Loop for Timber Tumbler at 4.2 g, so Velocity Viper has slack). Briar Blitz already has its own loop id (`SteelLoop`), which now gets its own, larger geometry instead of being a clone of `Loop`.

### 3. Entry speed ranges

Plausible speeds come from each model's inspiration. Speeds below are at the bottom of a drop from a lift of the stated height, as `sqrt(2 g h)` less about 3 % for the train's length.

| Model | Lift height range | Speed at the bottom | Launch |
|---|---|---|---|
| Briar Blitz | 25–40 m | 21–27 m/s | none |
| Timber Tumbler | 30–63 m | 24–34 m/s | 31.3 m/s |
| Velocity Viper | 30–50 m | 24–31 m/s (34 after a post-launch drop) | 31.3 m/s |

Each profile or roll piece gets a `RatedSpeeds { minimum_entry: f64, maximum_entry: f64 }` field on `Piece` (`None` for plain pieces). The maximum is a design input chosen per element from the table above; the minimum is an output of the solver: the entry speed at which the train's mean-height-corrected speed at the slowest point of the piece is 4 m/s. Descending pieces that are the time reversal of a climbing piece (half loop down, half corkscrew down) are rated by exit speed, and their entry maximum is `sqrt(maximum² − 2 g Δz)`; their entry minimum is 2 m/s.

| Element | Rated maximum entry speed | Design force (vertical) | Reasoning |
|---|---|---|---|
| Loop, LoopRight | 31.3 m/s | 4.2 g bottom, 1.0 g top | A launch straight into the loop must work on Velocity Viper; a 50 m drop reaches it on Timber Tumbler |
| SteelLoop, SteelLoopRight | 27 m/s | 3.7 g bottom, 1.0 g top | Briar Blitz with a 35–40 m lift |
| HalfLoopUp / HalfLoopDown | 31.3 m/s (down: at exit) | 4.2 g, 1.0 g at the top | Same as the loop; immelmanns and dive loops follow a big drop or a launch |
| TopHat | 27 m/s | 3.7 g pull-up, −0.5 g over the crest | Briar Blitz only |
| LevelToSixtyUp, SixtyDownToLevel, SteepToSixtyUp, SixtyToSteepDown | 34 m/s | 4.2 g | Pull-ups and pull-outs at the bottom of the largest Timber Tumbler drop |
| SixtyUpToLevel, LevelToSixtyDown, SixtyToSteepUp, SteepToSixtyDown | 22 m/s | −1.0 g | Crests at the top of a launched climb (31.3 m/s launch, 30 m climb gives 20 m/s); at a lift top they run at 3 m/s and are trivially fine |
| Corkscrew, CorkscrewRight | 20 m/s | 4.2 g | Classic corkscrews run at 13–18 m/s; the piece sits after a climb |
| HalfCorkscrew (4) | 20 m/s (down: at exit) | 4.2 g | Sized so an immelmann from a half loop at 31.3 m/s exits within limits |
| HeartlineRoll(Right) | 30 m/s | gravity only | Forces at the heart do not depend on speed; the cap keeps the roll above 1 s |
| RavenRoll (4) | 20 m/s | gravity plus the S-curve | Placed at the top of a half loop, 10–14 m/s in practice |
| BarrelDrop, TwistedDrop, BankedDrop (6) | 15 m/s | gravity plus the drop curvature | Used at a lift top |

The windows will be narrow for loops, on the order of 12–15 % of the maximum speed, because a loop sized for 4.2 g at speed `v` is tall enough that it needs about `0.88 v` to clear. That is physics, not a defect; real loops have one design speed. The report tells the player when they are outside the window (work item 11).

**How tests get the numbers.** The force test reads `piece.rated_speeds` and runs the full train at `minimum_entry` and `maximum_entry`; there is no third speed because forces peak at the top of the window and clearance binds at the bottom. A separate gameplay test proves the windows are reachable: it builds, for each model, a lift of the heights above (Briar 38 m, Timber Tumbler 50 m, Velocity Viper a launch) followed by level track, measures the train speed there, and asserts it lies inside the window of each inversion the model allows at the bottom of a drop (loops, half loop up, top hat, 60° pull-outs).

### 4. Curve families

Two families cover every element. Planar **profiles** cover anything that lives in the forward/up plane: loops, half loops, top hat, the eight 60° transitions. **Rolls** cover anything whose rider frame rotates about the direction of travel: heartline rolls, raven rolls, the three drop kinds, corkscrews and half corkscrews. Both are designed on the heart path and the rail is derived from it.

#### 4a. Planar profiles

Coordinates: `x` forward along the entry heading, `z` up, pitch angle `θ` from +x toward +z, rider up `(−sin θ, cos θ)` (always "upright" in the loop plane, so at `θ = π` the rider is inverted). Signed curvature `κ > 0` turns toward the rider's up.

Vertical rider force at the heart is `G = κ_h v² / g + cos θ`, so the heart curvature that produces a target force schedule `G(θ)` is

```
κ_h(θ) = g · (G(θ) − cos θ) · w(θ) / v_w(θ)²
```

where `w(θ)` is the end taper and `v_w` the worst-car speed (below). The rail is the heart offset by `−1.2 m` along up, a parallel curve, so its curvature is `κ_t = κ_h / (1 + 1.2 κ_h)` (both signs) and its arc length element is `ds_t = ds_h (1 + 1.2 κ_h)`.

Integrate the heart coordinates in `θ` with 2048 midpoint steps: `ds_h = dθ / κ_h`, `x_h += ds_h cos θ`, `z_h += ds_h sin θ`. Derive each rail node as `heart − 1.2 up(θ)`, and store its rail arc distance and curvature. Sampling reuses the quintic-per-segment Hermite interpolation already in `half_loop::sample`, which matches position, tangent and curvature at the nodes.

Force schedules `G(θ)`:

- Pull-up or pull-out (0° to 90°, 0° to 60°, 45° to 60°): constant `G_design`.
- Half loop and loop (0° to 180°, 0° to 360°): cosine blend `G(θ) = G_top + (G_max − G_top)(1 + cos θ) / 2`, so the bottom pulls `G_max` and the top `G_top = 1.0`. A constant `G_max` over the top would make the top radius a few metres and break the model's minimum radius; the blend is what real clothoid loops do.
- Crest (the top hat's 90° to −90°, the 60° crests): constant negative `G_design` (−0.5 g on Briar, −1.0 g on steel). The curvature formula gives `κ < 0` automatically.

End taper: `w(θ) = min(1, (Δθ_from_start / θ_taper)^(2/3), (Δθ_from_end / θ_taper)^(2/3))` with `θ_taper = 0.3 rad` for arcs of 60° or more and a quarter of the arc for the 15° arcs. The 2/3 power makes curvature grow as `s²` from a straight connector (zero curvature and zero curvature rate at the join) and keeps `∫ dθ / κ` finite. Do not use a taper that vanishes linearly in `θ`; the arc length diverges.

**Worst-car speed.** The physics gives the whole train one speed set by the mean height of its cars. With the train at entry speed `v_max` on level approach track at height `z_entry`, the speed when car `i` (0 is the front) is at rail distance `s` is

```
v_i(s)² = v_max² − 2 g (z̄_i(s) − z_entry),   z̄_i(s) = mean over j of z_rail(s + (i − j) Δ)
```

with `Δ` the car spacing. Beyond the piece ends, `z_rail` continues straight along the entry or exit tangent. The design speed is `v_w(s) = max_i v_i(s)`, taken over both train configurations in the catalog (6 cars and 12 cars, both 18 m long; constants in the solver). The speed depends on the curve being solved, so iterate: integrate with the point-mass speed, then re-integrate with `v_w` computed from the previous curve, four times or until the endpoint moves less than 1 mm. Assert convergence.

**Straight inserts** are the free parameters for snapping (decision 5). A profile is a list of segments, each an arc `(Δθ, schedule)` or an insert `(direction θ, length a ≥ 0)`. Inserts are integrated as straight lines (no `θ` change) and participate in the speed iteration like anything else.

**Rated speeds** come from the solved curve: `minimum_entry² = 2 g · max_s(z̄_worst(s) − z_entry) + 4²`, where `z̄_worst` uses the car with the highest mean height at each `s` (the mirror of the force case). Assert `minimum_entry < maximum_entry − 2` at design time; if it fails, lower `G_top` (fatter top, shorter loop) or the rated maximum. The solver also asserts the rail's minimum radius is at least the most restrictive allowing model's `minimum_radius_metres + 0.5`; if that fails, raise the crest or top force target. Both are design-time panics in the `LazyLock` initialiser with the piece name in the message, so a test failure names the piece and the cure.

Expected outcomes, to be replaced by the solver's numbers (print them in the test): Loop bottom heart radius 31 m, height about 39 m, minimum entry about 27 m/s; SteelLoop radius 27.5 m, height about 34 m, minimum about 25 m/s; HalfLoopUp rise about 36–38 m with the exit 10–12 m ahead of the entry; TopHat about 35 m tall with a 17 m crest radius; LevelToSixtyUp about 36 m forward and 20 m up (today's piece is 60 m by 40 m).

#### 4b. Rolls

A roll piece is a heart path `H(t)` plus a roll schedule `φ(t)`. The rail is

```
rail(t) = H(t) − 1.2 · up_h(t),   up_h = base_up(t) cos φ + left(t) sin φ
```

where `base_up` is the no-roll up vector of the heart tangent (the same formula `sample()` uses today) and `left = base_up × tangent`. `φ(t) = φ_entry + (φ_exit − φ_entry) · S7(t)` with `S7 = 35t⁴ − 84t⁵ + 70t⁶ − 20t⁷`, which has zero first, second and third derivatives at both ends so the rail's curvature is zero at straight connectors. One schedule drives both the rail offset and the reported up; today the position uses `S7` and the up uses the quintic `smooth`, and that mismatch alone moves the heart.

Heart endpoints are the rail connectors offset by `+1.2 up(bank)`, so the rail lands exactly on the connectors at `t = 0, 1`. `TrackSample.up` is `up_h` re-orthogonalised against the rail tangent (Gram–Schmidt), because `all_pieces_match_their_end_states_and_have_regular_curves` requires `up ⟂ tangent` to 1e-8; at the ends `φ' = 0` so `up_h` is already perpendicular and `roll` matches the connector bank. On a fast roll the rail tangent tilts up to about 15° from the heart tangent; the physics projects heart acceleration on this rail frame, which mixes at most 0.25 g between vertical and lateral during a 1 g field. Accepted.

Two heart paths:

- **Spline.** The quintic Hermite already in `raw()` between the heart endpoints with the connector tangents and zero end curvature. Used by HeartlineRoll (straight, `φ: 0 → ±2π`), RavenRoll (`y` shift of 8 m, `φ: 0 → ±π` or back), BarrelDrop (`φ: 0 → ±2π`, lengthened to 32 m forward and 8 m down because 16 m is a 15 g whip even at the heart), TwistedDrop (`φ: ±π/4 → 0`) and BankedDrop (constant `±π/4`). Forces at the heart are gravity rotated plus the spline's own curvature; the solver only checks them (full-train simulation at the rated speeds) and does not iterate.
- **Helix.** For corkscrews. Axis along the forward direction `f` at `r = 5 m` above the entry heart; `H(t) = A + f · L t + r (σ l sin φ − u cos φ)` with `σ = ±1` for left/right, `l` the left vector, `u = ẑ`; `up_h = (A + f L t − H) / r = u cos φ − σ l sin φ`. Full corkscrew `φ: 0 → 2π`, half corkscrew `0 → π` (exit inverted, `2r = 10 m` higher, no lateral offset, same heading). Vertical force at the heart is `κ_h v² / g + cos φ`; the only free parameter is the length `L`, chosen as the smallest multiple of 2 m for which the full-train check at the rated maximum stays below `G_design` (linear scan upward from 20 m; curvature falls monotonically with `L`). Expected: about 56 m for the full corkscrew and 28 m for the half, with a rail radius of 6.2 m.

Rail derivatives for roll shapes come from central finite differences of `rail(t)` in `t` with `δ = 1e-5`, evaluated inside `raw()` (the spline and `S7` are polynomials, so stepping outside `[0, 1]` at the ends is fine). This keeps the join test's 1e-8 curvature tolerance; the analytic alternative needs the derivative of `base_up`, which is not worth writing.

#### 4c. Speed loss with height

Both families use `v_w(s)² = v_max² − 2 g (z̄ − z_entry)` on rail heights with no rolling resistance or drag, which is conservative because resistance only slows the train. Descending pieces are the exact time reversal of their climbing twin (the set of car positions is the same, so the energy is the same), so HalfLoopDown samples the HalfLoopUp profile with the parameter reversed, as the WIP does, and the same holds for the four 60° pairs (one profile per pair) and the half corkscrew pairs.

### 5. Grid snapping

Only connectors must sit on the grid: integer units of 2 m in `x`, `y`, `z`, with discrete heading, slope and bank. Apex heights, loop widths and lateral extents in the middle of a piece are free. Diagonal headings are unreachable in practice (there is no 45° turn piece; `Heading::NorthEast` is used only by terrain), so `Piece::at` keeps its current rotation code and no diagonal variant is designed.

Rule: solve the curved core first with all inserts at zero, round each connector coordinate **outward** to the grid, and set the inserts to the difference. Inserts only add length in a fixed direction, so they are always non-negative and a solution always exists.

- **Half loop up** (entry level east, exit level west and inverted, `Δz` up, `Δx` ahead). Core gives `(Δx₀, Δz₀)`. Target `Δz_t = ceil(Δz₀ / 2) · 2`; solve the vertical insert `a` at `θ = 90°` by bisection on `Δz(a) = Δz_t` (monotone; the insert adds height directly and the upper half shrinks by less than that). Then `Δx_t = floor(Δx(a) / 2) · 2` and the exit insert (pointing west) `b = Δx(a) − Δx_t`. Two one-dimensional solves, no Jacobian.
- **Loop** (entry and exit level east at the same height, exit shifted `y = 2` units sideways, as today). Symmetric about the vertical axis by construction (symmetric schedule, symmetric worst-car speed under time reversal), so `Δz = 0` with no insert; `Δx_t = ceil(Δx₀ / 2) · 2`, exit level insert `b = Δx_t − Δx₀`. The sideways shift is a quintic smoothstep in `y` across the whole piece (4 m over about 120 m gives under 0.2 g lateral at 31 m/s); keep `up` in the loop plane as the existing `Loop` branch of `sample()` does. `LoopRight` mirrors the shift sign through `mirrored`.
- **Top hat** (level in, level out, same height). Pull-up arc, crest arc, pull-out arc with equal level inserts at entry and exit; no vertical inserts, so the apex height is an outcome (about 35 m). `Δz = 0` by symmetry; `Δx` snapped by the two equal level inserts.
- **60° pieces.** Level-to-60° and its reverse: solve the 60° insert from `Δz_t = ceil(Δz₀ / 2) · 2` (it adds `0.866 a` of height), then the level insert for `Δx`. 45°-to-60° and its reverse: two inserts along 45° and 60° give an exact 2×2 linear system `(Δx, Δz) = (Δx₀, Δz₀) + a₁ (cos 45°, sin 45°) + a₂ (cos 60°, sin 60°)`; choose the smallest grid point beyond the core endpoint inside the cone of the two directions so both lengths are non-negative. Crest pairs the same way with the opposite sign of `z`.
- **Rolls.** Spline hearts already span connectors by construction. Corkscrew length `L` is scanned in 2 m steps so it is on the grid by construction; the half corkscrew rises `2r = 10 m = 5 units`.

After solving, overwrite the last node with the exact target so `sample(length).position` matches `exit.position()` to the 1e-8 the tests demand.

**When no solution exists.** Snapping always succeeds. The things that can fail are the rated window (empty), the model's minimum rail radius, and convergence of the speed iteration; all three are design-time assertions with the piece named, and the fixes are listed in 4a. There is no per-model fallback geometry: a piece either has a valid window on every model that allows it or the design is changed before it ships.

### 6. Force check

`crates/sim/tests/inversion_physics.rs` becomes the authority, and the two point-sampled WIP tests in `physics/tests.rs` are deleted (the solver's own train-aware estimate replaces them as a unit test). The check:

1. Build the catalog once. For every piece with `rated_speeds`, build a circuit with `circuit_with_elements` (station, level approach, the piece, BFS return path, as today).
2. For each train configuration in use (6 cars and 12 cars; the forces depend on the train, not the model), for each entry speed in `{minimum_entry, maximum_entry}`: `Train::new` at the piece start with `unpowered()` (no resistance, no powered sections), step until the last car is past the piece end plus the train length, cap at 100 000 steps.
3. Collect every `CarReading` whose distance lies inside the piece. Pass criteria, checked against every model that allows the piece: `min vertical ≥ limit.min`, `max vertical ≤ limit.max`, `max |lateral| ≤ limit.lateral`, `max |longitudinal| ≤ limit.longitudinal`; every car recorded at least once inside the piece; `train.notes()` empty (no rollback); and at `minimum_entry` the train's lowest speed inside the piece is at least 3 m/s.
4. Print one line per piece and speed with the extremes, so the numbers are visible in CI output.

Cost: about 40 pieces × 2 trains × 2 speeds × 3000 steps × 9 cars × 3 samples, roughly 4 million samples, a few seconds in debug. Keep it there by breaking out as soon as the train clears, by building the catalog once per test, and by not adding a third speed.

A second test, compositions as a player would build them, uses `CoasterRun` with `PhysicsConfig::default()` (resistance on) and the real lift or launch: Timber Tumbler 50 m lift into Loop; Velocity Viper launch into Loop; immelmann, dive loop, raven turn and sea serpent after a 50 m lift; 60° first drop into `SixtyDownToLevel`; a climb into a Corkscrew; Briar Blitz 38 m lift into SteelLoop and into TopHat. Assert `report.completed`, the per-sample extremes from `report.samples` within the model's limits, and the expected inversion count. This replaces `train_completes_laps_through_combined_half_inversions` and the lifted half of `ride_test_reports_whether_the_train_clears_the_top_hat`.

### 7. Integration

- `curves.rs`: `Shape` becomes a data-carrying enum, `Plain | AirtimeHill | Overbank | Profile(Arc<SolvedProfile>) | Roll(Arc<RollGeometry>)`, `Clone + Debug + PartialEq` (parameters compared, not polylines) with a `hash_into` that writes a tag byte plus `reversed` and `mirrored`; `arc_lengths` already hashes the geometry. Delete the `Loop`, `Corkscrew`, `Heartline`, `HalfLoop`, `HalfCorkscrew`, `TopHat`, `OffsetHalfRoll` and `BarrelDrop` variants and their branches in `raw()` and `sample()`. `raw()` dispatches: `Profile` to `profile::sample`, `Roll` to `roll::rail`, otherwise the Hermite path. `sample()` gains `heart`. `Piece` gains `rated_speeds: Option<RatedSpeeds>`. `Piece::hash_into` calls `shape.hash_into`.
- New `track/profile.rs` (solver, polyline, Hermite sampling; replaces `half_loop.rs`, whose three tests move here) and `track/roll.rs`. Solved profiles live in `static PROFILES: LazyLock<...>` keyed by `PieceId`, built from a spec table, so `catalog()` only clones an `Arc`. `catalog()` is called in loops everywhere; solving inside it is not acceptable.
- `catalog.rs`: profile and roll pieces come out of their own builder loops that set `exit` from the solved displacement instead of the hand-typed `(x, y, z)` tuples. Mirrors keep working through `mirrored`. `SteelLoop` becomes its own spec, not a clone of `Loop`.
- `models.rs`: the limit values above. `allows_chain` still requires `Shape::Plain`; `available_pieces` still filters on `maximum_curvature()`.
- `physics/mod.rs`: `rider_g` on `heart`. `demo_circuit` has a 20 m lift (five `GentleUp`) into `Loop`; at 20 m/s it is below the new loop's window and will roll back. Raise the lift to ten `GentleUp` (44 m with the two transitions, about 29 m/s at the bottom, inside the loop's window) with ten `GentleDown`; the circuit still closes because only heights change.
- `construction/mod.rs::same_geometry`: compare `first.shape == second.shape`.
- Supports, clearance, the renderer (`sim-wasm/src/lib.rs`, every 2 m) and `track_support_obstacles` all go through `sample(distance)` and `length()` and need no change. Run their test modules; the loop's self-crossing clearance (`clearance.rs` around line 892) is the one place a bigger loop could trip something.
- The reachability test (`every_model_piece_variant_has_a_reachable_entry_connector`) needs no change: connector states of the converted pieces are unchanged. Tests that assume exact sizes must be updated: `half_loop_has_grid_aligned_forward_displacement` (assert integer units instead of 16), the `peak_height − start_height > 25` and `35 m/s` entries in `top_hat_circuit.rs` (use the rated maximum), and anything in `player_circuits.rs`, `timber_tumbler_prefix_boundary.rs` or the construction tests that counts on the 60 m 60° pieces.
- `Heartlining` labels and piece names stay as they are. Semantically, "heartlined" now means the heart path is straight; every roll is designed at the heart either way.
- Wire names in the typed worker protocol are unchanged because `PieceId` is unchanged.

## Work items

Each item is independently testable and ends green on `make test`'s Rust part (`cargo test --workspace`, skipping the seeded walks, and the `--features threads` run). Run the targeted test modules while editing and the full gate at the end of each item.

1. **Heart-based rider forces.** Add `HEART_HEIGHT_METRES`, `TrackSample.heart` (`position + up * 1.2` for all shapes), and rewrite `rider_g` on `heart`. Tests in `physics/tests.rs`: a straight level piece reads (1, 0, 0) g; a level `TurnSmall` at speed `v` reads lateral `v² / (g · 16)` within 2 % (the heart offset is vertical, so the radius is unchanged); a climbing `GentleUp` reads longitudinal from the slope. `TurnSmallStandalone` is a spline and does not have the assumed 16 m radius at its midpoint. Existing force expectations for the old inversion geometries must be replaced as those pieces are converted; the first heart-based check exposes failures before that conversion.
2. **Data-carrying `Shape`.** Convert `Shape` to the new enum with the old variants still present as tags, add `hash_into` and `PartialEq`, update `same_geometry`, `allows_chain` and the hash. No behaviour change; the whole suite stays green. This isolates the mechanical refactor from the geometry work.
   Implementation decision: profile and roll payload variants wait until their concrete types exist in later work items. For now, `Shape` keeps its current fieldless variants, compares by variant, and hashes the same explicit tag bytes the old enum cast produced.
3. **Profile solver.** `track/profile.rs`: segment list, schedules, taper, worst-car speed iteration, inserts, snapping helpers, Hermite sampling, rated speeds. Unit tests: a single 90° arc at constant `G` and fixed speed reproduces the heart radius `v² / ((G − 1) g)` at mid-arc within 1 %; the three ported `half_loop.rs` tests (continuity, bit-for-bit repeatability, convergence with step count); snapping lands on the grid to 1e-9; the solver's own worst-car force along the final curve never exceeds `G_design + 1e-6`; a loop spec yields `Δz = 0` to 1e-9.
4. **Half loop end to end.** Replace `HalfLoopUp`/`HalfLoopDown` with a profile spec (31.3 m/s, 4.2 g, `G_top` 1.0), delete `half_loop.rs`, add `rated_speeds`, rewrite `trains_cross_half_loops_within_model_force_limits` as the generic force check of decision 6 (it will run over only two pieces at this point). Also delete the two WIP point-sampled tests. This is the milestone that proves the method; stop and report the solved numbers before going on.
5. **Loops.** `Loop`, `LoopRight`, `SteelLoop`, `SteelLoopRight` as profile specs with the sideways shift. Fix `demo_circuit`. Clearance and construction tests green. Force check covers them.
6. **Top hat.** Three-arc profile for Briar Blitz. Update `top_hat_circuit.rs` to the rated speeds. Min-radius assertion against 8.5 m.
7. **60° transitions.** Four profiles (pull pair at 34 m/s, crest pair at 22 m/s, and the two 15° pairs), eight pieces. Fix tests that assumed the old sizes. The `slope_elements` BFS in `construction/elements.rs` picks these up automatically.
8. **Roll module with spline hearts.** `track/roll.rs`; convert HeartlineRoll, RavenRoll (4), BarrelDrop (lengthened to 32 m by 8 m down), TwistedDrop, BankedDrop. Unit test: a heartline roll's heart acceleration from `rider_g` at 25 m/s is below 0.05 g everywhere except gravity, so vertical runs 1 → −1 → 1 and lateral stays within ±1.05 g. Mirror and join tests in `track/tests.rs` green. Force check covers them.
9. **Corkscrews.** Helix hearts, length scan, `Corkscrew`, `CorkscrewRight`, four half corkscrews. Force check green at 20 m/s, including the half corkscrew down rated by exit speed.
10. **Compositions and reachable windows.** The `CoasterRun` composition test and the per-model "lift reaches the window" test from decisions 3 and 6, replacing the old combined-half-inversion lap test.
11. **Overspeed note in the ride report.** In `TestRunRecorder`, when a car enters a piece with `rated_speeds` faster than `maximum_entry`, add one note naming the piece and both speeds ("The train enters the vertical loop at 33 m/s; it is designed for up to 31 m/s."). One test with a 60 m lift into the Loop. Gives the player the only feedback that makes narrow windows workable. Smallest item; do it last and leave it out if time runs short, saying so.
12. **Gate.** `make lint`, full `make test`, delete dead branches, update this plan's "expected outcomes" with the solved numbers, and write the handoff.

## Profile prototype findings

The standalone prototype in `crates/sim/src/track/profile.rs` stops before catalog integration. Run it with `rustc --test --edition 2021 crates/sim/src/track/profile.rs -o /tmp/inversion-profile-tests && /tmp/inversion-profile-tests --nocapture`. Three checks pass and two fail. It uses 2,048 integration steps, point-mass initialization, four train-aware iterations, and the actual `Train::new` spacings of 18/6 and 18/12 metres. The prototype does not implement snapping or a full train simulation, so these are design-equation measurements rather than end-to-end proof.

For the unsnapped half loop at 31.3 m/s and the specified 4.2 g / 1.0 g schedule, the fourth iteration ends at x = 32.7515198465 m, z = 46.4191850317 m, length = 92.8449902767 m; the endpoint moved 0.0002204 m in that iteration. The estimated maximum vertical force from the solver's curvature and worst-car speed is 4.1278695372 g. Since the exit is horizontal, the prescribed 4 m/s clearance calculation requires at least 30.4424770727 m/s, leaving a 0.8575229273 m/s window, below the required 2 m/s. The specified upward snapping insert can only increase the rise. These numbers also differ from the estimated 36–38 m rise and 10–12 m forward displacement above. The speed-window assertion remains failing.

For a full loop with the same schedule and four iterations, the prototype ends at x = 59.8518799788 m, z = 7.1199103789 m, length = 159.4863633237 m. Its required zero-rise assertion remains failing. The stated symmetric schedule does not by itself establish symmetric geometry under the train-height-dependent speed calculation.

The specified fixed-speed 90-degree arc test also needs a correction: at 45 degrees the stated curvature equation gives radius `v² / (g * (G - cos(45°)))`, not `v² / (g * (G - 1))`. For `G = 4.2`, the latter is about 9.2% larger. The prototype tests the equation at 45 degrees. Design changes and an actual full-train force check are still required before converting the half loop.
