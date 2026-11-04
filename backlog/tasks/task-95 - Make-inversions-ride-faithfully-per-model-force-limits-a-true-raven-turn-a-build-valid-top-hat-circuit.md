---
id: TASK-95
title: >-
  Make inversions ride faithfully: per-model force limits, a true raven turn, a
  build-valid top hat circuit
status: To Do
assignee: []
created_date: '2026-11-04 10:29'
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
