---
id: TASK-73
title: Add the Velocity Viper and Briar Blitz player journeys
status: Done
assignee: []
created_date: '2026-11-03 20:05'
updated_date: '2026-11-04 10:01'
labels:
  - test
dependencies: []
priority: high
ordinal: 73000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Plan: docs/superpowers/plans/2026-11-03-flexible-track-and-gameplay-tests.md, work item 11. Velocity Viper keyboard-only; both pass three times in a row under make journeys.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
2026-11-03 night: Codex job task-mw1mhegf-7fk8d9 (gpt-6.1-terra, medium). Scope decision by the lead, to flag to the owner: these journeys cover building, closing and the ride test (Velocity Viper keyboard-only, two launches, climbing turn, slope-to-level; Briar Blitz curved gentle lift, descending banked turns, twisters); the guests, prices, opening and ride-memory loop stays covered once by the Timber Tumbler operate journey, because the full loop per model does not fit the 3-minute journey limit or the 5-minute make journeys budget.

2026-11-03 night: blocked. Sloped banked turns (climbing and descending) are generated but unreachable: bank transitions exist only on level track. Codex job task-mw1mvbm8-cjg2au adds gentle-slope bank transitions and a test that every piece each model offers is reachable from its station. Resume the journeys after that lands.

2026-11-03 night: unblocked by 271bf8e (sloped bank transitions). Codex job task-mw1nb915-szfup6 is building the two journeys.

2026-11-03 night: committed 25e24d3 (Velocity Viper about 39 s, Briar Blitz about 1.4 min, three passes each). Still to confirm: make journeys within 5 minutes on a quiet machine; the first try took 317 s while two other jobs compiled Rust on the same cores, and the operate journey timed out in it.

2026-11-04: make journeys passes (5 journeys) and the full gate is green at c30aa7e.
<!-- SECTION:NOTES:END -->
