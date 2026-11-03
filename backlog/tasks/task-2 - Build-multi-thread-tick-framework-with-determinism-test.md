---
id: TASK-2
title: Build multi-thread tick framework with determinism test
status: To Do
assignee: []
created_date: '2026-11-03 01:51'
labels:
  - simulation
  - performance
milestone: m-0
dependencies:
  - TASK-1
documentation:
  - docs/superpowers/specs/2026-11-02-theme-park-sim-design.md
priority: high
ordinal: 2000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The owner doubts one thread will scale, so the simulation is designed for threads from the start. A tick is explicit phases that read shared state and write per-partition output, merged in a fixed order. The browser build stays on one thread for now. Design section 10.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A native test runs the same seed and inputs on 1, 2 and 4 threads and finds identical state hashes
- [ ] #2 Benchmark reports speed against thread count on a synthetic workload of 10,000 agents
- [ ] #3 Browser build runs the same framework on one thread on stable Rust
<!-- AC:END -->
