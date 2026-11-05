---
id: TASK-59
title: Centralise projection and depth and split renderer stages
status: Done
assignee: []
created_date: '2026-11-03 16:47'
updated_date: '2026-11-04 13:55'
labels:
  - refactor
  - rendering
milestone: m-4
dependencies: []
documentation:
  - docs/reviews/2026-11-03-rendering-art-tests.md
priority: medium
ordinal: 59000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Rendering review findings and browser review: the rotated projection and depth expression is copied in about seven places, and the renderer relies on implicit state and pass ordering.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 One tested projection and depth function used everywhere; renderer inputs, stateful stages and resource setup are separated in small changes
- [ ] #2 Behaviour is unchanged unless stated, and the relevant tests pass while editing, with make ci green at the end
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
2026-11-04: Codex job task-mw24un3c-aiw1d5, worktree renderer-stages; must stay pixel-identical.

2026-11-04: merged; pixel-identical (111 screenshot and affected tests unchanged).
<!-- SECTION:NOTES:END -->
