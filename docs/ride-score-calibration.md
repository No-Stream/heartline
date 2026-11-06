# Ride score calibration observations

## 2026-11-05 Velocity Viper rebuild

The closed player circuit was measured with `make circuit-report` and a full
train. Both default and construction-test physics completed a lap without
recovery, with excitement 6.70 and 6.69 respectively. The smallest rated
entry-speed margin was 2.14 m/s. The force limits are 5.0 / −1.5 g vertical
and 1.5 g lateral and longitudinal; moving a trim before the roll gives at
least 0.30 g of longitudinal margin. The long return contributes 42.4 seconds
of dead time under default physics.

| Circuit | Excitement | Intensity | Nausea | Vertical max | Vertical min | Lateral peak | Longitudinal peak |
|---|---:|---:|---:|---:|---:|---:|---:|
| Velocity Viper | 6.70 | 6.61 | 4.19 | 3.23 g | −0.66 g | 1.10 g | 1.20 g |

The proposed Briar Blitz rebuild is not installed. A closed candidate using
the longer pullout and the new 8 × 64 m strong floater scored 3.57 under
default physics and 4.10 under construction-test physics. Its 16 m/s turn
had only 1.11 m/s of entry margin. Removing a trim raised excitement but
left the twister above the lateral-force limit. The longer pullout reduced
the candidate's peak vertical force at that transition to 2.61 g by default
and 2.65 g in the construction test. These measurements do not establish
that the full candidate meets the force and entry-speed requirements.

The current scores are provisional. On this scale, 10 should represent a very
high intensity or nausea score, or an excellent excitement score. Scores may
exceed 10. Earlier provisional targets for the four player circuits did not
account for their measured forces. The owner has dropped the ordering target
across those circuits; calibration requires purpose-built reference layouts.

The following idealized profiles are independent design references, not
measured rider ratings or proven constructible circuits. Each describes equal
one-metre samples. These broad excitement targets were selected as scale
anchors before checking the arithmetic: a straight ride at 10 m/s should score
2–3, a moderate ride 3–4, and a very fast ride with a large drop, airtime and
inversions 9–10. Excessive forces should reduce excitement and increase nausea.

| Profile | Excitement | Intensity | Nausea |
| --- | ---: | ---: | ---: |
| Straight at 10 m/s, 1 g vertical | 2.60 | 2.00 | 0.00 |
| 80% straight at 10 m/s; 20% at 20 m/s, 3.5 g vertical and 1 g lateral | 3.16 | 4.06 | 0.00 |
| Same as moderate, with 5 g vertical and 3 g lateral in the faster section | 1.83 | 5.20 | 3.50 |
| 30 m/s, a drop of at least 50 m, three inversions, and 20% airtime | 9.54 | 3.14 | 0.00 |

For the high-excitement profile, the airtime section has −0.1 g vertical and
the rest has +1.6 g; lateral and longitudinal forces are zero throughout. The
formula gives `1 + 0.16*30 + 2.5*0.2 + 2.4*(0.7/7) + 1.5 + 1.5 = 9.54`.
Intensity describes force exposure in this model; it does not directly include
speed, drops or inversions. These idealizations do not imply an actual circuit
can achieve these combinations.

The positive force contribution to excitement saturates per sample, so a brief
extreme force does not earn unlimited credit. Discomfort is not capped: nausea
is ten times the average excess-force exposure, and each nausea point above 5
subtracts 0.35 excitement points until excitement reaches zero. These
thresholds and coefficients are game-design choices, not measured passenger
response or ASTM limits. Distance weighting also differs from time weighting.

On 2026-11-04, the four circuits were run with the construction test's train
configuration: initial distance 0 m, initial speed 2 m/s, and lift speed 5 m/s.
All completed a lap. The former whole-train recorder and scoring formula
produced the following excitement / intensity / nausea scores:

| Circuit | Excitement | Intensity | Nausea |
| --- | ---: | ---: | ---: |
| Demo | 4.46 | 5.44 | 3.44 |
| Velocity Viper | 3.19 | 5.50 | 3.33 |
| Briar Blitz | 3.79 | 7.30 | 7.20 |
| Timber Tumbler | 0.00 | 13.18 | 28.95 |

These historical measurements cannot support calibration. The former construction
recorder independently selected the largest force on each axis from all cars in
a frame, then assigned those values to the front car's distance. The resulting
combined forces and duration of exposure need not describe any actual rider.

One additional hybrid reference was built with `Construction`, including a
chain lift and a vertical loop, and closed with `find_completion` and
`apply_completion`. With the model's full train, initial speed 2 m/s and lift
speed 5 m/s, it completed its 683.30 m lap in 88.60 s. Excitement, intensity,
and nausea were 4.81, 4.65, and 1.66. The worst-car peaks were 5.42 g positive
vertical and 4.76 g lateral; maximum speed was 30.37 m/s. The report flagged
an estimated force-limit penalty and a loop entry speed of 30.1 m/s, above its
24.7–27.5 m/s rated band. This is an over-limit reference, not a comfortable
intense ride. Its repeatable construction and lap test are in
`crates/sim/src/track/construction/completion_tests.rs`.

A flat hybrid oval made from Station, two TurnSmallStandalone pieces, Straight,
and another TurnSmallStandalone was closed by a further TurnSmallStandalone.
With the same initial speed it stalled after 56.5 m, so its partial score is
not a reference rating. Replacing the Straight with LaunchShort produced a
prefix for which the bounded completion search found no connection. These
observations do not establish that a gentle, banked, lift-powered circuit is
impossible. Constructible gentle, moderate, comfortable intense and low-speed
banked references remain to be measured.

Timber Tumbler also has extreme forces in the individual-car measurements:
the recorded train peaks were +11.42 g vertical, −13.43 g vertical and 15.79 g
lateral. Its high discomfort is therefore not explained entirely by combining
cars. The old excitement target is not appropriate for those forces. Velocity
Viper's maximum braking in
this run was 0.56 g.

The revised recorder was measured with the same initial settings and no other
trains. It scores each car independently, averages the car ratings, and reports
the most extreme statistic across cars. The track ribbon and ride advice use
the front car. These are game scores, not measured rider ratings.

| Circuit | Excitement | Intensity | Nausea | Peak vertical g | Minimum vertical g | Peak lateral g | Peak braking g |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Demo | 3.72 | 4.52 | 1.73 | 4.70 | 0.00 | 6.12 | 1.04 |
| Velocity Viper | 3.27 | 4.36 | 1.75 | 3.18 | 0.00 | 3.81 | 0.56 |
| Briar Blitz | 3.13 | 5.65 | 3.91 | 4.42 | −0.14 | 7.78 | 0.99 |
| Timber Tumbler | 0.00 | 9.11 | 15.90 | 11.42 | −13.43 | 15.79 | 8.17 |

All four reports included a note that an estimated force-limit penalty reduced
excitement. Timber Tumbler additionally reported four track pieces outside their
rated speed bands. The owner considers its low score appropriate for the
measured forces and wants the circuit redesigned.

The browser journeys now use broad, qualitative excitement bands on the 0–10
scale. Velocity Viper's 2–5 band reflects its two short launches and 20 m drop,
with no inversion. Briar Blitz's 2–6 band reflects its 36 m drop, one steel loop,
and lateral twister. These bands are independent design expectations rather
than values copied from the measured reports above. They describe excitement
only: a rating inside either band does not establish that a ride is comfortable;
the force and speed statistics still need review. The existing Velocity Viper
report screenshot shows 285.9 g braking; the measurement above is from the
current circuit and is not a reproduction of that screenshot.

The Timber Tumbler circuit has a 102 m descent from its 118 m lift crest. Its
train reaches about 44 m/s before the small airtime hill and tight turns. The
extreme forces above were recorded on individual cars, so choosing a different
car aggregation does not remove the underlying speed and geometry mismatch.
This circuit needs redesign before it can serve as a comfortable player ride.

The revised score includes a game estimate using partial ASTM F2291-06a force
limits, including an estimated combined-axis 200 ms limit. It does not evaluate
the full standard: events under 200 ms, transitions, and restraint differences
are not assessed. This estimate is advisory and is not a safety assessment.

The single-axis breakpoints were checked against figures 6–10 of
[ASTM F2291-06a](https://ia800607.us.archive.org/34/items/gov.law.astm.f2291.2006/astm.f2291.2006.pdf).
The forward longitudinal curve assumes a headrest and backrest. The positive
vertical graph marks 40 seconds; the game extends its final plateau beyond that
point. The combined-axis estimate uses the 2006 edition's 1.1 multiplier.
The game uses 0.1 g force slices through 6 g and 1 g slices through 32 g.
For paired axes, normalized magnitude slices are spaced 0.01 through 2, 0.1
through 5, and 1 through 16. Each slice must persist 200 ms before contributing.
The score uses the largest total among the single-axis and three paired-axis
estimates. These choices approximate the penalty and can miss or undercount
exposure. Single-axis scoring saturates above 32 g and pair scoring above a
normalized magnitude of 16; reported force peaks remain uncapped. Airtime
statistics classify vertical force below −1 g as ejector and −1 to 0 g as floater.

## 2026-11-05 calibration

The circuit scores below were measured before the subsequent fixes to dead-time
exemptions and active-only airtime and inversion scoring. They have not been
remeasured after those fixes and should not be treated as current scores.

The synthetic archetypes were scored from explicit `RideFeatures` and force-band
durations. The family profile specifies 0.5 seconds of laterals; this run assigns
those seconds to the strong lateral band and assumes a 1 g peak. It also assumes
a helix as the third element class, since its 15 m drop does not qualify as a
BigDrop. These are fixture assumptions. The scoring constants and plan bands did
not change for this measurement.

| Synthetic archetype | Excitement score | Plan band | Intensity score | Plan band | Nausea score | Plan band |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Family wooden coaster | 2.96 | 1.5–3.0 | 2.78 | 2.0–3.5 | 0.38 | 0.3–1.5 |
| Classic Arrow looper | 2.85 | 4.0–6.0 **miss** | 7.24 | 6.0–7.5 | 5.53 | 4.5–7.0 |
| B&M hyper | 6.62 | 6.5–8.5 | 6.31 | 5.0–6.5 | 0.53 | 1.0–2.5 **miss** |
| RMC hybrid | 10.67 | 8.5–11.0 | 7.26 | 7.0–8.5 | 3.55 | 3.5–5.5 |
| Intamin accelerator | 5.78 | 5.5–7.0 | 7.39 | 6.5–8.0 | 0.18 | 0.8–2.5 **miss** |

Three plan bands remain unmet. The specified excitement ordering also fails:
the family score exceeds Arrow's. The specified nausea ordering fails because
family nausea exceeds the accelerator's. The test stops at the excitement
ordering assertion, so the band and nausea comparisons above come from its
printed scores. The intensity ordering holds at the displayed precision. The
Arrow's dead time and comfort-envelope penalties lower its excitement; the
hyper and accelerator have low nausea for their specified force exposures.

The model does not use rider-survey data, so these values are calibrated
regression anchors rather than predictions of passenger response.

The game circuits were then measured with the existing player-circuit fixture
construction, the full model train, and each model’s force limits. The player fixtures use the same
construction helper as `crates/sim/tests/known_circuit_statistics.rs`, initial
speed 2 m/s, and lift speed 5 m/s; Demo uses its existing closed circuit and the
hybrid train with the same speeds. Every run completed. The table reports the
average car ratings and the most extreme per-car statistics.

| Circuit | Excitement | Intensity | Nausea | Peak +vertical g | Peak −vertical g | Peak lateral g | Peak speed m/s |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Demo | −1.00 | 8.88 | 6.98 | 4.70 | 0.00 | 6.12 | 27.79 |
| Timber Tumbler | 0.61 | 10.43 | 7.90 | 11.42 | −13.43 | 15.79 | 44.08 |
| Velocity Viper | 0.11 | 7.24 | 3.56 | 3.18 | 0.00 | 3.81 | 21.38 |
| Briar Blitz | −0.45 | 9.83 | 8.21 | 4.42 | −0.14 | 7.78 | 26.73 |

The detailed contributions printed by `calibration_table` show why these
circuits score poorly. Demo has a −2.13 excitement envelope contribution and
−1.35 from excessive forces; Velocity Viper has −1.26 from excessive forces
and −0.72 from the envelope; Briar Blitz has −1.50 from excessive forces and
−2.77 from the envelope. Timber Tumbler scores 0.61 excitement with 10.43
intensity and 7.90 nausea. Its peaks and speed remain far outside the
comfortable range, so I did not tune its score upward.

The three game-circuit excitement estimates from the earlier plan (Demo 2.5–4.5,
Velocity Viper 3–5, Briar Blitz 4–6) are not supported by these measured forces.
The test prints the measured game-circuit scores but does not assert score bands
for these rides. Their prior positive estimates are unmet, and the current
negative or near-zero values are not evidence that the rides meet a
comfortable-ride target. Their layouts need force review before these circuits
can anchor positive player-ride expectations. The synthetic tests retain the original plan bands; measured values have not been frozen as replacement bands.

Three existing assertions in `physics::testing::tests` still fail against this
model: `ratings_use_a_soft_ten_point_scale_without_an_upper_cap`,
`score_reference_profiles_and_extreme_force_saturation`, and
`reference_oval_and_drop_loop_produce_distinct_ratings`. They expect old score
levels from the previous formula: a single extreme sample above 10 intensity,
the straight-profile 2–3 excitement band, and Demo excitement 5–7. The calibrated
archetypes and the current Demo measurement contradict those expectations. I
left their assertions and tolerances unchanged. The new calibration test also
fails the excitement ordering assertion; the printed scores also miss the three plan bands listed above and the nausea ordering. No tests are skipped or thresholds relaxed. Review of these model and design conflicts remains open.

## 2026-11-05 selected scoring weights and approved Blitz target

The selected scoring weights reduce the maximum excitement penalties for
excessive-force band exposure from 1.5 to 0.7 and dead time from 1.5 to 0.5,
and raise the inversion contribution's maximum from 2.0 to 2.2. The ASTM
envelope penalty is unchanged. These choices restore credit for classic
inversions while still penalizing both excessive exposure and sustained
envelope violations. Excitement is floored at zero per car; a
minimum-excitement contribution reconciles each reported breakdown with that
floor. Airtime force bands use the model's existing classifier; all force bands
follow the overlay's displayed thresholds. The lead approved retaining these
weights and setting the synthetic Intamin Blitz excitement band to 9.0–11.5.
The calibration target change is limited to that band; it does not tune scoring
weights to the synthetic result.

The synthetic Blitz reference describes 85 seconds, three launches, ten airtime
moments, a top hat, three distinct heartlined inversions, and 45 m maximum drop.
Like the other archetypes it is a feature profile, not a verified constructible
circuit. Modern nausea targets were removed, rather than inferring nausea from a
coaster name. The table reports the synthetic profiles scored by the current
weights; its target columns match the assertions in `calibration_tests.rs`.

| Synthetic archetype | Excitement | Target | Intensity | Target | Nausea | Target |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Family wooden coaster | 2.96 | 1.5–3.0 | 2.78 | 2.0–3.5 | 0.38 | 0.3–1.5 |
| Classic Arrow looper | 4.05 | 4.0–5.0 | 7.24 | 6.0–7.5 | 5.53 | 4.5–7.0 |
| B&M hyper | 6.62 | 6.0–7.5 | 6.31 | 5.0–6.5 | 0.53 | — |
| RMC hybrid | 10.80 | 9.5–11.0 | 7.26 | 7.0–8.5 | 3.55 | — |
| Intamin accelerator | 5.78 | 5.5–7.0 | 7.39 | 6.5–8.0 | 0.18 | — |
| Intamin Blitz multi-launch | 11.09 | 9.0–11.5 | 7.92 | — | 2.98 | — |

The measured synthetic scores satisfy the selected target bands, including the
lead-approved Blitz band. The test also checks the intended excitement and
intensity ordering. All actual circuits below completed their test laps; their
layouts may differ from the earlier historical measurements above.

| Circuit | Excitement | Intensity | Nausea | Peak +vertical g | Peak −vertical g | Peak lateral g | Peak speed m/s |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Demo | 0.54 | 8.88 | 6.98 | 4.70 | 0.00 | 6.12 | 27.79 |
| Timber Tumbler | 1.48 | 10.43 | 7.90 | 11.42 | −13.43 | 15.79 | 44.08 |
| Velocity Viper | 1.98 | 9.71 | 5.95 | 5.43 | −1.76 | 7.45 | 31.30 |
| Briar Blitz | 0.17 | 9.36 | 6.85 | 4.42 | −0.40 | 7.20 | 26.74 |

These scores do not establish comfortable rides. In particular, the high measured
forces and envelope penalties warrant layout review independently of the score.

The focused calibration table tests passed (2 tests). Their output matched the
synthetic archetype scores and the current Demo and player-circuit measurements
above. The recorder suite now passes all 33 tests after five fixtures were
adjusted to reach their existing assertions. These results cover the focused
calibration and recorder tests only.

## 2026-11-05: Timber Tumbler player circuit

This circuit is installed in the player fixture. The acceptance test checks every car's unrounded entry speed and force readings under both physics configurations. The smallest rated-window margin is 1.785839 m/s for default physics and 2.091314 m/s for construction-test physics; the smallest force margin is respectively 0.341458 g and 0.297805 g. Both runs meet the requested margins of 1.5 m/s and 0.25 g. The reports below are the earlier rounded diagnostic output; the acceptance test supplies the current unrounded measurements. The long return consists mostly of straight pieces.

Candidate piece list:

```text
[Timber Tumbler]
+chain
LevelToGentleUp
GentleToSteepUp
SteepToGentleUp
GentleUpToLevel
-chain
LevelToGentleDown
GentleToSteepDown
SteepToGentleDown
GentleDownToLevel
EjectorHill16x96
EjectorHill16x96
ZeroGRollFastLeft
ZeroGRollFastRight
SpeedTurnLarge24Left
EjectorHill12x80OuterLeft
StallLeft
Brakes
SpeedTurnLarge20Left
FloaterHill12x96
WaveTurnLeft
FloaterHill12x80
Straight
Brakes
Brakes
Straight
TurnLargeRightStandalone
Straight
Straight
Straight
Straight
Straight
Straight
Straight
Straight
Straight
Straight
Straight
Straight
Straight
Straight
Straight
Straight
Straight
Straight
Straight
Straight
OneTileStraight
HalfTileStraight
HalfTileStraight
Straight
OneTileStraight
TurnSmallStandalone
Straight
OneTileStraight
OneTileStraight
TurnSmallStandalone
```

Default physics report:

```text
Timber Tumbler (default physics)
Station exit offset: x +0.0 m, y -16.0 m, z +0.0 m; heading North (station exit North)
# piece kind entry_m/s window_m/s margin_m/s v+_g v-_g lat_g long_g band roll_deg/s notes
0 Station Station 0.01[2.00] — — 1.00 1.00 0.00 0.31 Weak 0.0
1 LevelToGentleUp Lift 2.01[3.00] — — 1.03 0.89 0.00 0.45 Weak 0.0
2 GentleToSteepUp Lift 3.00 — — 0.90 0.71 0.00 0.71 Weak 0.0
3 SteepToGentleUp Lift 3.00 — — 0.89 0.71 0.00 0.71 Weak 0.0
4 GentleUpToLevel Lift 3.00 — — 1.00 0.89 0.00 0.45 Weak 0.0
5 LevelToGentleDown Track 3.00[5.00] — — 1.00 0.53 0.00 0.32 Weak 0.0
6 GentleToSteepDown Track 5.74[12.53] — — 0.89 -0.07 0.00 0.29 Weak 0.0
7 SteepToGentleDown Track 14.73[20.28] — — 2.50 0.71 0.00 0.45 Good 0.0
8 GentleDownToLevel Track 21.69[24.35] — — 3.95 0.89 0.00 1.05 Great 0.0
9 EjectorHill16x96 Track 24.50 18.42..28.34 +3.50 3.24 -0.63 0.00 0.79 Good 0.0
10 EjectorHill16x96 Track 24.30 18.42..28.34 +3.95 3.15 -0.54 0.00 0.71 Good 0.0
11 ZeroGRollFastLeft Track 23.82 15.25..27.00 +3.09 3.27 -0.41 0.48 1.16 Good 158.3
12 ZeroGRollFastRight Track 23.22 15.25..27.00 +3.57 3.18 -0.45 0.49 1.13 Good 154.8
13 SpeedTurnLarge24Left Track 22.79 21.00..27.00 +1.79 3.05 1.00 0.25 0.66 Good 86.3
14 EjectorHill12x80OuterLeft Track 22.73 17.14..26.59 +3.83 3.11 -0.25 0.34 0.86 Good 176.0
15 StallLeft Track 22.17 18.00..27.00 +4.08 2.98 -0.63 0.74 1.14 Good 154.2
16 Brakes Brakes 21.66[19.66] — — 1.00 1.00 0.00 0.32 Weak 0.0
17 SpeedTurnLarge20Left Track 19.52 17.00..23.00 +2.37 2.16 1.00 0.20 0.35 Good 66.2
18 FloaterHill12x96 Track 19.15 16.27..26.75 +2.63 2.27 0.46 0.00 0.59 Good 0.1
19 WaveTurnLeft Track 18.54 8.00..23.00 +4.29 1.49 1.01 0.97 0.34 Weak 107.2
20 FloaterHill12x80 Track 18.30 16.17..23.01 +1.87 2.54 0.27 0.00 0.74 Good 0.0
21 Straight Track 17.75 — — 1.00 1.00 0.00 0.32 Weak 0.0
22 Brakes Brakes 18.04[15.26] — — 1.00 1.00 0.00 0.32 Weak 0.0
23 Brakes Brakes 15.05[11.59] — — 1.00 1.00 0.00 0.31 Weak 0.0
24 Straight Track 11.34 — — 1.00 1.00 0.00 0.01 Weak 0.0
25 TurnLargeRightStandalone Track 11.25 — — 1.00 1.00 0.52 0.01 Weak 0.0
26 Straight Track 10.93 — — 1.00 1.00 0.00 0.01 Weak 0.0
27 Straight Track 10.84 — — 1.00 1.00 0.00 0.01 Weak 0.0
28 Straight Track 10.74 — — 1.00 1.00 0.00 0.01 Weak 0.0
29 Straight Track 10.64 — — 1.00 1.00 0.00 0.01 Weak 0.0
30 Straight Track 10.55 — — 1.00 1.00 0.00 0.01 Weak 0.0
31 Straight Track 10.45 — — 1.00 1.00 0.00 0.01 Weak 0.0
32 Straight Track 10.35 — — 1.00 1.00 0.00 0.01 Weak 0.0
33 Straight Track 10.25 — — 1.00 1.00 0.00 0.01 Weak 0.0
34 Straight Track 10.15 — — 1.00 1.00 0.00 0.01 Weak 0.0
35 Straight Track 10.05 — — 1.00 1.00 0.00 0.01 Weak 0.0
36 Straight Track 9.95 — — 1.00 1.00 0.00 0.01 Weak 0.0
37 Straight Track 9.85 — — 1.00 1.00 0.00 0.01 Weak 0.0
38 Straight Track 9.75 — — 1.00 1.00 0.00 0.01 Weak 0.0
39 Straight Track 9.64 — — 1.00 1.00 0.00 0.01 Weak 0.0
40 Straight Track 9.54 — — 1.00 1.00 0.00 0.01 Weak 0.0
41 Straight Track 9.44 — — 1.00 1.00 0.00 0.01 Weak 0.0
42 Straight Track 9.33 — — 1.00 1.00 0.00 0.01 Weak 0.0
43 Straight Track 9.23 — — 1.00 1.00 0.00 0.01 Weak 0.0
44 Straight Track 9.12 — — 1.00 1.00 0.00 0.01 Weak 0.0
45 Straight Track 9.02 — — 1.00 1.00 0.00 0.01 Weak 0.0
46 OneTileStraight Track 8.91 — — 1.00 1.00 0.00 0.01 Weak 0.0
47 HalfTileStraight Track 8.88 — — 1.00 1.00 0.00 0.01 Weak 0.0
48 HalfTileStraight Track 8.87 — — 1.00 1.00 0.00 0.01 Weak 0.0
49 Straight Track 8.85 — — 1.00 1.00 0.00 0.01 Weak 0.0
50 OneTileStraight Track 8.74 — — 1.00 1.00 0.00 0.01 Weak 0.0
51 TurnSmallStandalone Track 8.72 — — 1.00 1.00 0.63 0.01 Weak 0.0
52 Straight Track 8.54 — — 1.00 1.00 0.00 0.01 Weak 0.0
53 OneTileStraight Track 8.42 — — 1.00 1.00 0.00 0.01 Weak 0.0
54 OneTileStraight Track 8.40 — — 1.00 1.00 0.00 0.01 Weak 0.0
55 TurnSmallStandalone Track 8.37 — — 1.00 1.00 0.58 0.31 Weak 0.0
Completed lap: true; recovered: false
Note: Long slow stretches reduce excitement. Consider shortening them or adding elements.
Note: Ratings average all cars. The track ribbon and proximity events describe the front car; statistics and force advice report the most extreme values across cars.
Score breakdown:
Excitement: 8.13
  airtime (+2.0, 3.5 s)
  airtime moments (+0.6, 7.2)
  positive forces (+0.9, 5.1 s)
  lateral forces (+0.3, 1.1 s)
  longitudinal forces (+0.1, 0.8 s)
  excessive forces (-0.0, 0.0 s)
  inversions (+1.3, 1.9)
  top speed (+0.6, 12.8 m/s)
  biggest drop (+0.4, 32.0 m)
  pace (+0.2, 5.9 m/s)
  launch (+0.0, 0.0 g)
  variety (+0.4, 2.0)
  whip (+0.6, 17.5)
  roll rate (+0.4, 1529.3 degrees/s)
  ride time (+0.7, 112.5 s)
  short ride (-0.0, 112.5 s)
  proximity (+0.0, 0.0)
  theming (+0.0, 0.0)
  dead time (-0.5, 42.0 s at Standalone large right turn)
  forces above the ASTM comfort envelope (-0.0, 0.0 g-s)
  nausea (-0.0, 4.0)
  minimum excitement (+0.0, 0.0)
Intensity: 6.41
  peak force (+2.4, 0.6 at End gentle descent)
  sustained force (+2.5, 10.6 s)
  excessive forces (+0.0, 0.0 s)
  top speed (+0.7, 14.8 m/s)
  biggest drop (+0.5, 32.0 m)
  launch (+0.0, 0.0 g)
  inversions (+0.4, 3.0)
  forces above the ASTM comfort envelope (+0.0, 0.0 g-s)
Nausea: 4.04
  lateral forces (+0.8, 1.1 s)
  inversions (+0.7, 1.8)
  fast rolls (+1.5, 9.5 s)
  reversals (+0.0, 0.0)
  strong positive forces (+0.7, 5.1 s)
  time inverted (+0.4, 5.8 s)
  excessive forces (+0.0, 0.0 s)
  forces above the ASTM comfort envelope (+0.0, 0.0 g-s)
```

Construction-test physics report:

```text
Timber Tumbler (construction-test physics)
Station exit offset: x +0.0 m, y -16.0 m, z +0.0 m; heading North (station exit North)
# piece kind entry_m/s window_m/s margin_m/s v+_g v-_g lat_g long_g band roll_deg/s notes
0 Station Station 2.00 — — 1.00 1.00 0.00 0.31 Weak 0.0
1 LevelToGentleUp Lift 2.01[5.00] — — 1.10 0.89 0.00 0.46 Weak 0.0
2 GentleToSteepUp Lift 5.00 — — 0.93 0.71 0.00 0.71 Weak 0.0
3 SteepToGentleUp Lift 5.00 — — 0.89 0.69 0.00 0.71 Weak 0.0
4 GentleUpToLevel Lift 5.00 — — 1.00 0.83 0.00 0.46 Weak 0.0
5 LevelToGentleDown Track 5.00[6.40] — — 1.00 0.46 0.00 0.34 Weak 0.0
6 GentleToSteepDown Track 7.00[13.17] — — 0.89 -0.12 0.00 0.31 Weak 0.0
7 SteepToGentleDown Track 15.25[20.67] — — 2.55 0.71 0.00 0.47 Good 0.0
8 GentleDownToLevel Track 22.07[24.67] — — 4.03 0.89 0.00 1.07 Great 0.0
9 EjectorHill16x96 Track 24.82 18.42..28.34 +3.19 3.30 -0.69 0.00 0.74 Good 0.0
10 EjectorHill16x96 Track 24.61 18.42..28.34 +3.64 3.21 -0.60 0.00 0.80 Good 0.0
11 ZeroGRollFastLeft Track 24.13 15.25..27.00 +2.79 3.33 -0.39 0.47 1.20 Good 160.9
12 ZeroGRollFastRight Track 23.53 15.25..27.00 +3.26 3.24 -0.42 0.48 1.15 Good 157.3
13 SpeedTurnLarge24Left Track 23.09 21.00..27.00 +2.09 3.13 1.00 0.23 0.68 Good 87.4
14 EjectorHill12x80OuterLeft Track 23.04 17.14..26.59 +3.53 3.16 -0.28 0.38 0.88 Good 179.2
15 StallLeft Track 22.48 18.00..27.00 +4.36 3.03 -0.61 0.73 1.15 Good 156.6
16 Brakes Brakes 21.96[20.00] — — 1.00 1.00 0.00 0.32 Weak 0.0
17 SpeedTurnLarge20Left Track 19.86 17.00..23.00 +2.71 2.22 1.00 0.17 0.36 Good 67.3
18 FloaterHill12x96 Track 19.50 16.27..26.75 +2.97 2.31 0.43 0.00 0.60 Good 0.1
19 WaveTurnLeft Track 18.88 8.00..23.00 +3.95 1.50 1.01 0.97 0.35 Weak 109.2
20 FloaterHill12x80 Track 18.64 16.17..23.01 +2.22 2.60 0.23 0.00 0.76 Good 0.0
21 Straight Track 18.10 — — 1.00 1.00 0.00 0.32 Weak 0.0
22 Brakes Brakes 18.38[15.66] — — 1.00 1.00 0.00 0.32 Weak 0.0
23 Brakes Brakes 15.46[12.13] — — 1.00 1.00 0.00 0.31 Weak 0.0
24 Straight Track 11.88 — — 1.00 1.00 0.00 0.01 Weak 0.0
25 TurnLargeRightStandalone Track 11.79 — — 1.00 1.00 0.57 0.01 Weak 0.0
26 Straight Track 11.48 — — 1.00 1.00 0.00 0.01 Weak 0.0
27 Straight Track 11.38 — — 1.00 1.00 0.00 0.01 Weak 0.0
28 Straight Track 11.29 — — 1.00 1.00 0.00 0.01 Weak 0.0
29 Straight Track 11.20 — — 1.00 1.00 0.00 0.01 Weak 0.0
30 Straight Track 11.10 — — 1.00 1.00 0.00 0.01 Weak 0.0
31 Straight Track 11.00 — — 1.00 1.00 0.00 0.01 Weak 0.0
32 Straight Track 10.91 — — 1.00 1.00 0.00 0.01 Weak 0.0
33 Straight Track 10.81 — — 1.00 1.00 0.00 0.01 Weak 0.0
34 Straight Track 10.72 — — 1.00 1.00 0.00 0.01 Weak 0.0
35 Straight Track 10.62 — — 1.00 1.00 0.00 0.01 Weak 0.0
36 Straight Track 10.52 — — 1.00 1.00 0.00 0.01 Weak 0.0
37 Straight Track 10.42 — — 1.00 1.00 0.00 0.01 Weak 0.0
38 Straight Track 10.32 — — 1.00 1.00 0.00 0.01 Weak 0.0
39 Straight Track 10.22 — — 1.00 1.00 0.00 0.01 Weak 0.0
40 Straight Track 10.12 — — 1.00 1.00 0.00 0.01 Weak 0.0
41 Straight Track 10.02 — — 1.00 1.00 0.00 0.01 Weak 0.0
42 Straight Track 9.92 — — 1.00 1.00 0.00 0.01 Weak 0.0
43 Straight Track 9.82 — — 1.00 1.00 0.00 0.01 Weak 0.0
44 Straight Track 9.72 — — 1.00 1.00 0.00 0.01 Weak 0.0
45 Straight Track 9.62 — — 1.00 1.00 0.00 0.01 Weak 0.0
46 OneTileStraight Track 9.51 — — 1.00 1.00 0.00 0.01 Weak 0.0
47 HalfTileStraight Track 9.49 — — 1.00 1.00 0.00 0.01 Weak 0.0
48 HalfTileStraight Track 9.47 — — 1.00 1.00 0.00 0.01 Weak 0.0
49 Straight Track 9.46 — — 1.00 1.00 0.00 0.01 Weak 0.0
50 OneTileStraight Track 9.36 — — 1.00 1.00 0.00 0.01 Weak 0.0
51 TurnSmallStandalone Track 9.33 — — 1.00 1.00 0.72 0.01 Weak 0.0
52 Straight Track 9.16 — — 1.00 1.00 0.00 0.01 Weak 0.0
53 OneTileStraight Track 9.05 — — 1.00 1.00 0.00 0.01 Weak 0.0
54 OneTileStraight Track 9.02 — — 1.00 1.00 0.00 0.01 Weak 0.0
55 TurnSmallStandalone Track 9.00 — — 1.00 1.00 0.67 0.31 Weak 0.0
Completed lap: true; recovered: false
Note: Long slow stretches reduce excitement. Consider shortening them or adding elements.
Note: Ratings average all cars. The track ribbon and proximity events describe the front car; statistics and force advice report the most extreme values across cars.
Score breakdown:
Excitement: 8.21
  airtime (+2.0, 3.5 s)
  airtime moments (+0.6, 7.2)
  positive forces (+0.9, 5.2 s)
  lateral forces (+0.3, 1.2 s)
  longitudinal forces (+0.1, 0.9 s)
  excessive forces (-0.0, 0.0 s)
  inversions (+1.3, 1.9)
  top speed (+0.6, 13.2 m/s)
  biggest drop (+0.4, 32.0 m)
  pace (+0.2, 6.3 m/s)
  launch (+0.0, 0.0 g)
  variety (+0.4, 2.0)
  whip (+0.6, 20.2)
  roll rate (+0.4, 1605.6 degrees/s)
  ride time (+0.7, 109.2 s)
  short ride (-0.0, 109.2 s)
  proximity (+0.0, 0.0)
  theming (+0.0, 0.0)
  dead time (-0.5, 38.1 s at Standalone large right turn)
  forces above the ASTM comfort envelope (-0.0, 0.0 g-s)
  nausea (-0.0, 4.1)
  minimum excitement (+0.0, 0.0)
Intensity: 6.46
  peak force (+2.4, 0.7 at End gentle descent)
  sustained force (+2.5, 10.8 s)
  excessive forces (+0.0, 0.0 s)
  top speed (+0.7, 15.2 m/s)
  biggest drop (+0.5, 32.0 m)
  launch (+0.0, 0.0 g)
  inversions (+0.4, 3.0)
  forces above the ASTM comfort envelope (+0.0, 0.0 g-s)
Nausea: 4.07
  lateral forces (+0.8, 1.2 s)
  inversions (+0.7, 1.8)
  fast rolls (+1.5, 9.4 s)
  reversals (+0.0, 0.0)
  strong positive forces (+0.7, 5.2 s)
  time inverted (+0.4, 5.7 s)
  excessive forces (+0.0, 0.0 s)
  forces above the ASTM comfort envelope (+0.0, 0.0 g-s)
```

## 2026-11-06: Demo coaster

The demo coaster in the default park is now the fixed piece list in `crates/sim/src/physics/demo_circuit.txt`, measured with `make circuit-report CIRCUIT=Demo FILE=crates/sim/src/physics/demo_circuit.txt` (the report builds it piece by piece in the default park's world from its station at (450, 500, 10)). The chain lift climbs 32 m eastward from the station, high over the area west of x = 980 m where the browser tests build their player coasters. A 24 m gentle drop reaches about 21.5 m/s, followed by five ejector hills and three left turns banked for 20 m/s, all east and north of that area, with no inversions. The return runs south over the park path, under the lift (the self-crossing) and around the south of the pond; a four-piece brake run trims the train to brake speed and four 2 m descents with two more brake pieces bring it down to the station at walking pace through the small return turns. The game runs the demo with construction-test physics (lift 5 m/s, initial 2 m/s), where excitement is 5.10; under default physics it is 4.78. The smallest rated-window entry margin is 1.77 m/s (construction test, ejector hill 12 × 64 m) and 1.73 m/s (default physics, the last banked turn). Peak forces are 3.32 g vertical in the drop pullout and hill valleys, −0.80 g on the 12 × 64 m ejector hill, 0.38 g lateral in the first slow return turn and 1.10 g longitudinal, all at least 0.25 g inside the hybrid model's 4.5 / −1.5 / 1.5 / 1.5 g limits.

| Circuit | Physics | Excitement | Intensity | Nausea | Vertical max | Vertical min | Lateral peak | Longitudinal peak |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| Demo | construction test | 5.10 | 5.20 | 0.64 | 3.32 g | −0.80 g | 0.38 g | 1.10 g |
| Demo | default | 4.78 | 5.04 | 0.61 | 3.24 g | −0.71 g | 0.38 g | 1.08 g |

The automatic block search does not find a two-train layout for this demo within 60 million work steps (it does for the earlier demo shape, which the block tests keep as their own fixture). A three-section layout with a stopping run ending at the lift crest, the mid-course brake run and the final brakes verified for two trains on the previous version of this layout; the search's bootstrap does not try it.

Construction-test report:

```text
Demo (construction-test physics)
Station exit offset: x -16.0 m, y +0.0 m, z +0.0 m; heading East (station exit East)
# piece kind entry_m/s window_m/s margin_m/s v+_g v-_g lat_g long_g band roll_deg/s notes
0 Station Station 2.00 — — 1.00 1.00 0.00 0.31 Weak 0.0
1 LevelToGentleUp Lift 2.01[5.00] — — 1.10 0.89 0.00 0.46 Weak 0.0
2 GentleUp Lift 5.00 — — 0.89 0.89 0.00 0.45 Weak 0.0
3 GentleUp Lift 5.00 — — 0.89 0.89 0.00 0.45 Weak 0.0
4 GentleUp Lift 5.00 — — 0.89 0.89 0.00 0.45 Weak 0.0
5 GentleUpToLevel Lift 5.00 — — 1.00 0.83 0.00 0.46 Weak 0.0
6 LevelToGentleDown Track 5.00[6.39] — — 1.00 0.46 0.00 0.34 Weak 0.0
7 GentleDown Track 7.00[12.76] — — 0.89 0.89 0.00 0.19 Weak 0.0
8 GentleDown Track 13.68[17.79] — — 0.89 0.89 0.00 0.20 Weak 0.0
9 GentleDownToLevel Track 18.48[21.30] — — 3.28 0.89 0.00 0.86 Good 0.0
10 EjectorHill8x64 Track 21.47 13.49..25.22 +3.39 3.14 -0.72 0.00 0.99 Good 0.0
11 EjectorHill12x80 Track 21.27 16.14..26.59 +5.04 2.89 -0.38 0.00 0.77 Good 0.0
12 SpeedTurnLarge20Left Track 20.99 17.00..23.00 +1.83 2.49 1.00 0.06 0.42 Good 71.8
13 EjectorHill8x64 Track 20.88 13.49..25.22 +4.32 2.95 -0.54 0.00 0.91 Good 0.1
14 Straight Track 20.25 — — 1.00 1.00 0.00 0.17 Weak 0.0
15 Straight Track 20.54 — — 1.00 1.00 0.00 0.04 Weak 0.0
16 SpeedTurnLarge20Left Track 20.46 17.00..23.00 +2.54 2.34 1.00 0.12 0.38 Good 69.3
17 EjectorHill12x64 Track 20.10 15.94..21.88 +1.77 3.32 -0.80 0.00 1.10 Good 0.2
18 EjectorHill8x64 Track 19.36 13.49..25.22 +5.48 2.75 -0.35 0.00 0.85 Good 0.0
19 Straight Track 19.13 — — 1.00 1.00 0.00 0.17 Weak 0.0
20 Straight Track 19.45 — — 1.00 1.00 0.00 0.01 Weak 0.0
21 Straight Track 19.36 — — 1.00 1.00 0.00 0.02 Weak 0.0
22 OneTileStraight Track 19.28 — — 1.00 1.00 0.00 0.03 Weak 0.0
23 OneTileStraight Track 19.26 — — 1.00 1.00 0.00 0.04 Weak 0.0
24 SpeedTurnLarge20Left Track 19.24 17.00..23.00 +2.09 2.11 1.00 0.22 0.34 Good 65.2
25 Brakes Brakes 18.86[16.31] — — 1.00 1.00 0.00 0.31 Weak 0.0
26 Brakes Brakes 16.12[12.95] — — 1.00 1.00 0.00 0.31 Weak 0.0
27 Brakes Brakes 12.70[8.34] — — 1.00 1.00 0.00 0.31 Weak 0.0
28 Brakes Brakes 7.97[3.91] — — 1.00 1.00 0.00 0.31 Weak 0.0
29 TerrainFall Track 3.89[5.65] — — 1.32 0.73 0.00 0.28 Weak 0.0
30 TerrainFall Track 5.87[8.33] — — 1.37 0.54 0.00 0.66 Weak 0.0
31 Brakes Brakes 8.46[3.98] — — 1.00 1.00 0.00 0.30 Weak 0.0
32 TerrainFall Track 3.97[5.70] — — 1.27 0.73 0.00 0.34 Weak 0.0
33 Straight Track 5.91[7.20] — — 1.00 1.00 0.00 0.11 Weak 0.0
34 Straight Track 7.18 — — 1.00 1.00 0.00 0.01 Weak 0.0
35 Straight Track 7.05 — — 1.00 1.00 0.00 0.01 Weak 0.0
36 Straight Track 6.92 — — 1.00 1.00 0.00 0.01 Weak 0.0
37 TurnSmallRightStandalone Track 6.79 — — 1.00 1.00 0.38 0.01 Weak 0.0
38 Straight Track 6.57 — — 1.00 1.00 0.00 0.01 Weak 0.0
39 Straight Track 6.44 — — 1.00 1.00 0.00 0.01 Weak 0.0
40 Straight Track 6.30 — — 1.00 1.00 0.00 0.01 Weak 0.0
41 Straight Track 6.16 — — 1.00 1.00 0.00 0.01 Weak 0.0
42 Straight Track 6.02 — — 1.00 1.00 0.00 0.01 Weak 0.0
43 Straight Track 5.87 — — 1.00 1.00 0.00 0.01 Weak 0.0
44 Straight Track 5.73 — — 1.00 1.00 0.00 0.01 Weak 0.0
45 TurnSmallRightStandalone Track 5.57 — — 1.00 1.00 0.25 0.01 Weak 0.0
46 Straight Track 5.32 — — 1.00 1.00 0.00 0.01 Weak 0.0
47 Straight Track 5.15 — — 1.00 1.00 0.00 0.01 Weak 0.0
48 Straight Track 4.99 — — 1.00 1.00 0.00 0.01 Weak 0.0
49 TurnSmallRightStandalone Track 4.82 — — 1.00 1.00 0.19 0.11 Weak 0.0
50 TerrainFall Track 4.52[6.10] — — 1.20 0.74 0.00 0.57 Weak 0.0
51 Brakes Brakes 6.28[3.98] — — 1.00 1.00 0.00 0.31 Weak 0.0
52 OneTileStraight Track 3.97 — — 1.00 1.00 0.00 0.31 Weak 0.0
53 OneTileStraight Track 3.91 — — 1.00 1.00 0.00 0.31 Weak 0.0
Completed lap: true; recovered: false
Note: Long slow stretches reduce excitement. Consider shortening them or adding elements.
Note: Ratings average all cars. The track ribbon and proximity events describe the front car; statistics and force advice report the most extreme values across cars.
Score breakdown:
Excitement: 5.10
  airtime (+1.4, 1.9 s)
  airtime moments (+0.5, 5.0)
  positive forces (+0.9, 4.0 s)
  lateral forces (+0.0, 0.0 s)
  longitudinal forces (+0.1, 0.6 s)
  excessive forces (-0.0, 0.0 s)
  inversions (+0.0, -0.0)
  top speed (+0.5, 9.8 m/s)
  biggest drop (+0.4, 32.0 m)
  pace (+0.1, 1.5 m/s)
  launch (+0.0, 0.0 g)
  variety (+0.4, 2.0)
  whip (+0.5, 8.7)
  roll rate (+0.1, 11.1 degrees/s)
  ride time (+0.7, 121.2 s)
  short ride (-0.0, 121.2 s)
  proximity (+0.0, 0.0)
  theming (+0.0, 0.0)
  dead time (-0.4, 48.0 s at Large left turn banked for 20 m/s)
  forces above the ASTM comfort envelope (-0.0, 0.0 g-s)
  nausea (-0.0, 0.6)
  minimum excitement (+0.0, 0.0)
Intensity: 5.20
  peak force (+2.2, 0.5 at Ejector hill 12 × 64 m)
  sustained force (+2.0, 6.6 s)
  excessive forces (+0.0, 0.0 s)
  top speed (+0.6, 11.8 m/s)
  biggest drop (+0.5, 32.0 m)
  launch (+0.0, 0.0 g)
  inversions (+0.0, 0.0)
  forces above the ASTM comfort envelope (+0.0, 0.0 g-s)
Nausea: 0.64
  lateral forces (+0.0, 0.0 s)
  inversions (+0.0, -0.0)
  fast rolls (+0.0, 0.0 s)
  reversals (+0.0, 0.0)
  strong positive forces (+0.6, 4.0 s)
  time inverted (+0.0, -0.0 s)
  excessive forces (+0.0, 0.0 s)
  forces above the ASTM comfort envelope (+0.0, 0.0 g-s)
```

## 2026-11-06 positive vertical force band change

Positive vertical force is now rated in absolute g instead of as a share of
each model's limit. It is weak up to 2 g, good below the model's "great from"
value, great from that value up to the model's comfortable limit, too much up
to 1.3 times the limit, and dangerous beyond. "Great from" is 3.0 g on the
steel-track models (Timber Tumbler, limit 4.5 g; Velocity Viper, limit 5.0 g)
and 2.5 g on the wooden Briar Blitz (limit 4.0 g). Before, great started at
80% of the limit: 3.6 g, 4.0 g and 3.2 g respectively. Airtime, lateral and
longitudinal bands are unchanged.

Scores are excitement / intensity / nausea, as printed by the calibration
tests before and after the change. No scoring weights were adjusted.

| Synthetic archetype | Before | After |
| --- | --- | --- |
| Family wooden coaster | 2.96 / 2.78 / 0.38 | 2.96 / 2.78 / 0.38 |
| Classic Arrow looper | 4.05 / 7.24 / 5.53 | 4.05 / 7.24 / 5.53 |
| B&M hyper | 6.62 / 6.31 / 0.53 | 6.62 / 6.31 / 0.53 |
| RMC hybrid | 10.80 / 7.26 / 3.55 | 10.80 / 7.26 / 3.55 |
| Intamin accelerator | 5.78 / 7.39 / 0.18 | 5.78 / 7.39 / 0.18 |
| Intamin Blitz multi-launch | 11.09 / 7.92 / 2.98 | 11.09 / 7.92 / 2.98 |

The synthetic archetypes supply their force-band durations directly, and none
of them has any time in the good positive vertical band, so the change cannot
move them. To check whether more great pullout time could disturb the ordering,
a temporary run added 1, 2 and 4 seconds of great positive vertical force to
every archetype except the family coaster. With 4 extra seconds the scores were
Arrow 4.18, B&M hyper 6.81, RMC 10.99, Intamin accelerator 6.36 and Intamin
Blitz 11.28 excitement; the family coaster stays at 2.96. Positive force credit
saturates, so the owner's ordering holds with a wide margin: Intamin Blitz and
RMC far above the Arrow looper, clearly above the B&M hyper, family lowest.

The measured circuits do move, because their force histories are reclassified.
These are the rebuilt Timber Tumbler and Velocity Viper circuits on master
7f4502d, before and after the change:

| Measured circuit | Before | After |
| --- | --- | --- |
| Demo | 0.54 / 8.88 / 6.98 | 0.58 / 8.97 / 7.05 |
| Timber Tumbler | 8.21 / 6.46 / 4.07 | 8.24 / 6.56 / 4.14 |
| Velocity Viper | 6.69 / 6.61 / 4.19 | 6.73 / 6.67 / 4.24 |
| Briar Blitz | 0.18 / 9.36 / 6.85 | 0.24 / 9.42 / 6.92 |
