---
id: TASK-95
title: >-
  Make inversions ride faithfully: per-model force limits, a true raven turn, a
  build-valid top hat circuit
status: In Progress
assignee: []
created_date: '2026-11-04 10:29'
updated_date: '2026-11-04 11:50'
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
<!-- SECTION:NOTES:END -->
