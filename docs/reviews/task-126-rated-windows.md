# TASK-126 rated-window scan

## Status

The full coarse scan completed for all 94 templates, and boundary refinement emitted candidate endpoints for all 94. Rated-window values remain unchanged because the generation run aborted after 11 ejector train configurations failed the Great-at-maximum airtime requirement. The candidate endpoints below are diagnostic measurements, not accepted catalog windows. This write-up also records the verified circuit flag comparison and the earlier 28 failing endpoint combinations.

The scan uses the isolated-piece harness at a 5 ms integration step. For each catalog entry, it checks all allowed models and car counts over 0.25 m/s samples, through `max(100 m/s, 3 × current maximum)`, and compares the existing minimum and maximum directly. The coarse scan reported no internal gaps on its 0.25 m/s grid. Boundary refinement used seven bisections and inward 0.01 m/s rounding for force-bound edges; retained legacy minima and design-cap maxima keep their old precision. The scan rechecked candidate endpoints with the force evaluator, but its separate Great-at-maximum checks failed for 11 ejector configurations, so no generated table was written. The axis shown for an upper-bound diagnostic is from the first failing 0.25 m/s grid point after the last coarse passing sample; it is not the binding axis at the refined candidate maximum.

## Player-circuit flags

The task 1 full-train comparison ran the existing scoring eligibility rule on each circuit, then compared raw force bands with the evaluator. Indices are zero-based. A straight can inherit filter history from the preceding piece.

| Circuit | Newly flagged with evaluator | No longer flagged with evaluator | Coverage |
| --- | --- | --- | --- |
| Timber Tumbler | None | None | Incomplete: the run reached 205.07 m and did not complete the circuit. |
| Velocity Viper | 15 Straight; 43 Straight | 9 LevelToGentleDown; 12 GentleDownToLevel | Full circuit. |
| Briar Blitz | 35 Straight | 11 UnbankLeft; 12 GentleDownToLevel | Full circuit. |

These are the before/after flags from the task 1 comparison, not evidence that the rated-window scan or catalog update has passed. The finer 50 ms prototype comparison is also still a failed preview candidate: it exceeded the 0.1 g longitudinal bound on Velocity Viper and under-warned at two Briar Blitz metres; its p95 latency was 7.83–12.43 times the same-session master baseline. See [task-126-derisk.md](task-126-derisk.md) for the measurements and comparison method.

## Existing endpoint failures to resolve

The prior diagnostic checked 1,178 existing endpoint/model/train combinations for level-to-level rated elements. It found 28 non-passing combinations. These are diagnostic failures, not approved catalog changes.

The initial combination counts and measurements below are documented in [task-126-task4-findings.md](task-126-task4-findings.md). The full coarse and refined scans use the separate isolated-piece harness described in the [force-smoothing plan](../superpowers/plans/2026-11-05-force-smoothing.md).

| Count | Existing endpoint and failing configurations | Measured failure | Status |
| ---: | --- | --- | --- |
| 7 | Loop: Timber Tumbler, 4 cars; Velocity Viper, 3 and 4 cars. TopHat: Briar Blitz, 4–7 cars. All at the existing minimum. | Minimum-entry clearance failure. The lowest observed speed was 4.28514 m/s for the 3-car Velocity Viper loop, below the unchanged 5 m/s clearance rule. | Diagnose the rating/clearance interaction; do not lower the clearance threshold to make these pass. |
| 18 | StallLeft and StallRight at existing minimum entry. | Calculated minimum speeds of 5.9999936–5.9999948 m/s against the existing 6 m/s clearance rule. | Determine whether the rated-bound calculation or numerical integration needs correction. Do not change the threshold. |
| 3 | EjectorHill12x64, EjectorHill12x80, and EjectorHill16x96 for Timber Tumbler with 8 cars, at maximum entry. | At the first reported violation, filtered vertical forces were −1.5804, −1.5115, and −1.5188 g, respectively; the evaluator had observed 0.205 s beyond the sustained limit at that point. The full excursion durations are listed in the independent comparison below. | The held-speed force trace is now independently reproduced with a real-train warmup. Final catalog acceptance remains unresolved. |

The three ejector-hill failures account for the remaining three of the 28 combinations. A separate held-speed classification scan found additional acceptance failures at the existing ejector maxima: EjectorHill8x48 classified Good rather than Great for Timber Tumbler with 4 cars and Velocity Viper with 3 or 4 cars; WoodenEjectorHill6x48 classified Good for Briar Blitz with 4–9 cars. The current ejector-hill test now uses `rate_piece`'s filtered vertical minimum and `classify_airtime` at the existing catalog maximum; it no longer checks the raw force. The candidate-maximum sweep below uses refined candidate maxima, so the classification reports refer to different speeds.

The StallLeft/StallRight findings above use per-train effective minima and count failing model/train combinations. The full scan below checks each template's fixed catalog minimum and reports the measured minimum train speed; for Timber Tumbler with 4 cars it reports 5.764978 m/s at the 17.997049 m/s catalog minimum. These are different quantities from separate diagnostics, and both are strictly below the 6 m/s clearance rule. The evidence does not identify a sole cause.

The full scan reported failures at 13 existing minimum endpoints and 4 existing maximum endpoints. These comprise clearance or stall failures as well as force violations. The detail below reports the first failing allowed model/train configuration for each affected template; the total is 17 failed template endpoints, not the earlier 28 individual model/train combinations.

| Existing endpoint | Failing configuration | Measured diagnostic |
| --- | --- | --- |
| SixtyUpToLevel minimum, 25.437358 m/s | Timber Tumbler, 7 cars | Minimum train speed 4.782740 m/s, below the 5 m/s clearance rule. |
| SteepToSixtyUp minimum, 26.115622 m/s | Timber Tumbler, 8 cars | Minimum in-piece speed 2.103515 m/s and stall/rollback. The independent trace details follow below. |
| SixtyToSteepUp minimum, 27.000000 m/s | Timber Tumbler, 7 cars | Minimum train speed 4.394734 m/s, below the 5 m/s clearance rule. |
| Loop minimum, 24.731073 m/s | Timber Tumbler, 4 cars | Minimum train speed 4.789598 m/s, below the 5 m/s clearance rule. |
| TopHat minimum, 24.632463 m/s | Briar Blitz, 4 cars | Minimum train speed 4.501010 m/s, below the 5 m/s clearance rule. |
| ZeroGRollFastLeft and ZeroGRollFastRight minima, 15.249027 m/s | Timber Tumbler, 4 cars | Minimum train speed 5.568597 m/s, below the 6 m/s clearance rule. |
| ZeroGRollCompactLeft and ZeroGRollCompactRight minima, 16.189842 m/s | Timber Tumbler, 4 cars | Minimum train speed 5.183601 m/s, below the 6 m/s clearance rule. |
| StallLeft and StallRight minima, 17.997049 m/s | Timber Tumbler, 4 cars | Minimum train speed 5.764978 m/s, below the 6 m/s clearance rule. |
| UpGentle RolloutLeft and RolloutRight minima, 22.000000 m/s | Timber Tumbler, 7 cars | Minimum train speed 3.494712 m/s, below the 5 m/s clearance rule. |
| LevelToSixtyDown maximum, 19.000000 m/s | Timber Tumbler, 8 cars | Vertical violation on car 7 at 14.379879 m; minimum filtered vertical −1.641305 g; recorded peak force −1.552510 g during a 0.845 s excursion. |
| EjectorHill12x64 maximum, 21.879096 m/s | Timber Tumbler, 8 cars | Vertical violation on car 0 at 35.897761 m; minimum filtered vertical −1.626249 g; recorded peak force −1.613075 g during a 0.385 s excursion. |
| EjectorHill12x80 maximum, 26.585749 m/s | Timber Tumbler, 8 cars | Vertical violation on car 0 at 44.619418 m; minimum filtered vertical −1.554786 g; recorded peak force −1.511487 g during a 0.215 s excursion. |
| EjectorHill16x96 maximum, 28.343393 m/s | Timber Tumbler, 8 cars | Vertical violation on car 0 at 53.197191 m; minimum filtered vertical −1.536400 g; recorded peak force −1.518832 g during a 0.235 s excursion. |

An independent focused comparison has since reproduced the high-speed excursions using a real-train level warmup. For the three ejector hills, the real-train and synthetic held-speed first-violation forces were −1.580372281585/−1.580372281690 g for EjectorHill12x64, −1.511486605226/−1.511486605117 g for EjectorHill12x80, and −1.518832442234/−1.518832442394 g for EjectorHill16x96. Entry speeds matched to 12 decimals, and both paths identified the same car and axis. The full excursion durations were 0.385, 0.215, and 0.235 s; the corresponding peaks on those cars were −1.613074603, −1.511486605, and −1.518832442 g. The overall minima across all cars were −1.626248646, −1.554785855, and −1.536399630 g. Maximum full-trace differences between the real-train and synthetic paths were 1.782e−6, 1.363e−9, and 8.601e−7 g. The same comparison found first-violation forces of −1.540402021990/−1.540402022243 g at 19 m/s for LevelToSixtyDown, with a 0.845 s excursion, a −1.552509941 g peak on the first violating car, an overall minimum of −1.641305174 g, and a 5.255e−9 g maximum full-trace difference. These measurements show close trace agreement under the independent warmup; they do not resolve the rated-endpoint failures.

An independent check also confirmed that the SteepToSixtyUp minimum failure is a real clearance failure. At the old 26.115622 m/s minimum, the last Timber Tumbler car exited at 2.060943 m/s after 3.015 s, below the 5 m/s clearance rule; its minimum in-piece speed was 2.103515 m/s. The car stalled 0.256 m beyond the exit at 3.260 s, before completing the required 0.5 s tail. At a 26.75 m/s entry, the last car exited at 6.128298 m/s, completed the tail, and the minimum in-piece speed was 6.170869 m/s. This rules out the artificial tail length as the cause of the old-minimum failure; it remains an unresolved rated minimum.

## Refined candidate windows for all 94 templates (not accepted)

This table gives every existing window and the refined candidate range from the full scan. Coarse passing samples show the 0.25 m/s scan grid. The profile distinguishes repeated piece identifiers. The maximum-source column is based on the scan's `design_cap` result: `Design cap` retains the old catalog maximum, while `Force-bound candidate` is a refined upper boundary. The existing-endpoint column reports whether the current minimum and maximum passed for every allowed model and car count. `FAIL` means at least one configuration failed the full evaluator or its clearance/stall checks at that old endpoint. The last column is the first failing 0.25 m/s grid sample after the last coarse passing sample; its force axis or clearance/stall reason does not identify the exact binding condition at the refined maximum.

| Catalog template (entry slope/bank → exit slope/bank) | Existing window (m/s) | Coarse sampled range (m/s) | Refined candidate range (m/s) | Maximum source | Existing minimum / maximum check | First failing sample after coarse pass |
| --- | ---: | ---: | ---: | --- | --- | --- |
| `LevelToSixtyUp` (Level/Level → UpSixty/Level) | 28.337429–34.000000 | 28.25–36.50 | 28.337429–36.540000 | force-bound candidate | pass/pass | 36.75 m/s: Timber Tumbler 6 cars, car 0 vertical +4.515 g at 51.72 m, 0.230 s |
| `SixtyUpToLevel` (UpSixty/Level → Level/Level) | 25.437358–30.000000 | 26.00–31.00 | 25.980000–31.090000 | force-bound candidate | FAIL/pass | 31.25 m/s: Velocity Viper 3 cars, car 0 vertical -1.626 g at 37.10 m, 0.335 s |
| `LevelToSixtyDown` (Level/Level → DownSixty/Level) | 5.000000–19.000000 | 0.25–18.75 | 5.000000–18.770000 | force-bound candidate | pass/FAIL | 19.00 m/s: Timber Tumbler 8 cars, car 7 vertical -1.553 g at 14.38 m, 0.845 s |
| `SixtyDownToLevel` (DownSixty/Level → Level/Level) | 2.000000–23.820160 | 0.25–23.75 | 2.000000–23.860000 | force-bound candidate | pass/pass | 24.00 m/s: Timber Tumbler 8 cars, car 7 vertical +4.523 g at 35.85 m, 0.385 s |
| `SteepToSixtyUp` (UpSteep/Level → UpSixty/Level) | 26.115622–34.000000 | 26.75–39.25 | 26.510000–39.370000 | force-bound candidate | FAIL/pass | 39.50 m/s: Timber Tumbler 4 cars, car 0 vertical +5.029 g at 15.24 m, 0.105 s |
| `SixtyToSteepUp` (UpSixty/Level → UpSteep/Level) | 27.000000–30.000000 | 28.00–38.00 | 27.950000–38.110000 | force-bound candidate | FAIL/pass | 38.25 m/s: Timber Tumbler 4 cars, car 0 vertical -1.538 g at 22.59 m, 0.205 s |
| `SteepToSixtyDown` (DownSteep/Level → DownSixty/Level) | 3.000000–20.000000 | 0.25–26.00 | 3.000000–26.240000 | force-bound candidate | pass/pass | 26.25 m/s: Timber Tumbler 8 cars, car 7 vertical -1.513 g at 17.32 m, 0.205 s |
| `SixtyToSteepDown` (DownSixty/Level → DownSteep/Level) | 2.000000–28.000000 | 0.25–29.25 | 2.000000–29.340000 | force-bound candidate | pass/pass | 29.50 m/s: Timber Tumbler 8 cars, car 7 vertical +5.029 g at 13.95 m, 0.100 s |
| `Loop` (Level/Level → Level/Level) | 24.731073–27.500000 | 25.00–29.50 | 24.870000–29.560000 | force-bound candidate | FAIL/pass | 29.75 m/s: Timber Tumbler 8 cars, car 0 vertical +4.612 g at 54.65 m, 0.535 s |
| `Corkscrew` (Level/Level → Level/Level) | 14.000000–19.000000 | 13.50–20.50 | 14.000000–20.500000 | force-bound candidate | pass/pass | 20.75 m/s: Timber Tumbler 4 cars, car 0 vertical +4.538 g at 13.39 m, 0.210 s |
| `HeartlineRoll` (Level/Level → Level/Level) | 8.000000–20.000000 | 6.75–84.50 | 8.000000–84.660000 | force-bound candidate | pass/pass | 84.75 m/s: Timber Tumbler 5 cars, car 0 longitudinal +2.001 g at 49.52 m, 0.065 s, in tail |
| `HalfLoopUp` (Level/Level → Level/Inverted) | 27.956395–31.300000 | 28.00–33.00 | 27.956395–33.130000 | force-bound candidate | pass/pass | 33.25 m/s: Timber Tumbler 8 cars, car 0 vertical +4.576 g at 69.97 m, 0.460 s |
| `HalfLoopDown` (Level/Inverted → Level/Level) | 4.000000–15.301307 | 0.25–18.75 | 4.000000–18.750000 | force-bound candidate | pass/pass | 19.00 m/s: Timber Tumbler 8 cars, car 7 vertical +4.561 g at 24.47 m, 0.485 s |
| `HalfCorkscrewLeftUp` (Level/Level → Level/Inverted) | 17.000000–22.000000 | 16.25–29.00 | 17.000000–29.080000 | force-bound candidate | pass/pass | 29.25 m/s: Timber Tumbler 4 cars, car 0 lateral -2.016 g at 19.62 m, 0.190 s |
| `HalfCorkscrewLeftDown` (Level/Inverted → Level/Level) | 5.000000–15.765786 | 0.25–15.75 | 5.000000–15.765786 | design cap | pass/pass | 16.00 m/s: Timber Tumbler 4 cars, clearing speed exceeds 22 m/s\ |
| `RavenRollLeftUp` (Level/Level → Level/Inverted) | 8.000000–17.000000 | 0.75–18.25 | 8.000000–18.320000 | force-bound candidate | pass/pass | 18.50 m/s: Timber Tumbler 4 cars, car 0 vertical -1.521 g at 39.31 m, 0.225 s |
| `RavenRollLeftDown` (Level/Inverted → Level/Level) | 8.000000–18.000000 | 0.75–19.75 | 8.000000–19.830000 | force-bound candidate | pass/pass | 20.00 m/s: Timber Tumbler 4 cars, car 0 lateral -1.520 g at 18.00 m, 0.225 s |
| `LargeLoop` (Level/Level → Level/Level) | 27.588461–31.300000 | 27.25–33.00 | 27.588461–33.160000 | force-bound candidate | pass/pass | 33.25 m/s: Timber Tumbler 8 cars, car 0 vertical +4.559 g at 68.96 m, 0.385 s |
| `SteelLoop` (Level/Level → Level/Level) | 25.234231–27.000000 | 25.00–29.25 | 25.234231–29.440000 | force-bound candidate | pass/pass | 29.50 m/s: Briar Blitz 12 cars, car 0 vertical +4.023 g at 55.79 m, 0.555 s |
| `WaveTurnLeft` (Level/Level → Level/Level) | 8.000000–23.000000 | 6.25–24.50 | 8.000000–24.560000 | force-bound candidate | pass/pass | 24.75 m/s: Timber Tumbler 4 cars, car 0 lateral +1.526 g at 12.23 m, 0.210 s |
| `HybridOverbankLeft` (Level/Level → Level/Level) | 5.000000–13.000000 | 0.25–13.25 | 5.000000–13.450000 | force-bound candidate | pass/pass | 13.50 m/s: Timber Tumbler 4 cars, car 0 lateral -1.506 g at 28.35 m, 0.295 s |
| `HybridOverbankRight` (Level/Level → Level/Level) | 5.000000–13.000000 | 0.25–13.25 | 5.000000–13.450000 | force-bound candidate | pass/pass | 13.50 m/s: Timber Tumbler 4 cars, car 0 lateral +1.506 g at 28.35 m, 0.295 s |
| `WaveTurnRight` (Level/Level → Level/Level) | 8.000000–23.000000 | 6.25–24.50 | 8.000000–24.560000 | force-bound candidate | pass/pass | 24.75 m/s: Timber Tumbler 4 cars, car 0 lateral -1.526 g at 12.23 m, 0.210 s |
| `LoopRight` (Level/Level → Level/Level) | 24.731073–27.500000 | 24.50–29.50 | 24.731073–29.560000 | force-bound candidate | pass/pass | 29.75 m/s: Timber Tumbler 8 cars, car 0 vertical +4.612 g at 54.65 m, 0.535 s |
| `LargeLoopRight` (Level/Level → Level/Level) | 27.588461–31.300000 | 27.25–33.00 | 27.588461–33.160000 | force-bound candidate | pass/pass | 33.25 m/s: Timber Tumbler 8 cars, car 0 vertical +4.559 g at 68.96 m, 0.385 s |
| `SteelLoopRight` (Level/Level → Level/Level) | 25.234231–27.000000 | 25.00–29.25 | 25.234231–29.440000 | force-bound candidate | pass/pass | 29.50 m/s: Briar Blitz 12 cars, car 0 vertical +4.023 g at 55.79 m, 0.555 s |
| `CorkscrewRight` (Level/Level → Level/Level) | 14.000000–19.000000 | 13.50–20.50 | 14.000000–20.500000 | force-bound candidate | pass/pass | 20.75 m/s: Timber Tumbler 4 cars, car 0 vertical +4.538 g at 13.39 m, 0.210 s |
| `HeartlineRollRight` (Level/Level → Level/Level) | 8.000000–20.000000 | 6.75–84.50 | 8.000000–84.660000 | force-bound candidate | pass/pass | 84.75 m/s: Timber Tumbler 5 cars, car 0 longitudinal +2.001 g at 49.52 m, 0.065 s, in tail |
| `HalfCorkscrewRightUp` (Level/Level → Level/Inverted) | 17.000000–22.000000 | 16.25–29.00 | 17.000000–29.080000 | force-bound candidate | pass/pass | 29.25 m/s: Timber Tumbler 4 cars, car 0 lateral +2.016 g at 19.62 m, 0.190 s |
| `HalfCorkscrewRightDown` (Level/Inverted → Level/Level) | 5.000000–15.765786 | 0.25–15.75 | 5.000000–15.765786 | design cap | pass/pass | 16.00 m/s: Timber Tumbler 4 cars, clearing speed exceeds 22 m/s\ |
| `RavenRollRightUp` (Level/Level → Level/Inverted) | 8.000000–17.000000 | 0.75–18.25 | 8.000000–18.320000 | force-bound candidate | pass/pass | 18.50 m/s: Timber Tumbler 4 cars, car 0 vertical -1.521 g at 39.31 m, 0.225 s |
| `RavenRollRightDown` (Level/Inverted → Level/Level) | 8.000000–18.000000 | 0.75–19.75 | 8.000000–19.830000 | force-bound candidate | pass/pass | 20.00 m/s: Timber Tumbler 4 cars, car 0 lateral +1.520 g at 18.00 m, 0.225 s |
| `BankedDropLeft` (Level/Left → DownGentle/Left) | 12.000000–15.000000 | 0.25–21.00 | 12.000000–21.240000 | force-bound candidate | pass/pass | 21.25 m/s: Briar Blitz 12 cars, car 11 vertical -1.004 g at 11.85 m, 0.205 s |
| `BankedDropRight` (Level/Right → DownGentle/Right) | 12.000000–15.000000 | 0.25–21.00 | 12.000000–21.240000 | force-bound candidate | pass/pass | 21.25 m/s: Briar Blitz 12 cars, car 11 vertical -1.004 g at 11.85 m, 0.205 s |
| `TwistedDropLeft` (Level/Left → DownGentle/Level) | 12.000000–15.000000 | 0.25–21.00 | 12.000000–21.140000 | force-bound candidate | pass/pass | 21.25 m/s: Timber Tumbler 8 cars, car 7 vertical -1.530 g at 12.09 m, 0.220 s |
| `TwistedDropRight` (Level/Right → DownGentle/Level) | 12.000000–15.000000 | 0.25–21.00 | 12.000000–21.140000 | force-bound candidate | pass/pass | 21.25 m/s: Timber Tumbler 8 cars, car 7 vertical -1.530 g at 12.09 m, 0.220 s |
| `BarrelDropLeft` (Level/Left → DownGentle/Level) | 10.000000–15.000000 | 0.25–20.00 | 10.000000–20.160000 | force-bound candidate | pass/pass | 20.25 m/s: Timber Tumbler 8 cars, car 7 vertical -2.016 g at 30.44 m, 0.135 s |
| `BarrelDropRight` (Level/Right → DownGentle/Level) | 10.000000–15.000000 | 0.25–20.00 | 10.000000–20.160000 | force-bound candidate | pass/pass | 20.25 m/s: Timber Tumbler 8 cars, car 7 vertical -2.016 g at 30.44 m, 0.135 s |
| `TopHat` (Level/Level → Level/Level) | 24.632463–27.155403 | 24.75–27.50 | 24.730000–27.740000 | force-bound candidate | FAIL/pass | 27.75 m/s: Briar Blitz 12 cars, car 0 vertical -1.002 g at 71.85 m, 0.205 s |
| `LaunchedTopHat` (Level/Level → Level/Level) | 27.750989–34.597392 | 27.75–34.75 | 27.750989–34.900000 | force-bound candidate | pass/pass | 35.00 m/s: Velocity Viper 6 cars, car 0 vertical -1.560 g at 93.58 m, 0.360 s |
| `EjectorHill8x48` (Level/Level → Level/Level) | 13.198805–18.241231 | 12.50–19.25 | 13.198805–19.290000 | force-bound candidate | pass/pass | 19.50 m/s: Timber Tumbler 8 cars, car 0 vertical -1.655 g at 26.82 m, 0.355 s |
| `EjectorHill12x64` (Level/Level → Level/Level) | 15.938763–21.879096 | 15.25–21.50 | 15.938763–21.710000 | force-bound candidate | pass/FAIL | 21.75 m/s: Timber Tumbler 8 cars, car 0 vertical -1.529 g at 36.83 m, 0.220 s |
| `EjectorHill8x64` (Level/Level → Level/Level) | 13.490086–25.215898 | 12.50–25.00 | 13.490086–25.240000 | force-bound candidate | pass/pass | 25.25 m/s: Timber Tumbler 8 cars, car 7 vertical -1.501 g at 39.60 m, 0.210 s |
| `EjectorHill12x80` (Level/Level → Level/Level) | 16.135709–26.585749 | 15.50–26.50 | 16.135709–26.530000 | force-bound candidate | pass/FAIL | 26.75 m/s: Timber Tumbler 8 cars, car 0 vertical -1.574 g at 42.83 m, 0.415 s |
| `EjectorHill16x96` (Level/Level → Level/Level) | 18.417832–28.343393 | 17.75–28.25 | 18.417832–28.300000 | force-bound candidate | pass/FAIL | 28.50 m/s: Timber Tumbler 8 cars, car 0 vertical -1.563 g at 52.56 m, 0.455 s |
| `FloaterHill4x48` (Level/Level → Level/Level) | 10.324920–20.786765 | 9.00–25.50 | 10.324920–25.710000 | force-bound candidate | pass/pass | 25.75 m/s: Briar Blitz 4 cars, car 0 vertical +4.501 g at 7.33 m, 0.100 s |
| `FloaterHill8x64` (Level/Level → Level/Level) | 13.539968–21.601129 | 12.50–24.75 | 13.539968–24.830000 | force-bound candidate | pass/pass | 25.00 m/s: Briar Blitz 11 cars, car 0 vertical -1.032 g at 35.91 m, 0.235 s |
| `FloaterHill12x80` (Level/Level → Level/Level) | 16.169081–23.010352 | 15.50–25.75 | 16.169081–25.910000 | force-bound candidate | pass/pass | 26.00 m/s: Briar Blitz 12 cars, car 0 vertical -1.050 g at 44.45 m, 0.250 s |
| `FloaterHill6x64` (Level/Level → Level/Level) | 12.119852–24.072335 | 11.00–28.00 | 12.119852–28.160000 | force-bound candidate | pass/pass | 28.25 m/s: Briar Blitz 4 cars, car 0 vertical +4.508 g at 9.17 m, 0.130 s |
| `FloaterHill12x96` (Level/Level → Level/Level) | 16.274685–26.747140 | 15.50–30.75 | 16.274685–30.830000 | force-bound candidate | pass/pass | 31.00 m/s: Briar Blitz 10 cars, car 0 vertical -1.013 g at 53.41 m, 0.220 s |
| `FloaterHill16x112` (Level/Level → Level/Level) | 18.523464–27.856176 | 17.75–31.00 | 18.523464–31.010000 | force-bound candidate | pass/pass | 31.25 m/s: Briar Blitz 4 cars, car 0 vertical +4.010 g at 19.14 m, 0.205 s |
| `SpeedHill4x64` (Level/Level → Level/Level) | 10.534328–20.923626 | 9.00–28.00 | 10.534328–28.150000 | force-bound candidate | pass/pass | 28.25 m/s: Briar Blitz 4 cars, car 0 vertical +4.517 g at 7.06 m, 0.080 s |
| `SpeedHill6x80` (Level/Level → Level/Level) | 12.257937–23.860506 | 11.00–28.75 | 12.257937–28.910000 | force-bound candidate | pass/pass | 29.00 m/s: Briar Blitz 4 cars, car 0 vertical +4.511 g at 8.26 m, 0.100 s |
| `WoodenEjectorHill6x48` (Level/Level → Level/Level) | 11.825325–19.665504 | 11.00–20.00 | 11.825325–20.010000 | force-bound candidate | pass/pass | 20.25 m/s: Briar Blitz 11 cars, car 0 vertical -1.055 g at 25.91 m, 0.330 s |
| `EjectorHill8x48OuterLeft` (Level/Level → Level/Level) | 14.198805–18.241231 | 13.50–21.50 | 14.198805–21.680000 | force-bound candidate | pass/pass | 21.75 m/s: Timber Tumbler 8 cars, car 0 lateral +1.539 g at 27.71 m, 0.270 s |
| `EjectorHill8x48OuterRight` (Level/Level → Level/Level) | 14.198805–18.241231 | 13.50–21.50 | 14.198805–21.680000 | force-bound candidate | pass/pass | 21.75 m/s: Timber Tumbler 8 cars, car 0 lateral -1.539 g at 27.71 m, 0.270 s |
| `EjectorHill12x64OuterLeft` (Level/Level → Level/Level) | 16.938763–21.879096 | 16.25–23.75 | 16.938763–23.880000 | force-bound candidate | pass/pass | 24.00 m/s: Timber Tumbler 8 cars, car 0 lateral +1.534 g at 35.18 m, 0.330 s |
| `EjectorHill12x64OuterRight` (Level/Level → Level/Level) | 16.938763–21.879096 | 16.25–23.75 | 16.938763–23.880000 | force-bound candidate | pass/pass | 24.00 m/s: Timber Tumbler 8 cars, car 0 lateral -1.534 g at 35.18 m, 0.330 s |
| `EjectorHill8x64OuterLeft` (Level/Level → Level/Level) | 14.490086–25.215898 | 13.75–27.25 | 14.490086–27.320000 | force-bound candidate | pass/pass | 27.50 m/s: Timber Tumbler 8 cars, car 0 lateral +1.542 g at 35.00 m, 0.335 s |
| `EjectorHill8x64OuterRight` (Level/Level → Level/Level) | 14.490086–25.215898 | 13.75–27.25 | 14.490086–27.320000 | force-bound candidate | pass/pass | 27.50 m/s: Timber Tumbler 8 cars, car 0 lateral -1.542 g at 35.00 m, 0.335 s |
| `EjectorHill12x80OuterLeft` (Level/Level → Level/Level) | 17.135709–26.585749 | 16.25–28.25 | 17.135709–28.450000 | force-bound candidate | pass/pass | 28.50 m/s: Timber Tumbler 8 cars, car 0 lateral +1.512 g at 42.44 m, 0.230 s |
| `EjectorHill12x80OuterRight` (Level/Level → Level/Level) | 17.135709–26.585749 | 16.25–28.25 | 17.135709–28.450000 | force-bound candidate | pass/pass | 28.50 m/s: Timber Tumbler 8 cars, car 0 lateral -1.512 g at 42.44 m, 0.230 s |
| `EjectorHill16x96OuterLeft` (Level/Level → Level/Level) | 19.417832–28.343393 | 18.50–30.00 | 19.417832–30.050000 | force-bound candidate | pass/pass | 30.25 m/s: Timber Tumbler 8 cars, car 0 lateral +1.552 g at 50.00 m, 0.305 s |
| `EjectorHill16x96OuterRight` (Level/Level → Level/Level) | 19.417832–28.343393 | 18.50–30.00 | 19.417832–30.050000 | force-bound candidate | pass/pass | 30.25 m/s: Timber Tumbler 8 cars, car 0 lateral -1.552 g at 50.00 m, 0.305 s |
| `FloaterHill4x48OuterLeft` (Level/Level → Level/Level) | 11.324920–20.786765 | 10.25–27.25 | 11.324920–27.490000 | force-bound candidate | pass/pass | 27.50 m/s: Timber Tumbler 7 cars, car 6 vertical +5.001 g at 47.22 m, 0.085 s |
| `FloaterHill4x48OuterRight` (Level/Level → Level/Level) | 11.324920–20.786765 | 10.25–27.25 | 11.324920–27.490000 | force-bound candidate | pass/pass | 27.50 m/s: Timber Tumbler 7 cars, car 6 vertical +5.001 g at 47.22 m, 0.085 s |
| `FloaterHill8x64OuterLeft` (Level/Level → Level/Level) | 14.539968–21.601129 | 13.75–27.25 | 14.539968–27.400000 | force-bound candidate | pass/pass | 27.50 m/s: Timber Tumbler 4 cars, car 0 vertical +5.012 g at 9.34 m, 0.135 s |
| `FloaterHill8x64OuterRight` (Level/Level → Level/Level) | 14.539968–21.601129 | 13.75–27.25 | 14.539968–27.400000 | force-bound candidate | pass/pass | 27.50 m/s: Timber Tumbler 4 cars, car 0 vertical +5.012 g at 9.34 m, 0.135 s |
| `FloaterHill12x80OuterLeft` (Level/Level → Level/Level) | 17.169081–23.010352 | 16.25–29.00 | 17.169081–29.070000 | force-bound candidate | pass/pass | 29.25 m/s: Timber Tumbler 4 cars, car 0 vertical +5.024 g at 11.40 m, 0.185 s |
| `FloaterHill12x80OuterRight` (Level/Level → Level/Level) | 17.169081–23.010352 | 16.25–29.00 | 17.169081–29.070000 | force-bound candidate | pass/pass | 29.25 m/s: Timber Tumbler 4 cars, car 0 vertical +5.024 g at 11.40 m, 0.185 s |
| `FloaterHill6x64OuterLeft` (Level/Level → Level/Level) | 13.119852–24.072335 | 12.25–30.00 | 13.119852–30.060000 | force-bound candidate | pass/pass | 30.25 m/s: Timber Tumbler 4 cars, car 0 vertical +5.041 g at 9.37 m, 0.110 s |
| `FloaterHill6x64OuterRight` (Level/Level → Level/Level) | 13.119852–24.072335 | 12.25–30.00 | 13.119852–30.060000 | force-bound candidate | pass/pass | 30.25 m/s: Timber Tumbler 4 cars, car 0 vertical +5.041 g at 9.37 m, 0.110 s |
| `FloaterHill12x96OuterLeft` (Level/Level → Level/Level) | 17.274685–26.747140 | 16.50–33.25 | 17.274685–33.300000 | force-bound candidate | pass/pass | 33.50 m/s: Timber Tumbler 4 cars, car 0 vertical +5.028 g at 13.06 m, 0.185 s |
| `FloaterHill12x96OuterRight` (Level/Level → Level/Level) | 17.274685–26.747140 | 16.50–33.25 | 17.274685–33.300000 | force-bound candidate | pass/pass | 33.50 m/s: Timber Tumbler 4 cars, car 0 vertical +5.028 g at 13.06 m, 0.185 s |
| `FloaterHill16x112OuterLeft` (Level/Level → Level/Level) | 19.523464–27.856176 | 18.75–33.75 | 19.523464–33.930000 | force-bound candidate | pass/pass | 34.00 m/s: Timber Tumbler 7 cars, car 0 vertical +4.502 g at 19.86 m, 0.205 s |
| `FloaterHill16x112OuterRight` (Level/Level → Level/Level) | 19.523464–27.856176 | 18.75–33.75 | 19.523464–33.930000 | force-bound candidate | pass/pass | 34.00 m/s: Timber Tumbler 7 cars, car 0 vertical +4.502 g at 19.86 m, 0.205 s |
| `ZeroGRollFastLeft` (Level/Level → Level/Level) | 15.249027–27.000000 | 15.50–30.75 | 15.480000–30.890000 | force-bound candidate | FAIL/pass | 31.00 m/s: Timber Tumbler 4 cars, car 0 vertical +5.019 g at 9.76 m, 0.115 s |
| `ZeroGRollFastRight` (Level/Level → Level/Level) | 15.249027–27.000000 | 15.50–30.75 | 15.480000–30.890000 | force-bound candidate | FAIL/pass | 31.00 m/s: Timber Tumbler 4 cars, car 0 vertical +5.019 g at 9.76 m, 0.115 s |
| `ZeroGRollCompactLeft` (Level/Level → Level/Level) | 16.189842–20.000000 | 16.75–33.00 | 16.580000–33.200000 | force-bound candidate | FAIL/pass | 33.25 m/s: Timber Tumbler 5 cars, car 4 vertical +5.003 g at 75.56 m, 0.185 s |
| `ZeroGRollCompactRight` (Level/Level → Level/Level) | 16.189842–20.000000 | 16.75–33.00 | 16.580000–33.200000 | force-bound candidate | FAIL/pass | 33.25 m/s: Timber Tumbler 5 cars, car 4 vertical +5.003 g at 75.56 m, 0.185 s |
| `StallLeft` (Level/Level → Level/Level) | 17.997049–27.000000 | 18.25–31.00 | 18.110000–31.050000 | force-bound candidate | FAIL/pass | 31.25 m/s: Timber Tumbler 4 cars, car 0 vertical +5.038 g at 12.02 m, 0.150 s |
| `StallRight` (Level/Level → Level/Level) | 17.997049–27.000000 | 18.25–31.00 | 18.110000–31.050000 | force-bound candidate | FAIL/pass | 31.25 m/s: Timber Tumbler 4 cars, car 0 vertical +5.038 g at 12.02 m, 0.150 s |
| `BankedStraightLeft` (Level/Left → Level/Left) | 5.000000–30.000000 | 0.25–100.00 | 5.000000–30.000000 | design cap | pass/pass | No failure to scan ceiling; design cap. |
| `BankedStraightRight` (Level/Right → Level/Right) | 5.000000–30.000000 | 0.25–100.00 | 5.000000–30.000000 | design cap | pass/pass | No failure to scan ceiling; design cap. |
| `RolloutLeft` (Level/Left → Level/Level) | 5.000000–30.000000 | 0.25–45.75 | 5.000000–45.870000 | force-bound candidate | pass/pass | 46.00 m/s: Timber Tumbler 4 cars, car 0 lateral -2.007 g at 12.65 m, 0.165 s |
| `RolloutRight` (Level/Right → Level/Level) | 5.000000–30.000000 | 0.25–45.75 | 5.000000–45.870000 | force-bound candidate | pass/pass | 46.00 m/s: Timber Tumbler 4 cars, car 0 lateral +2.007 g at 12.65 m, 0.165 s |
| `BankedStraightLeft` (UpGentle/Left → UpGentle/Left) | 20.000000–30.000000 | 19.25–100.00 | 20.000000–30.000000 | design cap | pass/pass | No failure to scan ceiling; design cap. |
| `BankedStraightLeft` (DownGentle/Left → DownGentle/Left) | 5.000000–30.000000 | 0.25–100.00 | 5.000000–30.000000 | design cap | pass/pass | No failure to scan ceiling; design cap. |
| `BankedStraightRight` (UpGentle/Right → UpGentle/Right) | 20.000000–30.000000 | 19.25–100.00 | 20.000000–30.000000 | design cap | pass/pass | No failure to scan ceiling; design cap. |
| `BankedStraightRight` (DownGentle/Right → DownGentle/Right) | 5.000000–30.000000 | 0.25–100.00 | 5.000000–30.000000 | design cap | pass/pass | No failure to scan ceiling; design cap. |
| `RolloutLeft` (UpGentle/Left → UpGentle/Level) | 22.000000–30.000000 | 23.00–55.25 | 22.870000–55.330000 | force-bound candidate | FAIL/pass | 55.50 m/s: Timber Tumbler 4 cars, car 0 lateral -2.008 g at 12.65 m, 0.140 s |
| `RolloutLeft` (DownGentle/Left → DownGentle/Level) | 5.000000–30.000000 | 0.25–52.00 | 5.000000–52.160000 | force-bound candidate | pass/pass | 52.25 m/s: Timber Tumbler 8 cars, car 7 lateral -2.004 g at 12.79 m, 0.140 s |
| `RolloutRight` (UpGentle/Right → UpGentle/Level) | 22.000000–30.000000 | 23.00–55.25 | 22.870000–55.330000 | force-bound candidate | FAIL/pass | 55.50 m/s: Timber Tumbler 4 cars, car 0 lateral +2.008 g at 12.65 m, 0.140 s |
| `RolloutRight` (DownGentle/Right → DownGentle/Level) | 5.000000–30.000000 | 0.25–52.00 | 5.000000–52.160000 | force-bound candidate | pass/pass | 52.25 m/s: Timber Tumbler 8 cars, car 7 lateral +2.004 g at 12.79 m, 0.140 s |

These are unaccepted candidate values. The measured lower force edge is refined, then the old energy-derived or hand-set minimum is retained when it is higher. The candidate maximum is either the old design cap or the refined upper force boundary. The axis and force at the first failed grid sample are diagnostics near that boundary; they were not measured at the candidate maximum itself.

## Candidate ejector-hill airtime classifications

The airtime diagnostic classified the most negative filtered vertical force (the airtime peak) at each candidate maximum. This peak classification is separate from the sustained force verdict: the candidate endpoints pass the force evaluator, while 11 configurations fail the separate Great-at-maximum requirement. `Good` and `TooMuch` describe airtime classification; they do not by themselves mean the sustained force evaluator failed.

| Piece | Model | Cars | Filtered vertical minimum at candidate maximum (g) | Airtime rating |
| --- | --- | ---: | ---: | --- |
| `EjectorHill8x48` | Timber Tumbler | 4 | -1.15139782 | Great |
| `EjectorHill8x48` | Timber Tumbler | 5 | -1.26466777 | Great |
| `EjectorHill8x48` | Timber Tumbler | 6 | -1.37243923 | Great |
| `EjectorHill8x48` | Timber Tumbler | 7 | -1.48859335 | Great |
| `EjectorHill8x48` | Timber Tumbler | 8 | -1.58856071 | TooMuch |
| `EjectorHill8x48` | Velocity Viper | 3 | -1.03817173 | Great |
| `EjectorHill8x48` | Velocity Viper | 4 | -1.15139782 | Great |
| `EjectorHill8x48` | Velocity Viper | 5 | -1.26466777 | Great |
| `EjectorHill8x48` | Velocity Viper | 6 | -1.37243923 | Great |
| `EjectorHill12x64` | Timber Tumbler | 4 | -1.10115918 | Great |
| `EjectorHill12x64` | Timber Tumbler | 5 | -1.20744690 | Great |
| `EjectorHill12x64` | Timber Tumbler | 6 | -1.32328319 | Great |
| `EjectorHill12x64` | Timber Tumbler | 7 | -1.44437294 | Great |
| `EjectorHill12x64` | Timber Tumbler | 8 | -1.56696402 | TooMuch |
| `EjectorHill12x64` | Velocity Viper | 3 | -1.01173592 | Great |
| `EjectorHill12x64` | Velocity Viper | 4 | -1.10115918 | Great |
| `EjectorHill12x64` | Velocity Viper | 5 | -1.20744690 | Great |
| `EjectorHill12x64` | Velocity Viper | 6 | -1.32328319 | Great |
| `EjectorHill8x64` | Timber Tumbler | 4 | -1.36659033 | Great |
| `EjectorHill8x64` | Timber Tumbler | 5 | -1.41800106 | Great |
| `EjectorHill8x64` | Timber Tumbler | 6 | -1.46944882 | Great |
| `EjectorHill8x64` | Timber Tumbler | 7 | -1.51760093 | TooMuch |
| `EjectorHill8x64` | Timber Tumbler | 8 | -1.55935661 | TooMuch |
| `EjectorHill8x64` | Velocity Viper | 3 | -1.31633726 | Great |
| `EjectorHill8x64` | Velocity Viper | 4 | -1.36659033 | Great |
| `EjectorHill8x64` | Velocity Viper | 5 | -1.41800106 | Great |
| `EjectorHill8x64` | Velocity Viper | 6 | -1.46944882 | Great |
| `EjectorHill12x80` | Timber Tumbler | 4 | -1.31060305 | Great |
| `EjectorHill12x80` | Timber Tumbler | 5 | -1.36725811 | Great |
| `EjectorHill12x80` | Timber Tumbler | 6 | -1.42610165 | Great |
| `EjectorHill12x80` | Timber Tumbler | 7 | -1.48499281 | Great |
| `EjectorHill12x80` | Timber Tumbler | 8 | -1.54186371 | TooMuch |
| `EjectorHill12x80` | Velocity Viper | 3 | -1.27456756 | Great |
| `EjectorHill12x80` | Velocity Viper | 4 | -1.31060305 | Great |
| `EjectorHill12x80` | Velocity Viper | 5 | -1.36725811 | Great |
| `EjectorHill12x80` | Velocity Viper | 6 | -1.42610165 | Great |
| `EjectorHill16x96` | Timber Tumbler | 4 | -1.30920901 | Great |
| `EjectorHill16x96` | Timber Tumbler | 5 | -1.35267624 | Great |
| `EjectorHill16x96` | Timber Tumbler | 6 | -1.40580165 | Great |
| `EjectorHill16x96` | Timber Tumbler | 7 | -1.46602243 | Great |
| `EjectorHill16x96` | Timber Tumbler | 8 | -1.52639488 | TooMuch |
| `EjectorHill16x96` | Velocity Viper | 3 | -1.27589763 | Great |
| `EjectorHill16x96` | Velocity Viper | 4 | -1.30920901 | Great |
| `EjectorHill16x96` | Velocity Viper | 5 | -1.35267624 | Great |
| `EjectorHill16x96` | Velocity Viper | 6 | -1.40580165 | Great |
| `WoodenEjectorHill6x48` | Briar Blitz | 4 | -0.80258965 | Good |
| `WoodenEjectorHill6x48` | Briar Blitz | 5 | -0.83428478 | Good |
| `WoodenEjectorHill6x48` | Briar Blitz | 6 | -0.86690720 | Good |
| `WoodenEjectorHill6x48` | Briar Blitz | 7 | -0.90011321 | Great |
| `WoodenEjectorHill6x48` | Briar Blitz | 8 | -0.93311360 | Great |
| `WoodenEjectorHill6x48` | Briar Blitz | 9 | -0.96603623 | Great |
| `WoodenEjectorHill6x48` | Briar Blitz | 10 | -0.99713092 | Great |
| `WoodenEjectorHill6x48` | Briar Blitz | 11 | -1.02554423 | TooMuch |
| `WoodenEjectorHill6x48` | Briar Blitz | 12 | -1.05089286 | TooMuch |

The 11 Great-at-maximum exceptions are EjectorHill8x48, EjectorHill12x64, EjectorHill8x64, EjectorHill12x80, and EjectorHill16x96 for Timber Tumbler with the longest failing train lengths shown above, plus WoodenEjectorHill6x48 for Briar Blitz with 4, 5, 6, 11, and 12 cars. The first six exceptions are `TooMuch`; the three shorter wooden configurations are `Good`, and the two longest wooden configurations are `TooMuch`. The remaining 43 configurations are `Great`.

The earlier task-126 findings measured filtered airtime at current catalog maxima. This table measures the refined candidate maxima, so the results refer to different speeds and should not be compared as if they were the same endpoint. The 11 exceptions remain acceptance failures, and the generator did not write the table.

## Full old/new table and acceptance status

The refinement log contains candidate endpoints for all 94 templates, but `UPDATE_RATED_WINDOWS=1` aborted with exit code 101 because of the 11 airtime-classification failures above. No `rated_windows.generated.rs` file was created and no rated-window values were written to the catalog. The run reported no width failures; candidate endpoints were rechecked by the force evaluator before the airtime assertion. This does not make the candidate table accepted.

| Required result | Status |
| --- | --- |
| Old and refined candidate minimum/maximum for every template | Recorded above as unaccepted measurements. |
| Binding axis at each force-bound candidate maximum | Not reported; the axis at a nearby failing grid point is not the binding axis at the candidate endpoint. |
| No lost windows and continuous passing range | Every template had a contiguous passing range on the 0.25 m/s scan grid and refined candidate endpoints; failures between grid samples remain possible. |
| Required window width | No width failures were reported by the generation run. |
| Ejector hills classify Great and never Dangerous at each maximum | Failed: 11 configurations are Good or TooMuch at candidate maxima; no Dangerous result was reported. |
| Catalog build and `make bench-construction` unchanged | Candidate comparison not measured; rated-window values remain unchanged. |
| Restated rated-element, inversion, player-circuit, statistics, and targeted gates | Not reported here; these still need to run against an accepted table. |

The starting-tree construction benchmark was recorded as a single run: first catalog access took 7.4 ms for 249 catalog entries; first hybrid, multi-launch, and wooden choice searches took 41.6, 44.7, and 17.5 ms; first element-choice searches took 90.8, 89.0, and 30.4 ms; cached queries took 0.17–0.18 ms. These are baseline values only. A post-table comparison has not been measured.

The coarse scan log was `/tmp/task126-force-scan.log`, the refinement output is `/tmp/task126-force-refine.log`, and the complete airtime classification output is `/tmp/task126-airtime-classes.log`. The scan implementation is `crates/sim/examples/measure_force_windows.rs`. Generation failed on the Great-at-maximum requirement, so the candidate values remain unaccepted.
