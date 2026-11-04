---
id: TASK-60
title: Split the people and building art generators
status: Done
assignee: []
created_date: '2026-11-03 16:47'
updated_date: '2026-11-04 00:17'
labels:
  - refactor
  - art
milestone: m-4
dependencies: []
documentation:
  - docs/reviews/2026-11-03-rendering-art-tests.md
priority: low
ordinal: 60000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Rendering review: people.ts combines several jobs in one large function, buildings.ts repeats roof geometry, and atlas output needs protecting during regeneration.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 People and building generators are split into focused modules with repeated roof geometry extracted, and drawings stay byte-identical
- [x] #2 Behaviour is unchanged unless stated, and the relevant tests pass while editing, with make ci green at the end
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
People generator split into design, appearance and drawing modules; one module per building with shared gableRoofGeometry in building-primitives.ts; all 821 generated SVGs byte-identical. Atlas protection not done yet: web/art/pipeline/build-atlas.ts removes the output directory before rasterising; it should write into a sibling staging directory and swap in only on success.

Atlas regeneration now writes pages and atlas.json to a sibling staging directory and publishes them with a rename-based swap after generation succeeds. A failure test verifies that a partially generated replacement leaves the prior atlas files intact. `npm run test:art` passed (9 tests); `make art` packed 821 drawings. All five output file SHA-256 hashes matched the pre-change values.

Closed 2026-11-03: make ci at 9987c3e passed everything except the piece-catalog baselines (expected after the lift piece removal), refreshed in 8be816e.
<!-- SECTION:NOTES:END -->
