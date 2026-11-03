---
id: TASK-40
title: Speed up the WebAssembly bridge tests
status: To Do
assignee: []
created_date: '2026-11-03 14:12'
labels:
  - infra
  - performance
milestone: m-1
dependencies: []
priority: high
ordinal: 40000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
After the block sections work, the theme-park-sim-wasm test suite takes about 20 minutes (1,208 s) in make ci, against seconds before, which makes the gate painful for everyone.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The wasm crate's tests finish in under a minute in make ci with the same coverage
- [ ] #2 Long simulations in tests run in release mode or on shorter fixtures, with the reason noted
<!-- AC:END -->
