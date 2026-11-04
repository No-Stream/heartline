# Coaster scoring model

> **For agentic workers:** Each work item below becomes one Backlog ticket under TASK-104. Pick one up, write its test-first steps, and carry it out with superpowers:subagent-driven-development or superpowers:executing-plans. Items are ordered and depend on the ride-statistics job on branch `forces-fixes` (worktree `theme-park-sim-worktrees/ride-stats`) landing first: it adds `RideStatistics`, the worst-car envelope in `TestRunRecorder::record`, heart-line forces and the 0-10 rescale of the old formula. Every constant in this plan is a starting value; the calibration item at the end is where they get tuned, and the tables here are the targets it tunes toward.

**Goal:** Replace the current excitement, intensity and nausea formula (distance-weighted means in `crates/sim/src/physics/testing.rs` `ratings`) with a model that rewards what riders actually like (airtime first, then forces, variety, pacing, whip, theming, length and speed), uses the ASTM F2291 acceleration envelope as a soft comfort bound that progressively lowers scores when exceeded, and can explain every score in the test report ("Excitement 7.4: lots of ejector airtime (+2.1), varied elements (+1.0), long dead stretch after the lift (-0.6)"). Scores read on a roughly 0-10 scale and are not capped.

**Architecture:** Scoring becomes three pure stages in `crates/sim/src/physics/`: `features.rs` turns the per-car sample record plus the circuit pieces and proximity events into a flat `RideFeatures` struct (seconds, counts, peaks, locations); `comfort.rs` evaluates the ASTM envelope on the sample record and produces an `EnvelopeExceedance`; `scoring.rs` maps `RideFeatures` to three `ScoreBreakdown`s whose contributions sum exactly to the score. `TestRun` carries the breakdowns next to the scores, the wasm report serialises them, and the testing panel lists them under each score. Guest code keeps its 0-100 traits and reads the ride's intensity and nausea from the facility instead of a constant.

**Tech stack:** Rust (`crates/sim`), wasm-bindgen and `ts_rs` for the report (`crates/sim-wasm/src/reports.rs`), TypeScript for the panel (`web/src/coaster-testing.ts`), Rust unit tests on synthetic sample sequences, Rust calibration tests on the game's circuits, one Playwright assertion on the panel.

**Spec:** ticket TASK-104 (statistics and rescale, in progress) and TASK-95 (heart-line forces). Related: TASK-105 (force overlay), the player journeys in `web/e2e/player/circuits.txt`.

## What exists today

- `TestRunRecorder` (`crates/sim/src/physics/testing.rs`) stores one `TestSample` per track metre (distance, speed, vertical, lateral and longitudinal g, airtime flag, height, inverted flag). The stats branch records the worst value across all cars per step and adds `TestRunRecorder::for_circuit`, which keeps `piece_starts`, rated-speed windows and launch ranges. One recorder exists per car in `CoasterRun` (`crates/sim/src/physics/coaster_run.rs`), so each car already gets its own report, and `park/tick.rs` hands each seat the report of its car.
- `ratings()` computes the three scores from distance-weighted means plus a drop term, an inversion-passage term and a lateral discomfort term; proximity events add `0.8 * S / (S + 4)` excitement afterwards in `update_proximity_ratings`. The doc comment on `ratings` is the only explanation a player could get; the panel shows numbers and free-text notes.
- The stats branch adds `RideStatistics` with: ejector airtime seconds (below -0.5 g), floater seconds (-0.5 to +0.2 g), airtime moments, strong positive seconds (above 3.5 g) and the longest run, strong lateral seconds (above 1.0 g), max positive, negative, lateral, forward and braking g, max and average speed, duration, length, max height, biggest drop, steepest drop, inversions, inverted seconds, drops, launches, launch peak speed. Seconds come from `1 m / mean speed` per sample, which this plan reuses as the time base.
- Pieces know their geometry class: `Piece.shape` (`Plain`, `AirtimeHill`, `Overbank`, `BarrelDrop`, `Loop`, `Corkscrew`, `Heartline`, `HalfLoop`, `HalfCorkscrew`, `TopHat` in `crates/sim/src/track/curves.rs`), `Piece.inversion` (`VerticalLoop`, `Corkscrew`, `HeartlineRoll`), `Piece.heartlining` (`Heartlined`, `NotHeartlined`) and `Piece.kind` (`Track`, `Lift`, `Launch`, `Brakes`, `Station`). `TrackSample.roll` is unwrapped radians, but roll does not reach `CarReading` or `TestSample`.
- Proximity events (`crates/sim/src/track/proximity/mod.rs`) have kinds `NearMiss`, `Headchopper`, `Dueling`, `Tunnel`, `TerrainHugging` with a strength in 0-1, one per piece and kind.
- Guests: `GuestStore` gives each guest `intensity` and `nausea_tolerance` in 0-100 (`crates/sim/src/guests/storage.rs`). Ride choice (`decisions.rs:283`) scores a ride as `180 - |preference - place.intensity| - 2 * needs.nausea + 20 if favourite type`. After a ride (`behavior.rs:319`) enjoyment is `excitement - |intensity - preference| - needs.nausea + scenery_score`, and the nausea gained is `(nausea_rating - tolerance) / 4`. The coaster facility's `place.intensity` is the constant 50 (`crates/sim/src/facilities.rs:2018`), so guests choose the coaster without knowing how intense it is. The stats branch multiplies the three ratings by 10 at `behavior.rs:319` so the 0-100 guest traits keep working.
- Theming: guests score scenery around their own position (`behavior.rs:148`, `scenery_scores`), which enters ride enjoyment; nothing scores the scenery along the track.

## Research

### ASTM F2291 acceleration limits

The numbers below are read from the duration plots in ASTM F2291-06a (public copy: https://ia800607.us.archive.org/34/items/gov.law.astm.f2291.2006/astm.f2291.2006.pdf, section 7.1, figures 6-20) and cross-checked against Rohde, "Some Details about the Development of Acceleration Limits for Amusement Rides", 2nd edition 2024 (https://www.vdv-freizeittechnologie.de/files/bilder-vdv/pdf/Rohde-Accelerationlimits-2nd-edition-2024.pdf), which summarises F2291-23b and EN 13814-2019 and agrees with the 06a values where it states them. Current editions are paywalled; Rohde implies the structure is unchanged. The limits are total acceleration including gravity (standing still reads +1 g vertical), apply to seated restrained riders, assume a 5 Hz low-pass, and do not cover impacts under 200 ms. Our 1 m samples at 200 Hz integration are smoother than that filter, so no extra filtering is needed.

| Axis (sign as in `GForces`) | Up to 1 s | Transition | Sustained |
|---|---|---|---|
| +vertical (into the seat) | 6.0 g | 4.0 g from 2 s to 4 s, 3.0 g from 5 s to 11.8 s | 2.0 g from 12 s |
| -vertical (airtime) | -2.0 g at 0.2 s, -1.5 g from 0.5 s | -1.5 g to 4 s, easing to -1.1 g at 7 s | -1.1 g |
| lateral, either sign | 3.0 g | 2.0 g from 2 s | 2.0 g |
| +longitudinal (launch, into the seat) | 6.0 g | 4.0 g from 2 s to 4 s, 3.0 g from 5 s to 11.8 s | 2.5 g from 12 s |
| -longitudinal (braking, into the restraint) | -2.0 g at 0.2 s, -1.5 g from 0.5 s | none | -1.5 g |

Combined axes (section 7.1.5): for each pair of axes the admissible region is an ellipse per quadrant centred on the origin with semi-axes equal to 1.1 times the 0.2 s limits (+vertical 6.0, -vertical 2.0, lateral 3.0, +longitudinal 6.0, -longitudinal 2.0). Reversals (section 7.1.6): two lateral or longitudinal peaks of opposite sign less than 200 ms apart halve the peak limits. Push-pull (section 7.1.7.1): after more than 3 s of negative vertical g the positive limit for the next 6 s drops to 2.0 g beyond 2.5 s. Onset (section 7.1.7.2): going from weightlessness to +2 g or more, the onset must stay under 15 g/s; Rohde recommends designing to 5 g/s. The lateral note on figure 8 says sustained laterals above 0.5 g need special design consideration and above 0.7 g need seating rules, which is a useful comfort threshold even though it is not a limit.

How the model uses this: the envelope is a soft upper bound (owner decision 2026-11-04). It does not forbid or clamp anything. Every second a car spends above the duration curve, and every pair of axes outside its ellipse, adds to an exceedance measure that progressively lowers excitement and raises intensity and nausea, and the report names where it happened. The sustained plateaus (2.0 g vertical, 2.0 g lateral, -1.1 g airtime, 2.5 g launch, -1.5 g braking) double as the comfort baselines from which the duration clock starts. Intensity is otherwise about how much of the envelope a ride uses, which is a different question from whether riders enjoy it; that is why intensity and excitement are separate scores.

### What riders like

The research agent found no peer-reviewed model of coaster enjoyment and no published numeric cutoffs for ejector or floater airtime, so the thresholds in this plan are conventions, and the weights are the owner's judgement plus mine. What the sources do support:

- Airtime is the dominant feature of top-ranked rides. Mitch Hawker's poll (pairwise ranking by riders who rode both coasters; 2006 steel results at https://coastercritic.com/2007/02/worlds-best-roller-coasters-announced/, wood results at https://coastercritic.com/2011/02/2010s-wooden-roller-coasters-mitchs-coaster-poll-results/) and the Golden Ticket Awards (2025 list at https://www.coaster101.com/2025/09/08/full-list-of-the-2025-golden-ticket-award-winners/) are dominated by airtime-heavy hypers, Intamin and RMC rides and the airtime woodies (El Toro, The Voyage, Boulder Dash, Phoenix). Wikipedia's airtime page (https://en.wikipedia.org/wiki/Air_time_(rides)) describes floater as near weightlessness and ejector as sudden and violent, and lists Steel Vengeance at 27.2 s of airtime. Pendrill's accelerometer data on Helix shows airtime hills at about 0 g and about -1 g (https://physicsworld.com/a/twists-turns-thrills-and-spills-the-physics-of-rollercoasters/). Forum definitions disagree; the stats branch's cutoffs (ejector below -0.5 g, floater -0.5 to +0.2 g) sit inside the range people use.
- Forces: Coasterpedia (https://coasterpedia.net/wiki/G-Force) gives 5 g as the commonly cited positive ceiling and says greying out is normal but can feel unsafe. Coaster Critic (https://coastercritic.com/2017/07/23/bm-roller-coasters-not-forceless/) explains "forceless" as a relative label: rides with helices and grey-out turns (Goliath at Six Flags Magic Mountain) read as forceful, rides with only floater airtime do not, even when measured forces are high. So sustained positives and laterals should add excitement in moderation and nausea past a point.
- Pacing: reviews praise rides that "never let you take a breather" (Fury 325, https://captaincoaster.com/de/reviews/393; Steel Vengeance's mid-course brake is a "3 or 4 second" breather, https://coastercritic.com/2018/05/review-steel-vengeance/), while a CoasterForce thread argues short dead spots are fine (https://forums.coasterforce.com/threads/deadspot-appreciation.43217/post-1049156). The model therefore penalises long low-energy stretches and ignores short ones.
- Whip and jerk: Walker's "thrill factor" work found skin-conductance response tracks the rate of change of acceleration (press coverage at https://www.sciencefocus.com/future-technology/the-thrill-engineers/), and Pendrill and Eager, "Velocity, acceleration, jerk, snap and vibration", Physics Education 55 (2020) treat jerk as the rider-relevant quantity. Nobody publishes roll rates, so the whip term counts sharp force transitions and roll rate with modest weights.
- Variety: the only direct evidence is reviewers finding Colossus at Thorpe Park (ten inversions, four of them heartline rolls in a row) "worth one ride per visit" (https://captaincoaster.com/en/reviews/3381). The strong diminishing return on repeated identical inversions is the owner's call, supported by that.
- Headbanging on classic Arrow and Vekoma inversions is blamed by riders on restraints, maintenance and coarse transitions (https://coasterbuzz.com/Forums/PostLink/155948); the physics is that a non-heartlined roll swings the rider about a large radius, which shows up in our heart-line lateral and vertical readings as quick reversals. The model does not need a manufacturer flag: `Heartlining::NotHeartlined` pieces produce the reversals and laterals that the nausea terms count, and get a small inversion-value discount.
- Theming, near misses and terrain: no quantitative source; weights are design choices and are kept small enough that forces decide the ranking.

### Existing games

RollerCoaster Tycoon, read from the OpenRCT2 source (https://github.com/OpenRCT2/OpenRCT2/blob/develop/src/openrct2/ride/RideRatings.cpp and the per-coaster tables under `ride/rtd/coaster`, fetched 2026-11-04), is a linear sum: a per-ride-type base tuple (looping coaster 3.00 / 0.50 / 0.20, wooden 3.20 / 2.60 / 2.00, inverted 3.60 / 2.80 / 3.20), plus measured quantities times per-type coefficients (max speed, average speed, duration capped at 150 units, length capped, train length), plus shared sub-ratings: g-forces (max positive g adds excitement 0.08 per hundredth, intensity 0.80, nausea 0.26; max negative g adds excitement 0.24 per hundredth clamped at -2.5 g; lateral g adds excitement only up to 1.5 g while intensity and nausea keep rising, and above 2.8 g and 3.1 g lateral there are step penalties), drops (per drop up to nine, plus highest drop), turns (inversions add 26.7 excitement each up to six, 50 intensity each uncapped; helices small), sheltered track, a scenery count in an 11x11 area around the station, a proximity score from track-walk counters, and air time (samples at or below 0 g; excitement `min(air, 200) / 8`). "Requirements" (highest drop under 14 units, max speed, fewer than two drops, no negative g) halve all three ratings unless the ride has an inversion. The one coupling is the intensity cutoff: each of the bounds 10.00, 11.00, 12.00, 13.20 and 14.50 that intensity reaches cuts excitement by a quarter, compounding. Guests have an intensity range (half of guests get 4-15, the rest narrow bands like 2-5) and a nausea tolerance (thresholds 3.00, 6.00, 8.00, 10.00) and skip rides outside them; intensity is the "will they ride" axis, excitement the "how happy and how much they pay" axis.

Planet Coaster publishes no formulas. Frontier's guide (https://planetcoaster.com/player-guides/coaster-ride-building) says excitement comes from g-forces, direction and speed changes, inversions and head-choppers; fear from g-forces approaching and exceeding a threshold, with 5 "just right" and higher lowering prestige; nausea from inversions and prolonged or lateral g; and that all three "reduce over time while riding if there are no g-forces or variations in g-forces", so its stats are running values that decay on smooth track rather than end-of-test aggregates. Parkitect's scale is 0-100 and intensity above about 10 of 100 already cuts excitement (https://texelraptor.com/blog/devlog-update-89, community guide). NoLimits 2 has no rating, only g-force colouring that the community ties to F2291.

What to take and what to avoid: RCT's separation of "will they ride" (intensity and nausea against guest tolerances) from "how good is it" (excitement) is right and this plan keeps it. Its airtime term is binary (at or below 0 g) and tiny next to inversions and drops, which is the opposite of enthusiast rankings; its halving requirements are cliffs; its per-type base values make the model type-driven rather than layout-driven, which would be wrong here because the player builds the layout. Planet Coaster's decay idea is where the pacing term comes from, but as an explicit dead-time measure rather than a hidden running average, so the report can point at the stretch. Neither game explains its scores to the player, which is the main thing this plan adds.

## Design

### Inputs

`RideFeatures` is computed once per car report in `TestRunRecorder::finish` from three inputs:

1. The sample record, with two additions to `CarReading` and `TestSample`: `roll` (unwrapped radians, copied from `TrackSample.roll` where `inverted` is set in `physics/mod.rs:568`) and `piece_index`. Both are hashed (`write_sample`, `write_reading`; bump `TestRun-v2` to `v3` and `TestRunRecorder-v3` to `v4`).
2. The circuit pieces, which `for_circuit` already copies partially; it additionally keeps per piece `name`, `kind`, `shape`, `inversion`, `heartlining` and `length` in a `Vec<PieceSummary>`.
3. The proximity events and, later, a theming score, added after `finish` the way `add_proximity_events` does today.

Time base: each sample `i` lasts `dt_i = 1 m / mean(speed_i, speed_{i+1})` as in `ride_statistics`. A sample is *active* when its piece kind is `Track`, `Launch` or `Brakes` and its speed is above 1 m/s; station and lift samples are excluded from the time-based terms so a slow lift does not count as dead time or ride time. *Ride seconds* is the active total.

### Shared helpers

- `soft(x, scale) = 1 - exp(-x / scale)`, a saturating curve in [0, 1) that is linear near zero, 0.63 at `x = scale`, 0.86 at `2 * scale`. Every positive term is `weight * soft(measure, scale)`, so a weight is the most a term can ever contribute and a scale is the amount of the measure that earns two thirds of it. That keeps scores soft and uncapped while making each constant explainable.
- `Location { piece_index, piece_name, distance_m, seconds_from_start }` names where something happened; it is attached to penalties and to the worst case of each feature so the panel can say "at the valley after the first drop" (piece name plus distance).

### Envelope evaluation (`comfort.rs`)

For each axis the duration curve from the table above is a piecewise-linear function `limit(axis, seconds)`. Walking the active samples in order:

- The *excursion clock* for an axis starts when the reading passes the sustained plateau for that axis (2.0 g positive vertical, -1.1 g airtime, 2.0 g lateral, 2.5 g forward, -1.5 g braking) and resets when it comes back under. At each sample the allowed value is `limit(axis, clock)`, and the single-axis exceedance ratio is `max(0, |g| / limit - 1)`.
- The pair test uses the 0.2 s radii times 1.1: `ratio = sqrt((g_a / r_a)^2 + (g_b / r_b)^2)` for vertical-lateral, vertical-longitudinal and lateral-longitudinal, with the radius chosen by sign; exceedance is `max(0, ratio - 1)`.
- The reversal rule halves the lateral and longitudinal limits for a sample when the reading of opposite sign at least 0.5 g in magnitude occurred within the previous 0.2 s.
- The push-pull rule and the 15 g/s onset rule are applied as written: after more than 3 s below 0 g vertical, `limit(+vertical, clock)` is capped at 2.0 g for the next 6 s; an onset from at or below 0 g to at least 2 g faster than 15 g/s counts as a 0.5 ratio exceedance on that sample. Both are cheap and give the report something to say about pull-out valleys after long ejector hills.
- Per sample, `exceedance_i = max` of the single-axis and pair ratios. The output is `EnvelopeExceedance { g_seconds: sum(exceedance_i * dt_i), seconds: sum(dt_i where exceedance_i > 0), worst: Option<(Location, axis, value, allowed)>, stretches: Vec<(Location, seconds)> }`, where stretches are the contiguous exceeding runs longer than 0.2 s, longest first.

`g_seconds` is the quantity the scores use. A valley at 5 g for 1.8 s where 4 g is allowed is `0.25 * 1.8 = 0.45` g-seconds; the same valley at 4.4 g is 0.18.

### Features (`features.rs`)

`RideFeatures` holds, besides the `RideStatistics` fields the stats branch already computes:

- `ride_seconds`, `active_average_speed`.
- `inversion_passages: Vec<(Inversion, Heartlining)>` in ride order, taken from the pieces where the inverted flag first goes true (so a loop entered twice counts twice, as the owner wants for repeats).
- `element_classes: BTreeSet<ElementClass>` with `ElementClass` = `EjectorHill`, `FloaterHill`, `BigDrop` (20 m or more), `VerticalLoop`, `Corkscrew`, `HeartlineRoll`, `HalfLoop`, `HalfCorkscrew`, `TopHat`, `Overbank`, `BarrelDrop`, `Helix` (a turn held for 270° or more at speed), `BankedTurn`, `Launch`, `Tunnel`, `NearMiss` (near miss, headchopper or dueling), `TerrainHugging`. Shape-based classes come from `PieceSummary`, hill classes from the airtime band each moment reached, proximity classes from the events.
- Pacing: a sample is *eventful* when `|vertical - 1| >= 0.5`, `|lateral| >= 0.5`, `|longitudinal| >= 0.4`, speed at least 20 m/s, or inverted. A *dead run* is a maximal run of active, non-eventful samples; its *excess* is its seconds beyond 3.0. `dead_excess_seconds` is the sum of excesses, excluding the final brake run into the station (the last dead run before the station piece). `longest_dead_run: Option<(Location, seconds)>`. `highlight_gap_seconds` is the longest time between highlights (an airtime moment, an inversion passage, a strong positive or lateral run, a launch, a drop of 20 m or more, a proximity event).
- Whip: `snap_count` counts transitions where vertical g changes by 2.0 g or more within 0.4 s (a crest from +3 to -1 or a pull-out the other way); `max_roll_rate_deg_s` from `d(roll)/dt` over 0.5 s windows; `roll_rate_seconds` above 90°/s. `reversal_count` counts vertical or lateral sign changes of 1.5 g or more peak to peak within 0.3 s (this is the headbanging measure).
- Proximity: `proximity_strength` is the sum of clamped strengths, and the count per kind.
- `theming: f64` in 0-1, set by the park layer after finish (see the theming item); zero until then.
- `envelope: EnvelopeExceedance`.

### Scores (`scoring.rs`)

Every constant lives in one `SCORING` table (`struct ScoringConstants` with a `const SCORING: ScoringConstants`), one line per term, so calibration is a single diff. Each score is `Vec<Contribution>` summed; `Contribution { term: Term (an enum), value: f64, measure: f64, location: Option<Location> }`. `Term` carries a short label and the unit of its measure so the panel can print "lots of ejector airtime (+2.1, 7.6 s)". Negative contributions are listed with their location when they have one.

**Excitement** (typical range 2-10; the sum of all weights is about 17, which no real layout reaches):

| Term | Formula | Why |
|---|---|---|
| Ejector airtime | `2.6 * soft(ejector_seconds, 2.5)` | the owner's top priority; 8 s earns 2.5 |
| Floater airtime | `1.5 * soft(floater_seconds, 4.0)` | B&M-style floating is good but worth less per second |
| Airtime moments | `1.2 * soft(airtime_moments, 10)` | weak diminishing returns: the 10th hill still pays 60% of the first |
| Positive forces | `1.0 * soft(strong_positive_seconds, 2.0)` | forceful valleys and helices, in moderation |
| Laterals | `0.7 * soft(strong_lateral_seconds, 2.0)` | whippy transitions; excessive laterals are handled by nausea |
| Inversions | `2.0 * soft(inversion_points, 2.0)` with `inversion_points = sum over passages of 0.55^(k-1) * (1.0 if heartlined else 0.9)`, `k` the passage's rank among inversions of the same `Inversion` kind | strong diminishing returns for repeats: six loops score 1.33, a loop, a corkscrew and a heartline roll score 1.55 |
| Top speed | `1.2 * soft(max(max_speed - 12, 0), 20)` | 32 m/s earns 0.76 |
| Biggest drop | `0.8 * soft(biggest_drop, 40)` | |
| Pace | `0.6 * soft(max(active_average_speed - 8, 0), 12)` | the average-speed term the owner asked about |
| Launch | `0.8 * soft(max_forward_g, 1.0)` when launches > 0 | a 1.7 g launch earns 0.65 |
| Variety | `1.0 * soft(element_classes - 1, 4)` | five classes earn 0.63 |
| Whip | `0.6 * soft(snap_count, 4) + 0.5 * soft(max(max_roll_rate_deg_s - 60, 0), 90)` | aggression and roll speed; also counted by nausea |
| Ride time | `0.8 * soft(ride_seconds, 45) - 1.0 * max(0, 1 - ride_seconds / 20)` | 20 s or shorter costs up to a point |
| Proximity | `1.0 * soft(proximity_strength, 2.0)` | replaces `0.8 * S / (S + 4)` with the same shape |
| Theming | `0.8 * theming` | |
| Dead time | `-1.5 * soft(dead_excess_seconds, 6)`, located at the longest dead run | breathers under 3 s are free; 6 s of excess costs 0.95 |
| Above the envelope | `-3.0 * soft(envelope.g_seconds, 1.0)`, located at the worst stretch | the ASTM soft bound; 0.45 g-seconds costs 1.1 |
| Sickening | `-0.35 * max(0, nausea - 5)` | a nausea 8 ride loses 1.05 |

**Intensity** (typical range 2-9; measures how much of the envelope and how much energy the ride uses):

| Term | Formula |
|---|---|
| Peak force | `4.0 * soft(peak_ratio, 0.7)` where `peak_ratio` is the largest reading over its 0.2 s limit across axes (+vertical/6, -vertical/2, lateral/3, forward/6, braking/2) |
| Sustained force | `3.0 * soft(strong_positive_seconds + strong_lateral_seconds + ejector_seconds, 6)` |
| Speed | `1.5 * soft(max(max_speed - 10, 0), 25)` |
| Drop | `1.0 * soft(biggest_drop, 50)` |
| Launch | `0.8 * soft(max_forward_g, 1.0)` when launches > 0 |
| Inversions | `0.6 * soft(inversions, 3)` |
| Above the envelope | `1.5 * soft(envelope.g_seconds, 1.0)`, located |

**Nausea** (typical range 0.5-7):

| Term | Formula |
|---|---|
| Laterals | `2.5 * soft(strong_lateral_seconds, 3)` |
| Inversions | `1.5 * soft(sum over passages of (0.6 if heartlined else 1.0), 3)` |
| Fast rolls | `1.5 * soft(roll_rate_seconds, 2)` |
| Reversals | `1.5 * soft(reversal_count, 6)` |
| Grey-out | `1.0 * soft(strong_positive_seconds, 4)` |
| Time inverted | `0.5 * soft(inverted_seconds, 4)` |
| Above the envelope | `2.5 * soft(envelope.g_seconds, 1.0)`, located |

The proximity contribution moves inside scoring (it is a feature like any other), so `update_proximity_ratings` becomes "recompute scores from features" and the `base_excitement` and `base_intensity` fields go away. The existing free-text notes stay but are derived from the breakdown: "very intense" when intensity is at least 7, the lateral note when the laterals term is at least 1.5, the slow-layout note when `dead_excess_seconds` is at least 8, and a new note when the envelope term is non-zero: "Forces exceed the ASTM F2291 comfort envelope for {seconds} s, worst at {piece} ({axis} {value} g where {allowed} g is allowed). The scores are reduced; a gentler valley or a slower entry would recover them." Units stay SI in Rust and the panel converts them, following `formatTestingNote`.

### Report and panel

`TestingReport` gains `breakdown: { excitement: Contribution[], intensity: Contribution[], nausea: Contribution[] }` with `term` as a string key, `value`, `measure`, `unit` and an optional `location { pieceIndex, pieceName, distance, seconds }`. The panel renders each score as "Excitement 7.4" followed by the contributions sorted by absolute value, largest first, as "lots of ejector airtime (+2.1, 7.6 s)" and "forces above the ASTM comfort envelope for 1.8 s at Loop (-1.2)". Labels per term live in one table in `coaster-testing.ts`, next to `proximityLabels`. Clicking a located contribution selects it like `inspectProximity` does, so the ribbon highlights the stretch. Dropping the `measure` into words ("lots of" above 2 s, "some" otherwise) is deferred; the first version prints the number.

## Calibration

### Archetype targets

These are the bands the synthetic archetype tests assert, with the measures that define each archetype as a synthetic `RideFeatures` (not a track). The spot checks come from evaluating the formulas above by hand.

| Archetype | Defining measures | Excitement | Intensity | Nausea |
|---|---|---|---|---|
| Family wooden coaster | 1.5 s floater, 3 moments, 18 m/s, 15 m drop, 40 s, no inversions, 0.5 s laterals, 3 classes | 1.5-3.0 (check: 2.1) | 2.0-3.5 (2.9) | 0.3-1.5 (0.6) |
| Classic Arrow looper | 1 s floater, two loops and two corkscrews not heartlined, 5.5 g for 1.5 s, 4 s above 3.5 g, 2.5 s laterals, 5 reversals, 6 s dead time excess, 25 m/s, 35 m drop | 4.0-6.0 (5.2) | 6.0-7.5 (6.7) | 4.5-7.0 (5.3) |
| B&M hyper | 0.5 s ejector, 6 s floater, 7 moments, 3 s above 3.5 g, 35 m/s, 60 m drop, 70 s, 5 classes | 6.5-8.5 (7.9) | 5.0-6.5 (5.4) | 1.0-2.5 (1.5) |
| RMC hybrid | 9 s ejector, 2 s floater, 15 moments, two heartlined inversions of different kinds, 3 s laterals, 8 snaps, 150°/s rolls, 32 m/s, 55 m drop, 8 classes, headchoppers | 8.5-11 (10.3) | 7.0-8.5 (7.3) | 3.5-5.5 (4.7) |
| Intamin accelerator | 1.7 g launch, top hat with 1.5 s ejector, 4.5 g pull-out, 57 m/s, 120 m drop, 20 s, 4 classes | 5.5-7.0 (6.4) | 6.5-8.0 (6.9) | 0.8-2.5 (1.1) |

The ordering the tests must preserve regardless of tuning: excitement RMC > B&M hyper > accelerator > Arrow > family; nausea Arrow > RMC > B&M > accelerator > family; intensity RMC and accelerator above Arrow above B&M above family.

### The game's own circuits

The calibration item runs the model on real circuits with each model's train (`crates/sim/src/track/models.rs`: `wooden_twister_model`, `hybrid_model`, `multi_launch_model`): the demo circuit (`physics::demo_circuit`), Timber Tumbler, Velocity Viper and Briar Blitz from `web/e2e/player/circuits.txt` (a Rust test helper parses that file so the browser journeys and the calibration stay the same circuits), plus synthetic circuits built with `circuit_from_ids`: an out-and-back of six `AirtimeHill`s, the same with the hills replaced by `Straight`s (must score lower by at least 2 excitement), six `Loop`s in a row against loop, corkscrew and heartline roll (the varied one must score higher), and a layout with a 15 s flat stretch after the lift (must carry a located dead-time penalty).

Expected bands before tuning, to be replaced by measured values once the model runs: demo circuit 2.5-4.5, Velocity Viper 3-5, Briar Blitz 4-6, Timber Tumbler 4.5-6.5. The test prints a table (`cargo test calibration_table -- --nocapture`) with every contribution, the owner reviews it against the archetype bands, constants in `SCORING` are adjusted, and the bands are then frozen in the test with a tolerance of 0.75. Any later change to a constant has to update the table in the same commit.

## Guests

The 0-100 guest traits stay. The ride side changes so guests react to the real ride:

- `KnownPlace` gains `nausea: u8` next to `intensity` (hashed in `park.rs:260`; bump the tag). For flat rides the constants in `facilities.rs:350` get a nausea value (carousel 10, ferris wheel 5).
- The coaster facility (`facilities.rs:2005`, built in `park/block_analysis.rs:78` and `park/tick.rs:42`) takes `intensity = round(10 * report.intensity)` and `nausea = round(10 * report.nausea)` from the latest completed test report of the front car, or 50 and 30 while the coaster has never been tested. The park keeps that report's two numbers in `Park` so a facility rebuild after a track edit does not lose them.
- Ride choice (`decisions.rs:283`): rides whose `place.nausea` exceeds the guest's `nausea_tolerance + 30` are skipped ("too sick-making for me"); otherwise the existing score stands, with `place.intensity` now meaningful. This is the intensity-preference-by-trait link the owner asked for: a guest with preference 30 picks the family coaster at intensity 29 over the RMC at 73.
- After the ride (`behavior.rs:319`): enjoyment keeps the stats-branch form `10 * excitement - |10 * intensity - preference| - needs.nausea + scenery`, and the nausea gain uses the ride's nausea rating instead of `place.intensity`: `max(0, 10 * nausea - tolerance) / 4`. The seat's own car report still provides the numbers, so the back car on a whippy ride is rated differently from the front.
- A test in `guests/park.rs` opens a coaster with a known report and checks that a low-preference guest declines it and that the memory's nausea comes from the nausea rating.

## Testing strategy

- `comfort.rs` unit tests on synthetic sample sequences (a helper builds samples from `(seconds, vertical, lateral, longitudinal)` tuples at a fixed speed): 5 g for 0.8 s has zero exceedance; 5 g for 3 s exceeds from 2 s on; -1.3 g for 5 s is fine and -1.3 g for 8 s is not; 2.5 g lateral for 1.5 s is fine, for 3 s is not; 4 g vertical with 2.5 g lateral exceeds the ellipse while each alone passes; a +2.5 g to -2.5 g lateral flip within 0.2 s halves the limit; the push-pull and onset rules fire on a 4 s ejector hill into a 3 g valley at 20 g/s. The worst location points at the right sample.
- `features.rs` unit tests: inversion passages by kind with heartlining, element classes from a circuit of known pieces, dead time excluding the final brake run, snap and reversal counts on a crafted sequence, roll rate from a heartline roll piece.
- `scoring.rs` unit tests: every contribution list sums to its score (property test over random features); the archetype bands and the orderings above; six identical loops score less than three varied inversions; airtime moments 10 to 20 still adds at least 0.2 (weak diminishing returns); a feature set with 0.45 g-seconds of exceedance carries an excitement contribution of about -1.1 with the location passed through; `describe()` of that contribution reads "forces above the ASTM comfort envelope for 1.8 s at Loop (-1.1)".
- Report explanation test in `testing.rs`: a synthetic run through `TestRunRecorder` produces a report whose breakdown contains the expected terms, whose notes include the envelope note with the piece name, and whose scores equal the breakdown sums; `TestRun` hashing covers the breakdown.
- Calibration tests as described above, in the fast tier (each run is one lap of a small circuit).
- wasm wire test in `reports.rs` for the breakdown shape; a Vitest for the label table (every `Term` key has a label and the located sentence formats units through `formatTestingNote`); one Playwright assertion in the existing testing-panel spec that a tested Timber Tumbler shows an "Ejector airtime" line under Excitement.
- Run targeted suites (`cargo test -p sim physics::`) while editing; `make check` at the end of each item and `make ci` at the end of the last.

## Work items

1. **Sample record carries roll and piece index.**
   - Goal: `CarReading.roll` and `CarReading.piece_index` set in `physics/mod.rs` where `inverted` is set; `TestSample` carries both; `interpolate_reading` interpolates roll and keeps the piece of the nearer sample; `for_circuit` keeps `Vec<PieceSummary>` (name, kind, shape, inversion, heartlining, length); hash tags bumped; the active-sample and `dt` helpers extracted from `ride_statistics` into `features.rs` so the stats and the features share one time base.
   - Files: `crates/sim/src/physics/mod.rs`, `crates/sim/src/physics/testing.rs`, new `crates/sim/src/physics/features.rs`, `crates/sim/src/track/curves.rs` (make `Shape` `pub` or add a `pub fn element_shape`).
   - Acceptance: a recorded lap of the demo circuit has monotonic unwrapped roll through the loop, piece indices matching `circuit.piece_index`, and `ride_statistics` output unchanged; `make check` green; session replays unchanged except the hash tag.
   - Model: Codex Luna. Dependencies: the stats job landed.
2. **ASTM envelope evaluation.**
   - Goal: `comfort.rs` with the duration curves, excursion clocks, pair ellipses, reversal, push-pull and onset rules, and `EnvelopeExceedance` with located stretches.
   - Files: new `crates/sim/src/physics/comfort.rs`, `crates/sim/src/physics/mod.rs`.
   - Acceptance: the `comfort.rs` tests in the testing strategy pass; the function is pure over `&[TestSample]` and the piece summaries.
   - Model: Codex Terra (many rules, easy to get a clock subtly wrong). Dependencies: 1.
3. **Ride features.**
   - Goal: `RideFeatures` and `ride_features(samples, pieces, proximity_events)` with inversion passages, element classes, pacing, whip, reversals and proximity as specified; `Location` type.
   - Files: `crates/sim/src/physics/features.rs`.
   - Acceptance: the `features.rs` tests pass.
   - Model: Codex Luna. Dependencies: 1, 2.
4. **Scores and breakdown.**
   - Goal: `scoring.rs` with `SCORING`, `score(features) -> Scores { excitement, intensity, nausea, breakdown }`, `Contribution`, `Term` with labels and units, `describe()`, and the notes derived from the breakdown; `TestRun` replaces `ratings` and `update_proximity_ratings` with a recompute from features; `base_excitement` and `base_intensity` removed.
   - Files: new `crates/sim/src/physics/scoring.rs`, `crates/sim/src/physics/testing.rs`, `crates/sim/src/physics/tests.rs`.
   - Acceptance: `scoring.rs` and report-explanation tests pass; existing tests that assert relative ratings (hills beat flat, proximity raises excitement, the stall note) still pass with updated expected text; the old constants are gone.
   - Model: Codex Terra. Dependencies: 3.
5. **Report wire and panel.**
   - Goal: `TestingReport.breakdown` through `reports.rs` and `ts_rs`; the panel lists contributions under each score with located penalties clickable; label table and unit formatting in `coaster-testing.ts`; `gameTest.testingReport()` carries it.
   - Files: `crates/sim-wasm/src/reports.rs`, `crates/sim-wasm/src/construction.rs`, `web/src/coaster-testing.ts`, `web/src/coaster-testing.test.ts`, `web/src/generated/*`, the testing-panel Playwright spec.
   - Acceptance: wire test, Vitest label test and the Playwright assertion pass; `make check` green.
   - Model: Codex Luna. Dependencies: 4.
6. **Theming along the track.**
   - Goal: `track_theming(world, pieces) -> f64` in the park layer: for each sampled track metre count scenery within 3 tiles, weight 2 per piece and 4 when it matches the coaster's theme (there is no coaster theme yet, so all pieces weigh 2 for now), and take `soft(weighted pieces per 100 m, 12)`; `TestRun::set_theming(score)` recomputes the scores; `park/tick.rs` calls it where it adds proximity events.
   - Files: `crates/sim/src/park/tick.rs`, `crates/sim/src/physics/testing.rs`, `crates/sim/src/park.rs` tests.
   - Acceptance: a coaster surrounded by scenery scores higher than the same coaster on bare ground by at most 0.8 excitement; the test in `park.rs` covers both.
   - Model: Codex Luna. Dependencies: 4.
7. **Guests read the real ratings.**
   - Goal: `KnownPlace.nausea`, the coaster facility's intensity and nausea from the latest report, the skip rule in ride choice, and the nausea gain from the nausea rating, as in the Guests section.
   - Files: `crates/sim/src/guests/knowledge.rs`, `crates/sim/src/guests/decisions.rs`, `crates/sim/src/guests/behavior.rs`, `crates/sim/src/facilities.rs`, `crates/sim/src/park.rs`, `crates/sim/src/park/block_analysis.rs`, `crates/sim/src/park/tick.rs`, `crates/sim-wasm/src/guest_report.rs` if the place is reported.
   - Acceptance: the `guests/park.rs` test in the Guests section; the determinism tests pass with the new hash tag; `make check` green.
   - Model: Codex Luna. Dependencies: 4.
8. **Calibration on circuits and docs.**
   - Goal: the `circuits.txt` parser helper, the calibration table test with the synthetic layouts, the owner's review of the printed table, the tuned `SCORING` constants, the frozen bands, and `docs/status.md` updated with the model summary and the calibration table; TASK-104 closed with the deferred list below as follow-ups.
   - Files: `crates/sim/src/physics/calibration_tests.rs`, `crates/sim/src/physics/scoring.rs`, `docs/status.md`.
   - Acceptance: the calibration tests pass with the frozen bands; `make ci` green.
   - Model: Codex Luna for the harness, the owner for the review. Dependencies: 5, 6, 7.

## Deferred

- Per-model "character" terms (a wooden coaster's rattle adding intensity, a B&M's smoothness lowering nausea) and a roughness measure from track joins.
- Words instead of numbers in the panel ("lots of", "some").
- A coaster theme chosen by the player so matching scenery counts double.
- Queue-side effects (guests watching a ride's airtime from the path raising its appeal).
- Adaptive guest preferences (a guest learning to like intensity after good rides).

## Risks

- **The envelope clock on 1 m samples.** At station speeds a metre is a long time and at 50 m/s it is 20 ms; the clock is in seconds, so it is right either way, but slow samples inside a loop entry could put a 0.2 s peak over its duration limit. The comfort tests include a slow-entry case and the implementer can widen the first duration step if it fires spuriously.
- **Worst-car envelope versus per-car reports.** The stats branch records the worst value across cars in each recorder frame, while `CoasterRun` runs one recorder per car; the implementer confirms which path the guests' per-seat reports use and keeps per-car reports per car.
- **Double counting.** Laterals appear in excitement, intensity and nausea on purpose, each with a different scale; the calibration table is where the balance is checked, and the orderings in the archetype tests catch a term that dominates.
- **Tuning churn.** All constants in one table and the calibration test that prints every contribution keep it a one-file change.

## Owner decisions to make

1. Archetype bands: the table above is my proposal; the RMC band tops out above 10, which follows from "soft, not capped". Say if you want the best rides to land nearer 9.
2. Guest skip rule: a ride is skipped when its nausea rating (0-100) exceeds the guest's tolerance by more than 30. Alternative: no skip, only the existing choice score.
3. Theming for now counts all scenery equally; a coaster theme is deferred.
4. The intensity note threshold stays at 7; the envelope note appears whenever the term is non-zero.
