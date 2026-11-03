---
id: TASK-52
title: Add shared simulation helpers and remove duplicated logic
status: To Do
assignee: []
created_date: '2026-11-03 16:47'
labels:
  - refactor
  - simulation
milestone: m-4
dependencies: []
documentation:
  - docs/reviews/2026-11-03-simulation-code.md
priority: medium
ordinal: 52000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Simulation review step 2: the construction-pieces-plus-segments chain appears 11 times in the bridge, the facility footprint test 5 times, enum wire codes in several places, 4 FNV hash implementations, lift_speed 5.0 three times; track review: terrain triangle interpolation repeated in supports and proximity, support geometry assumptions duplicated in construction.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Construction::all_pieces, a Building rectangle and clearance box, FacilityKind::spec, enum wire-name functions, one hasher, shared terrain interpolation and support-owned search geometry replace the copies
- [ ] #2 Behaviour is unchanged unless stated, and the relevant tests pass while editing, with make ci green at the end
<!-- AC:END -->
