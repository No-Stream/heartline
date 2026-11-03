---
id: TASK-72
title: Replay exported sessions as regression tests
status: Done
assignee: []
created_date: '2026-11-03 20:05'
updated_date: '2026-11-03 22:30'
labels:
  - test
dependencies: []
priority: medium
ordinal: 72000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Plan: docs/superpowers/plans/2026-11-03-flexible-track-and-gameplay-tests.md, work item 10. replay.spec.ts over web/e2e/sessions, sessions:rehash.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented frozen real-worker replay for every exported session, checkpoint annotations and strict mode, and sessions:rehash. Recorded Timber Tumbler chain and paths/food stall through the UI. Focused Playwright: 2 passed in 3.0 s; altered path command diverged at tick 300. Focused Vitest: 5 passed; typecheck and web build passed. A later recording-helper rerun hit parkBounds in web/src/render/effects.ts with an empty new park (nothing to launch fireworks over), leaving gameTest.ready false; investigate empty-park fireworks rendering and rerun the helper. Existing exports were captured before that failure. make ci was not run as requested.
<!-- SECTION:NOTES:END -->
