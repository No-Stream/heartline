---
id: TASK-41
title: Triage slow tests into fast and slow tiers
status: To Do
assignee: []
created_date: '2026-11-03 16:26'
labels:
  - infra
  - performance
milestone: m-2
dependencies: []
priority: high
ordinal: 41000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The full gate takes about 7 minutes: Playwright about 4 minutes with 4 workers (slowest: wooden twister 49 s, daylight confetti 35 s, guest appearances 29 s, path rebuild 22 s), Rust suites about 50 s each and wasm about 46 s. Agents running it in parallel starve the machine and cause timeouts. Inputs: the rendering, art and tests review in docs/reviews/.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A fast tier (make check) runs in under 90 seconds and covers each feature at least once
- [ ] #2 Slow end-to-end flows run in a separate tier (make ci still runs everything) and none waits on wall-clock time
- [ ] #3 Each slow test's purpose is kept, with cheaper fixtures or checks moved to unit tests where possible
<!-- AC:END -->
