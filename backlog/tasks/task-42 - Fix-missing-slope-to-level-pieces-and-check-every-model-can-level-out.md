---
id: TASK-42
title: Fix missing slope-to-level pieces and check every model can level out
status: Done
assignee: []
created_date: '2026-11-03 16:34'
updated_date: '2026-11-03 18:12'
labels:
  - coaster
  - bug
milestone: m-1
dependencies: []
priority: high
ordinal: 42000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Owner report: on the Intamin-inspired Velocity Viper there is no obvious way to return from a slope to level. Its explicit piece list probably lacks the slope-to-level transitions.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Velocity Viper offers slope-to-level transitions from every slope it can reach
- [x] #2 A test for every model checks that from every reachable track end state (slope, bank, curvature) a sequence of allowed pieces returns to straight and level
- [x] #3 The builder shows the transition pieces under Slopes
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
A reachability test shows Velocity Viper can already return to level from every reachable slope state, so the pieces exist; Briar Blitz was missing six level-out pieces, now added. Open question: why the owner could not find the way back to level in the running builder. Next: reproduce in the browser on Velocity Viper (likely the slope stepper or the continuation pieces after a slope entry hiding the level-out choice).

Velocity Viper browser coverage now proves GentleUpToLevel and GentleDownToLevel are visible under Slopes, slope stepping reaches each, and placing each leaves the slope stepper at Level. Level-out exit tiles show a visible Level out label; no Rust or WASM choice changes were needed.
<!-- SECTION:NOTES:END -->
