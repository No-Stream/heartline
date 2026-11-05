# TASK-126 de-risking measurements

Measurements use the 0.5 g, 0.2 s allowance and frictionless isolated-piece ratings. They do not change the catalog or the production preview.

## Isolated pieces

The following are passing entry speeds at 1 m/s resolution over 1–100 m/s, intersected across allowed models and all car counts. The downhill approach makes 1–13 m/s unreachable for every configuration of `SixtyToSteepDown`; these speeds are unknown, not failures of the piece. The requested solver convergence over every target is therefore not met. An unpowered train descending the required 10 m approach gains speed even from rest. The descending half-corkscrew limit checks the existing 22 m/s speed after clearing the piece, not entry speed. No gaps were found in the measured passing sets.

| Piece | Passing entry speeds (m/s) |
| --- | ---: |
| SixtyToSteepDown | 14–29; 1–13 unreachable |
| HalfCorkscrewLeftDown | 1–15 |
| HalfCorkscrewRightDown | 1–15 |
| Loop | 25–29 |
| FloaterHill6x64 | 11–28 |

These preliminary ranges differ from the plan's approximate loop and half-corkscrew expectations. They are measurements at whole metres per second, not refined catalog endpoints.

The solver converged for 783 configuration and speed combinations for `SixtyToSteepDown`, with 117 unreachable; 900 for each descending half corkscrew and the loop; and 1,800 for the floater hill. All reported convergences met the 1e-6 m/s target.

## Preview accuracy and latency

The instrumented preview evaluates the front car at every integration step. Filtered forces are interpolated at common metre marks. Violation flags are ORed over steps ending within each metre. A 50 ms step can cross an entire metre; bins with no reading at 50 ms are excluded from the disagreement counts and are not treated as agreements. Timber's preview stalls, so its comparison covers only the first 197 metres.

| Circuit | Maximum difference vertical / lateral / longitudinal (g) | Matched metres | Disagreements: preview only / 5 ms only | Metres without a 50 ms reading | Within 3f accuracy bounds? |
| --- | --- | ---: | --- | ---: | --- |
| Timber Tumbler | 0.0021 / 0.0010 / 0.0535 | 197 | 1 / 24 | 0 | No |
| Velocity Viper | 0.4486 / 0.7142 / 0.2136 | 727 | 1 / 3 | 237 | No |
| Briar Blitz | 0.2840 / 0.7051 / 0.1355 | 994 | 0 / 3 | 165 | No |

All three exceed the allowed one flag disagreement, and some disagreements are missed warnings. Velocity and Briar also exceed the 0.1 g force bound. The missing metre bins require an explicit aggregation policy. The plan's proposed fallback is a 25 ms preview; it has not been measured here.

`make bench-player-circuits` measured baseline p95 of 27.834 ms for Timber, 23.217 ms for Velocity and 79.918 ms for Briar. In a separately instrumented preview, three baseline-to-evaluated p95 pairs in milliseconds were 18.483→17.102, 52.230→60.577 and 84.316→65.484 for Timber; 10.110→8.371, 28.049→37.572 and 40.071→50.188 for Velocity; and 35.844→36.867, 111.453→120.873 and 104.816→128.292 for Briar. Each used five warmups and 100 runs. Variation makes the 20% latency comparison inconclusive; the instrumentation does not include future per-metre flag storage.

Reproduce the comparison with `tools/force-measurement/run-preview.sh`; `--flags-only` omits timings. It instruments a temporary archive of master and leaves production code unchanged.

## Changed piece flags

The full-train comparison uses existing scoring eligibility (speed above 1 m/s on track, launches and brakes) for both rules; the evaluator remains continuous through the entire run. Indices are zero-based. A straight can be flagged due to filter history from the preceding piece.

| Circuit | Newly flagged under evaluator | No longer flagged under evaluator |
| --- | --- | --- |
| Timber Tumbler | None | None |
| Velocity Viper | 15 Straight, 43 Straight | 9 LevelToGentleDown, 12 GentleDownToLevel |
| Briar Blitz | 35 Straight | 11 UnbankLeft, 12 GentleDownToLevel |

Timber's test run was incomplete and reached 205.07 m, so its result does not describe the full circuit.
