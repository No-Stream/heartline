---
id: TASK-40
title: Speed up the WebAssembly bridge tests
status: Done
assignee: []
created_date: '2026-11-03 14:12'
updated_date: '2026-11-03 14:46'
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
- [x] #1 The wasm crate's tests finish in under a minute in make ci with the same coverage
- [x] #2 Long simulations in tests run in release mode or on shorter fixtures, with the reason noted
<!-- AC:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Package-specific opt-level for theme-park-sim in test builds; wasm tests fell from about 1,208 s to 43 s with the same assertions.
<!-- SECTION:FINAL_SUMMARY:END -->
