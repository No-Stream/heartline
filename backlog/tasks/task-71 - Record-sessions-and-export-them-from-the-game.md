---
id: TASK-71
title: Record sessions and export them from the game
status: Done
assignee: []
created_date: '2026-11-03 20:05'
updated_date: '2026-11-03 22:11'
labels:
  - test
dependencies: []
priority: medium
ordinal: 71000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Plan: docs/superpowers/plans/2026-11-03-flexible-track-and-gameplay-tests.md, work item 9. Worker command journal with tick stamps and hash checkpoints, input log, Export session control.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented session format theme-park-session v1, worker journal and checkpoints, input log, and Watch bar export (J). Replay must use manual block analysis and replay the recorded advances; set-surface corners must be restored to Int16Array. Journal fails before applying command 100001.

Verification: make build passed; npm test 204 passed; npm run test:art 9 passed; npm run typecheck passed. Real-worker Playwright passed with checkpoints at ticks 300 and 600. UI Playwright export passed with station, two pieces, five commands and matching tick after 37 frozen ticks.

Journey gate remains red in concurrently edited construction work: latest run failed Timber Tumbler on an ambiguous data-preview-name locator and expected climbing turn name, and smoke timed out waiting for readiness under concurrent browser load. Earlier smoke passed. Re-run the journey gate after the construction owner resolves the selector/name and investigate readiness timeout if it recurs. No make ci or commit.
<!-- SECTION:NOTES:END -->
