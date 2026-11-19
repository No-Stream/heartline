---
id: TASK-146
title: Builder force estimate disagrees with the test run
status: To Do
assignee: []
created_date: '2026-11-18 13:05'
updated_date: '2026-11-18 17:37'
labels:
  - coaster
  - physics
dependencies: []
priority: high
ordinal: 145000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Owner playtest 2026-11-18: a layout looked fine in the builder's live force estimate, then the test run showed -2.0 g dangerous airtime and 1.7 g / -1.9 g too much longitudinal on the same stretch. The estimate must predict what the test measures (same cars, same filtering, same speed). Compare estimate and measurement sample by sample on the owner's exported session and fix the source of the difference. Related: TASK-126 (filtered limits), TASK-142, TASK-144 (longitudinal on pullouts).
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
2026-11-18: blocked on the owner's exported session or exact track. TASK-144's diagnostic (branch longitudinal-diag, ignored test diagnose_steep_drop_pullout_longitudinal_forces) found the measured path gives at most 0.63 g longitudinal on a 60-degree drop pullout, so the large longitudinal values come from the estimate. Leading suspect: the estimate steps at 50 ms and interpolates acceleration when correcting sample forces (physics/preview.rs:111, sim-wasm construction.rs:483).

2026-11-18 diagnosis on the first version of the new demo layout (TASK-140, branch demo-ride; built in the UI with a 34 m drop, later made 2 m shallower). Scratch comparison: for every piece, the builder estimate (preview_open_track, front car, 50 ms steps) against a 200 Hz run of the same train read for the front car and for every car.

Vertical g per piece (min / max), estimate front car | test front car | test worst car:
- Crest, GentleToSteepDown: 0.30 / 0.89 | 0.30 / 0.89 | -0.24 / 0.89 (back car)
- LevelToGentleDown at the crest: 0.66 | 0.66 | 0.28 (back car)
- SteepToGentleDown: 2.16 | 2.17 | 2.66 (back car)
- Main pullout, GentleDownToLevel: 3.91 | 3.92 | 4.20 (back car); longitudinal min -0.49 | -0.50 | -0.93
- Ejector hill 16x96: -0.82 / 3.42 | -0.82 / 3.43 | -0.82 / 3.43
- Stall: -0.41 / 3.49, lateral 0.66 in all three; longitudinal min -0.74 | -0.82 | -1.04
- Zero-g roll: -0.37 / 3.33 | same | -0.39; longitudinal min -0.86 | -1.05 | -1.19
- Ejector hill 8x64: -1.08 / 3.54 in all three; longitudinal min -0.88 | -0.88 | -1.12
- TerrainFall on the return: 0.73 / 1.28 | 0.71 / 1.29 | 0.59 / 1.33

Conclusions:
1. The 50 ms stepping is not the main source. Vertical and lateral estimates match the 200 Hz front car within 0.02 g on every piece. Longitudinal extremes match within 0.2 g; the largest per-sample difference is 0.36 g (estimate -0.82, run -0.46) at 606 m inside the zero-g roll. Recording each step's mean acceleration at the step midpoint instead of its end did not change that difference, so the half-step lag is not the cause; the remaining longitudinal error inside rolls is unexplained.
2. The large gaps are front car versus worst car. The estimate reads only the front car (step_preview passes reading_count 1), while the test run's statistics, force advice and ride info report the most extreme car. Back cars are whipped over crests and into pullouts faster: on this layout the back car gets -0.24 g over the main crest where the estimate shows +0.30 g, and 4.20 g in the pullout where it shows 3.91 g. The owner's case (estimate fine, test -2.0 g airtime and about 1.8 g longitudinal) fits this: a back car over a crest after a fast drop.
3. The hill readout is a separate, larger error: a point mass at entry speed, so about 0.15-0.4 g weaker airtime than even the front car (TASK-151).
4. Latent bug: TestRunRecorder::record uses super::TIME_STEP (5 ms) for force-band and comfort durations even when the preview feeds it 50 ms readings, so preview durations are 10 times too short. Nothing in the UI reads those preview durations today.

Proposed failing test (crates/sim/src/physics/tests.rs): take the demo prefix through GentleToSteepDown (pieces 0..=8), run preview_open_track, and assert its minimum vertical g on piece 8 is within 0.1 g of the worst-car minimum that ConstructionTestRunRecorder records for piece 8 on the closed demo run. On the committed demo: estimate 0.40 g, worst car -0.13 g. Fix direction: step the preview with readings for every car (reading_count = cars), correct each car's sample at its own distance, and report the worst car per sample, matching merge_worst_car_statistics.

The native agreement report was run on all five catalog rides on 2026-11-18. Each row lists placement, finished-track and measured-front entry speeds in m/s, followed by minimum and maximum vertical, absolute lateral and absolute longitudinal force peaks in g. The worst column is the per-axis envelope across cars. Station index 0 is omitted because it is not a placed piece. Failures are attributed to check 2 (finished estimate versus front car) or check 3 (placement estimate versus the worst-car envelope). Missing values are shown as an em dash. The demo-ride comparison requested above remains pending the other branch.

### briar-blitz

```text
piece check2 check3 entry[placement/finished/test m/s] placement[min/max/lat/long g] finished[min/max/lat/long g] front[min/max/lat/long g] worst[min/max/lat/long g]
1 LevelToGentleUp          Passed Passed 2.079/2.079/2.001 0.898/1.105/0.000/0.463 0.898/1.105/0.000/0.463 0.898/1.105/0.000/0.463 0.898/1.105/0.000/0.463
2 GentleUp                 Passed Passed 5.000/5.000/5.000 0.894/0.894/0.000/0.447 0.894/0.894/0.000/0.447 0.894/0.894/0.000/0.447 0.894/0.894/0.000/0.447
3 GentleUp                 Passed Passed 5.000/5.000/5.000 0.894/0.894/0.000/0.447 0.894/0.894/0.000/0.447 0.894/0.894/0.000/0.447 0.894/0.894/0.000/0.447
4 TurnSmallStandalone      Passed Passed 5.000/5.000/5.000 0.881/0.929/0.179/0.451 0.881/0.929/0.179/0.451 0.881/0.929/0.179/0.451 0.881/0.929/0.179/0.451
5 GentleUpToLevel          Passed Passed 5.000/5.000/5.000 0.830/0.992/0.000/0.463 0.830/0.992/0.000/0.463 0.830/0.992/0.000/0.463 0.830/0.992/0.000/0.463
6 Straight                 Passed Passed 5.000/5.000/5.000 1.000/1.000/0.000/0.000 1.000/1.000/0.000/0.000 1.000/1.000/0.000/0.000 1.000/1.000/0.000/0.000
7 Straight                 Passed Passed 5.000/5.000/5.000 1.000/1.000/0.000/0.000 1.000/1.000/0.000/0.000 1.000/1.000/0.000/0.000 1.000/1.000/0.000/0.208
8 LevelToGentleDown        Passed Failed 5.000/5.000/5.000 0.813/1.000/0.000/0.312 0.813/1.000/0.000/0.312 0.813/1.000/0.000/0.308 0.416/1.000/0.000/0.370
9 BankLeft                 Passed Passed 6.999/6.999/7.133 0.633/0.894/0.897/0.206 0.633/0.894/0.897/0.206 0.633/0.894/0.897/0.202 0.598/0.894/1.150/0.202
10 BankedTurnStandalone     Passed Failed 13.585/13.585/13.752 0.633/2.127/1.322/0.475 0.633/2.127/1.322/0.475 0.634/2.127/1.322/0.476 0.634/2.665/2.041/0.690
11 UnbankLeft               Passed Failed 20.476/20.476/20.658 0.491/0.893/1.410/0.038 0.491/0.893/1.410/0.038 0.491/0.893/1.410/0.037 0.401/0.893/1.659/0.191
12 GentleDownToLevel        Passed Failed 23.874/23.874/23.882 0.895/4.100/0.000/1.205 0.895/4.100/0.000/1.205 0.895/4.100/0.007/1.192 0.895/4.405/0.013/1.192
13 Straight                 Passed Passed 26.330/26.330/26.359 1.000/1.000/0.000/0.234 1.000/1.000/0.000/0.234 1.000/1.000/0.000/0.219 1.000/1.000/0.000/0.219
14 Straight                 Passed Passed 26.732/26.732/26.731 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010
15 Straight                 Passed Passed 26.673/26.673/26.672 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010
16 Straight                 Passed Passed 26.614/26.614/26.613 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010
17 Straight                 Passed Passed 26.555/26.555/26.554 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010
18 Straight                 Passed Passed 26.496/26.496/26.494 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010
19 Straight                 Passed Passed 26.437/26.437/26.435 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.055
20 TopHat                   Passed Passed 26.377/26.377/26.376 -0.392/3.322/0.000/0.624 -0.392/3.322/0.000/0.624 -0.392/3.322/0.000/0.610 -0.392/3.322/0.000/0.613
21 Straight                 Passed Passed 25.874/25.874/25.879 1.000/1.000/0.000/0.051 1.000/1.000/0.000/0.051 1.000/1.000/0.000/0.043 1.000/1.000/0.000/0.043
22 Straight                 Passed Passed 25.889/25.889/25.888 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010
23 Straight                 Passed Passed 25.829/25.829/25.829 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010
24 Straight                 Passed Passed 25.770/25.770/25.770 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010
25 Straight                 Passed Passed 25.711/25.711/25.711 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010
26 Straight                 Passed Passed 25.652/25.652/25.651 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010
27 Straight                 Passed Passed 25.593/25.593/25.592 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010
28 Straight                 Passed Passed 25.534/25.534/25.533 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010
29 Straight                 Passed Passed 25.475/25.475/25.474 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010
30 Straight                 Passed Passed 25.416/25.416/25.415 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010 1.000/1.000/0.000/0.010
31 TightTwisterLeft         Passed Passed 25.357/25.357/25.356 1.000/1.000/7.203/0.010 1.000/1.000/7.203/0.010 1.000/1.000/7.203/0.010 1.000/1.000/7.203/0.010
32 TightTwisterRight        Passed Passed 25.283/25.283/25.282 1.000/1.000/7.156/0.010 1.000/1.000/7.156/0.010 1.000/1.000/7.156/0.010 1.000/1.000/7.156/0.010
33 TightTwisterLeft         Passed Passed 25.210/25.210/25.208 1.000/1.000/7.107/0.010 1.000/1.000/7.107/0.010 1.000/1.000/7.107/0.010 1.000/1.000/7.107/0.010
34 TightTwisterLeft         Passed Passed 25.136/25.136/25.134 1.000/1.000/7.070/0.009 1.000/1.000/7.070/0.009 1.000/1.000/7.070/0.009 1.000/1.000/7.070/0.009
35 Straight                 Passed Passed 25.063/25.063/25.060 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009
36 Straight                 Passed Passed 25.004/25.004/25.001 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009
37 Straight                 Passed Passed 24.945/24.945/24.942 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009
38 Straight                 Passed Passed 24.885/24.885/24.883 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009
39 Straight                 Passed Passed 24.826/24.826/24.824 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009
40 Straight                 Passed Passed 24.767/24.767/24.764 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009
41 Straight                 Passed Passed 24.708/24.708/24.705 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009
42 Straight                 Passed Passed 24.649/24.649/24.646 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009
43 Straight                 Passed Passed 24.589/24.589/24.587 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009
44 Straight                 Passed Passed 24.530/24.530/24.527 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009
45 Straight                 Passed Passed 24.471/24.471/24.468 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009
46 Straight                 Passed Passed 24.412/24.412/24.409 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009
47 Straight                 Passed Passed 24.352/24.352/24.350 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009
48 Straight                 Passed Passed 24.293/24.293/24.290 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009
49 Straight                 Passed Passed 24.234/24.234/24.231 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009
50 Straight                 Passed Passed 24.174/24.174/24.172 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009
51 Straight                 Passed Passed 24.115/24.115/24.112 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009
52 Straight                 Passed Passed 24.056/24.056/24.053 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009
53 Straight                 Passed Passed 23.996/23.996/23.994 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009
54 Straight                 Passed Passed 23.937/23.937/23.934 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009
55 Straight                 Passed Passed 23.878/23.878/23.875 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009
56 TurnLargeRightStandalone Passed Passed 23.818/23.818/23.815 1.000/1.000/2.376/0.009 1.000/1.000/2.376/0.009 1.000/1.000/2.376/0.010 1.000/1.000/2.376/0.010
57 Straight                 Passed Passed 23.621/23.621/23.619 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.009
58 TurnSmallStandalone      Passed Passed 23.562/23.562/23.559 1.000/1.000/4.657/0.009 1.000/1.000/4.657/0.009 1.000/1.000/4.657/0.009 1.000/1.000/4.657/0.009
8 PlacementEstimateVsWorstCar: minimum vertical g (estimate 0.813 g differs from measurement 0.416 g by 0.397 g, over the 0.3 g tolerance)
10 PlacementEstimateVsWorstCar: maximum vertical g (estimate 2.127 g differs from measurement 2.665 g by 0.539 g, over the 0.3 g tolerance)
10 PlacementEstimateVsWorstCar: maximum vertical g (estimated rating Good is milder than measured rating Great)
10 PlacementEstimateVsWorstCar: lateral g (estimate 1.322 g differs from measurement 2.041 g by 0.718 g, over the 0.3 g tolerance)
10 PlacementEstimateVsWorstCar: lateral g (estimated rating Strong is milder than measured rating Dangerous)
11 PlacementEstimateVsWorstCar: lateral g (estimated rating Strong is milder than measured rating TooMuch)
12 PlacementEstimateVsWorstCar: maximum vertical g (estimate 4.100 g differs from measurement 4.405 g by 0.305 g, over the 0.3 g tolerance)
Agreement: fail
```

### daisy-dash

```text
piece check2 check3 entry[placement/finished/test m/s] placement[min/max/lat/long g] finished[min/max/lat/long g] front[min/max/lat/long g] worst[min/max/lat/long g]
1 LevelToGentleUp          Passed Passed 2.079/2.079/2.001 0.898/1.105/0.000/0.463 0.898/1.105/0.000/0.463 0.898/1.105/0.000/0.463 0.898/1.105/0.000/0.463
2 TurnSmallStandalone      Passed Passed 5.000/5.000/5.000 0.881/0.929/0.180/0.451 0.881/0.929/0.180/0.451 0.881/0.929/0.180/0.451 0.881/0.929/0.180/0.451
3 GentleUpToLevel          Passed Passed 5.000/5.000/5.000 0.830/1.000/0.000/0.463 0.830/1.000/0.000/0.463 0.830/1.000/0.000/0.463 0.830/1.000/0.000/0.463
4 LevelToGentleDown        Passed Failed 5.000/5.000/5.000 0.815/0.991/0.000/0.283 0.815/0.991/0.000/0.283 0.815/0.991/0.000/0.279 0.487/0.991/0.000/0.324
5 OneTileGentleDown        Passed Passed 7.215/7.215/7.285 0.894/0.894/0.000/0.164 0.894/0.894/0.000/0.164 0.894/0.894/0.000/0.159 0.894/0.894/0.000/0.159
6 GentleDownToLevel        Passed Passed 9.049/9.049/9.350 0.905/1.763/0.000/0.498 0.905/1.763/0.000/0.498 0.905/1.763/0.000/0.490 0.905/2.000/0.000/0.490
7 FamilyHill4x40           Passed Passed 14.143/14.143/14.261 -0.237/2.113/0.000/0.900 -0.237/2.113/0.000/0.900 -0.237/2.114/0.000/0.887 -0.237/2.116/0.000/0.887
8 LevelToGentleDown        Passed Failed 13.921/13.921/14.024 -0.163/0.941/0.000/0.477 -0.163/0.941/0.000/0.477 -0.163/0.941/0.000/0.468 -0.479/0.941/0.000/0.572
9 OneTileGentleDown        Passed Passed 15.436/15.436/15.452 0.894/0.894/0.000/0.174 0.894/0.894/0.000/0.174 0.894/0.894/0.000/0.164 0.894/0.894/0.000/0.164
10 GentleDownToLevel        Passed Passed 16.370/16.370/16.516 0.919/2.644/0.000/0.745 0.919/2.644/0.000/0.745 0.919/2.644/0.000/0.733 0.919/2.895/0.000/0.733
11 SpeedTurnLarge20Left     Passed Passed 19.634/19.634/19.713 1.000/2.253/0.149/0.362 1.000/2.253/0.149/0.362 1.000/2.253/0.150/0.363 1.000/2.253/0.152/0.363
12 LevelToGentleUp          Passed Passed 19.733/19.733/19.736 0.907/2.849/0.000/0.625 0.907/2.849/0.000/0.625 0.907/2.849/0.000/0.614 0.903/2.849/0.000/0.740
13 OneTileGentleUp          Passed Passed 18.926/18.926/18.834 0.894/0.894/0.000/0.147 0.894/0.894/0.000/0.147 0.894/0.894/0.000/0.135 0.894/0.894/0.000/0.135
14 GentleUpToLevel          Passed Passed 18.090/18.090/18.062 -0.386/0.967/0.000/0.571 -0.386/0.967/0.000/0.571 -0.386/0.967/0.000/0.561 -0.386/0.971/0.000/0.561
15 OneTileStraight          Passed Passed 14.311/14.311/14.254 1.000/1.000/0.000/0.198 1.000/1.000/0.000/0.198 1.000/1.000/0.000/0.189 1.000/1.000/0.000/0.189
16 FamilyHill4x40           Passed Failed 13.910/13.910/13.882 -0.056/1.988/0.000/0.739 -0.056/1.988/0.000/0.739 -0.056/1.988/0.000/0.728 -0.056/1.988/0.000/0.816
17 FamilyHill4x40           Passed Failed 12.951/12.951/12.991 -0.024/1.945/0.000/0.745 -0.024/1.945/0.000/0.745 -0.024/1.946/0.000/0.734 -0.024/1.948/0.000/0.785
18 TerrainRise              Passed Passed 12.755/12.755/12.778 -0.030/1.935/0.000/0.700 -0.030/1.935/0.000/0.700 -0.030/1.935/0.000/0.691 -0.030/1.942/0.000/0.691
19 LevelToGentleDown        Passed Failed 12.398/12.398/12.294 0.218/0.944/0.000/0.403 0.218/0.944/0.000/0.403 0.218/0.944/0.000/0.395 -0.091/0.949/0.000/0.475
20 GentleDownToLevel        Passed Passed 12.887/12.887/12.928 0.896/2.120/0.000/0.608 0.896/2.120/0.000/0.608 0.896/2.120/0.000/0.597 0.896/2.359/0.000/0.597
21 SpeedTurnLarge16Left     Passed Passed 16.583/16.583/16.635 1.000/1.703/0.040/0.189 1.000/1.703/0.040/0.189 1.000/1.703/0.040/0.190 1.000/1.703/0.042/0.310
22 Brakes                   Passed Passed 16.717/16.717/16.619 1.000/1.000/0.000/0.313 1.000/1.000/0.000/0.313 1.000/1.000/0.000/0.313 1.000/1.000/0.000/0.313
23 FamilyHill4x40           Passed Passed 13.609/13.609/13.559 -0.036/1.966/0.000/0.836 -0.036/1.966/0.000/0.836 -0.033/1.964/0.000/0.824 -0.033/1.964/0.000/0.824
24 OneTileStraight          Passed Passed 12.770/12.770/12.839 1.000/1.000/0.000/0.174 1.000/1.000/0.000/0.174 1.000/1.000/0.000/0.167 1.000/1.000/0.000/0.167
25 OneTileStraight          Passed Passed 13.180/13.180/13.206 1.000/1.000/0.000/0.084 1.000/1.000/0.000/0.084 1.000/1.000/0.000/0.078 1.000/1.000/0.000/0.078
26 Straight                 Passed Passed 13.329/13.329/13.321 1.000/1.000/0.000/0.013 1.000/1.000/0.000/0.013 1.000/1.000/0.000/0.009 1.000/1.000/0.000/0.125
27 TerrainFall              Passed Failed 13.275/13.275/13.258 -0.112/2.003/0.000/0.701 -0.112/2.003/0.000/0.701 -0.109/2.000/0.000/0.695 -0.270/2.169/0.000/0.744
28 TerrainFall              Passed Passed 14.046/14.046/14.068 -0.313/2.186/0.000/0.767 -0.313/2.186/0.000/0.767 -0.311/2.184/0.000/0.765 -0.505/2.322/0.000/0.892
29 SpeedTurnLarge16Left     Passed Passed 15.309/15.309/15.313 1.000/1.546/0.135/0.157 1.000/1.546/0.135/0.157 1.000/1.544/0.136/0.158 1.000/1.544/0.137/0.158
30 TerrainRise              Passed Passed 15.535/15.535/15.525 -0.441/2.308/0.000/0.856 -0.441/2.308/0.000/0.856 -0.439/2.306/0.000/0.849 -0.439/2.306/0.000/0.849
31 Straight                 Passed Passed 14.744/14.744/14.705 1.000/1.000/0.000/0.140 1.000/1.000/0.000/0.140 1.000/1.000/0.000/0.139 1.000/1.000/0.000/0.139
32 Straight                 Passed Passed 14.113/14.113/14.098 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.177
33 FamilyHill4x40           Passed Passed 14.039/14.039/14.024 -0.112/2.031/0.000/0.784 -0.112/2.031/0.000/0.784 -0.109/2.029/0.000/0.771 -0.109/2.029/0.000/0.831
34 TerrainFall              Passed Failed 13.258/13.258/13.265 -0.176/2.092/0.000/0.723 -0.176/2.092/0.000/0.723 -0.173/2.089/0.000/0.721 -0.357/2.222/0.000/0.828
35 Straight                 Passed Passed 14.594/14.594/14.660 1.000/1.000/0.000/0.125 1.000/1.000/0.000/0.125 1.000/1.000/0.000/0.124 1.000/1.000/0.000/0.313
36 Brakes                   Passed Passed 15.010/15.010/14.852 1.000/1.000/0.000/0.313 1.000/1.000/0.000/0.313 1.000/1.000/0.000/0.313 1.000/1.000/0.000/0.313
37 Brakes                   Passed Passed 11.305/11.305/11.073 1.000/1.000/0.000/0.312 1.000/1.000/0.000/0.312 1.000/1.000/0.000/0.312 1.000/1.000/0.000/0.312
4 PlacementEstimateVsWorstCar: minimum vertical g (estimate 0.815 g differs from measurement 0.487 g by 0.328 g, over the 0.3 g tolerance)
8 PlacementEstimateVsWorstCar: minimum vertical g (estimate -0.163 g differs from measurement -0.479 g by 0.316 g, over the 0.3 g tolerance)
8 PlacementEstimateVsWorstCar: minimum vertical g (estimated rating Weak is milder than measured rating Good)
16 PlacementEstimateVsWorstCar: longitudinal g (estimated rating Fine is milder than measured rating Strong)
17 PlacementEstimateVsWorstCar: longitudinal g (estimated rating Fine is milder than measured rating Strong)
19 PlacementEstimateVsWorstCar: minimum vertical g (estimate 0.218 g differs from measurement -0.091 g by 0.309 g, over the 0.3 g tolerance)
27 PlacementEstimateVsWorstCar: minimum vertical g (estimated rating Weak is milder than measured rating Good)
34 PlacementEstimateVsWorstCar: minimum vertical g (estimated rating Weak is milder than measured rating Good)
34 PlacementEstimateVsWorstCar: longitudinal g (estimated rating Fine is milder than measured rating Strong)
Agreement: fail
```

### horizon-hawk

```text
piece check2 check3 entry[placement/finished/test m/s] placement[min/max/lat/long g] finished[min/max/lat/long g] front[min/max/lat/long g] worst[min/max/lat/long g]
1 LevelToGentleUp          Passed Passed 2.079/2.034/2.003 0.898/1.105/0.000/0.463 0.898/1.105/0.000/0.463 0.898/1.105/0.000/0.463 0.898/1.105/0.000/0.463
2 GentleToSteepUp          Passed Passed 5.000/5.000/5.000 0.708/0.929/0.000/0.712 0.708/0.929/0.000/0.712 0.708/0.929/0.000/0.712 0.708/0.929/0.000/0.712
3 SteepUp                  Passed Passed 5.000/5.000/5.000 0.707/0.707/0.000/0.707 0.707/0.707/0.000/0.707 0.707/0.707/0.000/0.707 0.707/0.707/0.000/0.707
4 SteepUp                  Passed Passed 5.000/5.000/5.000 0.707/0.707/0.000/0.707 0.707/0.707/0.000/0.707 0.707/0.707/0.000/0.707 0.707/0.707/0.000/0.707
5 SteepToGentleUp          Passed Passed 5.000/5.000/5.000 0.691/0.894/0.000/0.712 0.691/0.894/0.000/0.712 0.691/0.894/0.000/0.712 0.691/0.894/0.000/0.712
6 GentleUpToLevel          Passed Passed 5.000/5.000/5.000 0.830/1.000/0.000/0.463 0.830/1.000/0.000/0.463 0.830/1.000/0.000/0.463 0.816/1.000/0.000/0.469
7 WideLevelToSixtyDown     Passed Failed 5.000/5.000/5.000 0.435/0.982/0.000/0.541 0.435/0.982/0.000/0.541 0.435/0.982/0.000/0.532 -0.803/0.982/0.000/1.195
8 WideSixtyDownToLevel     Passed Failed 17.222/17.222/17.624 0.500/2.970/0.000/0.254 0.500/2.971/0.000/0.254 0.500/2.971/0.000/0.240 0.500/3.162/0.000/0.323
9 Brakes                   Passed Passed 34.552/34.579/34.616 1.000/1.000/0.000/0.323 1.000/1.000/0.000/0.323 1.000/1.000/0.000/0.323 1.000/1.000/0.000/0.323
10 FloaterHill24x160        Passed Passed 33.201/33.200/33.126 -0.676/3.197/0.000/0.455 -0.676/3.197/0.000/0.455 -0.665/3.187/0.000/0.436 -0.665/3.187/0.000/0.455
11 SpeedTurnLarge32Left     Passed Passed 32.437/32.437/32.368 1.000/3.152/0.072/0.343 1.000/3.152/0.072/0.343 1.000/3.139/0.076/0.338 1.000/3.139/0.077/0.338
12 Brakes                   Passed Passed 31.832/31.860/31.768 1.000/1.000/0.000/0.320 1.000/1.000/0.000/0.320 1.000/1.000/0.000/0.321 1.000/1.000/0.000/0.321
13 Brakes                   Passed Passed 30.227/30.256/30.155 1.000/1.000/0.000/0.321 1.000/1.000/0.000/0.321 1.000/1.000/0.000/0.320 1.000/1.000/0.000/0.320
14 FloaterHill20x128        Passed Passed 28.626/28.626/28.519 -0.604/3.074/0.000/0.534 -0.604/3.074/0.000/0.534 -0.587/3.059/0.000/0.519 -0.587/3.059/0.000/0.541
15 FloaterHill20x128        Passed Passed 27.989/27.989/27.887 -0.512/2.995/0.000/0.530 -0.512/2.995/0.000/0.529 -0.496/2.981/0.000/0.515 -0.496/2.981/0.000/0.521
16 SpeedTurnLarge28Left     Passed Passed 27.435/27.435/27.340 1.000/2.979/0.152/0.403 1.000/2.979/0.152/0.402 1.000/2.959/0.158/0.399 1.000/2.959/0.159/0.399
17 Brakes                   Passed Passed 27.059/26.955/26.921 1.000/1.000/0.000/0.317 1.000/1.000/0.000/0.317 1.000/1.000/0.000/0.317 1.000/1.000/0.000/0.317
18 FloaterHill16x112        Passed Passed 25.254/25.116/25.105 -0.036/3.036/0.000/0.689 -0.019/3.012/0.000/0.683 -0.020/3.013/0.000/0.673 -0.020/3.013/0.000/0.680
19 FloaterHill12x96         Passed Passed 24.572/24.421/24.449 -0.122/3.114/0.000/0.796 -0.105/3.089/0.000/0.789 -0.105/3.090/0.000/0.780 -0.105/3.090/0.000/0.782
20 FloaterHill12x96         Passed Passed 24.081/23.930/23.963 -0.076/3.043/0.000/0.778 -0.059/3.018/0.000/0.771 -0.059/3.019/0.000/0.762 -0.059/3.019/0.000/0.763
21 FloaterHill12x96         Passed Passed 23.669/23.519/23.550 -0.030/2.975/0.000/0.761 -0.014/2.951/0.000/0.754 -0.014/2.952/0.000/0.746 -0.014/2.952/0.000/0.746
22 Straight                 Passed Passed 23.258/23.109/23.139 1.000/1.000/0.000/0.145 1.000/1.000/0.000/0.145 1.000/1.000/0.000/0.135 1.000/1.000/0.000/0.317
23 Brakes                   Passed Passed 23.447/23.289/23.292 1.000/1.000/0.000/0.317 1.000/1.000/0.000/0.317 1.000/1.000/0.000/0.317 1.000/1.000/0.000/0.317
24 SpeedTurnLarge20Left     Passed Passed 21.328/21.172/21.114 1.000/2.512/0.049/0.376 1.000/2.482/0.061/0.370 1.000/2.471/0.066/0.370 1.000/2.471/0.068/0.370
25 FloaterHill6x64          Passed Passed 20.979/20.831/20.782 -0.023/2.845/0.000/1.013 -0.007/2.820/0.000/1.001 0.002/2.810/0.000/0.989 0.002/2.810/0.000/0.989
26 FloaterHill6x64          Passed Passed 20.256/20.106/20.078 0.008/2.779/0.000/0.983 0.024/2.754/0.000/0.972 0.030/2.744/0.000/0.956 0.030/2.748/0.000/0.970
27 FloaterHill6x64          Passed Passed 19.971/19.820/19.832 0.039/2.731/0.000/1.012 0.055/2.706/0.000/1.001 0.062/2.696/0.000/0.985 0.062/2.700/0.000/0.985
28 FloaterHill8x64          Passed Passed 19.686/19.534/19.505 -0.198/3.040/0.000/1.152 -0.178/3.010/0.000/1.140 -0.170/2.999/0.000/1.123 -0.170/3.001/0.000/1.123
29 FloaterHill8x64          Passed Passed 19.276/19.123/19.137 -0.159/2.981/0.000/1.113 -0.138/2.951/0.000/1.100 -0.131/2.940/0.000/1.082 -0.131/2.944/0.000/1.211
30 Brakes                   Passed Passed 18.985/18.720/18.731 1.000/1.000/0.000/0.313 1.000/1.000/0.000/0.311 1.000/1.000/0.000/0.313 1.000/1.000/0.000/0.313
31 SpeedTurnLarge16Left     Passed Passed 17.077/16.785/16.753 1.000/1.710/0.043/0.178 1.000/1.671/0.047/0.171 1.000/1.667/0.050/0.171 1.000/1.667/0.052/0.309
32 Brakes                   Passed Passed 16.612/16.405/16.373 1.000/1.000/0.000/0.312 1.000/1.000/0.000/0.312 1.000/1.000/0.000/0.312 1.000/1.000/0.000/0.312
33 TerrainFall              Passed Failed 13.485/13.324/13.183 -0.140/2.014/0.000/0.728 -0.116/1.993/0.000/0.717 -0.097/1.976/0.000/0.704 -0.234/2.050/0.000/0.721
34 OneTileStraight          Passed Passed 13.896/13.896/13.786 1.000/1.000/0.000/0.093 1.000/1.000/0.000/0.093 1.000/1.000/0.000/0.093 1.000/1.000/0.000/0.204
7 PlacementEstimateVsWorstCar: minimum vertical g (estimate 0.435 g differs from measurement -0.803 g by 1.237 g, over the 0.3 g tolerance)
7 PlacementEstimateVsWorstCar: minimum vertical g (estimated rating Weak is milder than measured rating Good)
7 PlacementEstimateVsWorstCar: longitudinal g (estimate 0.541 g differs from measurement 1.195 g by 0.654 g, over the 0.3 g tolerance)
7 PlacementEstimateVsWorstCar: longitudinal g (estimated rating Fine is milder than measured rating Strong)
8 PlacementEstimateVsWorstCar: maximum vertical g (estimated rating Good is milder than measured rating Great)
33 PlacementEstimateVsWorstCar: minimum vertical g (estimated rating Weak is milder than measured rating Good)
Agreement: fail
```

### timber-tumbler

```text
piece check2 check3 entry[placement/finished/test m/s] placement[min/max/lat/long g] finished[min/max/lat/long g] front[min/max/lat/long g] worst[min/max/lat/long g]
1 LevelToGentleUp          Passed Passed 2.079/2.079/2.001 0.898/1.105/0.000/0.463 0.898/1.105/0.000/0.463 0.898/1.105/0.000/0.463 0.898/1.105/0.000/0.463
2 GentleToSteepUp          Passed Passed 5.000/5.000/5.000 0.708/0.929/0.000/0.712 0.708/0.929/0.000/0.712 0.708/0.929/0.000/0.712 0.708/0.929/0.000/0.712
3 SteepToGentleUp          Passed Passed 5.000/5.000/5.000 0.691/0.894/0.000/0.712 0.691/0.894/0.000/0.712 0.691/0.894/0.000/0.712 0.691/0.894/0.000/0.712
4 GentleUpToLevel          Passed Passed 5.000/5.000/5.000 0.830/0.994/0.000/0.463 0.830/0.994/0.000/0.463 0.830/0.994/0.000/0.463 0.830/0.994/0.000/0.463
5 LevelToGentleDown        Passed Failed 5.000/5.000/5.000 0.820/0.999/0.000/0.294 0.820/0.999/0.000/0.294 0.820/0.999/0.000/0.290 0.460/0.999/0.000/0.340
6 GentleToSteepDown        Passed Failed 6.989/6.989/7.160 0.406/0.892/0.000/0.222 0.406/0.892/0.000/0.222 0.406/0.892/0.000/0.218 -0.124/0.892/0.000/0.306
7 SteepToGentleDown        Passed Failed 15.244/15.244/15.378 0.708/2.052/0.000/0.473 0.708/2.052/0.000/0.473 0.708/2.052/0.000/0.466 0.708/2.545/0.000/0.466
8 GentleDownToLevel        Passed Passed 22.048/22.048/22.096 0.898/3.734/0.000/1.081 0.898/3.734/0.000/1.081 0.899/3.735/0.000/1.067 0.899/4.018/0.000/1.067
9 EjectorHill16x96         Passed Passed 24.815/24.815/24.854 -0.687/3.294/0.000/0.740 -0.687/3.294/0.000/0.740 -0.686/3.294/0.000/0.726 -0.686/3.294/0.000/0.726
10 EjectorHill16x96         Passed Passed 24.608/24.608/24.615 -0.587/3.208/0.000/0.726 -0.587/3.208/0.000/0.726 -0.587/3.208/0.000/0.715 -0.587/3.208/0.000/0.721
11 ZeroGRollFastLeft        Passed Passed 24.127/24.127/24.155 -0.369/3.326/0.467/1.093 -0.369/3.326/0.467/1.093 -0.369/3.326/0.468/1.083 -0.391/3.326/0.469/1.114
12 ZeroGRollFastRight       Passed Passed 23.522/23.522/23.548 -0.400/3.236/0.482/1.125 -0.400/3.236/0.482/1.125 -0.400/3.236/0.483/1.113 -0.424/3.236/0.483/1.113
13 SpeedTurnLarge24Left     Passed Passed 23.088/23.088/23.091 1.000/3.128/0.230/0.639 1.000/3.128/0.230/0.639 1.000/3.128/0.231/0.641 1.000/3.128/0.231/0.643
14 EjectorHill12x80OuterLeft Passed Passed 23.036/23.036/23.043 -0.272/3.163/0.365/0.826 -0.272/3.163/0.365/0.826 -0.272/3.163/0.365/0.815 -0.272/3.163/0.365/0.821
15 StallLeft                Passed Passed 22.466/22.466/22.496 -0.607/3.031/0.729/0.891 -0.607/3.031/0.729/0.891 -0.607/3.031/0.730/0.881 -0.607/3.031/0.730/1.143
16 Brakes                   Passed Passed 21.850/21.850/21.933 1.000/1.000/0.000/0.317 1.000/1.000/0.000/0.317 1.000/1.000/0.000/0.317 1.000/1.000/0.000/0.317
17 SpeedTurnLarge20Left     Passed Passed 19.774/19.774/19.859 1.000/2.196/0.178/0.324 1.000/2.196/0.178/0.324 1.000/2.219/0.169/0.329 1.000/2.219/0.170/0.329
18 FloaterHill12x96         Passed Passed 19.377/19.377/19.504 0.441/2.295/0.000/0.547 0.441/2.295/0.000/0.547 0.430/2.311/0.000/0.542 0.430/2.311/0.000/0.553
19 WaveTurnLeft             Passed Passed 18.757/18.757/18.926 1.066/1.498/0.971/0.310 1.066/1.498/0.971/0.310 1.067/1.505/0.971/0.310 1.065/1.505/0.972/0.310
20 FloaterHill12x80         Passed Passed 18.517/18.517/18.661 0.244/2.574/0.000/0.748 0.244/2.574/0.000/0.748 0.229/2.595/0.000/0.744 0.229/2.595/0.000/0.744
21 Straight                 Passed Passed 17.963/17.963/18.162 1.000/1.000/0.000/0.154 1.000/1.000/0.000/0.275 1.000/1.000/0.000/0.144 1.000/1.000/0.000/0.316
22 Brakes                   Passed Passed 18.130/18.130/18.241 1.000/1.000/0.000/0.315 1.000/1.000/0.000/0.315 1.000/1.000/0.000/0.316 1.000/1.000/0.000/0.316
23 Brakes                   Passed Passed 15.177/15.177/15.295 1.000/1.000/0.000/0.314 1.000/1.000/0.000/0.314 1.000/1.000/0.000/0.314 1.000/1.000/0.000/0.314
24 Straight                 Passed Passed 11.656/11.656/11.875 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007
25 TurnLargeRightStandalone Passed Passed 11.562/11.562/11.782 1.000/1.000/0.552/0.007 1.000/1.000/0.552/0.007 1.000/1.000/0.574/0.007 1.000/1.000/0.574/0.007
26 Straight                 Passed Passed 11.250/11.250/11.473 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007
27 Straight                 Passed Passed 11.155/11.155/11.379 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007
28 Straight                 Passed Passed 11.060/11.060/11.285 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007
29 Straight                 Passed Passed 10.964/10.964/11.190 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007
30 Straight                 Passed Passed 10.868/10.868/11.095 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007
31 Straight                 Passed Passed 10.772/10.772/10.999 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007
32 Straight                 Passed Passed 10.675/10.675/10.903 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007
33 Straight                 Passed Passed 10.577/10.577/10.807 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007
34 Straight                 Passed Passed 10.479/10.479/10.710 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007
35 Straight                 Passed Passed 10.381/10.381/10.613 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007
36 Straight                 Passed Passed 10.282/10.282/10.515 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007
37 Straight                 Passed Passed 10.182/10.182/10.417 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007
38 Straight                 Passed Passed 10.082/10.082/10.318 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007
39 Straight                 Passed Passed 9.982/9.982/10.219 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006
40 Straight                 Passed Passed 9.881/9.881/10.119 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006
41 Straight                 Passed Passed 9.779/9.779/10.018 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006
42 Straight                 Passed Passed 9.677/9.677/9.918 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006
43 Straight                 Passed Passed 9.574/9.574/9.816 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006
44 Straight                 Passed Passed 9.470/9.470/9.714 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006
45 Straight                 Passed Passed 9.366/9.366/9.611 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006
46 OneTileStraight          Passed Passed 9.261/9.261/9.508 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006
47 HalfTileStraight         Passed Passed 9.235/9.235/9.482 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006
48 HalfTileStraight         Passed Passed 9.222/9.222/9.469 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006
49 Straight                 Passed Passed 9.208/9.208/9.456 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006
50 OneTileStraight          Passed Passed 9.102/9.102/9.352 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006
51 TurnSmallStandalone      Passed Passed 9.076/9.076/9.326 1.000/1.000/0.684/0.006 1.000/1.000/0.684/0.006 1.000/1.000/0.724/0.006 1.000/1.000/0.724/0.006
52 Straight                 Passed Passed 8.898/8.898/9.154 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006
53 OneTileStraight          Passed Passed 8.790/8.790/9.048 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006
54 OneTileStraight          Passed Passed 8.762/8.762/9.021 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006
55 TurnSmallStandalone      Passed Passed 8.735/8.735/8.994 1.000/1.000/0.633/0.006 1.000/1.000/0.633/0.006 1.000/1.000/0.672/0.006 1.000/1.000/0.672/0.006
5 PlacementEstimateVsWorstCar: minimum vertical g (estimate 0.820 g differs from measurement 0.460 g by 0.360 g, over the 0.3 g tolerance)
6 PlacementEstimateVsWorstCar: minimum vertical g (estimate 0.406 g differs from measurement -0.124 g by 0.530 g, over the 0.3 g tolerance)
7 PlacementEstimateVsWorstCar: maximum vertical g (estimate 2.052 g differs from measurement 2.545 g by 0.493 g, over the 0.3 g tolerance)
Agreement: fail
```

### velocity-viper

```text
piece check2 check3 entry[placement/finished/test m/s] placement[min/max/lat/long g] finished[min/max/lat/long g] front[min/max/lat/long g] worst[min/max/lat/long g]
1 LaunchLong               Passed Passed 2.221/2.221/2.003 1.000/1.000/0.000/0.571 1.000/1.000/0.000/0.571 1.000/1.000/0.000/0.571 1.000/1.000/0.000/0.571
2 LaunchLong               Passed Passed 19.187/19.187/19.050 1.000/1.000/0.000/0.571 1.000/1.000/0.000/0.571 1.000/1.000/0.000/0.571 1.000/1.000/0.000/0.571
3 Launch                   Passed Passed 26.996/26.996/26.861 1.000/1.000/0.000/0.571 1.000/1.000/0.000/0.571 1.000/1.000/0.000/0.571 1.000/1.000/0.000/0.571
4 LaunchedTopHat           Passed Passed 29.995/29.995/30.000 -0.221/3.224/0.000/0.430 -0.221/3.224/0.000/0.430 -0.221/3.225/0.000/0.411 -0.221/3.225/0.000/0.451
5 Brakes                   Passed Passed 29.135/29.135/29.155 1.000/1.000/0.000/0.322 1.000/1.000/0.000/0.322 1.000/1.000/0.000/0.322 1.000/1.000/0.000/0.322
6 Brakes                   Passed Passed 27.383/27.383/27.386 1.000/1.000/0.000/0.322 1.000/1.000/0.000/0.322 1.000/1.000/0.000/0.321 1.000/1.000/0.000/0.321
7 Brakes                   Passed Passed 25.484/25.484/25.482 1.000/1.000/0.000/0.320 1.000/1.000/0.000/0.320 1.000/1.000/0.000/0.320 1.000/1.000/0.000/0.320
8 SpeedTurnLarge24Left     Passed Passed 23.528/23.528/23.467 1.000/3.156/0.218/0.679 1.000/3.156/0.218/0.679 1.000/3.142/0.223/0.673 1.000/3.142/0.227/0.673
9 EjectorHill16x96         Passed Passed 23.149/23.149/23.100 -0.312/2.941/0.000/0.652 -0.312/2.941/0.000/0.652 -0.302/2.932/0.000/0.638 -0.302/2.932/0.000/0.656
10 Brakes                   Passed Passed 22.471/22.471/22.476 1.000/1.000/0.000/0.317 1.000/1.000/0.000/0.317 1.000/1.000/0.000/0.317 1.000/1.000/0.000/0.317
11 ZeroGRollFastRight       Passed Passed 20.424/20.424/20.381 -0.637/2.648/0.645/0.843 -0.637/2.648/0.645/0.843 -0.640/2.641/0.648/0.833 -0.665/2.641/0.649/0.833
12 SpeedTurnLarge20Left     Passed Passed 19.634/19.634/19.609 1.000/2.249/0.155/0.324 1.000/2.249/0.155/0.324 1.000/2.241/0.159/0.324 1.000/2.241/0.161/0.324
13 FloaterHill12x96         Passed Passed 19.651/19.651/19.617 0.413/2.333/0.000/0.562 0.413/2.333/0.000/0.562 0.416/2.327/0.000/0.553 0.416/2.327/0.000/0.564
14 WaveTurnLeft             Passed Passed 19.030/19.030/19.028 1.116/1.512/0.971/0.302 1.116/1.512/0.971/0.302 1.115/1.510/0.971/0.299 1.089/1.511/0.972/0.318
15 Brakes                   Passed Passed 18.749/18.749/18.691 1.000/1.000/0.000/0.315 1.000/1.000/0.000/0.315 1.000/1.000/0.000/0.315 1.000/1.000/0.000/0.571
16 LaunchShort              Passed Passed 16.379/16.379/16.302 1.000/1.000/0.000/0.571 1.000/1.000/0.000/0.571 1.000/1.000/0.000/0.571 1.000/1.000/0.000/0.571
17 EjectorHill12x64         Passed Passed 18.720/18.720/18.692 -0.453/3.000/0.000/0.960 -0.453/3.000/0.000/0.960 -0.440/2.988/0.000/0.947 -0.440/2.988/0.000/1.197
18 Brakes                   Passed Passed 17.851/17.851/17.824 1.000/1.000/0.000/0.314 1.000/1.000/0.000/0.314 1.000/1.000/0.000/0.314 1.000/1.000/0.000/0.314
19 SpeedTurnLarge16Left     Passed Passed 15.556/15.556/15.447 1.000/1.507/0.165/0.152 1.000/1.507/0.165/0.152 1.000/1.499/0.171/0.151 1.000/1.499/0.172/0.309
20 Brakes                   Passed Passed 15.095/15.095/14.965 1.000/1.000/0.000/0.313 1.000/1.000/0.000/0.313 1.000/1.000/0.000/0.313 1.000/1.000/0.000/0.313
21 TurnSmallRightStandalone Passed Passed 11.571/11.571/11.466 1.000/1.000/1.110/0.007 1.000/1.000/1.110/0.007 1.000/1.000/1.097/0.007 1.000/1.000/1.097/0.007
22 TurnSmallRightStandalone Passed Passed 11.382/11.382/11.313 1.000/1.000/1.081/0.007 1.000/1.000/1.081/0.007 1.000/1.000/1.068/0.007 1.000/1.000/1.068/0.007
23 Straight                 Passed Passed 11.225/11.225/11.153 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007
24 Straight                 Passed Passed 11.130/11.130/11.057 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007
25 Straight                 Passed Passed 11.035/11.035/10.962 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007
26 Straight                 Passed Passed 10.939/10.939/10.866 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007
27 Straight                 Passed Passed 10.843/10.843/10.769 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007
28 Straight                 Passed Passed 10.746/10.746/10.672 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007
29 Straight                 Passed Passed 10.649/10.649/10.574 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007
30 Straight                 Passed Passed 10.551/10.551/10.476 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007
31 Straight                 Passed Passed 10.453/10.453/10.378 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007
32 Straight                 Passed Passed 10.355/10.355/10.279 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.007 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006
33 Straight                 Passed Passed 10.256/10.256/10.180 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006
34 Straight                 Passed Passed 10.156/10.156/10.080 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006
35 Straight                 Passed Passed 10.056/10.056/9.979 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006
36 Straight                 Passed Passed 9.955/9.955/9.878 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006
37 Straight                 Passed Passed 9.854/9.854/9.776 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006
38 Straight                 Passed Passed 9.752/9.752/9.674 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006
39 TurnLargeStandalone      Passed Passed 9.650/9.650/9.571 1.000/1.000/0.383/0.006 1.000/1.000/0.383/0.006 1.000/1.000/0.377/0.006 1.000/1.000/0.377/0.006
40 TurnLargeStandalone      Passed Passed 9.306/9.306/9.225 1.000/1.000/0.355/0.006 1.000/1.000/0.355/0.006 1.000/1.000/0.349/0.006 1.000/1.000/0.349/0.006
41 OffsetLeft               Passed Passed 8.954/8.954/8.871 1.000/1.000/0.209/0.006 1.000/1.000/0.209/0.006 1.000/1.000/0.206/0.006 1.000/1.000/0.206/0.006
42 OneTileStraight          Passed Passed 8.791/8.791/8.708 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006
43 OneTileStraight          Passed Passed 8.764/8.764/8.681 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006 1.000/1.000/0.000/0.006
Agreement: pass
```

<!-- SECTION:NOTES:END -->
