# Ride score calibration observations

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
is ten times the average excess-force exposure, and each nausea point subtracts
0.45 excitement points until excitement reaches zero. These thresholds and
coefficients are game-design choices, not measured passenger response or ASTM
limits. Distance weighting also differs from time weighting.

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
