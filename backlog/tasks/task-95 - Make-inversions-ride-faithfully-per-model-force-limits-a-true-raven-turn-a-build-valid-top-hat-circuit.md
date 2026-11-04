---
id: TASK-95
title: >-
  Make inversions ride faithfully: per-model force limits, a true raven turn, a
  build-valid top hat circuit
status: In Progress
assignee: []
created_date: '2026-11-04 10:29'
updated_date: '2026-11-04 10:58'
labels:
  - coaster
  - construction
dependencies: []
priority: medium
ordinal: 95000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Follow-up to TASK-89 (00c93be). (1) Models define minimum curve radii but no force limits, so tests only check finite forces; add per-model comfortable force limits and check every inversion at realistic entry speeds. (2) The 'sea serpent' combines half corkscrews that keep their heading, so it only approximates a raven turn; build a faithful raven turn and sea serpent. (3) The top-hat lap and report fixtures build circuits with Circuit::new, so their return paths are not checked against construction clearance; add a circuit built through Construction that passes a top hat.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
2026-11-04: Codex job task-mw206l8l-oqayml in worktree theme-park-sim-worktrees/inversion-forces.
<!-- SECTION:NOTES:END -->
