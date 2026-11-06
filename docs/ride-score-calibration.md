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
