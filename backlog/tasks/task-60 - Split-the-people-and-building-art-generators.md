---
id: TASK-60
title: Split the people and building art generators
status: To Do
assignee: []
created_date: '2026-11-03 16:47'
labels:
  - refactor
  - art
milestone: m-4
dependencies: []
documentation:
  - docs/reviews/2026-11-03-rendering-art-tests.md
priority: low
ordinal: 60000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Rendering review: people.ts combines several jobs in one large function, buildings.ts repeats roof geometry, and atlas output needs protecting during regeneration.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 People and building generators are split into focused modules with repeated roof geometry extracted, and drawings stay byte-identical
- [ ] #2 Behaviour is unchanged unless stated, and the relevant tests pass while editing, with make ci green at the end
<!-- AC:END -->
