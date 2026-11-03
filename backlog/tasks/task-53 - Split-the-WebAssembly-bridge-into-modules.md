---
id: TASK-53
title: Split the WebAssembly bridge into modules
status: To Do
assignee: []
created_date: '2026-11-03 16:47'
updated_date: '2026-11-03 18:44'
labels:
  - refactor
  - simulation
milestone: m-4
dependencies: []
documentation:
  - docs/reviews/2026-11-03-simulation-code.md
priority: medium
ordinal: 53000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
crates/sim-wasm/src/lib.rs is about 190 KB in one file, with about 3,500 lines of tests inside it. Simulation review step 3.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 The bridge is split by domain (world, guests, construction, blocks, facilities, scenery, reports) with tests in separate files and no behaviour change
- [ ] #2 Behaviour is unchanged unless stated, and the relevant tests pass while editing, with make ci green at the end
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Split into world, guests, construction, blocks, facilities, scenery and reports modules with tests under src/tests; 80 bridge tests before and after; exported names and signatures unchanged (declaration order in the generated .d.ts differs).
<!-- SECTION:NOTES:END -->
