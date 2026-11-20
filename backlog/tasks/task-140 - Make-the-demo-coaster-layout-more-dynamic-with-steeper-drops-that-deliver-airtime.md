---
id: TASK-140
title: >-
  Make the demo coaster layout more dynamic, with steeper drops that deliver
  airtime
status: To Do
assignee: []
created_date: '2026-11-18 12:15'
updated_date: '2026-11-18 20:11'
labels:
  - coaster
dependencies: []
priority: low
ordinal: 140000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Owner playtest 2026-11-18: the demo coaster layout should be more dynamic and realistic. Drops should be steeper and deliver airtime. On the camelback layout the hill shapes are roughly fine, but the main drop itself should give airtime at the crest (owner target about -1 g airtime, brief peaks fine; see TASK-139 for Horizon Hawk). Low priority.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
2026-11-18 (branch demo-ride): new demo layout (now in content/rides/demo.ride.json), still the Timber Tumbler (hybrid steel box track on wood, rated as steel); the registry gave no reason to switch.

Layout: 45-degree chain lift to 60 m; a 32 m drop at 45 degrees to 28 m (5 m above the ground) at 89 km/h; Ejector hill 16 x 96 m; large banked turn rated 76-97 km/h; stall; large banked turn; fast zero-g roll; Ejector hill 8 x 64 m; a trim brake; a smaller banked turn rated 61-83 km/h; brake run; the old return path to the station (two straights shorter, because the return leg now runs 32 m further west to make room for the trim brake).

Measured (native construction test, worst car): airtime -0.13 g over the main drop crest (back car), -0.70 g on the 16 x 96 hill, -0.46 g stall, -0.43 g zero-g roll, -0.96 g on the 8 x 64 hill; pullout 4.04 g (back car; 3.75 g front car, under 0.7 s), hills and turns 3.2-3.5 g; lateral at most 0.71 g; longitudinal at most 1.17 g; top speed 91 km/h. Excitement 7.16, intensity 6.28, nausea 2.93 (old demo 5.12 / 5.25 / 0.67). The ride window in the default park shows 8.7 excitement because it adds proximity and theming from the park. Every rated piece is entered at least 1.5 m/s inside its window by every car, and all forces stay 0.25 g inside the limits (known_circuit_statistics demo test).

What limits it: a 2 m deeper drop (tried first, built through the UI) gave -1.08 g on the 8 x 64 hill but the back car entered it at 23.78 m/s, 0.06 m/s past the margin, and no other 64 m hill is rated faster. With the 2 m shallower drop the last large banked turn was entered 0.07 m/s below its margin, so a trim brake now slows the train into the smaller turn; a straight between the hill and the trim keeps the back car's braking on the hill at 1.16 g instead of 1.43 g. -1 g over the main drop crest is not reachable with the catalog's crest pieces at chain speed (TASK-154). Main pullout is above the owner's 3-3.5 g steel target as a brief peak.

Window origins (per the owner's per-model window decision, force-rule): none of the windows the demo uses comes from another model's limits on this branch. The hill windows come from each hill's own design spec (EjectorHill8x64's 25.22 m/s maximum is the speed at which its generated profile reaches a -1.2 g crest force, crates/sim/src/track/profile.rs); speed-turn windows are fixed design bands; stall and zero-g roll windows are rescaled per train by effective_rated_speeds. The binding constraint was the 8 x 64 hill's 25.22 m/s design maximum, not a Timber Tumbler limit. By Timber Tumbler's own limits the deeper first version is fine: with a TerrainFall instead of the Straight before the drop (34 m drop), the 8 x 64 hill at 23.78 m/s gives -1.08 g and 1.14 g longitudinal on the worst car, inside the -1.25 g and 1.25 g margins, and excitement 7.45. Once force-rule gives Timber Tumbler its own windows, re-check that version: if its own 8 x 64 maximum is at least 25.3 m/s, switch to it (TerrainFall before the drop, the 8 x 64 hill straight after the zero-g roll, a 24 m speed turn instead of the trim brake and 20 m turn, five brakes, return leg back at x 988 m) and update the known_circuit_statistics demo test.

2026-11-18: demo-ride merged (98e2ee2). Master gates: Rust, Vitest, lint, typecheck, feature 194/195 then coaster boarding re-recorded (one-pixel difference), journeys pass. Remaining: about -1 g at the main-drop crest (TASK-154) and the deeper version that waits on per-model windows (TASK-126).
<!-- SECTION:NOTES:END -->
