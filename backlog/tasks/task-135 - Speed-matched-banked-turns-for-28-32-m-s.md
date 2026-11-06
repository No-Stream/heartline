---
id: TASK-135
title: Speed-matched banked turns for 28-32 m/s
status: Done
assignee: []
created_date: '2026-11-06 07:12'
updated_date: '2026-11-06 21:34'
labels: []
dependencies: []
priority: medium
ordinal: 135000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Layout experiments (docs/reviews/2026-11-05-layout-experiments.md on branch layout-lab) found fast rides (Velocity Viper at 28-30 m/s, Timber returns) cannot turn without either shedding speed or exceeding 1.5 g sideways; the shipped speed-matched turns stop at 24 m/s design speed (window to 27). The 28 and 32 m/s large turns were dropped because their bank transitions did not fit within the turn at a 90 deg/s roll-rate cap. Add larger-radius (or longer-transition) speed-matched turns for 28-32 m/s that meet the same full-train acceptance (lateral under 0.35 g at design speed, roll rate cap), and return geometry so they can connect back to the station.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Large turns banked for 28 m/s (48 m quarter turn, window 25-31) and 32 m/s (64 m quarter turn, window 29-35) use the shipped eased heart-path geometry on a wider footprint. Full-train scan, all models, 6 and 12 cars: 28 m/s peak bank 73.00 deg, vertical up to 3.752 g, design-speed lateral 0.109 g, roll rate 68.6 deg/s at design speed and 75.9 deg/s at the window top; 32 m/s peak bank 72.59 deg, up to 3.664 g, 0.080 g, 58.5 and 64.0 deg/s. Allowed for all three models (Briar Blitz included, under its 4.0 g). Speed turns now group by hand only, so the builder lists 16-32 m/s and recommends the widest-margin window. Geometries are fitted on first use to keep the hybrid cold choices() at the master timing. Measurements in docs/superpowers/plans/2026-11-05-good-circuits.md section 3a.
<!-- SECTION:NOTES:END -->
