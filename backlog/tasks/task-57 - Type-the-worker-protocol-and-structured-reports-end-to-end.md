---
id: TASK-57
title: Type the worker protocol and structured reports end to end
status: To Do
assignee: []
created_date: '2026-11-03 16:47'
labels:
  - refactor
  - interface
  - simulation
milestone: m-4
dependencies: []
documentation:
  - docs/reviews/2026-11-03-browser-code.md
priority: high
ordinal: 57000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Browser review finding 1 and simulation review step 7: the main thread and worker have no shared typed protocol, some sent messages are never handled, four wire formats cross the bridge (typed arrays with strides in comments, tab-separated strings, hand-built JSON, Debug output as ids), and TypeScript types have drifted from Rust.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 One protocol.ts with typed send helpers and exhaustive handling; structured reports use serde with explicit wire names and generated TypeScript types; test configuration arrives in an init message
- [ ] #2 Behaviour is unchanged unless stated, and the relevant tests pass while editing, with make ci green at the end
<!-- AC:END -->
