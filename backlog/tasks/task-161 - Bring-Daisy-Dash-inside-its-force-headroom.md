---
id: TASK-161
title: Bring Daisy Dash inside its force headroom
status: Done
assignee: []
created_date: '2026-11-21 03:15'
updated_date: '2026-11-21 21:25'
labels: []
dependencies: []
ordinal: 157000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
On master 6661fe4, tools/design check content/rides/daisy-dash.ride.json reports force headroom failures: default physics +2.82/-0.41 g, construction-test physics +2.89/-0.51 g, against 2.75/-0.35 g limits (model limits minus 0.25 g). check exits 0 because the ride file does not set designMargins (see TASK-160). Soften its worst pullout and airtime piece with the design tool, set designMargins: true, re-record its digests and goldens with reasons, and run ride-agreement. Layout work goes to Opus.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
2026-11-21 attempt (branch daisy-headroom, from 45e0702): stopped without a layout change; content/rides/daisy-dash.ride.json and designMargins are untouched, so no digests, hashes or goldens changed.

Baseline (check, construction-test / default physics): excitement 3.55 / 2.94, intensity 3.86, nausea 0.39; peaks +2.89 / -0.51 g and +2.82 / -0.41 g against +2.75 / -0.35 g. Seven pieces break the headroom in construction-test physics, not one: GentleDownToLevel into the 12 m valley (piece 10, +2.89, car 9), LevelToGentleUp out of it (piece 12, +2.84, car 0), LevelToGentleDown after the first family hill (piece 8, -0.48, car 9), GentleUpToLevel at the 22 m crest (piece 14, -0.39), the second of the two TerrainFall pieces before the last turn (piece 28, -0.49), the TerrainRise after that turn (piece 30, -0.41), and the TerrainFall after the last hill (piece 34, -0.34, passes by 0.01 g). An 8-car train does not help (+2.90 / -0.47 g).

Why small edits do not work: the speed-turn rated rule leaves a gap. SpeedTurnLarge16 needs a worst-car entry of 14.5-17.5 m/s and SpeedTurnLarge20 needs 18.5-21.5 m/s, under both physics. The 12 m valley gives 19.6 / 19.2 m/s, which suits the 20 m/s turn but is too fast for the classic gentle level-out and climb (about 2.85-2.9 g). A 14 m valley softens them (+2.71 / +2.67 g) but puts the turn at 18.6 / 18.2 m/s, inside the gap (measured). LongGentleDownToLevel (allowed on this model, 2.1 g instead of 2.9 g at 16.4 m/s) fixes the descent side, but there is no long level-to-climb piece. In the last third, the brake leaves the train at 16 m and 13.3 m/s and only TerrainFall can lose the 4 m to the 12 m station level; it pulls -0.27 g at 13.3 m/s and -0.49 g at 14 m/s, and a slower train reaches the last turn below 14.5 m/s. Reordering the shallow descents changed nothing (-0.49 / -0.50 g).

Full attempt (second half rebuilt, no mid-course brake): first hill, TerrainRise x2 to 26 m, drop to 18 m, SpeedTurnLarge16Left, climb to 26 m, TerrainFall x2 to 22 m, two 4x40 hills, the old rise and drop to a 16 m SpeedTurnLarge16Left, then two camelbacks to 24 m with FamilyHill3x32 hills around a 16 m SpeedTurnLarge16Left, ending with LevelToGentleDown + LongGentleDownToLevel to 12 m and the station brakes. Forces pass with margin (+2.35 / -0.27 g construction-test) but excitement falls to 2.13 (target 2.5-4.5), both 3x32 hills fail their rated windows (rear car enters at 7.5-9.8 m/s against a 10.4 m/s minimum), and the train reaches the station brakes at 17.6 m/s and leaves them at 10.7 m/s. Excitement fell because the airtime and strong-force credits came from exactly the moments over the headroom (airtime +0.4 -> 0, airtime moments +0.7 -> +0.3, whip +0.3 -> 0).

Conclusion: with the classic family-coaster pieces, the 0.25 g headroom and the rated-window rule, Daisy Dash cannot keep its excitement band (default physics 2.94 against a 2.5 floor) with local edits; it needs a full re-layout, or one of: a gentle level-to-climb piece (a long LevelToGentleUp), a speed turn rated around 18 m/s, shaped elements for family_coaster, or an owner decision on the family coaster headroom or excitement target. Experiment command lists are in the lead session scratchpad (orig.cmds, a.cmds, h.cmds).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Superseded by TASK-167. Daisy Dash’s profile, force-headroom redesign, ride-file digests/goldens, and validation under both physics configurations move into its all-model rollout; the local redesign attempts recorded here did not produce a viable layout.
<!-- SECTION:FINAL_SUMMARY:END -->
