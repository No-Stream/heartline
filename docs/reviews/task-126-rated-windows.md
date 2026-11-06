# TASK-126 rated-window scan

## Result

All 101 rated catalog templates (94 before master's speed-matched turns and the wooden strong floater hill were merged) now take their entry-speed windows from `crates/sim/src/track/rated_windows.generated.rs`, which `make generate-rated-windows` writes from the isolated-piece scan in `crates/sim/examples/measure_force_windows.rs`. The scan passed its acceptance checks on 2026-11-06 after the merge: every endpoint passes for every allowed model and train length, every window is at least 2 m/s wide (1.5 m/s for vertical loops), no piece lost its window, and every straight ejector hill gives Great airtime to each model's default train at its new maximum, so no hill was relabelled a floater. Eight maxima fell and thirteen minima rose; the rest of the maxima rose or stayed. The earlier partial and candidate scans that this page used to record are superseded by the table below.

## How a window is measured

A speed passes when every allowed model and every allowed train length (with the train length scaled as construction scales it) passes the duration-aware force evaluator, the stall and clearance checks, and the tail check, and when the train's filtered force stays within the model's comfortable limit on every axis. The last condition means the overlay rates nothing above Great (or Strong for lateral and longitudinal force) for any train at a speed inside the window; a brief excursion that the duration rule allows still fails it. The scan samples every 0.25 m/s from the catalog minimum to `max(100 m/s, 3 × old maximum)` and refines each edge with twelve bisections, rounded inward to 0.01 m/s, then rechecks both endpoints.

The minimum is the catalog's energy-derived or hand value when it passes for every train. When it fails, it rises to the first passing speed above it; this covers the clearance (5 m/s, or 6 m/s on zero-g rolls and stalls) and the requirement that the whole train clears the piece and its tail. One minimum now applies to every train length: the per-train minimum adjustment for zero-g rolls and stalls (`effective_rated_speeds`) is gone, because the energy estimate it used left a four-car Timber Tumbler train below the 6 m/s clearance speed. The maximum is the top of the contiguous passing run above the minimum. Three exceptions apply. Pieces whose forces stay comfortable past any sensible speed keep their old maximum as a design cap: the heartline rolls, the banked straights and the descending half corkscrews (plan section 5), plus any piece that still passes at the scan ceiling. The zero-g rolls, stalls and half corkscrews also keep the existing 200 degrees per second roll-rate design rule, which the tests `rolled_hill_inversions_limit_roll_rate_for_full_trains` and `half_roll_and_barrel_roll_rates_at_maximum_speed` already enforced; the scan now applies it through the same `Piece` methods those tests call. The descending half corkscrews keep their 22 m/s clearing-speed rule. The speed-matched turns keep their windows of 3 m/s either side of the speed they are banked for, as design caps, because the construction tool picks a turn by which window contains the entry speed. Finally, a floater hill (including the wooden strong floater) must not give any model's default train more than Good airtime at its maximum, the counterpart of the ejector rule; otherwise it would ride as an ejector.

## Windows before and after

"Kept" and "raised: clearance" describe the minimum; "force limit", "roll rate" and "design cap" describe the maximum. The last column is the first failing configuration 0.01 m/s above a force-bound maximum, or the roll rate there for a roll-rate-bound one.

| Template (entry → exit slope) | Old window (m/s) | New window (m/s) | Minimum | Maximum | First failure just above the new maximum |
| --- | ---: | ---: | --- | --- | --- |
| `LevelToSixtyUp` (Level → UpSixty) | 28.34–34.00 | 28.34–36.41 | kept | force limit | Timber Tumbler 8 cars: vertical +4.501 g too much |
| `SixtyUpToLevel` (UpSixty → Level) | 25.44–30.00 | 25.98–30.87 | raised: clearance | force limit | Velocity Viper 3 cars: vertical -1.501 g too much |
| `LevelToSixtyDown` (Level → DownSixty) | 5.00–19.00 | 5.00–18.34 | kept | force limit | Timber Tumbler 8 cars: vertical -1.500 g too much |
| `SixtyDownToLevel` (DownSixty → Level) | 2.00–23.82 | 2.00–23.30 | kept | force limit | Timber Tumbler 8 cars: vertical +4.501 g too much |
| `SteepToSixtyUp` (UpSteep → UpSixty) | 26.12–34.00 | 26.51–37.35 | raised: clearance | force limit | Timber Tumbler 6 cars: vertical +4.500 g too much |
| `SixtyToSteepUp` (UpSixty → UpSteep) | 27.00–30.00 | 27.95–35.28 | raised: clearance | force limit | Velocity Viper 3 cars: vertical -1.501 g too much |
| `SteepToSixtyDown` (DownSteep → DownSixty) | 3.00–20.00 | 3.00–22.12 | kept | force limit | Timber Tumbler 8 cars: vertical -1.500 g too much |
| `SixtyToSteepDown` (DownSixty → DownSteep) | 2.00–28.00 | 2.00–26.62 | kept | force limit | Timber Tumbler 8 cars: vertical +4.500 g too much |
| `Loop` (Level → Level) | 24.73–27.50 | 24.87–29.49 | raised: clearance | force limit | Timber Tumbler 8 cars: vertical +4.501 g too much |
| `Corkscrew` (Level → Level) | 14.00–19.00 | 14.00–19.82 | kept | force limit | Timber Tumbler 8 cars: vertical +4.503 g too much |
| `HeartlineRoll` (Level → Level) | 8.00–20.00 | 8.00–20.00 | kept | design cap | — |
| `HalfLoopUp` (Level → Level) | 27.96–31.30 | 27.96–33.10 | kept | force limit | Timber Tumbler 8 cars: vertical +4.505 g too much |
| `HalfLoopDown` (Level → Level) | 4.00–15.30 | 4.00–18.64 | kept | force limit | Timber Tumbler 8 cars: vertical +4.501 g too much |
| `HalfCorkscrewLeftUp` (Level → Level) | 17.00–22.00 | 17.00–22.06 | kept | roll rate | roll rate 200.0 degrees/s |
| `HalfCorkscrewLeftDown` (Level → Level) | 5.00–15.77 | 5.00–15.77 | kept | design cap | — |
| `RavenRollLeftUp` (Level → Level) | 8.00–17.00 | 8.00–17.68 | kept | force limit | Timber Tumbler 4 cars: vertical -1.500 g too much |
| `RavenRollLeftDown` (Level → Level) | 8.00–18.00 | 8.00–19.25 | kept | force limit | Timber Tumbler 4 cars: lateral -1.500 g too much |
| `LargeLoop` (Level → Level) | 27.59–31.30 | 27.59–33.09 | kept | force limit | Timber Tumbler 8 cars: vertical +4.501 g too much |
| `SteelLoop` (Level → Level) | 25.23–27.00 | 25.23–29.44 | kept | force limit | Briar Blitz 12 cars: vertical +4.003 g too much |
| `WaveTurnLeft` (Level → Level) | 8.00–23.00 | 8.00–23.51 | kept | force limit | Timber Tumbler 8 cars: lateral +1.500 g too much |
| `HybridOverbankLeft` (Level → Level) | 5.00–13.00 | 5.00–13.42 | kept | force limit | Timber Tumbler 4 cars: lateral -1.500 g too much |
| `HybridOverbankRight` (Level → Level) | 5.00–13.00 | 5.00–13.42 | kept | force limit | Timber Tumbler 4 cars: lateral +1.500 g too much |
| `WaveTurnRight` (Level → Level) | 8.00–23.00 | 8.00–23.51 | kept | force limit | Timber Tumbler 8 cars: lateral -1.500 g too much |
| `LoopRight` (Level → Level) | 24.73–27.50 | 24.73–29.49 | kept | force limit | Timber Tumbler 8 cars: vertical +4.501 g too much |
| `LargeLoopRight` (Level → Level) | 27.59–31.30 | 27.59–33.09 | kept | force limit | Timber Tumbler 8 cars: vertical +4.501 g too much |
| `SteelLoopRight` (Level → Level) | 25.23–27.00 | 25.23–29.44 | kept | force limit | Briar Blitz 12 cars: vertical +4.003 g too much |
| `CorkscrewRight` (Level → Level) | 14.00–19.00 | 14.00–19.82 | kept | force limit | Timber Tumbler 8 cars: vertical +4.503 g too much |
| `HeartlineRollRight` (Level → Level) | 8.00–20.00 | 8.00–20.00 | kept | design cap | — |
| `HalfCorkscrewRightUp` (Level → Level) | 17.00–22.00 | 17.00–22.06 | kept | roll rate | roll rate 200.0 degrees/s |
| `HalfCorkscrewRightDown` (Level → Level) | 5.00–15.77 | 5.00–15.77 | kept | design cap | — |
| `RavenRollRightUp` (Level → Level) | 8.00–17.00 | 8.00–17.68 | kept | force limit | Timber Tumbler 4 cars: vertical -1.500 g too much |
| `RavenRollRightDown` (Level → Level) | 8.00–18.00 | 8.00–19.25 | kept | force limit | Timber Tumbler 4 cars: lateral +1.500 g too much |
| `BankedDropLeft` (Level → DownGentle) | 12.00–15.00 | 12.00–20.03 | kept | force limit | Briar Blitz 12 cars: vertical -1.000 g too much |
| `BankedDropRight` (Level → DownGentle) | 12.00–15.00 | 12.00–20.03 | kept | force limit | Briar Blitz 12 cars: vertical -1.000 g too much |
| `TwistedDropLeft` (Level → DownGentle) | 12.00–15.00 | 12.00–20.77 | kept | force limit | Timber Tumbler 8 cars: vertical -1.500 g too much |
| `TwistedDropRight` (Level → DownGentle) | 12.00–15.00 | 12.00–20.77 | kept | force limit | Timber Tumbler 8 cars: vertical -1.500 g too much |
| `BarrelDropLeft` (Level → DownGentle) | 10.00–15.00 | 10.00–16.37 | kept | force limit | Timber Tumbler 8 cars: vertical -1.500 g too much |
| `BarrelDropRight` (Level → DownGentle) | 10.00–15.00 | 10.00–16.37 | kept | force limit | Timber Tumbler 8 cars: vertical -1.500 g too much |
| `TopHat` (Level → Level) | 24.63–27.16 | 24.73–27.58 | raised: clearance | force limit | Briar Blitz 12 cars: vertical -1.004 g too much |
| `LaunchedTopHat` (Level → Level) | 27.75–34.60 | 27.75–34.80 | kept | force limit | Velocity Viper 6 cars: vertical -1.502 g too much |
| `EjectorHill8x48` (Level → Level) | 13.20–18.24 | 13.20–19.02 | kept | force limit | Timber Tumbler 8 cars: vertical -1.500 g too much |
| `EjectorHill12x64` (Level → Level) | 15.94–21.88 | 15.94–21.51 | kept | force limit | Timber Tumbler 8 cars: vertical -1.501 g too much |
| `EjectorHill8x64` (Level → Level) | 13.49–25.22 | 13.49–24.97 | kept | force limit | Timber Tumbler 8 cars: vertical -1.500 g too much |
| `EjectorHill12x80` (Level → Level) | 16.14–26.59 | 16.14–26.34 | kept | force limit | Timber Tumbler 8 cars: vertical -1.501 g too much |
| `EjectorHill16x96` (Level → Level) | 18.42–28.34 | 18.42–28.18 | kept | force limit | Timber Tumbler 8 cars: vertical -1.501 g too much |
| `FloaterHill4x48` (Level → Level) | 10.32–20.79 | 10.32–23.68 | kept | force limit | Timber Tumbler 8 cars: longitudinal -1.505 g too much |
| `FloaterHill8x64` (Level → Level) | 13.54–21.60 | 13.54–23.84 | kept | force limit | Briar Blitz 11 cars: vertical +4.001 g too much |
| `FloaterHill12x80` (Level → Level) | 16.17–23.01 | 16.17–25.25 | kept | floater airtime | default-train airtime above Good at 25.26 m/s |
| `FloaterHill6x64` (Level → Level) | 12.12–24.07 | 12.12–26.14 | kept | force limit | Briar Blitz 9 cars: vertical +4.000 g too much |
| `FloaterHill12x96` (Level → Level) | 16.27–26.75 | 16.27–28.96 | kept | force limit | Briar Blitz 12 cars: vertical +4.001 g too much |
| `FloaterHill16x112` (Level → Level) | 18.52–27.86 | 18.52–30.12 | kept | force limit | Briar Blitz 12 cars: vertical +4.001 g too much |
| `SpeedHill4x64` (Level → Level) | 10.53–20.92 | 10.53–24.72 | kept | force limit | Timber Tumbler 8 cars: longitudinal +1.502 g too much |
| `SpeedHill6x80` (Level → Level) | 12.26–23.86 | 12.26–26.78 | kept | force limit | Briar Blitz 10 cars: vertical +4.000 g too much |
| `WoodenEjectorHill6x48` (Level → Level) | 11.83–19.67 | 11.83–19.79 | kept | force limit | Briar Blitz 12 cars: vertical -1.002 g too much |
| `WoodenEjectorHill8x64` (Level → Level) | 13.50–22.89 | 13.50–22.85 | kept | floater airtime | default-train airtime above Good at 22.86 m/s |
| `EjectorHill8x48OuterLeft` (Level → Level) | 14.20–18.24 | 14.20–19.08 | kept | force limit | Timber Tumbler 8 cars: longitudinal +1.502 g too much |
| `EjectorHill8x48OuterRight` (Level → Level) | 14.20–18.24 | 14.20–19.08 | kept | force limit | Timber Tumbler 8 cars: longitudinal +1.502 g too much |
| `EjectorHill12x64OuterLeft` (Level → Level) | 16.94–21.88 | 16.94–23.78 | kept | force limit | Timber Tumbler 8 cars: lateral +1.501 g too much |
| `EjectorHill12x64OuterRight` (Level → Level) | 16.94–21.88 | 16.94–23.78 | kept | force limit | Timber Tumbler 8 cars: lateral -1.501 g too much |
| `EjectorHill8x64OuterLeft` (Level → Level) | 14.49–25.22 | 14.49–27.20 | kept | force limit | Timber Tumbler 8 cars: lateral +1.502 g too much |
| `EjectorHill8x64OuterRight` (Level → Level) | 14.49–25.22 | 14.49–27.20 | kept | force limit | Timber Tumbler 8 cars: lateral -1.502 g too much |
| `EjectorHill12x80OuterLeft` (Level → Level) | 17.14–26.59 | 17.14–28.32 | kept | force limit | Timber Tumbler 8 cars: lateral +1.502 g too much |
| `EjectorHill12x80OuterRight` (Level → Level) | 17.14–26.59 | 17.14–28.32 | kept | force limit | Timber Tumbler 8 cars: lateral -1.502 g too much |
| `EjectorHill16x96OuterLeft` (Level → Level) | 19.42–28.34 | 19.42–29.92 | kept | force limit | Timber Tumbler 8 cars: lateral +1.502 g too much |
| `EjectorHill16x96OuterRight` (Level → Level) | 19.42–28.34 | 19.42–29.92 | kept | force limit | Timber Tumbler 8 cars: lateral -1.502 g too much |
| `FloaterHill4x48OuterLeft` (Level → Level) | 11.32–20.79 | 11.32–23.68 | kept | force limit | Timber Tumbler 8 cars: longitudinal -1.502 g too much |
| `FloaterHill4x48OuterRight` (Level → Level) | 11.32–20.79 | 11.32–23.68 | kept | force limit | Timber Tumbler 8 cars: longitudinal -1.502 g too much |
| `FloaterHill8x64OuterLeft` (Level → Level) | 14.54–21.60 | 14.54–25.68 | kept | force limit | Timber Tumbler 8 cars: vertical +4.500 g too much |
| `FloaterHill8x64OuterRight` (Level → Level) | 14.54–21.60 | 14.54–25.68 | kept | force limit | Timber Tumbler 8 cars: vertical +4.500 g too much |
| `FloaterHill12x80OuterLeft` (Level → Level) | 17.17–23.01 | 17.17–27.25 | kept | force limit | Timber Tumbler 8 cars: vertical +4.500 g too much |
| `FloaterHill12x80OuterRight` (Level → Level) | 17.17–23.01 | 17.17–27.25 | kept | force limit | Timber Tumbler 8 cars: vertical +4.500 g too much |
| `FloaterHill6x64OuterLeft` (Level → Level) | 13.12–24.07 | 13.12–28.17 | kept | force limit | Timber Tumbler 6 cars: vertical +4.501 g too much |
| `FloaterHill6x64OuterRight` (Level → Level) | 13.12–24.07 | 13.12–28.17 | kept | force limit | Timber Tumbler 6 cars: vertical +4.501 g too much |
| `FloaterHill12x96OuterLeft` (Level → Level) | 17.27–26.75 | 17.27–31.21 | kept | force limit | Timber Tumbler 8 cars: vertical +4.502 g too much |
| `FloaterHill12x96OuterRight` (Level → Level) | 17.27–26.75 | 17.27–31.21 | kept | force limit | Timber Tumbler 8 cars: vertical +4.502 g too much |
| `FloaterHill16x112OuterLeft` (Level → Level) | 19.52–27.86 | 19.52–32.45 | kept | force limit | Timber Tumbler 8 cars: vertical +4.502 g too much |
| `FloaterHill16x112OuterRight` (Level → Level) | 19.52–27.86 | 19.52–32.45 | kept | force limit | Timber Tumbler 8 cars: vertical +4.502 g too much |
| `ZeroGRollFastLeft` (Level → Level) | 15.25–27.00 | 15.48–28.83 | raised: clearance | roll rate | roll rate 200.0 degrees/s |
| `ZeroGRollFastRight` (Level → Level) | 15.25–27.00 | 15.48–28.83 | raised: clearance | roll rate | roll rate 200.0 degrees/s |
| `ZeroGRollCompactLeft` (Level → Level) | 16.19–20.00 | 16.58–21.96 | raised: clearance | roll rate | roll rate 200.0 degrees/s |
| `ZeroGRollCompactRight` (Level → Level) | 16.19–20.00 | 16.58–21.96 | raised: clearance | roll rate | roll rate 200.0 degrees/s |
| `StallLeft` (Level → Level) | 18.00–27.00 | 18.11–27.99 | raised: clearance | roll rate | roll rate 200.0 degrees/s |
| `StallRight` (Level → Level) | 18.00–27.00 | 18.11–27.99 | raised: clearance | roll rate | roll rate 200.0 degrees/s |
| `SpeedTurnLarge16Left` (Level → Level) | 13.00–19.00 | 13.00–19.00 | kept | design cap | — |
| `SpeedTurnLarge16Right` (Level → Level) | 13.00–19.00 | 13.00–19.00 | kept | design cap | — |
| `SpeedTurnLarge20Left` (Level → Level) | 17.00–23.00 | 17.00–23.00 | kept | design cap | — |
| `SpeedTurnLarge20Right` (Level → Level) | 17.00–23.00 | 17.00–23.00 | kept | design cap | — |
| `SpeedTurnLarge24Left` (Level → Level) | 21.00–27.00 | 21.00–27.00 | kept | design cap | — |
| `SpeedTurnLarge24Right` (Level → Level) | 21.00–27.00 | 21.00–27.00 | kept | design cap | — |
| `BankedStraightLeft` (Level → Level) | 5.00–30.00 | 5.00–30.00 | kept | design cap | — |
| `BankedStraightRight` (Level → Level) | 5.00–30.00 | 5.00–30.00 | kept | design cap | — |
| `RolloutLeft` (Level → Level) | 5.00–30.00 | 5.00–36.82 | kept | force limit | Timber Tumbler 4 cars: lateral -1.500 g too much |
| `RolloutRight` (Level → Level) | 5.00–30.00 | 5.00–36.82 | kept | force limit | Timber Tumbler 4 cars: lateral +1.500 g too much |
| `BankedStraightLeft` (UpGentle → UpGentle) | 20.00–30.00 | 20.00–30.00 | kept | design cap | — |
| `BankedStraightLeft` (DownGentle → DownGentle) | 5.00–30.00 | 5.00–30.00 | kept | design cap | — |
| `BankedStraightRight` (UpGentle → UpGentle) | 20.00–30.00 | 20.00–30.00 | kept | design cap | — |
| `BankedStraightRight` (DownGentle → DownGentle) | 5.00–30.00 | 5.00–30.00 | kept | design cap | — |
| `RolloutLeft` (UpGentle → UpGentle) | 22.00–30.00 | 22.87–44.69 | raised: clearance | force limit | Briar Blitz 4 cars: lateral -1.500 g too much |
| `RolloutLeft` (DownGentle → DownGentle) | 5.00–30.00 | 5.00–40.86 | kept | force limit | Timber Tumbler 8 cars: lateral -1.500 g too much |
| `RolloutRight` (UpGentle → UpGentle) | 22.00–30.00 | 22.87–44.69 | raised: clearance | force limit | Briar Blitz 4 cars: lateral +1.500 g too much |
| `RolloutRight` (DownGentle → DownGentle) | 5.00–30.00 | 5.00–40.86 | kept | force limit | Timber Tumbler 8 cars: lateral +1.500 g too much |

## Maxima that fell

Seven maxima are lower than before. At each old maximum some allowed train's filtered force went past a comfortable limit, which the stricter rule no longer allows even when the excursion is brief. `LevelToSixtyDown` (19.00 to 18.34 m/s), `SixtyDownToLevel` (23.82 to 23.30 m/s) and `SixtyToSteepDown` (28.00 to 26.62 m/s) are bound by an eight-car Timber Tumbler train: too much airtime on the drop into the sixty-degree section, or too much vertical force in the pull out of it. The four straight steel ejector hills `EjectorHill12x64` (21.88 to 21.51), `EjectorHill8x64` (25.22 to 24.97), `EjectorHill12x80` (26.59 to 26.34) and `EjectorHill16x96` (28.34 to 28.18 m/s) are bound by airtime past −1.5 g on the eight-car Timber Tumbler train; these are the three excursions the earlier findings recorded at the old maxima, plus `EjectorHill8x64`, which the stricter Great-or-less criterion now also catches. The default six-car trains still get Great airtime (−1.25 to −1.41 g) at the new maxima.

Two floater maxima are set by that floater airtime rule. `FloaterHill12x80` stops at 25.25 m/s instead of its force-bound 25.30 m/s, because above that the default 12-car Briar Blitz train gets Great airtime. The wooden strong floater `WoodenEjectorHill8x64` falls from 22.89 to 22.85 m/s for the same reason; at its force-bound 23.36 m/s the default train would get −1.00 g, which is ejector airtime.

## Minima that rose

Thirteen minima rose, all for clearance or for the train clearing its tail; none rose for force. These fix the known integration failures. The vertical loop rose from 24.73 to 24.87 m/s because a four-car Timber Tumbler train crossed its top at 4.79 m/s at the old minimum. The top hat rose from 24.63 to 24.73 m/s (a four-car Briar Blitz train at 4.50 m/s). `SixtyUpToLevel` rose from 25.44 to 25.98 m/s and `SixtyToSteepUp` from 27.00 to 27.95 m/s (seven-car Timber Tumbler trains at 4.78 and 4.39 m/s), and `SteepToSixtyUp` from 26.12 to 26.51 m/s (an eight-car Timber Tumbler train stalled at 2.10 m/s). The zero-g rolls rose from 15.25 to 15.48 m/s (fast) and 16.19 to 16.58 m/s (compact), and the stalls from 18.00 to 18.11 m/s, all for four-car Timber Tumbler trains that dropped below the 6 m/s clearance speed (5.57, 5.18 and 5.76 m/s). The uphill rollouts rose from 22.00 to 22.87 m/s (a seven-car Timber Tumbler train at 3.49 m/s).

The right-hand vertical loop `LoopRight` keeps its 24.73 m/s minimum because every allowed train passes there, while the left-hand loop does not. The scan reports this asymmetry between two mirrored pieces but does not explain it; it needs a follow-up check of the mirrored geometry.

## Ejector and floater airtime

At each new maximum, every straight ejector hill gives Great airtime to each allowed model's default train, so no hill was relabelled. Shorter trains on the wooden ejector hill rate Good: on `WoodenEjectorHill6x48` at 19.79 m/s, Briar Blitz trains of 4 to 8 cars get −0.75 to −0.88 g, while the default 12-car train gets −1.00 g, exactly at the wooden airtime limit. On the steel ejector hills every allowed train length rates Great at the new maximum. The floater hills give Good airtime to every default train at their new maxima (−0.68 to −0.89 g), except Briar Blitz on `FloaterHill12x80`, which reaches −0.91 g (Great).

| Hill | New maximum (m/s) | Default-train airtime at the maximum |
| --- | ---: | --- |
| `EjectorHill8x48` | 19.02 | Timber Tumbler and Velocity Viper, 6 cars: −1.286 g, Great |
| `EjectorHill12x64` | 21.51 | Timber Tumbler and Velocity Viper, 6 cars: −1.254 g, Great |
| `EjectorHill8x64` | 24.97 | Timber Tumbler and Velocity Viper, 6 cars: −1.409 g, Great |
| `EjectorHill12x80` | 26.34 | Timber Tumbler and Velocity Viper, 6 cars: −1.383 g, Great |
| `EjectorHill16x96` | 28.18 | Timber Tumbler and Velocity Viper, 6 cars: −1.378 g, Great |
| `WoodenEjectorHill6x48` | 19.79 | Briar Blitz, 12 cars: −1.000 g, Great |

## Gaps in the passing set

One template has a gap above its window. `SpeedHill4x64` passes up to 24.72 m/s, fails at 24.75 m/s on an eight-car Timber Tumbler train (filtered longitudinal force +1.502 g against the 1.5 g limit) and passes again at 25.00 m/s. The earlier step-size comparison found the same force moves by about 0.02 g between a 5 ms and a 1.25 ms integration step here, so the gap is most likely numerical noise at the limit. The window ends below the gap. No other template passed again above its first failure on the 0.25 m/s grid; failures between grid samples remain possible.

## Player-circuit flags

The task 1 full-train comparison ran the existing scoring eligibility rule on each circuit, then compared raw force bands with the evaluator. Indices are zero-based. A straight can inherit filter history from the preceding piece.

| Circuit | Newly flagged with evaluator | No longer flagged with evaluator | Coverage |
| --- | --- | --- | --- |
| Timber Tumbler | None | None | Incomplete: the run reached 205.07 m and did not complete the circuit. |
| Velocity Viper | 15 Straight; 43 Straight | 9 LevelToGentleDown; 12 GentleDownToLevel | Full circuit. |
| Briar Blitz | 35 Straight | 11 UnbankLeft; 12 GentleDownToLevel | Full circuit. |

These are the before/after flags from the task 1 comparison, not evidence that the rated-window scan or catalog update has passed. The finer 50 ms prototype comparison is also still a failed preview candidate: it exceeded the 0.1 g longitudinal bound on Velocity Viper and under-warned at two Briar Blitz metres; its p95 latency was 7.83–12.43 times the same-session master baseline. See [task-126-derisk.md](task-126-derisk.md) for the measurements and comparison method.


## Test changes

The rated tests are the currentness guard for the table. `every_rated_endpoint_passes_for_every_train_within_the_comfortable_limits` in `rated_element_physics.rs` replaces two older endpoint tests: it rates every template at both endpoints for every allowed model and train length and requires the same Great-or-less condition the scan uses. The raw-force helper `assert_train_crosses_element` now runs a force evaluator per car, as the plan asked; an excursion that began on an earlier piece counts against that piece, as in the isolated rating. That attribution matters for `banked_straights_and_rollouts_pass_full_train_at_rated_window_ends`: at 30 m/s a six-car Briar Blitz train already exceeds the lateral limit on the unrated bank-in transition before the banked straight (−2.44 g filtered), and the old raw check missed it because it looked only inside the straight. The corkscrew's extra 1 g lateral check went with the raw rule (plan section 3d).

The sequence check in `all_constructible_half_inversion_combinations_pass_at_rated_element_bands` now starts the sequence at the first element's minimum and at the highest first-element speed at which, by energy, every later element is still entered within its own window. At the raised half-loop maximum of 33.1 m/s the following raven roll would be entered above its 17.68 m/s maximum and then shows −1.54 g airtime; that is the player overspeeding the second element, not a fault in either window. The per-element checks in the same test still run every element at both of its endpoints. The crest test `connected_crest_endpoints` dropped its 30 m/s upper bound, which recorded the old maxima; `SixtyUpToLevel` and `SixtyToSteepUp` now reach 30.87 and 35.28 m/s. The loop geometry and hill catalog tests now compare their design maxima with the catalog before the table is applied (`unmeasured_catalog`), since the table owns the rated maximum. The diagnostic test for the ejector hill's violation side runs 1 m/s above the new maximum, because the maximum itself now passes. The speed-matching test asks for 24.5 m/s instead of 25 m/s, because the 8 × 64 m ejector hill's maximum fell to 24.97 m/s.

Four per-train minimum tests were deleted with `effective_rated_speeds`: `configured_train_rolled_inversion_speed_windows_are_physically_valid` and `default_train_uses_exact_catalog_rating_at_any_elevation` in `track/tests.rs`, `configured_train_changes_rolled_inversion_speed_match` in `speed_matched_variants.rs`, and `configured_train_changes_rolled_inversion_speed_note` in `physics/testing.rs`. They asserted that a short train gets a lower roll minimum than a long one. With one minimum for every train they have nothing left to check.

Merging master brought more raw-force tests, which were restated the same way. The speed-matched turn scan in `rated_element_physics.rs` now reports the evaluator verdict instead of raw force bounds; its raw lateral figures at the design speed and its roll-rate checks are unchanged. The wooden strong floater test rates every allowed Briar Blitz train length with the filtered airtime at the maximum instead of the raw airtime of unscaled 6- and 12-car trains. `every_floater_hill_gives_default_trains_at_most_good_airtime_at_its_maximum` is new and checks the floater airtime rule. Master's `configured_train_changes_rolled_inversion_speed_match` was deleted for the same reason as the other per-train minimum tests, and its train-aware selector became `select_speed_matched_variant_for_limits`, which keeps master's per-model airtime target. Master's preview force correction (`correct_sample_forces`) only ran at a preview step coarser than 5 ms; this branch previews at 5 ms with filtered forces, so the correction and its test were dropped.

## For the owner

Most maxima rose, because filtered forces stay within the limits at higher speeds than raw forces did. Loops can now be entered up to about 2 m/s faster, the floater hills about 2 to 4.5 m/s faster, banked and twisted drops about 5 m/s faster, and the rollouts 7 to 15 m/s faster (up to 44.7 m/s on the uphill rollout). The rollouts are bound only by lateral force, so a rollout at 40 m/s is within the limits but unlike any real ride; adding them to the design caps at 30 m/s, like the banked straights, is a one-line change. Raising any design cap (the heartline rolls at 20 m/s, the banked straights at 30 m/s) is also your call.

Composition matters more than before. A piece can be entered within its window and still pass the next piece above that one's window, as with a half loop at 33.1 m/s followed by a raven roll. The overlay shows the result for the actual track, but the rated windows alone no longer guarantee that standard pairs of half inversions combine comfortably at the top of the first piece's window.
