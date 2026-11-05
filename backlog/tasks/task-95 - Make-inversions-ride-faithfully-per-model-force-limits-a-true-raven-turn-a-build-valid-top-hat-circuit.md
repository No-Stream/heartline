---
id: TASK-95
title: >-
  Make inversions ride faithfully: per-model force limits, a true raven turn, a
  build-valid top hat circuit
status: Done
assignee: []
created_date: '2026-11-04 10:29'
updated_date: '2026-11-05 04:22'
labels:
  - coaster
  - construction
dependencies: []
priority: high
ordinal: 95000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Follow-up to TASK-89 (00c93be). (1) Models define minimum curve radii but no force limits, so tests only check finite forces; add per-model comfortable force limits and check every inversion at realistic entry speeds. (2) The 'sea serpent' combines half corkscrews that keep their heading, so it only approximates a raven turn; build a faithful raven turn and sea serpent. (3) The top-hat lap and report fixtures build circuits with Circuit::new, so their return paths are not checked against construction clearance; add a circuit built through Construction that passes a top hat.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
2026-11-04: Codex job task-mw206l8l-oqayml in worktree theme-park-sim-worktrees/inversion-forces.

2026-11-04: first attempt (branch inversion-forces, 04f3efc, not merged) found master's half loop peaks at 19.3 g at 32 m/s; its redesign reached 5.7 g against a provisional 5 g limit; top hat, corkscrews, half corkscrews and heartline rolls also exceed targets. Second attempt task-mw2216w2-uo13uq (Terra, high effort) shapes elements from rider forces (clothoid loops, heartline rolls, speed-sized helices).

2026-11-04: second attempt (task-mw2216w2-uo13uq) stopped after 8 minutes without redesigning geometry; it left an uncommitted same-piece support clearance check. Claude Fable is writing a design plan at docs/superpowers/plans/2026-11-04-force-limited-inversions.md in the inversion-forces worktree (entry speeds, limits, curve families, grid snapping, force checks, work breakdown); implementation follows from it.

2026-11-04: Fable's plan committed on branch inversion-forces (874af14). Implementation job task-mw235gmd-2tmaf0 (Terra, medium) following its twelve work items; stops after the half-loop milestone if the method does not hold.

2026-11-04: implementation stopped at the half-loop milestone (36f093e on branch): a half loop rated for 31.3 m/s needs a 46 m rise with a 0.86 m/s entry window, and a full loop closed 7 m high. Fable is revising the plan (loop sizes per entry-speed band, mirrored climbing half for closure, top-of-loop g target).

2026-11-04: half-loop milestone met on branch (6be1f3e): 38 m rise, 3.95-3.96 g full-train peaks across the rated window 27.96-31.3 m/s, down from about 19 g. Job task-mw26x03f-9m6xa6 implements the remaining work items; TopHat untouched pending the owner's decision.

2026-11-04 evening: the first implementation stopped unfinished after 4 hours. A Fable audit (plan section 'Audit and revised plan') found the 284 m heartline roll came from measuring longitudinal force along the tilted rail, the long corkscrews from a too-concentrated roll schedule and a modern-sized radius, and the slow construction from a brute-force support self-clearance check plus per-click catalog rebuilds. Work continues on branch forces-fixes (seven ordered tasks); the old inversion-forces worktree has a stray Codex thread and is no longer used.

2026-11-04 late: merged to master as 0cb7533. Luna's embedded profile table (381aa05) was replaced after an independent Terra review found runtime curvature decoupled from the path; now 512-point uniform tables from one interpolant (46bcacb), solver no longer emits near-duplicate points, SixtyToSteepDown window restored to 28 m/s. Journeys re-routed for the resized pieces and the Timber Tumbler session re-recorded. Plan outcome section has the measured numbers.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Inversions, drops and 60 degree transitions keep full trains inside per-model rider force limits across rated entry-speed bands; corkscrews are transition helices and half corkscrews are half rolls; solved profiles are embedded; construction queries are within the synchronous worker budget. Gates: make lint, make test, make journeys (8/8) on the merged branch.
<!-- SECTION:FINAL_SUMMARY:END -->
