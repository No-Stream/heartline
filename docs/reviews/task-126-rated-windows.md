# TASK-126 rated-window scan

## Sustained-limit scan: partial results

A later scan applies a stricter criterion: each allowed model and car count must pass the existing duration-aware evaluator and clearance checks, and its filtered force must remain within the model's sustained limits on every axis. These results are distinct from the duration-aware candidate recorded below. The scan stopped with exit code 101 after refining 51 of 94 templates: `SpeedHill4x64` had a noncontiguous coarse-grid result, with passing samples at 24.5 and 25.0 m/s separated by a failed sample. The scan asserted that windows must be contiguous and did not refine that template or process the remaining templates. No final 94-template table or catalog acceptance follows from this run.

For SpeedHill4x64 and an eight-car Timber Tumbler, the maximum filtered longitudinal force at 24.75 m/s was 1.504663 g at a 5 ms step, 1.483473 g at 2.5 ms, and 1.478773 g at 1.25 ms; the sustained limit is 1.5 g. At 25.00 m/s, the 5 ms result passed but the 1.25 ms result was 1.503418 g and failed. A separate constant-speed scratch comparison, not a full-train simulation, found a maximum longitudinal error of about 0.391 g and RMS error of 0.0164 g for both the f32 profile and a 512-point f64 direct-solved profile; a 2048-point f64 profile reduced those to 0.0430 g and 0.00578 g. Geometry aliasing is a hypothesis, not an established cause. These measurements do not establish a safe monotonic window.

The table lists old catalog windows and the partial refined measurements. Profiles distinguish repeated identifiers. For a force or clearance boundary, the cause is the first failure at the upper coarse sample after the last passing sample; it is not necessarily the binding cause at the refined boundary. A design cap means the scan reached its ceiling without an upper failure and retained the old maximum. The 0.25 m/s grid cannot rule out gaps between sampled speeds.

| Catalog template (entry slope/bank → exit slope/bank) | Old window (m/s) | Sustained-limit scan window (m/s) | Upper boundary observation |
| --- | ---: | ---: | --- |
| `LevelToSixtyUp` (Level/Level → UpSixty/Level) | 28.337429–34.000000 | 28.337429–36.410000 | Vertical (first failing upper sample) |
| `SixtyUpToLevel` (UpSixty/Level → Level/Level) | 25.437358–30.000000 | 25.980000–30.870000 | Vertical (first failing upper sample) |
| `LevelToSixtyDown` (Level/Level → DownSixty/Level) | 5.000000–19.000000 | 5.000000–18.340000 | Vertical (first failing upper sample) |
| `SixtyDownToLevel` (DownSixty/Level → Level/Level) | 2.000000–23.820160 | 2.000000–23.300000 | Vertical (first failing upper sample) |
| `SteepToSixtyUp` (UpSteep/Level → UpSixty/Level) | 26.115622–34.000000 | 26.510000–37.350000 | Vertical (first failing upper sample) |
| `SixtyToSteepUp` (UpSixty/Level → UpSteep/Level) | 27.000000–30.000000 | 27.950000–35.280000 | Vertical (first failing upper sample) |
| `SteepToSixtyDown` (DownSteep/Level → DownSixty/Level) | 3.000000–20.000000 | 3.000000–22.120000 | Vertical (first failing upper sample) |
| `SixtyToSteepDown` (DownSixty/Level → DownSteep/Level) | 2.000000–28.000000 | 2.000000–26.620000 | Vertical (first failing upper sample) |
| `Loop` (Level/Level → Level/Level) | 24.731073–27.500000 | 24.870000–29.490000 | Vertical (first failing upper sample) |
| `Corkscrew` (Level/Level → Level/Level) | 14.000000–19.000000 | 14.000000–19.820000 | Vertical (first failing upper sample) |
| `HeartlineRoll` (Level/Level → Level/Level) | 8.000000–20.000000 | 8.000000–67.160000 | Longitudinal (first failing upper sample) |
| `HalfLoopUp` (Level/Level → Level/Inverted) | 27.956395–31.300000 | 27.956395–33.100000 | Vertical (first failing upper sample) |
| `HalfLoopDown` (Level/Inverted → Level/Level) | 4.000000–15.301307 | 4.000000–18.640000 | Vertical (first failing upper sample) |
| `HalfCorkscrewLeftUp` (Level/Level → Level/Inverted) | 17.000000–22.000000 | 17.000000–23.260000 | Lateral (first failing upper sample) |
| `HalfCorkscrewLeftDown` (Level/Inverted → Level/Level) | 5.000000–15.765786 | 5.000000–15.760000 | clearing speed exceeds 22 m/s (first failing upper sample) |
| `RavenRollLeftUp` (Level/Level → Level/Inverted) | 8.000000–17.000000 | 8.000000–17.680000 | Vertical (first failing upper sample) |
| `RavenRollLeftDown` (Level/Inverted → Level/Level) | 8.000000–18.000000 | 8.000000–19.250000 | Lateral (first failing upper sample) |
| `LargeLoop` (Level/Level → Level/Level) | 27.588461–31.300000 | 27.588461–33.090000 | Vertical (first failing upper sample) |
| `SteelLoop` (Level/Level → Level/Level) | 25.234231–27.000000 | 25.234231–29.440000 | Vertical (first failing upper sample) |
| `WaveTurnLeft` (Level/Level → Level/Level) | 8.000000–23.000000 | 8.000000–23.510000 | Lateral (first failing upper sample) |
| `HybridOverbankLeft` (Level/Level → Level/Level) | 5.000000–13.000000 | 5.000000–13.420000 | Lateral (first failing upper sample) |
| `HybridOverbankRight` (Level/Level → Level/Level) | 5.000000–13.000000 | 5.000000–13.420000 | Lateral (first failing upper sample) |
| `WaveTurnRight` (Level/Level → Level/Level) | 8.000000–23.000000 | 8.000000–23.510000 | Lateral (first failing upper sample) |
| `LoopRight` (Level/Level → Level/Level) | 24.731073–27.500000 | 24.731073–29.490000 | Vertical (first failing upper sample) |
| `LargeLoopRight` (Level/Level → Level/Level) | 27.588461–31.300000 | 27.588461–33.090000 | Vertical (first failing upper sample) |
| `SteelLoopRight` (Level/Level → Level/Level) | 25.234231–27.000000 | 25.234231–29.440000 | Vertical (first failing upper sample) |
| `CorkscrewRight` (Level/Level → Level/Level) | 14.000000–19.000000 | 14.000000–19.820000 | Vertical (first failing upper sample) |
| `HeartlineRollRight` (Level/Level → Level/Level) | 8.000000–20.000000 | 8.000000–67.160000 | Longitudinal (first failing upper sample) |
| `HalfCorkscrewRightUp` (Level/Level → Level/Inverted) | 17.000000–22.000000 | 17.000000–23.260000 | Lateral (first failing upper sample) |
| `HalfCorkscrewRightDown` (Level/Inverted → Level/Level) | 5.000000–15.765786 | 5.000000–15.760000 | clearing speed exceeds 22 m/s (first failing upper sample) |
| `RavenRollRightUp` (Level/Level → Level/Inverted) | 8.000000–17.000000 | 8.000000–17.680000 | Vertical (first failing upper sample) |
| `RavenRollRightDown` (Level/Inverted → Level/Level) | 8.000000–18.000000 | 8.000000–19.250000 | Lateral (first failing upper sample) |
| `BankedDropLeft` (Level/Left → DownGentle/Left) | 12.000000–15.000000 | 12.000000–20.030000 | Vertical (first failing upper sample) |
| `BankedDropRight` (Level/Right → DownGentle/Right) | 12.000000–15.000000 | 12.000000–20.030000 | Vertical (first failing upper sample) |
| `TwistedDropLeft` (Level/Left → DownGentle/Level) | 12.000000–15.000000 | 12.000000–20.770000 | Vertical (first failing upper sample) |
| `TwistedDropRight` (Level/Right → DownGentle/Level) | 12.000000–15.000000 | 12.000000–20.770000 | Vertical (first failing upper sample) |
| `BarrelDropLeft` (Level/Left → DownGentle/Level) | 10.000000–15.000000 | 10.000000–16.370000 | Vertical (first failing upper sample) |
| `BarrelDropRight` (Level/Right → DownGentle/Level) | 10.000000–15.000000 | 10.000000–16.370000 | Vertical (first failing upper sample) |
| `TopHat` (Level/Level → Level/Level) | 24.632463–27.155403 | 24.730000–27.580000 | Vertical (first failing upper sample) |
| `LaunchedTopHat` (Level/Level → Level/Level) | 27.750989–34.597392 | 27.750989–34.800000 | Vertical (first failing upper sample) |
| `EjectorHill8x48` (Level/Level → Level/Level) | 13.198805–18.241231 | 13.198805–19.020000 | Longitudinal (first failing upper sample) |
| `EjectorHill12x64` (Level/Level → Level/Level) | 15.938763–21.879096 | 15.938763–21.510000 | Vertical (first failing upper sample) |
| `EjectorHill8x64` (Level/Level → Level/Level) | 13.490086–25.215898 | 13.490086–24.970000 | Vertical (first failing upper sample) |
| `EjectorHill12x80` (Level/Level → Level/Level) | 16.135709–26.585749 | 16.135709–26.340000 | Vertical (first failing upper sample) |
| `EjectorHill16x96` (Level/Level → Level/Level) | 18.417832–28.343393 | 18.417832–28.180000 | Vertical (first failing upper sample) |
| `FloaterHill4x48` (Level/Level → Level/Level) | 10.324920–20.786765 | 10.324920–23.680000 | Longitudinal (first failing upper sample) |
| `FloaterHill8x64` (Level/Level → Level/Level) | 13.539968–21.601129 | 13.539968–23.840000 | Vertical (first failing upper sample) |
| `FloaterHill12x80` (Level/Level → Level/Level) | 16.169081–23.010352 | 16.169081–25.300000 | Vertical (first failing upper sample) |
| `FloaterHill6x64` (Level/Level → Level/Level) | 12.119852–24.072335 | 12.119852–26.140000 | Vertical (first failing upper sample) |
| `FloaterHill12x96` (Level/Level → Level/Level) | 16.274685–26.747140 | 16.274685–28.960000 | Vertical (first failing upper sample) |
| `FloaterHill16x112` (Level/Level → Level/Level) | 18.523464–27.856176 | 18.523464–30.120000 | Vertical (first failing upper sample) |

The source log is `/tmp/task126-owner-force-scan-provenance.log`, a temporary artifact. The old duration-aware candidates and their airtime classifications below describe an earlier, different criterion and are not the final windows for this scan.

## Earlier duration-aware scan status

The earlier full coarse scan completed for all 94 templates, and boundary refinement emitted candidate endpoints for all 94. Rated-window values remain unchanged because the generation run aborted after 11 ejector train configurations failed the Great-at-maximum airtime requirement. The candidate endpoints below are diagnostic measurements, not accepted catalog windows. This write-up also records the verified circuit flag comparison and the earlier 28 failing endpoint combinations.

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

This table gives every existing window and the refined candidate range from the full scan. Coarse passing samples show the 0.25 m/s scan grid. The profile distinguishes repeated piece identifiers. The maximum-source column is based on the scan's `design_cap` result: `Design cap` retains the old catalog maximum, while `Force-bound candidate` is a refined upper boundary. The existing-endpoint column reports whether the current minimum and maximum passed for every allowed model and car count. `FAIL` means at least one configuration failed the full evaluator or its clearance/stall checks at that old endpoint. The coarse failure column reports the first failing 0.25 m/s sample after the last coarse passing sample. The final column reports the first failure at the candidate maximum plus 0.01 m/s, scanning allowed models and car counts in order; “pass all” means every allowed configuration passed at that speed. A failure there identifies an axis or clearance condition at the sampled speed, not necessarily the exact binding condition at the candidate maximum. Design caps retain their existing maximum even when the sampled speed passes.

| Catalog template (entry slope/bank → exit slope/bank) | Existing window (m/s) | Coarse sampled range (m/s) | Refined candidate range (m/s) | Maximum source | Existing minimum / maximum check | First failing sample after coarse pass | At candidate maximum + 0.01 m/s |
| --- | ---: | ---: | ---: | --- | --- | --- | --- |
| `LevelToSixtyUp` (Level/Level → UpSixty/Level) | 28.337429–34.000000 | 28.25–36.50 | 28.337429–36.540000 | force-bound candidate | pass/pass | 36.75 m/s: Timber Tumbler 6 cars, car 0 vertical +4.515 g at 51.72 m, 0.230 s | vertical +4.505 g, Timber Tumbler 8 cars, car 0 |
| `SixtyUpToLevel` (UpSixty/Level → Level/Level) | 25.437358–30.000000 | 26.00–31.00 | 25.980000–31.090000 | force-bound candidate | FAIL/pass | 31.25 m/s: Velocity Viper 3 cars, car 0 vertical -1.626 g at 37.10 m, 0.335 s | vertical -1.521 g, Velocity Viper 3 cars, car 0 |
| `LevelToSixtyDown` (Level/Level → DownSixty/Level) | 5.000000–19.000000 | 0.25–18.75 | 5.000000–18.770000 | force-bound candidate | pass/FAIL | 19.00 m/s: Timber Tumbler 8 cars, car 7 vertical -1.553 g at 14.38 m, 0.845 s | vertical -1.501 g, Timber Tumbler 8 cars, car 7 |
| `SixtyDownToLevel` (DownSixty/Level → Level/Level) | 2.000000–23.820160 | 0.25–23.75 | 2.000000–23.860000 | force-bound candidate | pass/pass | 24.00 m/s: Timber Tumbler 8 cars, car 7 vertical +4.523 g at 35.85 m, 0.385 s | vertical +4.506 g, Timber Tumbler 8 cars, car 7 |
| `SteepToSixtyUp` (UpSteep/Level → UpSixty/Level) | 26.115622–34.000000 | 26.75–39.25 | 26.510000–39.370000 | force-bound candidate | FAIL/pass | 39.50 m/s: Timber Tumbler 4 cars, car 0 vertical +5.029 g at 15.24 m, 0.105 s | vertical +5.000 g, Timber Tumbler 7 cars, car 0 |
| `SixtyToSteepUp` (UpSixty/Level → UpSteep/Level) | 27.000000–30.000000 | 28.00–38.00 | 27.950000–38.110000 | force-bound candidate | FAIL/pass | 38.25 m/s: Timber Tumbler 4 cars, car 0 vertical -1.538 g at 22.59 m, 0.205 s | vertical -1.537 g, Timber Tumbler 4 cars, car 0 |
| `SteepToSixtyDown` (DownSteep/Level → DownSixty/Level) | 3.000000–20.000000 | 0.25–26.00 | 3.000000–26.240000 | force-bound candidate | pass/pass | 26.25 m/s: Timber Tumbler 8 cars, car 7 vertical -1.513 g at 17.32 m, 0.205 s | vertical -1.513 g, Timber Tumbler 8 cars, car 7 |
| `SixtyToSteepDown` (DownSixty/Level → DownSteep/Level) | 2.000000–28.000000 | 0.25–29.25 | 2.000000–29.340000 | force-bound candidate | pass/pass | 29.50 m/s: Timber Tumbler 8 cars, car 7 vertical +5.029 g at 13.95 m, 0.100 s | pass all |
| `Loop` (Level/Level → Level/Level) | 24.731073–27.500000 | 25.00–29.50 | 24.870000–29.560000 | force-bound candidate | FAIL/pass | 29.75 m/s: Timber Tumbler 8 cars, car 0 vertical +4.612 g at 54.65 m, 0.535 s | vertical +4.512 g, Timber Tumbler 8 cars, car 0 |
| `Corkscrew` (Level/Level → Level/Level) | 14.000000–19.000000 | 13.50–20.50 | 14.000000–20.500000 | force-bound candidate | pass/pass | 20.75 m/s: Timber Tumbler 4 cars, car 0 vertical +4.538 g at 13.39 m, 0.210 s | vertical +4.517 g, Timber Tumbler 8 cars, car 0 |
| `HeartlineRoll` (Level/Level → Level/Level) | 8.000000–20.000000 | 6.75–84.50 | 8.000000–84.660000 | force-bound candidate | pass/pass | 84.75 m/s: Timber Tumbler 5 cars, car 0 longitudinal +2.001 g at 49.52 m, 0.065 s, in tail | longitudinal +2.001 g, Timber Tumbler 5 cars, car 0 |
| `HalfLoopUp` (Level/Level → Level/Inverted) | 27.956395–31.300000 | 28.00–33.00 | 27.956395–33.130000 | force-bound candidate | pass/pass | 33.25 m/s: Timber Tumbler 8 cars, car 0 vertical +4.576 g at 69.97 m, 0.460 s | vertical +4.517 g, Timber Tumbler 8 cars, car 0 |
| `HalfLoopDown` (Level/Inverted → Level/Level) | 4.000000–15.301307 | 0.25–18.75 | 4.000000–18.750000 | force-bound candidate | pass/pass | 19.00 m/s: Timber Tumbler 8 cars, car 7 vertical +4.561 g at 24.47 m, 0.485 s | pass all |
| `HalfCorkscrewLeftUp` (Level/Level → Level/Inverted) | 17.000000–22.000000 | 16.25–29.00 | 17.000000–29.080000 | force-bound candidate | pass/pass | 29.25 m/s: Timber Tumbler 4 cars, car 0 lateral -2.016 g at 19.62 m, 0.190 s | lateral -2.000 g, Timber Tumbler 4 cars, car 0 |
| `HalfCorkscrewLeftDown` (Level/Inverted → Level/Level) | 5.000000–15.765786 | 0.25–15.75 | 5.000000–15.765786 | design cap | pass/pass | 16.00 m/s: Timber Tumbler 4 cars, clearing speed exceeds 22 m/s | clearing speed > 22 m/s, Timber Tumbler 4 cars |
| `RavenRollLeftUp` (Level/Level → Level/Inverted) | 8.000000–17.000000 | 0.75–18.25 | 8.000000–18.320000 | force-bound candidate | pass/pass | 18.50 m/s: Timber Tumbler 4 cars, car 0 vertical -1.521 g at 39.31 m, 0.225 s | vertical -1.500 g, Timber Tumbler 4 cars, car 0 |
| `RavenRollLeftDown` (Level/Inverted → Level/Level) | 8.000000–18.000000 | 0.75–19.75 | 8.000000–19.830000 | force-bound candidate | pass/pass | 20.00 m/s: Timber Tumbler 4 cars, car 0 lateral -1.520 g at 18.00 m, 0.225 s | pass all |
| `LargeLoop` (Level/Level → Level/Level) | 27.588461–31.300000 | 27.25–33.00 | 27.588461–33.160000 | force-bound candidate | pass/pass | 33.25 m/s: Timber Tumbler 8 cars, car 0 vertical +4.559 g at 68.96 m, 0.385 s | vertical +4.507 g, Timber Tumbler 8 cars, car 0 |
| `SteelLoop` (Level/Level → Level/Level) | 25.234231–27.000000 | 25.00–29.25 | 25.234231–29.440000 | force-bound candidate | pass/pass | 29.50 m/s: Briar Blitz 12 cars, car 0 vertical +4.023 g at 55.79 m, 0.555 s | pass all |
| `WaveTurnLeft` (Level/Level → Level/Level) | 8.000000–23.000000 | 6.25–24.50 | 8.000000–24.560000 | force-bound candidate | pass/pass | 24.75 m/s: Timber Tumbler 4 cars, car 0 lateral +1.526 g at 12.23 m, 0.210 s | lateral +1.501 g, Timber Tumbler 8 cars, car 0 |
| `HybridOverbankLeft` (Level/Level → Level/Level) | 5.000000–13.000000 | 0.25–13.25 | 5.000000–13.450000 | force-bound candidate | pass/pass | 13.50 m/s: Timber Tumbler 4 cars, car 0 lateral -1.506 g at 28.35 m, 0.295 s | lateral -1.500 g, Timber Tumbler 4 cars, car 0 |
| `HybridOverbankRight` (Level/Level → Level/Level) | 5.000000–13.000000 | 0.25–13.25 | 5.000000–13.450000 | force-bound candidate | pass/pass | 13.50 m/s: Timber Tumbler 4 cars, car 0 lateral +1.506 g at 28.35 m, 0.295 s | lateral +1.500 g, Timber Tumbler 4 cars, car 0 |
| `WaveTurnRight` (Level/Level → Level/Level) | 8.000000–23.000000 | 6.25–24.50 | 8.000000–24.560000 | force-bound candidate | pass/pass | 24.75 m/s: Timber Tumbler 4 cars, car 0 lateral -1.526 g at 12.23 m, 0.210 s | lateral -1.501 g, Timber Tumbler 8 cars, car 0 |
| `LoopRight` (Level/Level → Level/Level) | 24.731073–27.500000 | 24.50–29.50 | 24.731073–29.560000 | force-bound candidate | pass/pass | 29.75 m/s: Timber Tumbler 8 cars, car 0 vertical +4.612 g at 54.65 m, 0.535 s | vertical +4.512 g, Timber Tumbler 8 cars, car 0 |
| `LargeLoopRight` (Level/Level → Level/Level) | 27.588461–31.300000 | 27.25–33.00 | 27.588461–33.160000 | force-bound candidate | pass/pass | 33.25 m/s: Timber Tumbler 8 cars, car 0 vertical +4.559 g at 68.96 m, 0.385 s | vertical +4.507 g, Timber Tumbler 8 cars, car 0 |
| `SteelLoopRight` (Level/Level → Level/Level) | 25.234231–27.000000 | 25.00–29.25 | 25.234231–29.440000 | force-bound candidate | pass/pass | 29.50 m/s: Briar Blitz 12 cars, car 0 vertical +4.023 g at 55.79 m, 0.555 s | pass all |
| `CorkscrewRight` (Level/Level → Level/Level) | 14.000000–19.000000 | 13.50–20.50 | 14.000000–20.500000 | force-bound candidate | pass/pass | 20.75 m/s: Timber Tumbler 4 cars, car 0 vertical +4.538 g at 13.39 m, 0.210 s | vertical +4.517 g, Timber Tumbler 8 cars, car 0 |
| `HeartlineRollRight` (Level/Level → Level/Level) | 8.000000–20.000000 | 6.75–84.50 | 8.000000–84.660000 | force-bound candidate | pass/pass | 84.75 m/s: Timber Tumbler 5 cars, car 0 longitudinal +2.001 g at 49.52 m, 0.065 s, in tail | longitudinal +2.001 g, Timber Tumbler 5 cars, car 0 |
| `HalfCorkscrewRightUp` (Level/Level → Level/Inverted) | 17.000000–22.000000 | 16.25–29.00 | 17.000000–29.080000 | force-bound candidate | pass/pass | 29.25 m/s: Timber Tumbler 4 cars, car 0 lateral +2.016 g at 19.62 m, 0.190 s | lateral +2.000 g, Timber Tumbler 4 cars, car 0 |
| `HalfCorkscrewRightDown` (Level/Inverted → Level/Level) | 5.000000–15.765786 | 0.25–15.75 | 5.000000–15.765786 | design cap | pass/pass | 16.00 m/s: Timber Tumbler 4 cars, clearing speed exceeds 22 m/s | clearing speed > 22 m/s, Timber Tumbler 4 cars |
| `RavenRollRightUp` (Level/Level → Level/Inverted) | 8.000000–17.000000 | 0.75–18.25 | 8.000000–18.320000 | force-bound candidate | pass/pass | 18.50 m/s: Timber Tumbler 4 cars, car 0 vertical -1.521 g at 39.31 m, 0.225 s | vertical -1.500 g, Timber Tumbler 4 cars, car 0 |
| `RavenRollRightDown` (Level/Inverted → Level/Level) | 8.000000–18.000000 | 0.75–19.75 | 8.000000–19.830000 | force-bound candidate | pass/pass | 20.00 m/s: Timber Tumbler 4 cars, car 0 lateral +1.520 g at 18.00 m, 0.225 s | pass all |
| `BankedDropLeft` (Level/Left → DownGentle/Left) | 12.000000–15.000000 | 0.25–21.00 | 12.000000–21.240000 | force-bound candidate | pass/pass | 21.25 m/s: Briar Blitz 12 cars, car 11 vertical -1.004 g at 11.85 m, 0.205 s | vertical -1.004 g, Briar Blitz 12 cars, car 11 |
| `BankedDropRight` (Level/Right → DownGentle/Right) | 12.000000–15.000000 | 0.25–21.00 | 12.000000–21.240000 | force-bound candidate | pass/pass | 21.25 m/s: Briar Blitz 12 cars, car 11 vertical -1.004 g at 11.85 m, 0.205 s | vertical -1.004 g, Briar Blitz 12 cars, car 11 |
| `TwistedDropLeft` (Level/Left → DownGentle/Level) | 12.000000–15.000000 | 0.25–21.00 | 12.000000–21.140000 | force-bound candidate | pass/pass | 21.25 m/s: Timber Tumbler 8 cars, car 7 vertical -1.530 g at 12.09 m, 0.220 s | vertical -1.507 g, Timber Tumbler 8 cars, car 7 |
| `TwistedDropRight` (Level/Right → DownGentle/Level) | 12.000000–15.000000 | 0.25–21.00 | 12.000000–21.140000 | force-bound candidate | pass/pass | 21.25 m/s: Timber Tumbler 8 cars, car 7 vertical -1.530 g at 12.09 m, 0.220 s | vertical -1.507 g, Timber Tumbler 8 cars, car 7 |
| `BarrelDropLeft` (Level/Left → DownGentle/Level) | 10.000000–15.000000 | 0.25–20.00 | 10.000000–20.160000 | force-bound candidate | pass/pass | 20.25 m/s: Timber Tumbler 8 cars, car 7 vertical -2.016 g at 30.44 m, 0.135 s | vertical -2.001 g, Timber Tumbler 8 cars, car 7 |
| `BarrelDropRight` (Level/Right → DownGentle/Level) | 10.000000–15.000000 | 0.25–20.00 | 10.000000–20.160000 | force-bound candidate | pass/pass | 20.25 m/s: Timber Tumbler 8 cars, car 7 vertical -2.016 g at 30.44 m, 0.135 s | vertical -2.001 g, Timber Tumbler 8 cars, car 7 |
| `TopHat` (Level/Level → Level/Level) | 24.632463–27.155403 | 24.75–27.50 | 24.730000–27.740000 | force-bound candidate | FAIL/pass | 27.75 m/s: Briar Blitz 12 cars, car 0 vertical -1.002 g at 71.85 m, 0.205 s | vertical -1.002 g, Briar Blitz 12 cars, car 0 |
| `LaunchedTopHat` (Level/Level → Level/Level) | 27.750989–34.597392 | 27.75–34.75 | 27.750989–34.900000 | force-bound candidate | pass/pass | 35.00 m/s: Velocity Viper 6 cars, car 0 vertical -1.560 g at 93.58 m, 0.360 s | vertical -1.509 g, Velocity Viper 6 cars, car 0 |
| `EjectorHill8x48` (Level/Level → Level/Level) | 13.198805–18.241231 | 12.50–19.25 | 13.198805–19.290000 | force-bound candidate | pass/pass | 19.50 m/s: Timber Tumbler 8 cars, car 0 vertical -1.655 g at 26.82 m, 0.355 s | vertical -1.501 g, Timber Tumbler 8 cars, car 7 |
| `EjectorHill12x64` (Level/Level → Level/Level) | 15.938763–21.879096 | 15.25–21.50 | 15.938763–21.710000 | force-bound candidate | pass/FAIL | 21.75 m/s: Timber Tumbler 8 cars, car 0 vertical -1.529 g at 36.83 m, 0.220 s | vertical -1.502 g, Timber Tumbler 8 cars, car 7 |
| `EjectorHill8x64` (Level/Level → Level/Level) | 13.490086–25.215898 | 12.50–25.00 | 13.490086–25.240000 | force-bound candidate | pass/pass | 25.25 m/s: Timber Tumbler 8 cars, car 7 vertical -1.501 g at 39.60 m, 0.210 s | vertical -1.501 g, Timber Tumbler 8 cars, car 7 |
| `EjectorHill12x80` (Level/Level → Level/Level) | 16.135709–26.585749 | 15.50–26.50 | 16.135709–26.530000 | force-bound candidate | pass/FAIL | 26.75 m/s: Timber Tumbler 8 cars, car 0 vertical -1.574 g at 42.83 m, 0.415 s | vertical -1.502 g, Timber Tumbler 8 cars, car 7 |
| `EjectorHill16x96` (Level/Level → Level/Level) | 18.417832–28.343393 | 17.75–28.25 | 18.417832–28.300000 | force-bound candidate | pass/FAIL | 28.50 m/s: Timber Tumbler 8 cars, car 0 vertical -1.563 g at 52.56 m, 0.455 s | vertical -1.503 g, Timber Tumbler 8 cars, car 0 |
| `FloaterHill4x48` (Level/Level → Level/Level) | 10.324920–20.786765 | 9.00–25.50 | 10.324920–25.710000 | force-bound candidate | pass/pass | 25.75 m/s: Briar Blitz 4 cars, car 0 vertical +4.501 g at 7.33 m, 0.100 s | vertical +4.500 g, Briar Blitz 7 cars, car 6 |
| `FloaterHill8x64` (Level/Level → Level/Level) | 13.539968–21.601129 | 12.50–24.75 | 13.539968–24.830000 | force-bound candidate | pass/pass | 25.00 m/s: Briar Blitz 11 cars, car 0 vertical -1.032 g at 35.91 m, 0.235 s | vertical -1.004 g, Briar Blitz 12 cars, car 0 |
| `FloaterHill12x80` (Level/Level → Level/Level) | 16.169081–23.010352 | 15.50–25.75 | 16.169081–25.910000 | force-bound candidate | pass/pass | 26.00 m/s: Briar Blitz 12 cars, car 0 vertical -1.050 g at 44.45 m, 0.250 s | vertical -1.002 g, Briar Blitz 12 cars, car 0 |
| `FloaterHill6x64` (Level/Level → Level/Level) | 12.119852–24.072335 | 11.00–28.00 | 12.119852–28.160000 | force-bound candidate | pass/pass | 28.25 m/s: Briar Blitz 4 cars, car 0 vertical +4.508 g at 9.17 m, 0.130 s | vertical +4.500 g, Briar Blitz 12 cars, car 11 |
| `FloaterHill12x96` (Level/Level → Level/Level) | 16.274685–26.747140 | 15.50–30.75 | 16.274685–30.830000 | force-bound candidate | pass/pass | 31.00 m/s: Briar Blitz 10 cars, car 0 vertical -1.013 g at 53.41 m, 0.220 s | vertical -1.003 g, Briar Blitz 12 cars, car 0 |
| `FloaterHill16x112` (Level/Level → Level/Level) | 18.523464–27.856176 | 17.75–31.00 | 18.523464–31.010000 | force-bound candidate | pass/pass | 31.25 m/s: Briar Blitz 4 cars, car 0 vertical +4.010 g at 19.14 m, 0.205 s | pass all |
| `SpeedHill4x64` (Level/Level → Level/Level) | 10.534328–20.923626 | 9.00–28.00 | 10.534328–28.150000 | force-bound candidate | pass/pass | 28.25 m/s: Briar Blitz 4 cars, car 0 vertical +4.517 g at 7.06 m, 0.080 s | vertical +4.501 g, Briar Blitz 7 cars, car 6 |
| `SpeedHill6x80` (Level/Level → Level/Level) | 12.257937–23.860506 | 11.00–28.75 | 12.257937–28.910000 | force-bound candidate | pass/pass | 29.00 m/s: Briar Blitz 4 cars, car 0 vertical +4.511 g at 8.26 m, 0.100 s | pass all |
| `WoodenEjectorHill6x48` (Level/Level → Level/Level) | 11.825325–19.665504 | 11.00–20.00 | 11.825325–20.010000 | force-bound candidate | pass/pass | 20.25 m/s: Briar Blitz 11 cars, car 0 vertical -1.055 g at 25.91 m, 0.330 s | vertical -1.002 g, Briar Blitz 12 cars, car 11 |
| `EjectorHill8x48OuterLeft` (Level/Level → Level/Level) | 14.198805–18.241231 | 13.50–21.50 | 14.198805–21.680000 | force-bound candidate | pass/pass | 21.75 m/s: Timber Tumbler 8 cars, car 0 lateral +1.539 g at 27.71 m, 0.270 s | pass all |
| `EjectorHill8x48OuterRight` (Level/Level → Level/Level) | 14.198805–18.241231 | 13.50–21.50 | 14.198805–21.680000 | force-bound candidate | pass/pass | 21.75 m/s: Timber Tumbler 8 cars, car 0 lateral -1.539 g at 27.71 m, 0.270 s | pass all |
| `EjectorHill12x64OuterLeft` (Level/Level → Level/Level) | 16.938763–21.879096 | 16.25–23.75 | 16.938763–23.880000 | force-bound candidate | pass/pass | 24.00 m/s: Timber Tumbler 8 cars, car 0 lateral +1.534 g at 35.18 m, 0.330 s | lateral +1.501 g, Timber Tumbler 8 cars, car 0 |
| `EjectorHill12x64OuterRight` (Level/Level → Level/Level) | 16.938763–21.879096 | 16.25–23.75 | 16.938763–23.880000 | force-bound candidate | pass/pass | 24.00 m/s: Timber Tumbler 8 cars, car 0 lateral -1.534 g at 35.18 m, 0.330 s | lateral -1.501 g, Timber Tumbler 8 cars, car 0 |
| `EjectorHill8x64OuterLeft` (Level/Level → Level/Level) | 14.490086–25.215898 | 13.75–27.25 | 14.490086–27.320000 | force-bound candidate | pass/pass | 27.50 m/s: Timber Tumbler 8 cars, car 0 lateral +1.542 g at 35.00 m, 0.335 s | lateral +1.501 g, Timber Tumbler 8 cars, car 0 |
| `EjectorHill8x64OuterRight` (Level/Level → Level/Level) | 14.490086–25.215898 | 13.75–27.25 | 14.490086–27.320000 | force-bound candidate | pass/pass | 27.50 m/s: Timber Tumbler 8 cars, car 0 lateral -1.542 g at 35.00 m, 0.335 s | lateral -1.501 g, Timber Tumbler 8 cars, car 0 |
| `EjectorHill12x80OuterLeft` (Level/Level → Level/Level) | 17.135709–26.585749 | 16.25–28.25 | 17.135709–28.450000 | force-bound candidate | pass/pass | 28.50 m/s: Timber Tumbler 8 cars, car 0 lateral +1.512 g at 42.44 m, 0.230 s | lateral +1.502 g, Timber Tumbler 8 cars, car 0 |
| `EjectorHill12x80OuterRight` (Level/Level → Level/Level) | 17.135709–26.585749 | 16.25–28.25 | 17.135709–28.450000 | force-bound candidate | pass/pass | 28.50 m/s: Timber Tumbler 8 cars, car 0 lateral -1.512 g at 42.44 m, 0.230 s | lateral -1.502 g, Timber Tumbler 8 cars, car 0 |
| `EjectorHill16x96OuterLeft` (Level/Level → Level/Level) | 19.417832–28.343393 | 18.50–30.00 | 19.417832–30.050000 | force-bound candidate | pass/pass | 30.25 m/s: Timber Tumbler 8 cars, car 0 lateral +1.552 g at 50.00 m, 0.305 s | lateral +1.503 g, Timber Tumbler 8 cars, car 0 |
| `EjectorHill16x96OuterRight` (Level/Level → Level/Level) | 19.417832–28.343393 | 18.50–30.00 | 19.417832–30.050000 | force-bound candidate | pass/pass | 30.25 m/s: Timber Tumbler 8 cars, car 0 lateral -1.552 g at 50.00 m, 0.305 s | lateral -1.503 g, Timber Tumbler 8 cars, car 0 |
| `FloaterHill4x48OuterLeft` (Level/Level → Level/Level) | 11.324920–20.786765 | 10.25–27.25 | 11.324920–27.490000 | force-bound candidate | pass/pass | 27.50 m/s: Timber Tumbler 7 cars, car 6 vertical +5.001 g at 47.22 m, 0.085 s | vertical +5.001 g, Timber Tumbler 7 cars, car 6 |
| `FloaterHill4x48OuterRight` (Level/Level → Level/Level) | 11.324920–20.786765 | 10.25–27.25 | 11.324920–27.490000 | force-bound candidate | pass/pass | 27.50 m/s: Timber Tumbler 7 cars, car 6 vertical +5.001 g at 47.22 m, 0.085 s | vertical +5.001 g, Timber Tumbler 7 cars, car 6 |
| `FloaterHill8x64OuterLeft` (Level/Level → Level/Level) | 14.539968–21.601129 | 13.75–27.25 | 14.539968–27.400000 | force-bound candidate | pass/pass | 27.50 m/s: Timber Tumbler 4 cars, car 0 vertical +5.012 g at 9.34 m, 0.135 s | vertical +5.002 g, Timber Tumbler 7 cars, car 6 |
| `FloaterHill8x64OuterRight` (Level/Level → Level/Level) | 14.539968–21.601129 | 13.75–27.25 | 14.539968–27.400000 | force-bound candidate | pass/pass | 27.50 m/s: Timber Tumbler 4 cars, car 0 vertical +5.012 g at 9.34 m, 0.135 s | vertical +5.002 g, Timber Tumbler 7 cars, car 6 |
| `FloaterHill12x80OuterLeft` (Level/Level → Level/Level) | 17.169081–23.010352 | 16.25–29.00 | 17.169081–29.070000 | force-bound candidate | pass/pass | 29.25 m/s: Timber Tumbler 4 cars, car 0 vertical +5.024 g at 11.40 m, 0.185 s | vertical +5.000 g, Timber Tumbler 8 cars, car 7 |
| `FloaterHill12x80OuterRight` (Level/Level → Level/Level) | 17.169081–23.010352 | 16.25–29.00 | 17.169081–29.070000 | force-bound candidate | pass/pass | 29.25 m/s: Timber Tumbler 4 cars, car 0 vertical +5.024 g at 11.40 m, 0.185 s | vertical +5.000 g, Timber Tumbler 8 cars, car 7 |
| `FloaterHill6x64OuterLeft` (Level/Level → Level/Level) | 13.119852–24.072335 | 12.25–30.00 | 13.119852–30.060000 | force-bound candidate | pass/pass | 30.25 m/s: Timber Tumbler 4 cars, car 0 vertical +5.041 g at 9.37 m, 0.110 s | vertical +5.001 g, Timber Tumbler 6 cars, car 5 |
| `FloaterHill6x64OuterRight` (Level/Level → Level/Level) | 13.119852–24.072335 | 12.25–30.00 | 13.119852–30.060000 | force-bound candidate | pass/pass | 30.25 m/s: Timber Tumbler 4 cars, car 0 vertical +5.041 g at 9.37 m, 0.110 s | vertical +5.001 g, Timber Tumbler 6 cars, car 5 |
| `FloaterHill12x96OuterLeft` (Level/Level → Level/Level) | 17.274685–26.747140 | 16.50–33.25 | 17.274685–33.300000 | force-bound candidate | pass/pass | 33.50 m/s: Timber Tumbler 4 cars, car 0 vertical +5.028 g at 13.06 m, 0.185 s | vertical +5.000 g, Timber Tumbler 8 cars, car 7 |
| `FloaterHill12x96OuterRight` (Level/Level → Level/Level) | 17.274685–26.747140 | 16.50–33.25 | 17.274685–33.300000 | force-bound candidate | pass/pass | 33.50 m/s: Timber Tumbler 4 cars, car 0 vertical +5.028 g at 13.06 m, 0.185 s | vertical +5.000 g, Timber Tumbler 8 cars, car 7 |
| `FloaterHill16x112OuterLeft` (Level/Level → Level/Level) | 19.523464–27.856176 | 18.75–33.75 | 19.523464–33.930000 | force-bound candidate | pass/pass | 34.00 m/s: Timber Tumbler 7 cars, car 0 vertical +4.502 g at 19.86 m, 0.205 s | vertical +4.500 g, Timber Tumbler 8 cars, car 0 |
| `FloaterHill16x112OuterRight` (Level/Level → Level/Level) | 19.523464–27.856176 | 18.75–33.75 | 19.523464–33.930000 | force-bound candidate | pass/pass | 34.00 m/s: Timber Tumbler 7 cars, car 0 vertical +4.502 g at 19.86 m, 0.205 s | vertical +4.500 g, Timber Tumbler 8 cars, car 0 |
| `ZeroGRollFastLeft` (Level/Level → Level/Level) | 15.249027–27.000000 | 15.50–30.75 | 15.480000–30.890000 | force-bound candidate | FAIL/pass | 31.00 m/s: Timber Tumbler 4 cars, car 0 vertical +5.019 g at 9.76 m, 0.115 s | vertical +5.001 g, Timber Tumbler 7 cars, car 6 |
| `ZeroGRollFastRight` (Level/Level → Level/Level) | 15.249027–27.000000 | 15.50–30.75 | 15.480000–30.890000 | force-bound candidate | FAIL/pass | 31.00 m/s: Timber Tumbler 4 cars, car 0 vertical +5.019 g at 9.76 m, 0.115 s | vertical +5.001 g, Timber Tumbler 7 cars, car 6 |
| `ZeroGRollCompactLeft` (Level/Level → Level/Level) | 16.189842–20.000000 | 16.75–33.00 | 16.580000–33.200000 | force-bound candidate | FAIL/pass | 33.25 m/s: Timber Tumbler 5 cars, car 4 vertical +5.003 g at 75.56 m, 0.185 s | vertical +5.001 g, Timber Tumbler 8 cars, car 7 |
| `ZeroGRollCompactRight` (Level/Level → Level/Level) | 16.189842–20.000000 | 16.75–33.00 | 16.580000–33.200000 | force-bound candidate | FAIL/pass | 33.25 m/s: Timber Tumbler 5 cars, car 4 vertical +5.003 g at 75.56 m, 0.185 s | vertical +5.001 g, Timber Tumbler 8 cars, car 7 |
| `StallLeft` (Level/Level → Level/Level) | 17.997049–27.000000 | 18.25–31.00 | 18.110000–31.050000 | force-bound candidate | FAIL/pass | 31.25 m/s: Timber Tumbler 4 cars, car 0 vertical +5.038 g at 12.02 m, 0.150 s | vertical +5.000 g, Timber Tumbler 8 cars, car 7 |
| `StallRight` (Level/Level → Level/Level) | 17.997049–27.000000 | 18.25–31.00 | 18.110000–31.050000 | force-bound candidate | FAIL/pass | 31.25 m/s: Timber Tumbler 4 cars, car 0 vertical +5.038 g at 12.02 m, 0.150 s | vertical +5.000 g, Timber Tumbler 8 cars, car 7 |
| `BankedStraightLeft` (Level/Left → Level/Left) | 5.000000–30.000000 | 0.25–100.00 | 5.000000–30.000000 | design cap | pass/pass | No failure to scan ceiling; design cap. | pass all |
| `BankedStraightRight` (Level/Right → Level/Right) | 5.000000–30.000000 | 0.25–100.00 | 5.000000–30.000000 | design cap | pass/pass | No failure to scan ceiling; design cap. | pass all |
| `RolloutLeft` (Level/Left → Level/Level) | 5.000000–30.000000 | 0.25–45.75 | 5.000000–45.870000 | force-bound candidate | pass/pass | 46.00 m/s: Timber Tumbler 4 cars, car 0 lateral -2.007 g at 12.65 m, 0.165 s | lateral -2.000 g, Timber Tumbler 4 cars, car 2 |
| `RolloutRight` (Level/Right → Level/Level) | 5.000000–30.000000 | 0.25–45.75 | 5.000000–45.870000 | force-bound candidate | pass/pass | 46.00 m/s: Timber Tumbler 4 cars, car 0 lateral +2.007 g at 12.65 m, 0.165 s | lateral +2.000 g, Timber Tumbler 4 cars, car 2 |
| `BankedStraightLeft` (UpGentle/Left → UpGentle/Left) | 20.000000–30.000000 | 19.25–100.00 | 20.000000–30.000000 | design cap | pass/pass | No failure to scan ceiling; design cap. | pass all |
| `BankedStraightLeft` (DownGentle/Left → DownGentle/Left) | 5.000000–30.000000 | 0.25–100.00 | 5.000000–30.000000 | design cap | pass/pass | No failure to scan ceiling; design cap. | pass all |
| `BankedStraightRight` (UpGentle/Right → UpGentle/Right) | 20.000000–30.000000 | 19.25–100.00 | 20.000000–30.000000 | design cap | pass/pass | No failure to scan ceiling; design cap. | pass all |
| `BankedStraightRight` (DownGentle/Right → DownGentle/Right) | 5.000000–30.000000 | 0.25–100.00 | 5.000000–30.000000 | design cap | pass/pass | No failure to scan ceiling; design cap. | pass all |
| `RolloutLeft` (UpGentle/Left → UpGentle/Level) | 22.000000–30.000000 | 23.00–55.25 | 22.870000–55.330000 | force-bound candidate | FAIL/pass | 55.50 m/s: Timber Tumbler 4 cars, car 0 lateral -2.008 g at 12.65 m, 0.140 s | lateral -2.000 g, Timber Tumbler 4 cars, car 0 |
| `RolloutLeft` (DownGentle/Left → DownGentle/Level) | 5.000000–30.000000 | 0.25–52.00 | 5.000000–52.160000 | force-bound candidate | pass/pass | 52.25 m/s: Timber Tumbler 8 cars, car 7 lateral -2.004 g at 12.79 m, 0.140 s | lateral -2.000 g, Timber Tumbler 8 cars, car 7 |
| `RolloutRight` (UpGentle/Right → UpGentle/Level) | 22.000000–30.000000 | 23.00–55.25 | 22.870000–55.330000 | force-bound candidate | FAIL/pass | 55.50 m/s: Timber Tumbler 4 cars, car 0 lateral +2.008 g at 12.65 m, 0.140 s | lateral +2.000 g, Timber Tumbler 4 cars, car 0 |
| `RolloutRight` (DownGentle/Right → DownGentle/Level) | 5.000000–30.000000 | 0.25–52.00 | 5.000000–52.160000 | force-bound candidate | pass/pass | 52.25 m/s: Timber Tumbler 8 cars, car 7 lateral +2.004 g at 12.79 m, 0.140 s | lateral +2.000 g, Timber Tumbler 8 cars, car 7 |

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
| Binding axis at each force-bound candidate maximum | The table reports the first failure at maximum + 0.01 m/s where observed. Ten force-bound candidates pass that sample; the fine-grid scans below identify the first failure at a higher sampled speed, not the exact boundary. |
| No lost windows and continuous passing range | Every template had a contiguous passing range on the 0.25 m/s scan grid and refined candidate endpoints; failures between grid samples remain possible. |
| Required window width | No width failures were reported by the generation run. |
| Ejector hills classify Great and never Dangerous at each maximum | Failed: 11 configurations are Good or TooMuch at candidate maxima; no Dangerous result was reported. |
| Catalog build and `make bench-construction` unchanged | Candidate comparison not measured; rated-window values remain unchanged. |
| Restated rated-element, inversion, player-circuit, statistics, and targeted gates | Not reported here; these still need to run against an accepted table. |

The starting-tree construction benchmark was recorded as a single run: first catalog access took 7.4 ms for 249 catalog entries; first hybrid, multi-launch, and wooden choice searches took 41.6, 44.7, and 17.5 ms; first element-choice searches took 90.8, 89.0, and 30.4 ms; cached queries took 0.17–0.18 ms. These are baseline values only. A post-table comparison has not been measured.

A separate held-speed scan tested every candidate maximum plus 0.01 m/s, using the six-decimal candidate values in the refinement log. It found first failures on the vertical axis for 54 templates, lateral for 20, longitudinal for 2, and the 22 m/s clearing-speed condition for 2; 16 passed all allowed configurations. Of the eight design caps, six passed and two failed the clearing-speed condition. The full model, car, force, and location diagnostics are in `/tmp/task126-near-boundaries.log`; its disposable source is `/tmp/task126-boundary/src/main.rs`.

A 0.001 m/s scan of all allowed configurations found SixtyToSteepDown passed from 29.250 through 29.350 m/s and failed from 29.351 through 29.500 m/s; HalfLoopDown passed from 18.750 through 18.760 m/s and failed from 18.761 through 19.000 m/s. Both failures involved vertical force on car 7 of an eight-car Timber Tumbler. Seven bisection steps returned passing lower endpoints of 29.349609375 and 18.759765625 m/s, which were floored to 29.34 and 18.75 m/s, respectively. This finite precision explains why maximum + 0.01 m/s passed for these two; there were no sampled pass/fail gaps in either band. The complete scans and bisection traces are `/tmp/task126-fine-boundaries.log` and `/tmp/task126-bisection-trace.log`.

The other eight force-bound candidates that passed maximum + 0.01 m/s were also scanned at 0.001 m/s intervals through maximum + 0.03 m/s, evaluating every allowed configuration. Each passing lower endpoint from seven bisection steps was below the next hundredth, which was itself a passing sample. Flooring therefore explains these eight cases as well. No sampled pass/fail gaps appeared in these intervals; this does not establish continuity between samples. The first failures below are listed in model and car-count order, and the full results and bisection traces are `/tmp/task126-remaining-fine-boundaries.log` and `/tmp/task126-remaining-bisection.log`.

| Template | Passing sampled range (m/s) | Failing sampled range (m/s) | First failing configuration and force |
| --- | ---: | ---: | --- |
| RavenRollLeftDown | 19.830–19.841 | 19.842–19.860 | Timber Tumbler 5 cars, car 4 lateral −1.500678 g |
| RavenRollRightDown | 19.830–19.841 | 19.842–19.860 | Timber Tumbler 5 cars, car 4 lateral +1.500678 g |
| SteelLoop | 29.440–29.451 | 29.452–29.470 | Briar Blitz 12 cars, car 0 vertical +4.000879 g |
| SteelLoopRight | 29.440–29.451 | 29.452–29.470 | Briar Blitz 12 cars, car 0 vertical +4.000879 g |
| FloaterHill16x112 | 31.010–31.021 | 31.022–31.040 | Briar Blitz 12 cars, car 0 vertical +4.000104 g |
| SpeedHill6x80 | 28.910–28.920 | 28.921–28.940 | Briar Blitz 12 cars, car 11 vertical +4.500133 g |
| EjectorHill8x48OuterLeft | 21.680–21.691 | 21.692–21.710 | Timber Tumbler 8 cars, car 0 lateral +1.500401 g |
| EjectorHill8x48OuterRight | 21.680–21.691 | 21.692–21.710 | Timber Tumbler 8 cars, car 0 lateral −1.500401 g |

The coarse scan log was `/tmp/task126-force-scan.log`, the refinement output is `/tmp/task126-force-refine.log`, and the complete airtime classification output is `/tmp/task126-airtime-classes.log`. The scan implementation is `crates/sim/examples/measure_force_windows.rs`. Generation failed on the Great-at-maximum requirement, so the candidate values remain unaccepted.

A focused regression in `physics::rated_piece::tests::held_approach_matches_real_train_at_force_boundaries` now compares the shared held-speed harness with a train stepped along the level approach. It checks the first force violation, when present, and the minimum filtered vertical force for the three ejector hills and LevelToSixtyDown. The test passed against the current catalog; it does not establish acceptance of the candidate windows.

A disposable integration experiment checked the proposed lookup against 94 synthetic rows. The catalog initialized with all 94 distinct piece and slope keys, but the experiment did not validate their physics. The draft also needs its generated constant's visibility corrected for the lint gate. Its common lower bound includes both force and clearance failures, so applying it to every train would conservatively constrain shorter trains to the worst scanned train's clearance boundary; the intended treatment of train-specific minima needs to be resolved before applying the lookup.

The original connected-circuit force checks for crests, pulls, half-loops, half-rolls, banked drops and corkscrews have been restored with per-car evaluators and their original fixed starts. Their focused connected tests passed (7 tests). The crest test also retains its existing requirement that windows stay within 3–30 m/s. That requirement conflicts with the measured candidate maxima of 31.09 m/s for SixtyUpToLevel and 38.11 m/s for SixtyToSteepUp; it cannot hold alongside those candidate windows without an explicit change to the test expectation or the rating rule.

## Ejector airtime feasibility on the candidate force windows

A diagnostic sampled each candidate force window at 0.25 m/s intervals and evaluated every allowed model and car count. A speed is included below only when every configuration passes the force evaluator and its minimum filtered vertical force classifies as Great. The candidate maxima are the force-bound values above, not accepted catalog windows.

| Straight ejector hill | Sampled speeds where all configurations pass and classify Great (m/s) | Highest sampled speed (m/s) | Candidate force-bound maximum (m/s) |
| --- | --- | ---: | ---: |
| EjectorHill8x48 | 19.00 | 19.00 | 19.29 |
| EjectorHill12x64 | 21.50 | 21.50 | 21.71 |
| EjectorHill8x64 | 23.50–24.75 | 24.75 | 25.24 |
| EjectorHill12x80 | 25.00–26.25 | 26.25 | 26.53 |
| EjectorHill16x96 | 27.00–28.00 | 28.00 | 28.30 |
| WoodenEjectorHill6x48 | None | None | 20.01 |

For WoodenEjectorHill6x48, no sampled speed in the candidate window satisfies both requirements for every configuration. At 20.00 m/s, all Briar Blitz train lengths pass the force evaluator; 4–7 cars classify Good, 8–10 classify Great, and 11–12 classify TooMuch. These are grid observations and do not prove whether a common speed exists between samples. This diagnostic does not recommend changing rated windows to satisfy the Great-at-maximum test.

The scratch source and manifest are `/tmp/task126-airtime-feasibility/src/main.rs` and `/tmp/task126-airtime-feasibility/Cargo.toml`. From the repository root, reproduce the logged run with `tools/run-in-budget.sh cargo run --release --manifest-path /tmp/task126-airtime-feasibility/Cargo.toml > /tmp/task126-airtime-feasibility.log 2>&1`. The log records the filtered minimum, classification, and force result for each sampled configuration. These `/tmp` artifacts are temporary.
