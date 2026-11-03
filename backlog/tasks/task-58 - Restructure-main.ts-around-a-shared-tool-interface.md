---
id: TASK-58
title: Restructure main.ts around a shared tool interface
status: To Do
assignee: []
created_date: '2026-11-03 16:47'
labels:
  - refactor
  - interface
milestone: m-4
dependencies: []
documentation:
  - docs/reviews/2026-11-03-browser-code.md
priority: medium
ordinal: 58000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Browser review findings 2 to 4: main.ts is 654 lines with about 40 module-level variables, dispatch mixes routing with picking maths, station and facility placement have no tool modules, frame() builds the scene inline, panels follow three conventions, and gameTest.ready = false appears 20 times.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A shared tool interface with station and facility tool modules and unit tests, picking helpers in their own module, scene assembly out of frame(), panels emitting actions through dispatch, and one revision counter replacing the scattered test readiness flags
- [ ] #2 Behaviour is unchanged unless stated, and the relevant tests pass while editing, with make ci green at the end
<!-- AC:END -->
