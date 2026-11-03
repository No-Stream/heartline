---
id: TASK-48
title: Fix food facilities refusing placement with missing access
status: To Do
assignee: []
created_date: '2026-11-03 16:40'
labels:
  - world
  - bug
milestone: m-1
dependencies: []
priority: high
ordinal: 48000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Owner report: placing food facilities says 'missing access'. Likely a regression from explicit path edge connections: the facility entrance check may now require an explicit edge that a neighbouring path does not have.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Food, drink and other facilities place next to a path in every direction and connect to it
- [ ] #2 A browser test places each facility kind through the UI next to an existing path
<!-- AC:END -->
