---
id: TASK-162
title: Explain Briar Blitz's 7 g lateral and 4.4 g vertical in the design check
status: Done
assignee: []
created_date: '2026-11-21 05:39'
updated_date: '2026-11-21 14:40'
labels: []
dependencies: []
ordinal: 158000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found during TASK-160 (2026-11-20): tools/design check content/rides/briar-blitz.ride.json reports rated-margin failures on BankLeft, UnbankLeft and TopHat, positive vertical 4.34-4.41 g against a 3.75 g headroom limit, and lateral 7.09-7.26 g against 1.25 g. The check still exits 0 because designMargins is opt-in. 7 g lateral is not plausible for a ride that tests fine in the game, so first establish whether these are real ride forces or a measurement artifact (e.g. bank and unbank transitions in the force evaluation), then either fix the measurement or the ride. The browser-agreement fast circuit fixture also shows 1.59 g lateral against 1.25 g. Until both are resolved, check cannot make rated-window and force-headroom failures fatal for every ride (TASK-160 item 3).
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
2026-11-20 investigation (branch briar-forces, from 0bff7fd). Verdict: the forces are real for this layout; the measurement is not at fault. No code, ride file, digest or golden changed.

Where the peaks are (`tools/design check --json` and `ride ... report --samples`, construction-test / default physics): lateral 7.26 / 7.09 g on the first TightTwisterLeft (piece 31, car 0), with pieces 32-34 at 7.11-7.21 g; positive vertical 4.41 / 4.34 g on GentleDownToLevel (piece 12, car 11) at the bottom of the 36 m first drop. Other pieces over the 1.25 g lateral headroom: BankedTurnStandalone (piece 10, 2.03 g, car 11), UnbankLeft (piece 11, 1.67 g, car 11), the closing TurnSmallStandalone (piece 58, 4.66 g) and TurnLargeRightStandalone (piece 56, 2.38 g). The ride scores excitement 0.36, intensity 9.42, nausea 6.88.

Why these are not an artifact: the four tight twisters are flat, unbanked 90 degree turns (vertical stays at exactly 1.00 g) taken at 25.1-25.4 m/s, straight after the top hat. In the one-metre front-car trace, lateral force holds at 6.9-7.25 g for about 12 m of each 20 m piece, so it is not a single-sample spike, and a 5 Hz filter would not change it. Integrating lateral acceleration over speed squared across piece 31 gives a heading change of 90.4 degrees, exactly the piece's turn, so the lateral axis and frame are right. 7.25 g at 25.36 m/s means a 9.0 m radius, above the model's 8 m minimum radius. The same flat TurnSmallStandalone gives 1.55 g at 13.6 m/s in the browser-agreement-fast fixture and 4.66 g at 23.6 m/s in Briar Blitz; both match a 12.2 m radius, so the fast fixture's 1.59 g (piece 3, after the 14 m/s launch and airtime hill) is also a real flat turn at speed. GentleDownToLevel: integrating (vertical - 1) g over speed squared gives about 25 degrees of pitch change, matching the 26.6 degree gentle slope; 4.1-4.4 g at 24-26 m/s means a pull-out radius of about 22 m. UnbankLeft's 1.4-1.7 g is gravity on the banked straight (up to about 0.8 g) plus the sideways acceleration of riders above the rail as the track rolls back to level at 21 m/s; rider_g (crates/sim/src/physics/mod.rs) differentiates the heartline position along the rail, which is the correct kinematics.

In-game test report: `make ride-agreement RIDE=briar-blitz` passed (browser report parity 58/58 pieces). That test requires the browser test report's per-piece front-car and worst-car peaks to equal the native ones within 1e-6 g, so the game's own ride test shows the same 7.25 g and 4.41 g. "Runs fine" only means the train completes the circuit; the game does not stop a ride for force.

Rated-window failures are also real: BankLeft entry 5.74 m/s (default physics) is inside its 5.0-23.1 m/s window but under the 1.5 m/s edge rule; UnbankLeft's rear car enters at 23.3 / 23.6 m/s against 24.3; the TopHat window is only 3.1 m/s wide (24.6-27.7), so the 1.5 m/s rule leaves a 0.11 m/s band (26.13-26.24) that has to hold under both physics configurations, whose entries differ by 0.43 m/s here. That TopHat rule is effectively unattainable; I flag it for the owner and did not change it.

Cause: Briar Blitz was moved from a programmatic circuit fixture (commit daa27ae). Its expectations only count features (a TopHat, two TightTwister/Twister pieces, a banked turn after the lift). It was never shaped for forces: the back half is the top hat, four flat tight turns, 21 straights and two flat turns, all at 23.5-26.5 m/s, and the train reaches the station at 23.5 m/s.

Proposed ride changes (not applied; a layout job for a design session):
1. Replace GentleDownToLevel (step 13) with LongGentleDownToLevel. Estimate at the open end: 2.9 g instead of 4.4 g, forward 0.4 g instead of 1.2 g. It is refused at this exact spot ("Supports cannot be placed here"), so it needs one more GentleDown first or a shifted drop (`estimate GentleDown LongGentleDownToLevel` gives the same 2.9 g).
2. The tight twisters cannot run at 25 m/s on any model: a flat 9 m turn stays under 1.25 g only below 10.5 m/s, and under the owner's 0.6 g lateral target only below 7.3 m/s. Banked Twister pieces do not help either; a 12 m banked turn at 25 m/s is over 5 g. The section needs about 15 m/s less speed: a climb of about 30 m after the top hat (energy kept, preferred), or a brake run (the guide treats big brake runs as a design smell). The owner should decide whether this fixture should stay a feature-coverage fixture or become a designed ride; if designed, the back half needs a re-layout: top hat, climb, the four twisters near the top at about 9-10 m/s, then a descent with shaped banked turns and a slower station approach.
3. The closing flat TurnLargeRightStandalone and TurnSmallStandalone need the same lower speed (12 m radius allows about 12 m/s at 1.25 g).
4. BankLeft needs about 0.8 m/s more entry speed in default physics, and the bank and unbank sequence should be checked again after the drop changes.

Proposed piece-design change: the flat classic turns (TurnSmallStandalone, TurnLargeStandalone, TightTwister, TwisterFan) have no rated speed window, so `choices` and the builder show "—" and nothing warns before the test. Giving them a maximum rated entry speed from the model's lateral limit (about 11.5 m/s for TightTwister at 1.5 g on wooden_twister) would flag this kind of layout at placement time.

Not a blocker for TASK-160 item 3 on measurement grounds: the numbers are honest. Making margins fatal for every ride still needs Briar Blitz (and Daisy Dash, TASK-161) redesigned or exempted by the owner.

Minor observation: the first sample of the report trace (distance 0, piece 0) carries the station arrival of the previous lap (23.5 m/s, 0.97 g lateral) before the speed resets to 2.0 m/s. It does not affect peaks here, but it looks like a lap-wrap sample worth a separate check.

2026-11-21 redesign landed (branch briar-redesign, 8973ba1, merged with master as 2d4b675 and fast-forwarded into master). Briar Blitz is now a force-shaped wooden twister: steep straight first drop with LongGentleDownToLevel pull-out, banked 28 m/s speed turn, top hat, floater hills, a mid-course brake, 20 m/s banked turns and S-bends, ejector hills, then a climb to four tight twisters at 6-7 m/s and a gentle drop into the station brakes. `tools/design check`: lateral 0.43 g default / 0.58 g construction-test (was 7.26 g), positive vertical 3.38 / 3.44 g (was 4.41 g), minimum vertical -0.45 / -0.53 g, forward 1.03 / 1.06 g, braking 1.07 / 1.08 g; every force-headroom row passes. Excitement 4.72 default / 5.44 construction-test. All expectations pass. Remaining design-margin failure: TopHat (piece 16) rated margin -0.24 m/s default and -0.53 m/s construction-test under the current narrow-window centre rule (window 24.63-27.74 m/s, entries 26.48 / 26.77); by my calculation both would pass the owner's proposed min(1.5 m/s, 25% of width) edge rule at about +0.48 and +0.19 m/s, which is not on master yet. Also an advisory placement-estimate note on piece 44 (longitudinal 0.007 g estimate vs 0.346 g measured, classified as downstream track; does not fail the agreement). Tests changed by the branch: Briar Blitz builder state hash and output trace digest re-recorded (crates/sim-wasm/src/builder_session.rs); the prefix-ordering expectation test in crates/design-tools/src/commands.rs builds its own feature list instead of reading Briar Blitz's expect block (the BankedTurnStandalone-after-lift expectation is gone from the ride); the journey's "Descending small left-banked turn" label check moved to web/src/construction-choices.test.ts. Gates on 2d4b675: Rust workspace --no-fail-fast 1139 passed, 0 failed; Vitest 497 + art 9; typecheck; lint; make check (@fast 47 passed); ride-agreement briar-blitz 2/2 (parity 45/45); feature 200/200; journeys 11/11.
<!-- SECTION:NOTES:END -->
