# TASK-126 de-risking measurements

Measurements use the 0.5 g, 0.2 s allowance and frictionless isolated-piece ratings. They do not change the catalog or the production preview.

## Isolated pieces

The scan holds speed constant on the matching-slope approach, evaluates each car's force history there, and releases the train at the piece entry. It tests whole-metre-per-second entry speeds from 1 to 100 m/s, intersecting all allowed models and car counts. Every target was reachable and the passing sets had no gaps. These are measured passing speeds, not refined catalog endpoints; the existing energy-derived or hand-set minimum remains in force even when a lower scan speed passes.

| Piece | Passing entry speeds (m/s) | Existing catalog minimum (m/s) |
| --- | ---: | ---: |
| SixtyToSteepDown | 1–29 | 2 |
| HalfCorkscrewLeftDown | 1–15 | 5 |
| HalfCorkscrewRightDown | 1–15 | 5 |
| Loop | 25–29 | 24.7311 |
| FloaterHill6x64 | 11–28 | 12.1199 |

The scan evaluated 900 model, car-count and speed combinations for each piece except the hill, which had 1,800. At 30 m/s the descending transition exceeds the vertical limit; at 16 m/s the descending half corkscrews exceed the existing 22 m/s clearing-speed rule. The 60° approach is slightly curved: sampling 1,001 positions found tangent vertical components from −0.870854 to −0.866025 and maximum curvature of 0.002075 m⁻¹. At a fixed 25 m/s, its rider vertical force varies from 0.363 to 0.628 g, so the filter has this real approach history rather than a perfectly constant equilibrium. Integer-grid connectors cannot represent an exact straight at 60°. The four other approaches are level and have zero curvature.

## Preview accuracy and latency

The comparison runs the front-car evaluator continuously and linearly interpolates filtered forces at matching whole-metre distances. It keeps the first outward passage through each mark when a train rolls back. For a flag at a mark, it conservatively ORs the two step flags bracketing that mark; this is a measurement convention, not yet a production aggregation policy. The 237 and 165 missing metre bins in the earlier Velocity and Briar comparison were binning artifacts: 50 ms integration steps crossed entire bins, and interpolation covers the intervening distance. They were not gaps in the train's trajectory.

| Circuit | Step | Maximum difference vertical / lateral / longitudinal (g) | Common metres | Flag disagreements: preview only / 5 ms only | Within accuracy bounds? |
| --- | ---: | --- | ---: | --- | --- |
| Timber Tumbler | 50 ms | 0.00211 / 0.00099 / 0.05345 | 197 | 0 / 0 | Only the reached portion |
| Timber Tumbler | 25 ms | 0.00061 / 0.00027 / 0.02095 | 197 | 0 / 0 | Only the reached portion |
| Velocity Viper | 50 ms | 0.44863 / 0.71416 / 0.21364 | 964 | 6 / 1 | No |
| Velocity Viper | 25 ms | 0.10468 / 0.16520 / 0.05990 | 965 | 2 / 0 | No |
| Briar Blitz | 50 ms | 0.28398 / 0.70514 / 0.13553 | 1159 | 8 / 0 | No |
| Briar Blitz | 25 ms | 0.06754 / 0.16974 / 0.05748 | 1159 | 2 / 0 | No |

The plan requires at most 0.1 g on each axis and at most one disagreement per circuit, and any disagreement must be an over-warning. Neither step meets those bounds. The former Timber flag discrepancy mixed the return trip with outward readings; the corrected first-passage comparison has no disagreement over the reached portion.

Timber does not complete at either resolution: at 5 ms it leaves the final lift piece at 202.677 m and 2.989 m/s, reaches 205.066 m, rolls back and is recovered at the station. The preview and the normal test run share this behavior. The chain is present on the intended nine pieces; lift drive stops when the front car leaves the last one while rear cars remain on the uphill track. A disposable experiment keeping drive active while any rear car remains on a lift completed the circuit, but also materially changed its forces. The code and existing test do not establish whether that lift engagement rule is intended, so the experiment is not a validated production fix. Full-circuit Timber accuracy remains unmeasured and the lift behavior needs an owner decision.

Latency was measured with three interleaved runs of `make bench-player-circuits` in disposable archives of `bc3a68e` and the instrumented evaluator, with five warmups and 100 measured previews per run. The current master (`6f974f8`) differs from that revision only in backlog notes. Each cell is the median of the three p95 values, in milliseconds. The evaluator runs at every step and supplies filtered forces to the recorder; this prototype does not yet store per-metre violation flags, so these are not final task 5 timings.

| Circuit | Master 50 ms | Evaluator 50 ms | Change | Evaluator 25 ms | Change |
| --- | ---: | ---: | ---: | ---: | ---: |
| Timber Tumbler | 21.529 | 19.363 | −10.1% | 32.168 | +49.4% |
| Velocity Viper | 9.154 | 11.074 | +21.0% | 17.183 | +87.7% |
| Briar Blitz | 33.139 | 37.431 | +13.0% | 61.438 | +85.4% |

The 50 ms prototype narrowly exceeds the 20% latency bound on Velocity; all 25 ms results exceed it. The individual master p95s were 21.529/15.927/23.274 ms for Timber, 9.154/8.773/10.232 ms for Velocity and 33.139/32.541/38.981 ms for Briar. The evaluator p95s were 19.363/16.355/26.206, 8.663/11.908/11.074 and 34.501/37.431/37.588 ms at 50 ms, and 33.479/32.168/31.801, 13.521/17.183/20.296 and 59.278/63.924/61.438 ms at 25 ms, in the same circuit order. Variation makes the near-boundary 50 ms Velocity result uncertain. Neither step satisfies the accuracy requirements, so no step is selected.

Reproduce the comparison with `tools/force-measurement/run-preview.sh --comparison-only`. The runner instruments a temporary archive of master and leaves production code unchanged.

## Changed piece flags

The full-train comparison uses existing scoring eligibility (speed above 1 m/s on track, launches and brakes) for both rules; the evaluator remains continuous through the entire run. Indices are zero-based. A straight can be flagged due to filter history from the preceding piece.

| Circuit | Newly flagged under evaluator | No longer flagged under evaluator |
| --- | --- | --- |
| Timber Tumbler | None | None |
| Velocity Viper | 15 Straight, 43 Straight | 9 LevelToGentleDown, 12 GentleDownToLevel |
| Briar Blitz | 35 Straight | 11 UnbankLeft, 12 GentleDownToLevel |

Timber's test run was incomplete and reached 205.07 m, so its result does not describe the full circuit.
