---
id: TASK-66
title: 'Let players charge at the park gate and per ride, both adjustable'
status: Done
assignee: []
created_date: '2026-11-03 20:05'
updated_date: '2026-11-03 20:22'
labels:
  - economy
  - interface
dependencies: []
priority: high
ordinal: 66000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Plan: docs/superpowers/plans/2026-11-03-flexible-track-and-gameplay-tests.md, work item 4. Owner decision: support an entrance fee and a coaster ticket price, each adjustable; riders pay at boarding; coaster income shown.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Players can set an entrance fee, affordable guests pay it on admission, and park revenue records it.
- [x] #2 Players can set a coaster ticket price (default 6), riders pay at boarding, and coaster income is shown.
- [x] #3 Zero entrance and coaster prices remain valid, and a browser test changes both prices through their UI controls.
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented entrance fees through admission affordability and lifetime spending, adjustable coaster price defaulting to 6, boarding income exposure, and UI steppers for both controls. Targeted Rust, bridge, Vitest, typecheck, build, and browser results are being recorded for this task.

Verification passed: focused sim tests for affordability, guest admission fees, Park tick revenue, paid coaster boarding/income, and zero-price boarding; sim-wasm default/zero-price setter test; construction-effects Vitest (2 tests); TypeScript typecheck; make build; and coaster-boarding.spec.ts on PLAYWRIGHT_PORT=4517 (1 test). Cargo formatting check passed. No full CI was run.

Formatting scope clarification: rustfmt --check on the TASK-66 Rust files and git diff --check passed. A later workspace-wide cargo fmt --all -- --check reports formatting differences in concurrent work at crates/sim/src/track/construction/flexibility_tests.rs and crates/sim/tests/timber_tumbler_prefix_dead_end.rs; those files were left untouched.

Final production-build browser verification also passed: coaster-boarding.spec.ts on the independently checked free port 4518 (1 test, 2.3 minutes).
<!-- SECTION:NOTES:END -->
