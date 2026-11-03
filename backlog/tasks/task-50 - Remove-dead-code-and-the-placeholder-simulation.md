---
id: TASK-50
title: Remove dead code and the placeholder simulation
status: Done
assignee: []
created_date: '2026-11-03 16:47'
updated_date: '2026-11-03 17:06'
labels:
  - refactor
  - chore
milestone: m-4
dependencies: []
documentation:
  - docs/reviews/2026-11-03-browser-code.md
priority: high
ordinal: 50000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Browser review finding 6 and simulation review step 1: the unused renderer.ts and guest-panel index re-export, block-marker drag actions that are sent but never handled, the coaster model choice passed as a fake piece id, and the placeholder Agent and Simulation in crates/sim/src/lib.rs whose threaded test exercises the placeholder instead of guests. Six bridge exports are never called.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Dead code is removed, the coaster model choice has its own action, Executor lives in its own module and tick_bench uses real guests
- [x] #2 Behaviour is unchanged unless stated, and the relevant tests pass while editing, with make ci green at the end
<!-- AC:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Placeholder simulation removed, Executor in its own module, benchmarks and threaded determinism tests use real guests, dead browser files removed, coaster model choice has its own action. Left for later: the worker still accepts the legacy model:<name> value, block-marker drag-move and drag-cancel actions are unhandled in main.ts, and five bridge exports are unused (world_state_hash, construction_can_append, construction_preview_valid, construction_preview_reason, construction_set_block_sections).
<!-- SECTION:FINAL_SUMMARY:END -->
