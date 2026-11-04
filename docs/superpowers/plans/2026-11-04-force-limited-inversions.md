# Force-limited inversions and drops

> **For agentic workers:** Work items are in the last section, in order, each with its test. Do them with superpowers:subagent-driven-development or superpowers:executing-plans. The first milestone (the large half loop, work item 4) has numbers to check against in "Milestone check"; stop and report if the solver is more than 3 % away from them.

**Goal:** Every inversion, drop and 60° transition in the catalog keeps a full train inside its model's rider force limits across a stated entry speed band, lands exactly on grid connectors, and is produced by one general method instead of hand-tuned constants.

**Branch:** `inversion-forces`. Head `36f093e` contains, on top of the master merge: the heart-based `rider_g` and `TrackSample.heart` (work item 1 done), a `Shape` with `PartialEq` and explicit hash tags (work item 2 half done), a standalone profile prototype in `crates/sim/src/track/profile.rs`, and the second attempt's `vertical_loop.rs`, `vertical_top_hat.rs` and `half_loop.rs` with enlarged catalog sizes from `874af14` (Loop 112 m forward, half loop 34 m by 40 m, top hat 176 m). Sixteen tests in `physics/tests.rs` are red: fourteen panic in `demo_circuit` (`physics/mod.rs:146`; the demo's hand-listed return leg stopped closing when the Loop's forward displacement went from 64 m to 112 m), plus the two old-geometry force tests. Work item 0 fixes the demo; the force tests are replaced in work item 4.

**Owner decision pending:** Keep the existing Briar Blitz `TopHat` and its tests. Do not replace it with `Camelback`; exclude `TopHat` from force-limit assertions until the owner decides. This supersedes the camelback replacement instructions below.

**Crest decision pending:** The original `SixtyUpToLevel` has a measured force-compliant window of approximately 30.800–32.251 m/s, narrower than the planned 2 m/s minimum and outside the proposed 3–30 m/s scan. A profile redesign is present in the working tree, but the owner has not yet approved replacing the planned quintic geometry.

**Roll measurements:** With the specified seventh-order roll schedule and rail-distance physics, the full corkscrew's first passing 2 m axial increment is 86 m and the half corkscrew's is 64 m. The heartline roll needs a 284 m span to meet the 0.05 g longitudinal criterion at 25 m/s; the 248 m candidate measured 0.057 g. The catalog uses these precomputed spans. A test-only full-train scan recomputes the smallest passing 2 m increments, so normal catalog creation does not run the candidate scan.

## Revision after the half-loop milestone

The first version of this plan predicted a 36–38 m half loop with a 2 m/s window at 31.3 m/s. The prototype measured 46.4 m rise, 32.8 m forward, a 0.86 m/s window, and a full loop ending 7.1 m above its entry. A Python replica of the prototype's equations reproduces those numbers to 0.1 m, so the method is sound and the inputs were wrong. What changed:

1. **The force schedule, not the speed, made the loop tall.** The cosine blend from `G_max` at the entry to `G_top` at the top had already cut the force to 2.6 g by the vertical, so the climb was wide. Holding `G_max` until 120° and blending only over the last 60° gives 38.2 m at 31.3 m/s with the same peak. Real clothoid loops hold their entry force well past the vertical.
2. **Windows are set by the maximum train-mean height and are about 12 % of the rated speed.** The minimum entry speed leaves 6 m/s at that height. The worst-car speed used to design the curvature at the top is measured at a different train position, so it cannot be substituted into that calculation. With the corrected schedule the large half loop has a worst-car top speed of 15.8 m/s and a 3.3 m/s window. Lowering the top force target does **not** widen the window: a lower top force means a larger top radius, a taller loop and a slower top. The top target goes up from 1.0 g to 1.5 g, which is what real loops read and helps the model minimum radius.
3. **Loops come in sizes with entry-speed bands**, as the coordinator proposed, because a 12 % window cannot cover a model whose drop speeds span 24–34 m/s. Steel gets two loop sizes; Briar Blitz gets one. Half loops stay in one size (the large one) because immelmanns and dive loops are large-ride elements. The ride report flags under- and overspeed.
4. **Full loops close by construction:** the climbing half is solved with the descent assumed to be its mirror image, then mirrored. There is no apex taper and no closure assertion to fail.
5. **The vertical top hat is infeasible under peak limits at any speed** and becomes a 45° camelback hill (decision 4a). This is scale-invariant physics, not a tuning problem.
6. **Grid snapping of a half loop's height uses the hold angle, not a vertical insert**, because an insert can only add height and would eat the window.
7. **Crest geometry is conditional on a valid rated window.** The 15° crest pieces retain their quintic geometry because simulation finds valid windows. The 60° crest pair did not have a traversable, force-compliant window from 3 to 30 m/s, so `SixtyUpToLevel` and its mirrored `LevelToSixtyDown` use a bounded negative-g profile.

## Why the two earlier attempts failed

Three separate errors stacked up.

1. **Shapes were sums of a spline and a circle.** `Piece::raw` in `curves.rs` built a quintic Hermite from entry to exit and added a circular offset for `Loop`, `Corkscrew` and `Heartline`. Curvature was not controlled anywhere, so the master half loop reached 19.3 g. The second attempt's `vertical_loop.rs` and `half_loop.rs` integrate constant-force curves but hit their grid targets with hand-found constants.
2. **Forces were designed for a point mass but measured on a train.** The physics gives the whole train one speed from the mean slope of all cars (`Train::gravity_acceleration`). When the front car is at the top of a 32 m half loop the train's mean height is 10–15 m lower, so the front car is 15–25 % faster than a point mass. The WIP half loop measured 5.68 g with a train while its point-sampled test passed at 4.4 g.
3. **Forces were computed on the rail, not at the rider.** A heartline roll is defined by the rider's heart staying still while the rail orbits it, so on the rail it read as a 1 m helix: 4 g lateral at 32 m/s. The 16 m barrel-roll drop read tens of g and the half corkscrews 2.9 g lateral. Fixed on the branch by `TrackSample.heart` and the heart-based `rider_g`.

## Decisions

### 1. Heart height and where forces are measured

Done on the branch: `HEART_HEIGHT_METRES = 1.2` in `curves.rs`, `TrackSample.heart` (`position + up * 1.2` for existing shapes, the designed heart path for roll shapes once they exist), and `rider_g` as a second difference of `heart` over 0.02 m with one-sided stencils at the piece ends. Keep it.

### 2. Force limits per model

Limits are **peak** values over all cars and time steps of the heart reading with the full train simulated. Sustained load is the ride report's business (8 m above 3 g note, nausea above 3.5 g per sample); leave it alone. Every real loop produces the 8 m note.

| Model | Inspiration | Vertical max | Vertical min | Lateral | Longitudinal |
|---|---|---|---|---|---|
| Briar Blitz | GCI wooden twisters (Wodan, Thunderhead, Mystic Timbers) | 4.0 | −1.0 | 1.5 | 1.5 |
| Timber Tumbler | RMC I-Box hybrids (Steel Vengeance, Iron Gwazi, Twisted Timbers) | 4.5 | −1.5 | 1.5 | 1.5 |
| Velocity Viper | Intamin and Mack multi-launch (Velocicoaster, Taron, Maverick) | 5.0 | −1.5 | 1.5 | 1.5 |

Timber Tumbler drops from 5.0 to 4.5 (RMC rides peak near 4.2 g); longitudinal rises to 1.5 on all three because the physics reports `g (t_car − t̄_train)` on a vertical section, which reaches 0.9 g for the front car with no design lever against it. Design targets are the limit minus 0.3 g on both signs: 4.2 g / −1.2 g for steel, 3.7 g / −0.7 g for wood. A piece has one geometry, designed for the most restrictive model that allows it.

### 3. Entry speed bands

Speeds at the bottom of a drop from a lift of height `h` are about `0.97 · sqrt(2 g h)` with the train's length accounted for. Briar Blitz lifts 25–40 m give 21–27 m/s; Timber Tumbler lifts 30–63 m give 24–34 m/s; Velocity Viper launches at 31.3 m/s and reaches 34 m/s after a drop.

Each profile or roll piece carries `RatedSpeeds { minimum_entry, maximum_entry }` on `Piece`. The maximum is a design input; the minimum is solved: the entry speed at which the train-mean speed at the slowest point of the piece is 6 m/s (`minimum_entry² = 2 g · max_s(z̄(s) − z_entry) + 36`). Six, not four, so the top of a loop at the slowest rated speed reads about −0.6 g rather than −0.9 g. Descending twins are rated by exit speed; their entry maximum is `sqrt(maximum² − 2 g Δz)` and their entry minimum 2 m/s.

The loop family, with numbers from the replica of the prototype (the Rust solver should land within 3 %; see "Milestone check"):

| Piece (ids) | Model(s) | Rated max | Band | Rise | Exit ahead of entry | Rail length | Lifts that hit the band |
|---|---|---|---|---|---|---|---|
| `Loop`, `LoopRight` | Timber Tumbler, Velocity Viper | 27.5 | 24.7–27.5 | 30.5 m | 53 m | 128 m | 32–40 m |
| `LargeLoop`, `LargeLoopRight` (new) | Timber Tumbler, Velocity Viper | 31.3 | 27.6–31.3 | 37.9 m | 69 m | 161 m | 40–51 m, or the launch |
| `SteelLoop`, `SteelLoopRight` | Briar Blitz (3.7 g / 1.0 g top) | 27 | 25.2–27 | 31.8 m | 63 m | 137 m | 34–38 m |
| `HalfLoopUp` / `HalfLoopDown` | Timber Tumbler, Velocity Viper | 31.3 (down: at exit) | 28.0–31.3 | 38.2 m | 27.9 m | 87 m | 41–51 m, or the launch |

Lift pieces step 4 m, which is 0.6–0.8 m/s at these speeds, so every band contains at least two lift heights. Timber Tumbler's 63 m lift (34 m/s) overspeeds the large loop by 9 % and reads about 5.0 g; the report says so (work item 11). A third size for 34 m/s is one more spec row if the owner wants it.

The rest:

| Element | Rated max | Design force | Reasoning |
|---|---|---|---|
| `Camelback` (replaces `TopHat`; Briar Blitz) | 27 | 3.7 g pull-up to 45°, −0.7 g over the crest | See 4a; about 31 m tall, 118 m span, band 25.0–27 |
| 60° pull pair (`LevelToSixtyUp`, `SixtyDownToLevel`) and 15° pull pair (`SteepToSixtyUp`, `SixtyToSteepDown`) | 34 | 4.2 g | Bottom of the largest Timber Tumbler drop; the 60° pull-up is about 56 m forward and 27 m up with a 31 m minimum rail radius |
| 60° crest pair (`SixtyUpToLevel`, `LevelToSixtyDown`) and 15° crest pair (`SixtyToSteepUp`, `SteepToSixtyDown`) | solved by simulation scan | keep today's quintic geometry | A descending crest cannot be designed from a force target (4a); the rated maximum is the largest entry speed, scanned down from 30 m/s in 1 m/s steps, at which the full-train check passes with −1.2 g; expect a lift-top speed. Trivially fine at 3 m/s |
| `Corkscrew`, `CorkscrewRight` | 20 | 4.2 g | Classic corkscrews run at 13–18 m/s |
| `HalfCorkscrew` (4) | 22 (down: at exit) | 4.2 g | The large half loop tops out at 15.8 m/s; a 10 m descent from there exits at 21.2 m/s |
| `HeartlineRoll(Right)` | 30 | gravity only | Heart forces do not depend on speed; the cap keeps a roll above 1 s |
| `RavenRoll` (4) | 20 | gravity plus the S-curve | 10–16 m/s in practice, at the top of a half loop |
| `BarrelDrop`, `TwistedDrop`, `BankedDrop` (6) | 15 | gravity plus the drop curvature | Lift-top pieces |

**How tests get the numbers.** The force test reads `piece.rated_speeds` and runs the full train at `minimum_entry` and `maximum_entry`. A gameplay test builds, per model, a lift at the low and high end of each loop's "lifts that hit the band" column plus level track, measures the speed there, and asserts it falls inside the band; Velocity Viper does the same off the launch for the large loop and the half loop.

### 4. Curve families

Planar **profiles** cover loops, half loops, the camelback, the four 60° pull pieces (`LevelToSixtyUp`, `SixtyDownToLevel`, `SteepToSixtyUp`, `SixtyToSteepDown`), and the 60° crest pair `SixtyUpToLevel` / `LevelToSixtyDown`. The 15° crest pair keeps its quintic shape because it has a valid rated window. **Rolls** cover heartline and raven rolls, the three drop kinds, corkscrews and half corkscrews. Both are designed on the heart path; the rail is derived from it.

#### 4a. Planar profiles

Coordinates: `x` forward along the entry heading, `z` up, pitch angle `θ` from +x toward +z, rider up `(−sin θ, cos θ)`. Signed curvature `κ > 0` turns toward the rider's up. Vertical rider force at the heart is `G = κ_h v² / g + cos θ`, so

```
κ_h(θ) = g · (G(θ) − cos θ) · w(θ) / v_w(θ)²
```

with `w` the end taper and `v_w` the worst-car speed. Integrate the heart in `θ` (2048 midpoint steps, smoothstep-parametrised as in the prototype), derive each rail node as `heart − 1.2 · up(θ)` with rail curvature `κ_h / (1 + 1.2 κ_h)` and rail arc element `ds_h (1 + 1.2 κ_h)`, and sample with the quintic-per-segment Hermite already in `profile.rs`. The prototype's `integrate` does all of this; keep it and add schedules, inserts, the mirror extension and snapping.

Force schedules `G(θ)`:

- **Pull-up or pull-out** (0°→90°, 0°→60°, 45°→60° and reverses): constant `G_design`.
- **Loop family** (0°→180°): `G(θ) = G_max` for `θ ≤ θ_hold`, then `G_top + (G_max − G_top)(1 + cos(π (θ − θ_hold) / (π − θ_hold))) / 2` to the top. `G_max = 4.2` (3.7 on Briar), `G_top = 1.5` (1.0 on Briar, to keep the top rail radius above 8.5 m), `θ_hold ≈ 120°` (the snapping knob, decision 5).
- **No crest may be entered steeply.** On a crest the force is `cos θ − |κ| v² / g`; near the vertical `cos θ ≈ 0`, so any curvature there is pure negative g, and holding even −1.2 g from the vertical to the apex needs a rail radius of `v² / g` at the vertical. The replica shows a 4.2 g pull-up from 31.3 m/s rising 33 m of the 50 m energy height with the crest then needing 25 m more; with a 4.7 g pull-up and a −1.5 g crest it still does not clear. Real top hats take brief −2 g hangs at the vertical-to-crest transitions, which peak limits forbid. Hence the **camelback**: pull-up to 45° at `G_design`, crest arc 45°→−45° at constant `−0.7 g` (Briar) and pull-out. At 27 m/s it is 31 m tall with a 2.0 m/s band. The solver must iterate to convergence here (up to 8 passes; near-stall designs move about 1 m per pass early on).
- **Crest ratings follow the force scan.** A constant negative-g target across a long 60° crest produced a 52 m rise at 34 m/s, leaving no speed window below 30 m/s. A 0.1 rad end taper and a −1.45 g target at 30 m/s produce a 24 m snapped rise and a 25.437–30 m/s rated band for `SixtyUpToLevel`. `LevelToSixtyDown` uses the mirrored profile and rates 3–19 m/s after simulation; 20–30 m/s fail because downhill acceleration pushes forces past model limits. Its rated test uses a 32 m level approach so the train enters with its full length on level track. The 15° quintic crest windows remain `SixtyToSteepUp` 27–30 m/s and `SteepToSixtyDown` 3–20 m/s. Every band passes full-train checks at both endpoints for six- and twelve-car trains on both models that allow the piece. The camelback's crest remains symmetric about the apex and only ±45° wide; it is solved as a climbing half with the mirror extension and reflected, like a loop.

End taper: `w = min(1, (Δθ_start / θ_taper)^(2/3), (Δθ_end / θ_taper)^(2/3))`, `θ_taper = 0.3 rad`, or a quarter of the arc for the 15° pieces. The 2/3 power keeps `∫ dθ / κ` finite. The climbing half of a **full loop has no end taper** (its apex is not a connector); the half loop has both.

**Worst-car speed.** When car `i` (0 is the front) is at rail distance `s`: `v_i(s)² = v_max² − 2 g (z̄_i(s) − z_entry)`, `z̄_i` the mean rail height over the cars at `s + (i − j) Δ`. Design speed `v_w = max_i v_i` over both train layouts (6 and 12 cars, 18 m). Beyond the ends `z_rail` continues along the connector tangent, **except for a full loop's climbing half, where the extension beyond the apex is the mirror image of the curve solved so far**. Iterate from the point-mass curve until the endpoint moves under 1 mm (4 passes for loops, up to 8 for the camelback); assert it.

**Full loop assembly.** Solve the climbing half with the mirror extension and no apex taper; the descent is the reflection about the vertical line through the apex. Curvature is continuous at the apex because `dG/dθ = 0` and `dv/dθ = 0` there. The exit is level at entry height by construction, `2 x_apex` ahead of the entry (53 m standard, 69 m large), with the sideways shift of decision 5 so the crossing tracks clear each other.

**Rated speeds** from the solved curve as in decision 3. Design-time assertions, each a panic in the `LazyLock` initialiser naming the piece: window at least 1.5 m/s for loops and 2 m/s for everything else; rail minimum radius at least the most restrictive allowing model's `minimum_radius_metres + 0.5`; convergence. Cures: raise `θ_hold` (shorter loop, wider window, smaller top radius), raise `G_top`, or lower the rated maximum.

#### 4b. Rolls

Unchanged from the first version. A roll piece is a heart path `H(t)` and a roll schedule `φ(t) = φ_entry + (φ_exit − φ_entry) · S7(t)`, `S7 = 35t⁴ − 84t⁵ + 70t⁶ − 20t⁷`; rail `= H − 1.2 · up_h`, `up_h = base_up cos φ + left sin φ`; `TrackSample.up` is `up_h` re-orthogonalised against the rail tangent; `heart = H`. Heart endpoints are the rail connectors plus `1.2 · up(bank)`. Rail derivatives by central differences in `t` with `δ = 1e-5`.

- **Spline hearts** (the existing quintic Hermite between heart endpoints): HeartlineRoll (`0 → ±2π`), RavenRoll (8 m lateral shift, `0 → ±π`), BarrelDrop (lengthened to 32 m by 8 m down, `0 → ±2π`), TwistedDrop (`±π/4 → 0`), BankedDrop (constant `±π/4`). Checked by simulation only.
- **Helix hearts** for corkscrews: axis along forward `f` at `r = 5 m` above the entry heart, `H(t) = A + f L t + r (σ l sin φ − u cos φ)`, `up_h = u cos φ − σ l sin φ`. Full corkscrew `0 → 2π`; half `0 → π`. Grid-compatible half corkscrews rise 12 m at the rail, which gives a 9.6 m heart rise and 4.8 m heart radius. The test-only full-train scan selects 86 m for the full corkscrew at 20 m/s and 64 m for the half at its rated maximum. The catalog stores those spans directly so normal catalog creation does not run the scan.

#### 4c. Speed loss with height

Both families use `v² = v_max² − 2 g (z̄ − z_entry)` on rail heights, no resistance (conservative). Descending pieces are the time reversal of their climbing twin: `HalfLoopDown` samples the `HalfLoopUp` profile reversed; the 60° pairs share one profile each; half corkscrew down mirrors half corkscrew up.

### 5. Grid snapping

Only connectors must sit on the grid (2 m units in `x`, `y`, `z`; discrete heading, slope, bank). Apex heights and mid-piece extents are free. Diagonal headings are unreachable (no 45° turn piece; `Heading::NorthEast` is used only by terrain), so `Piece::at` is unchanged.

- **Half loop up.** Height first: bisect `θ_hold` in [100°, 140°] so the rise lands on the **nearest** grid height (38.2 m → 38 m; a larger `θ_hold` is shorter). The height adjustment changes the forward displacement to about 28.053 m. Then the exit insert (pointing back along the exit heading) takes that displacement **down** to 28 m on the grid. Both adjustments are one-dimensional; no vertical insert is needed, so the window is preserved. If `θ_hold` leaves the range, fall back to a vertical insert at 90° and say so in the test output.
- **Full loops.** `Δz = 0` by construction; the exit insert takes `2 x_apex` **up** to the grid. The sideways shift stays `y = 2` units (4 m), applied as a quintic smoothstep in `y` across the whole piece, because the teardrop's climb and descent cross each other (the climb reaches `x ≈ 33 m` at mid-height while the descent passes `x ≈ 20 m` there). Lateral force from the shift is under 0.2 g. `LoopRight` and `LargeLoopRight` mirror the shift through `mirrored`.
- **Camelback.** `Δz = 0` by symmetry (solve the climbing half with the mirror extension and reflect, as for loops); two equal level inserts take the span up to the grid.
- **60° pull pieces.** Level-to-60° and reverse: the 60° insert from `Δz_t = ceil(Δz₀ / 2) · 2` (it adds `0.866 a` of height and `0.5 a` forward), then the level insert for `Δx`. 45°-to-60° and reverse: two inserts along 45° and 60° give an exact 2×2 linear system; pick the smallest grid point in the cone beyond the core endpoint so both lengths are non-negative.
- **Rolls.** Spline hearts span connectors by construction; the test-only corkscrew scan checks lengths in 2 m steps; the half corkscrew rises 12 m (6 grid units) at the rail while its heart rises 9.6 m.

Overwrite the last node with the exact target so `sample(length).position` matches `exit.position()` to 1e-8.

**When no solution exists.** Snapping always succeeds. The design-time assertions of 4a (window, minimum radius, convergence) are the only failures, each with the piece named and the cure listed. No per-model fallback geometry.

### 6. Force check

`crates/sim/tests/inversion_physics.rs` is the authority; the point-sampled tests `inversion_and_drop_forces_remain_within_model_comfort_limits` and `half_loop_forces_across_entry_speeds` in `physics/tests.rs` are deleted and the solver's own train-aware estimate becomes a unit test in `profile.rs`.

1. Build the catalog once. For every piece with `rated_speeds`, build a circuit with `circuit_with_elements` (station, level approach, the piece, BFS return path).
2. For each train layout (6 and 12 cars) and each entry speed in `{minimum_entry, maximum_entry}`: `Train::new` at the piece start with `unpowered()`, step until the last car is past the piece end plus the train length, cap 100 000 steps.
3. Collect every `CarReading` inside the piece. Pass, against every model that allows the piece: vertical within `[min, max]`, `|lateral| ≤ lateral`, `|longitudinal| ≤ longitudinal`; every car recorded inside the piece; `train.notes()` empty; at `minimum_entry` the train's lowest speed inside an ascending piece at least 5 m/s. Descending pieces are rated from 2 m/s at entry, so that 5 m/s assertion does not apply to them.
4. Print one line per piece and speed with the extremes.

Cost is a few seconds in debug (about 4 million samples); keep it there by breaking out when the train clears and by not adding a third speed.

A composition test uses `CoasterRun` with `PhysicsConfig::default()` and real lifts or launches: Timber Tumbler 36 m lift into `Loop`, 46 m lift into `LargeLoop`; Velocity Viper launch into `LargeLoop`; immelmann, dive loop, raven turn and sea serpent after a 46 m lift; 60° first drop into `SixtyDownToLevel`; a climb into a `Corkscrew`; Briar Blitz 36 m lift into `SteelLoop` and into `Camelback`. Assert `report.completed`, extremes from `report.samples` within the model's limits, and the expected inversion count.

### 7. Integration

- `curves.rs`: `Shape` becomes `Plain | AirtimeHill | Overbank | Profile(Arc<SolvedProfile>) | Roll(Arc<RollGeometry>)`, `Clone + Debug + PartialEq` (parameters compared) with a `hash_into` writing a tag byte plus `reversed` and `mirrored`. Delete `Loop`, `Corkscrew`, `Heartline`, `HalfLoop`, `HalfCorkscrew`, `TopHat`, `OffsetHalfRoll`, `BarrelDrop` and their branches in `raw()` and `sample()`; `raw()` dispatches to `profile::sample`, `roll::rail`, or the Hermite path. `Piece` gains `rated_speeds`.
- `profile.rs` grows from the prototype into the solver (schedules, inserts, mirror extension, snapping, rated speeds) and keeps its tests. `roll.rs` is new. Solved profiles live in a `LazyLock` keyed by `PieceId`; `catalog()` clones an `Arc`. Delete `half_loop.rs`, `vertical_loop.rs` and `vertical_top_hat.rs`. The supports change from `874af14` (the same-piece clearance check) is independent and stays.
- `catalog.rs`: profile and roll pieces come from builder loops that set `exit` from the solved displacement. New ids `LargeLoop`, `LargeLoopRight`, `Camelback`; `TopHat` is removed. Wire names in `crates/sim-wasm/src/construction.rs` (the exhaustive `PieceId` match near line 1114) gain the three and lose `TopHat`; grep `web/` for the string `TopHat` as well. `SteelLoop` becomes its own spec.
- `models.rs`: the limits above; `LargeLoop`/`LargeLoopRight` join both steel allowed lists; `Camelback` replaces `TopHat` on Briar Blitz.
- `physics/mod.rs::demo_circuit`: the return leg becomes a search (work item 0), and the lift uses four `GentleUp` pieces plus its two transitions (40 m total, about 27 m/s) for the standard loop.
- `construction/mod.rs::same_geometry` compares `first.shape == second.shape`. `construction/elements.rs` keeps its element names; the immelmann, dive loop, raven turn and sea serpent use the half loops as today.
- Supports, clearance, the renderer and `track_support_obstacles` go through `sample` and `length` and need no change; rerun their tests. `connected_loop_still_checks_track_far_from_the_join` in `clearance.rs` sets `exit = entry` artificially and still holds. The sideways-shift test in `track/tests.rs` (around line 1256, `displacement.dot(sideways) >= 4.0`) still holds for `Loop`; extend it to `LargeLoop`.
- Reachability (`every_model_piece_variant_has_a_reachable_entry_connector`) needs no change: connector states are unchanged and the new ids reuse level connectors. Tests that assume sizes must change: `half_loop_has_grid_aligned_forward_displacement` (assert integer units), everything in `top_hat_circuit.rs` (renamed to the camelback, Briar 36 m lift, rated speeds), and anything in `player_circuits.rs`, `timber_tumbler_prefix_boundary.rs` or the construction tests that counts on the 60 m 60° pull pieces or the loop's 64 m / 112 m forward displacement.
- Speed-band note in the ride report (work item 11): `TestRunRecorder` adds one note when a car enters a rated piece faster than `maximum_entry` or slower than `minimum_entry`, naming the piece and both speeds.

## Milestone check (work item 4, large half loop)

Inputs: 31.3 m/s, `G_max = 4.2`, `G_top = 1.5`, `θ_hold = 120°`, taper 0.3 rad at both ends, both train layouts, level extension beyond both ends, 2048 steps, iterate to under 1 mm.

| Quantity | Replica value | Tolerance |
|---|---|---|
| Rise (unsnapped) | 38.2 m | ±3 % |
| Forward displacement (unsnapped) | 27.9 m | ±3 % |
| Rail length | 87.1 m | ±3 % |
| Solver's own peak vertical estimate | 4.20 g | ±0.02 |
| Minimum rail radius | 11.4 m | ±5 % |
| Speed at the top (worst car) | 15.8 m/s | ±3 % |
| Minimum entry (6 m/s at the top) | 28.0 m/s | ±0.3 |
| Snapped exit | 38 m up, 28 m ahead | exact |

**Measured snapping correction:** The height-first procedure above gives 28.053 m forward before the exit insert, so rounding down to the nearest grid point gives 28 m, not the original milestone's 26 m. The Rust solver produces a 38 m rise and a 28 m forward exit; the owner accepted this correction.

**Measured full-train check:** With matching profile and catalog connectors, the ascending half loop peaks at 3.953 g with six cars and 3.960 g with twelve cars at 31.3 m/s, below the predicted 4.2–4.35 g range. At the solved 27.956 m/s minimum entry both layouts clear at 6.000 m/s. The descending half passes both rated endpoints, peaking at 3.958 g. The reason for the gap between the solver estimate and full-train maximum has not yet been established.

Then the full-train test at 31.3 m/s must read at most 4.5 g vertical (expect 4.2–4.35; the margin absorbs rail-frame projection and interpolation), at least −1.5 g, lateral under 0.3 g; at 28.0 m/s the train clears with at least 5 m/s and no rollback. Same procedure for the loops in work item 5: standard loop rise 30.5 m, apex 26.5 m ahead, rail 128 m, minimum entry 24.7 m/s; large loop rise 37.9 m, apex 34.5 m ahead, rail 161 m, minimum entry 27.6 m/s; Briar steel loop rise 31.8 m, apex 31.3 m ahead, rail 137 m, minimum entry 25.2 m/s, minimum rail radius 9.1 m. For reference, the same replica with the first plan's schedule gives the prototype's 46.4 m and 32.8 m, so a solver that reproduces one set reproduces the other.

## Measured 60° crest limits

The original `SixtyUpToLevel` quintic rises 40 m. In a full-train scan on a closed circuit, neither the six-car nor the twelve-car 18 m train can clear it at 30 m/s. Bisection using both train layouts, a minimum 5 m/s speed while a car is on the crest, full-train clearance, vertical force between −1.2 and 4.2 g, and lateral and longitudinal limits of 1.5 g gives a passing entry-speed interval of approximately 30.7997–32.2508 m/s. Its width is 1.4511 m/s, below the planned 2 m/s window; the upper limit is longitudinal force. The originally proposed scan down from 30 m/s therefore cannot rate this geometry. These measurements were made against the original crest in commit `f6da810`, before the concurrent crest redesign.

For the original geometry, integer full-train scans with the same force and clearance checks found 2–9 m/s for `LevelToSixtyDown`, 27–30 m/s for `SixtyToSteepUp`, and 2–20 m/s for `SteepToSixtyDown`, with the ascending 15° crest scan capped at 30 m/s. The 15° descending pull reaches 1.510 g longitudinal at its energy-derived 28.334 m/s entry with twelve cars, so its catalog maximum is 28 m/s and needs the accompanying full-train endpoint check.

## Measured half-corkscrew grid correction

A 5 m heart-path radius rises 10 m through a half roll, but the rail-to-heart offset flips from 1.2 m above the rail to 1.2 m below it. The rail would therefore rise 12.4 m, which cannot land on the 2 m height grid. The owner accepted a 12 m rail rise, giving a 9.6 m heart rise and a 4.8 m heart radius for cardinal placement. Diagonal placements derive their dimensions from their rotated grid connectors. The descending rating uses an energy-derived maximum entry speed for a 22 m/s exit after a 12 m rail drop; full-train tests also check clearance speed.

The full-train scan selects 86 m for the full corkscrew and 64 m for the half corkscrew. Running the scan through catalog construction took 35.16 seconds in the measured workspace run, so the catalog now stores these spans directly and the scan is compiled only for tests. A model force-limit change requires rerunning the test-only scan. The scan rebuilds a circuit for each candidate span because the connector changes.

## Work items

Each item ends green on the Rust part of `make test` (`cargo test --workspace` skipping the seeded walks, plus the `--features threads` run). Run targeted modules while editing, the full gate at the end of each item.

0. **Demo circuit closes by search.** Replace the demo's hand-listed return leg (`OffsetRight, TurnLargeEntry, TurnLargeExit, Straight, ...`) with a breadth-first search from the loop exit back to the start, as `circuit_with_elements` in `inversion_physics.rs` does. The solved standard loop exits at an odd grid coordinate, so include `OffsetLeft`, `OffsetRight`, both small turns, both large turns, and `Straight` in the search; the small-turn-only graph cannot correct that parity. The fourteen tests panicking at `physics/mod.rs:146` go green and stay green when the loop changes size again. Keep `demo_has_track_crossing_with_vertical_clearance` passing (if the found path no longer crosses the demo path, add the crossing straight explicitly before the search).
1. **Heart-based rider forces.** Done (`36f093e`). Verify its four new tests still pass after item 0.
2. **`Shape` with payload variants.** Add `Profile(Arc<SolvedProfile>)` and `Roll(Arc<RollGeometry>)` now (placeholder structs are fine), with `PartialEq` and `hash_into`, so later items do not touch the enum again.
3. **Profile solver.** Extend `profile.rs`: hold-then-blend loop schedule, crest blend, inserts, mirror extension, `θ_hold` and insert snapping, rated speeds, assertions. Unit tests: the fixed-speed arc matches `κ = g (G − cos θ) / v²` at 45° within 1 % (already there, corrected); continuity, repeatability and convergence (already there); snapping lands on the grid to 1e-9; the solver's own worst-car force never exceeds `G_design + 1e-6`; a mirrored full loop closes to 1e-9; the large half loop matches the milestone table.
4. **Large half loop end to end.** `HalfLoopUp`/`HalfLoopDown` from the solver, `rated_speeds`, delete `half_loop.rs`, rewrite `trains_cross_half_loops_within_model_force_limits` as the generic force check, delete the two point-sampled tests. Report the solver's numbers against the milestone table before continuing.
5. **Loops.** `Loop`, `LoopRight`, new `LargeLoop`, `LargeLoopRight`, `SteelLoop`, `SteelLoopRight`; delete `vertical_loop.rs`; wire names; set the demo lift to four `GentleUp` pieces plus both transitions for 40 m; clearance and construction tests green; the sideways-shift test extended.
6. **Camelback.** Replace `TopHat` with `Camelback` (id, wire name, Briar allowed list, construction, `top_hat_circuit.rs` renamed and rewritten around the rated band); delete `vertical_top_hat.rs`.
7. **60° pull pieces and crests.** Two pull profiles (60° at 34 m/s: about 56 m forward, 27 m up, 31 m minimum rail radius; 15° at 34 m/s: about 19 m forward, 2.5 m up before inserts), four pull pieces. Simulate ratings for all four crest pieces. Keep the 15° quintic pair because it has a valid window; redesign the 60° pair as the bounded negative-g profile described above. Fix tests that assumed the old sizes.
8. **Roll module with spline hearts.** HeartlineRoll, RavenRoll (4), BarrelDrop (32 m by 8 m down), TwistedDrop, BankedDrop. Unit test: a heartline roll at 25 m/s reads vertical from +1 to −1 and back and lateral within ±1.05 g, with no other acceleration above 0.05 g.
9. **Corkscrews.** Helix hearts, length scan, six pieces at the 22 m/s half-corkscrew rating and 20 m/s full-corkscrew rating.
10. **Compositions and reachable bands.** The `CoasterRun` composition test and the per-model lift-reaches-the-band test, replacing the old combined-half-inversion lap test.
11. **Speed-band note in the ride report.** One test with a 60 m lift into `LargeLoop` (overspeed) and one with a 30 m lift into `Loop` (underspeed: the train stalls and the note names the band).
12. **Gate.** `make lint`, full `make test`, delete dead branches, replace the replica values in this plan with the solver's, write the handoff.

## Audit and revised plan (2026-11-04 evening)

Read-only audit of branch `inversion-forces` at `1447278`. Every number below was measured this session on this machine (native release builds through `tools/run-in-budget.sh`) unless it says "replica", which means a Python copy of the roll equations run in the scratchpad. The sections answer, in order: why the rolls came out several times too long, where construction time goes, whether the top hat join test should get a tolerance, and what remains, as ordered tasks.

### What is sound and stays

The heart-based `rider_g`, the `Shape` payload variants, the profile solver with its loop, half-loop, 60° pull and 60° crest profiles, the rated-window idea with constants verified by full-train endpoint tests, the composition tests, the speed-band note in the ride report (`physics/testing.rs:417`), and the requirement that a support column must not pass through its own inversion's track. The loop family's dimensions are realistic (30–38 m tall), the crest ratings are measured, and `inversion_physics` (14), `rated_element_physics` (4), `inversion_compositions` (3), `physics::tests` (39) and `track::roll` (11) all pass in release.

What the branch gets wrong is confined to the roll family (sizes, schedule, criterion), to performance (support self-clearance, profile solving at first use, catalog rebuilt per call), and to tests that still assume old sizes. Current red tests in release, which the implementer's report undercounted: `track::tests::joined_pieces_have_matching_position_tangent_roll_and_curvature` (top hat exit curvature), `player_circuits_close` (the Timber Tumbler circuit in `web/e2e/player/circuits.txt` no longer closes because the 15° and 60° pieces changed size; the journeys use the same file), `timber_tumbler_prefix_boundary::station_backed_prefix_refusals...` (the 86 m corkscrew leaves the world), and `top_hat_circuit::wooden_twister_completes...` (the test expects the 128 m top hat; the catalog has the 176 m one from `874af14`, exit 280 instead of 256). The worktree also carries an uncommitted edit to `web/e2e/world.spec.ts` (the pick point moved from `y: 250.5` to `250.65` to dodge a demo coaster support); it is not part of this audit and should be folded into the browser-baseline task.

### 1. Element dimensions: why the rolls are three to ten times too long

**Cause A, the heartline roll's 284 m: the criterion measures the wrong thing.** `roll.rs::heartline_force_peaks` projects the heart force onto `sample.tangent`, which is the *rail* tangent. In a heartline roll the rail spirals around the straight heart path, so the rail tangent pitches up and down by `atan(1.2 φ' / L)`, and gravity projected onto that tilt reads as "longitudinal" force: about `1.2 · φ'(t) · sin φ / L`, which falls off as 1/L. The heart's own longitudinal acceleration (the thing a heartline designer cares about) falls off as 1/L³ and is already negligible at a third of the span. The replica reproduces the implementer's numbers exactly:

| span | longitudinal along the rail tangent | heart-path acceleration along the roll axis | peak roll rate (S7 schedule, 25 m/s) |
|---|---|---|---|
| 48 m | 0.175 g | 0.334 g | 410°/s |
| 64 m | 0.110 g | 0.153 g | 308°/s |
| 96 m | 0.111 g | 0.049 g | 205°/s |
| 128 m | 0.096 g | 0.026 g | 154°/s |
| 248 m | 0.057 g (the implementer measured 0.057) | 0.001 g | 79°/s |
| 284 m | 0.0498 g (the implementer measured ≤ 0.05) | 0.001 g | 69°/s |

So the 0.05 g threshold was chasing gravity on a pitched rail. That reading is physically real (the car body does pitch with the rail), it is also what `rider_g` reports in the game, and it is bounded by the 1.5 g model limit, not by 0.05 g. Nobody designs a roll to it. Real designers constrain the roll *rate* (peak roughly 150–200°/s for modern heartline and zero-g rolls, slower on long high-speed rolls) and the roll *acceleration*, and then check forces. The arc-length table is not a contributor: `heart_acceleration_converges_as_arc_table_resolution_increases` holds, and the replica gets the same numbers from a 4000-sample table.

**Cause B, both corkscrews and the heartline roll: the S7 schedule concentrates the roll.** `smooth7` (zero first, second and third derivatives at both ends) has a peak rate of 2.19 times the mean and a peak roll acceleration of about `7.5 · 2π / T²`. On a helix the rider feels the roll acceleration as a tangential term `r · α` in the *lateral* direction, on top of gravity's full 1 g lateral at 90° and 270° of roll. With the 1.5 g lateral limit, that leaves 0.5 g for `r · α`, and the span scan grew the corkscrew until the S7 peak fitted: 86 m. The measured 1.48 g lateral at 86 m is exactly that limit binding. A rate profile that ramps up, holds, and ramps down (constant rate in the middle, where the lateral gravity peaks, and roll acceleration only near the ends, where gravity's lateral part is small) removes the binding constraint and has a peak rate of only 1.33 times the mean.

**Cause C, corkscrews: the rated speed and radius are a modern corkscrew's, the name and the owner's intent are a classic one's.** A 5 m heart radius means a 6.2 m rail radius and a rail apex 12.4 m above the entry, which needs about 16–17 m/s just to carry a train over; rating it at 18–20 m/s and designing the lateral limit at 20 m/s is what sized it. Arrow corkscrews (the Corkscrew at Cedar Point, 1976, and the Arrow double corkscrews that followed) are roughly 10 m tall, 25–35 m long per 360°, and are taken at 13–17 m/s with the rider's heart 4 m or so from the axis. B&M corkscrews taken at 20 m/s and more are 40–50 m long (Kumba's corkscrews). Half corkscrews in cobra rolls and sea serpents are about 30 m each. Heartline rolls: Mack and Intamin heartline rolls run 35–50 m at 15–20 m/s (Helix at Liseberg, Maverick), RMC barrel rolls take 1.5–2 s each at 15–20 m/s (the two near the end of Steel Vengeance), B&M zero-g rolls take about 2 s at 15–20 m/s at the top of a hill, and the one widely known fast roll, the long heartline roll over the lagoon on VelociCoaster, is a deliberately slow roll of roughly 2.5 s at high speed, so around 60–75 m. A roll that is "a few tens of metres" is a roll taken at 15–20 m/s; at 30 m/s even a real one is 60–75 m.

**Targets.** Roll schedule: roll rate `ω(u) = ω_peak · ramp(u)` with a quintic-smoothstep ramp over the first and last quarter of the piece (`f = 0.25`) and a constant rate in between; the roll angle is its integral (a closed-form polynomial), normalised so the total is the piece's roll. Peak rate is `1/(1 − f) = 1.33` times the mean; the roll angle is C² with zero rate and zero acceleration at both connectors, so the rail still has zero curvature at the joins and the join tests keep passing. Rated windows for rolls are found by a *speed* scan on fixed geometry (bisection over entry speed with full trains, the pattern already used for the 60° crests), stored as catalog constants and guarded by the endpoint tests; the span scan in `roll_scan.rs` goes away.

| Piece | Keep or change | Geometry target | Rated window target | Expected readings (both steel models, 6 and 12 cars) | Real reference |
|---|---|---|---|---|---|
| `HeartlineRoll(Right)` | change | straight heart, 48 m span (24 units), full 360° | 8–20 m/s (at 20 m/s: 2.4 s, mean 150°/s, peak 200°/s) | vertical −1..+1, lateral ≤ 1.0, longitudinal ≈ 0.2 along the pitched rail (gravity) plus ≈ 0.07 from the heart slowing while the rail spirals; heart acceleration ≤ 0.1 g | Mack and Intamin heartline rolls, RMC barrel rolls |
| `FastHeartlineRoll(Right)` (new, optional, one spec row; drop it if the owner wants one size) | add | 72 m span (36 units) | 18–30 m/s (peak 200°/s at 30 m/s) | as above | the VelociCoaster lagoon roll |
| `Corkscrew(Right)` (classic, not heartlined) | change | heart radius 4.0 m (rail 5.2 m), rail apex 10.4 m above entry, 32 m span (16 units), level exit at entry height | about 16–20 m/s, measured; widen the span to 36 m if the apex exceeds 4.2 g at the maximum, lower the maximum if the window is under 2 m/s | apex +1.5..+3 g, sides lateral ≤ 1.3 g, entry and exit near 1 g (rate is zero there), peak roll rate 150–180°/s | Arrow corkscrews |
| `HalfCorkscrew{Left,Right}{Up,Down}` | change span only | keep the owner-accepted 12 m rail rise (heart radius 4.8 m, 9.6 m heart rise); 32 m span (16 units); down pieces mirror up pieces | up: 18–22 m/s (keep the 22 m/s maximum, measure the minimum); down: energy-derived maximum for a 22 m/s exit, minimum 2 m/s | lateral ≤ 1.3 g, vertical within limits; widen to 36 m if the 22 m/s endpoint fails | halves of cobra rolls and sea serpents |
| `RavenRoll*` (4), `BarrelDrop*`, `TwistedDrop*`, `BankedDrop*` | keep geometry, switch to the new schedule, re-measure | as today (49 m raven roll, 32 m by 8 m barrel drop) | as today unless the re-measurement moves an endpoint | within limits at both ends | — |

For every roll the test asserts, at the rated maximum: peak roll rate ≤ 200°/s (from `roll` samples over rail distance times speed), heart acceleration excluding gravity ≤ 0.1 g in every direction for the heartline rolls (the purity test, replacing the 0.05 g longitudinal criterion), and the full-train force check at both window ends (already in `rated_element_physics`). The plan's "Roll measurements" paragraph and decision 4b's helix numbers (86 m, 64 m, 5 m) are superseded by this table.

### 2. Construction performance: where the seconds go

Measured with a scratch harness against this branch and against master (same prefix: station, three straights, a gentle climb and descent, two straights; flat 128×128 world; hybrid model), native release:

| Measurement | this branch | master |
|---|---|---|
| `catalog()` first call | 2.21 s (profile solving in the `LazyLock`s) | 46 ms |
| `catalog()` every later call | 127–133 ms (190 arc tables rebuilt) | 46 ms |
| `Piece::at(origin)` + `maximum_curvature()`, Corkscrew / HeartlineRoll | 2.4 + 2.4 ms / 1.5 + 1.5 ms | 0.25 + 0.25 ms |
| `choices()` cold, hybrid / multi-launch / wooden | 3.04 s / 2.42 s / 26 ms | 30 ms / — / — |
| `element_choices()` cold, hybrid / multi-launch / wooden | 0.97 s / 0.85 s / 41 ms | — |
| `validate()` per piece: CorkscrewRight, Corkscrew, HalfLoopUp, RavenRoll*Up, Loop, HalfCorkscrew*Up, LoopRight, LargeLoop(Right) | 328, 319, 294, 161/148, 153, 151, 139, 139/136 ms | every piece ≤ 2.1 ms |
| `validate()` for every non-inversion piece | ≤ 0.9 ms | ≤ 1.1 ms |

So the per-placement cost is **not** rated windows or force profiles: those are `LazyLock`s and are never recomputed on placement. Three things are:

1. **Support self-clearance for inversion pieces (about 2.1 s of the 2.4–3.0 s).** `route_supports` samples the candidate piece every 0.25 m into three rails (1,200–1,900 segments for a 100–160 m piece), twice (`inversion_track_segments` and `inverted_track_rail_segments`), and for every anchor and every footing candidate runs `segments_clear` (a segment-to-segment distance) against every segment whose 32-segment block bound is near the column. A column under a loop is near most blocks, so the bound prunes little. The wooden model has no inversions, which is why it validates in 26 ms. In the browser the same code runs in wasm, roughly two to three times slower than native, after every placement whose state hash changed, and `element_choices` repeats it for the half loop, two half corkscrews, two raven rolls and the two sea serpents. That is the "interaction timeout" in the construction spec and the implementer's 2.38 s / 4.64 s debug numbers.
2. **Profile solving at first catalog use (2.2 s native, several seconds in wasm).** Seven profiles at 2,048 steps, each integrated up to twelve times with 23 `sample` lookups per step for the train-mean height, then `minimum_entry` does the same over every point. In the browser this lands on the worker's first `catalog()` call, which is the startup timeout in the world spec.
3. **`catalog()` rebuilt per call (127 ms) and roll geometry derivatives by finite differences.** `catalog()` returns a fresh `Vec<Piece>` with every arc table recomputed; `construction_select` calls it on every click just to map a wire id, and `reports.rs`, `demo_circuit` and `available_pieces` call it too. Roll pieces sample the rail by central differences (three rail evaluations, each with its own heart kinematics and up vector) so `at()` and `maximum_curvature()` cost ten times master's per piece, about 60 ms per `choices()` over the twelve roll templates.

**Design.** Keep the worker synchronous and the caches as they are; make the work cheap instead.

- *Supports (task 1 below).* Build the candidate piece's rail segments once per `route_supports` at a step comparable to the beam clearance rather than 0.25 m, put them in a uniform 2D grid (4 m cells keyed by x and y, shared by the 3D check and the z-projected check, which uses the same segments with z = 0), and test a column only against the cells its footprint touches. Target ≤ 5 ms per inversion piece, so a cold `choices()` is ≤ 100 ms and `element_choices()` ≤ 150 ms native release for the hybrid model after the eight-piece prefix. Add `crates/sim/examples/construction_bench.rs` (the scratch harness: catalog timings, per-piece `validate()`, cold and cached `choices()` and `element_choices()` for all three models) and a `make bench-construction` target so the number is reproducible. The support positions must not change: snapshot the supports of a circuit containing a Loop, a Corkscrew and a HalfLoopUp before the rewrite and assert equality after.
- *Profiles (task 4).* Generate the solved profiles once and embed them. A test, run with `UPDATE_SOLVED_PROFILES=1` (the same pattern as `generate-report-types`), writes `crates/sim/src/track/profiles.generated.rs` holding each profile's points downsampled to 256 per profile (`profile::sample` already interpolates between points with position, tangent and curvature, so a coarser table reproduces the 2,048-point profile; assert ≤ 1 mm position and ≤ 1e-4 1/m curvature error against the full solve in the currency test) plus its solved `minimum_entry`. The `LazyLock`s read the table; `solved_profiles_are_current` re-solves and compares, so the solver stays the source of truth and still runs in tests. At 256 points the seven profiles add about 90 KB to a 1.7 MB wasm; at full resolution they would add 690 KB, which is why they are downsampled. First `catalog()` then costs the arc tables only.
- *Catalog and roll geometry (task 2).* `catalog()` becomes a `LazyLock` and returns `&'static [Piece]`; callers that need owned pieces clone (a handful of `into_iter()` sites, mostly tests). `Piece` gains `maximum_curvature: f64` computed when the template is built; `at()` copies the arc table and the curvature for even rotations (a rigid motion; diagonals are unreachable in play and keep the rebuild). `construction_select` maps a wire id without touching the catalog. Analytic rail derivatives for the helix are a possible later improvement, not needed once the table is reused.

With these three the "rated window at placement" worry is moot: windows and profiles are constants, the per-placement cost is clearance and supports, and both are tens of milliseconds. TASK-105 (the live overlay) needs exactly this budget, so task 1 is its prerequisite.

### 3. The top hat join test

The red test is `track::tests::joined_pieces_have_matching_position_tangent_roll_and_curvature` (`tests.rs:661`, `|Δκ| < 1e-8`). The 0.000212 1/m at the top hat's exit is deterministic, not numerical noise: `vertical_top_hat.rs::descent_profile` labels every point with the curvature of the step that *ends* at it, so the final point carries the last tapered step's curvature, `g · (cos 0 + 1) · sqrt(0.5 · step / 0.12) / v² ≈ 2e-4`, instead of the connector's zero, and the quintic interpolation faithfully reproduces that value at the connector. The ascent's last point has the same flaw at the plateau join inside the piece. Physically it is a 0.02 g step at 30 m/s and no rider would notice, so a 1e-3 tolerance would be harmless for this piece; but 1e-3 1/m at 34 m/s is a 0.12 g step, it would lower the bar for every other join, and the cause is a one-line fix (set the curvature of the final point of each half to 0, as the profile solver's pieces already do). Verdict: do not add the tolerance; fix the top hat's endpoint curvature and keep 1e-8. TASK-98 will replace the top hat geometry anyway and should keep the rule that a profile ends with its connector's curvature exactly.

### 4. Remaining work, in order

Each task is sized for one implementation agent. "Gate" lists what must be green before the task is done; the full workspace gate runs once at the end. Run targeted suites while editing; the machine is shared, so take the browser suites one at a time and never while another agent's browser run holds the lock.

**Task 1: fast support self-clearance (Terra, medium).** Rewrite the self-clearance check in `supports.rs` as described in section 2 (one segment list per call, coarser step, uniform grid, shared by both checks). Add `crates/sim/examples/construction_bench.rs` and `make bench-construction`. Add a snapshot test of support positions for a circuit with a Loop, a Corkscrew and a HalfLoopUp, recorded from the current implementation before the rewrite. Acceptance: bench shows every `validate()` ≤ 5 ms, cold `choices()` ≤ 100 ms and `element_choices()` ≤ 150 ms for the hybrid model (native release); snapshot unchanged. Gate: `cargo test -p theme-park-sim --release --lib track::supports track::construction` (two invocations), `cargo test -p theme-park-sim --release --test inversion_physics`, `make bench-construction`.

**Task 2: catalog and template caching (Luna, xhigh).** `catalog()` as `LazyLock` returning `&'static [Piece]`; `Piece::maximum_curvature` field; `at()` reuses the table for even rotations; `construction_select` without a catalog rebuild; `available_pieces`, `reports.rs` and `demo_circuit` on the static. Acceptance: bench shows later `catalog()` calls ≤ 1 ms and `at()` + `maximum_curvature()` for roll pieces ≤ 0.1 ms. Gate: `cargo test -p theme-park-sim --release --lib track::`, `cargo test -p theme-park-sim-wasm`, `npm test`, `npm run typecheck`, `make bench-construction`.

**Task 3: roll schedule and corkscrew / heartline redesign (Terra, medium).** In `roll.rs` replace `smooth7` as the roll schedule with the ramp-hold-ramp profile (value, first and second derivative in closed form; `spline_kinematics` keeps its Hermite basis). In `catalog.rs` set the spans, radii and rated constants from the table in section 1 (heartline 48 m; optional 72 m fast roll; corkscrew 4.0 m radius, 32 m; half corkscrews 32 m). Delete `roll_scan.rs`, its `corkscrew_spans` use, `full_corkscrew_uses_the_shortest_passing_two_metre_grid_length` and `heartline_roll_keeps_the_rider_on_a_straight_heart_path`. Rewrite `heartline_force_peaks` into the purity and roll-rate assertions of section 1 and add a contiguous-window speed scan for `Corkscrew` and `HalfCorkscrewLeftUp` modelled on `sixty_degree_ascending_crest_has_a_contiguous_rated_window`, which prints the measured window so the constants can be set. Update the construction element names if a piece is added. Acceptance: dimensions per the table; every roll passes both window ends with 6 and 12 cars on both steel models; windows ≥ 2 m/s; demo and composition circuits still close. Gate: `cargo test -p theme-park-sim --release --lib track::roll track::tests` (two invocations), `--test rated_element_physics --test inversion_physics --test inversion_compositions`, `cargo test -p theme-park-sim-wasm` (wire names), `npm test`.

**Task 4: embedded solved profiles (Luna, xhigh).** Generator test with `UPDATE_SOLVED_PROFILES=1`, `make generate-profiles`, `profiles.generated.rs` at 256 points per profile with the solved `minimum_entry`, `solved_profiles_are_current`, accuracy assertions (≤ 1 mm, ≤ 1e-4 1/m). Acceptance: first `catalog()` ≤ 150 ms native release before task 2's static lands, ≤ 20 ms after; wasm growth ≤ 150 KB (report the measured delta). Gate: `cargo test -p theme-park-sim --release --lib track::profile`, `--test inversion_physics --test rated_element_physics`, `make build`, `make bench-construction`.

**Task 5: tests that assumed old sizes, and the top hat exit curvature (Luna, xhigh).** Set the final-point curvature to zero in both halves of `vertical_top_hat.rs` so `joined_pieces_have_matching_position_tangent_roll_and_curvature` passes at 1e-8. Rewrite `top_hat_circuit.rs` to close its return leg by the breadth-first search used elsewhere instead of a hand-counted leg, so the test survives TASK-98's resize. Rewrite the Timber Tumbler circuit in `web/e2e/player/circuits.txt` for the new 15° and 60° pieces (and the Velocity Viper and Briar Blitz ones if they fail), keeping a chain lift, so `player_circuits_close` and the journeys agree. Re-record `timber_tumbler_prefix_boundary.rs` against the 32 m corkscrew from task 3. Delete the `TopHat` exclusion in `every_buildable_inversion_drop_and_sixty_transition_has_a_rated_window` only when TASK-98 gives it a window; until then leave the exclusion with a comment naming TASK-98. Gate: `cargo test --workspace -- --skip track::construction::walk_tests::seeded_` and the `--features threads` run, both green with zero failures.

**Task 6: demo circuit, browser baselines and journeys (Luna, xhigh).** Fold in the uncommitted `world.spec.ts` pick-point edit. Re-record the screenshot baselines that the resized pieces moved (construction preview, demo park at noon, dusk and night) after tasks 1–5, and check the construction spec's interaction timeouts are gone (they are the task 1 latency). Gate, one suite at a time, feature project: `npm run e2e -- construction.spec.ts`, `track.spec.ts`, `world.spec.ts`, `piece-catalog.spec.ts`, `immelmann.spec.ts`, `multi-launch-coaster.spec.ts`, `wooden-twister.spec.ts`, `construction-controls.spec.ts`; then `make journeys`.

**Task 7: close-out (Luna, xhigh).** Replace the replica numbers and the superseded roll paragraphs in this plan with the solver's and the measured windows; `make lint`; full `make test`; `make e2e` when the machine is free; handoff note; TASK-95 implementation notes.

**Revert or redo, explicitly:** the 86 m and 64 m corkscrew spans, the 5 m full-corkscrew radius, the 284 m heartline roll, the S7 roll schedule for rolls, `roll_scan.rs` and the two span tests, the 0.05 g longitudinal criterion, the per-call `catalog()` rebuild in `construction_select`, and the brute-force self-clearance loop (the requirement stays, the implementation goes). Nothing in the profile family, the force sampling, the rated-window tests or the compositions needs to be redone.

**Deferred to the follow-up tickets.** TASK-98 (top hats): the Briar Blitz top hat's rated window and the Velocity Viper launched top hat; until then the top hat stays excluded from the rated-window test and keeps the 176 m geometry with the exit-curvature fix from task 5. TASK-101 (RMC-style pieces): the zero-g roll is a heartline roll placed on a hill crest and the stall is a slow inverted roll; both reuse task 3's schedule and the profile solver, and outer banks reuse the overbank code, so they wait for task 3. TASK-104 (stats and scores): nothing here blocks it; it reads `CarReading`s that already exist. TASK-105 (live g-force overlay): depends on task 1 (and task 2) because it adds a per-placement simulation to the same synchronous worker budget.
