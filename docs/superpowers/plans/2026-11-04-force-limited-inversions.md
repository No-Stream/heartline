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
