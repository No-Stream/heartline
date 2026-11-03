---
id: TASK-46
title: Fix path tool placing one tile at a time
status: To Do
assignee: []
created_date: '2026-11-03 16:40'
labels:
  - world
  - interface
  - bug
milestone: m-1
dependencies: []
priority: high
ordinal: 46000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Owner report: the path tool now seems to place one square at a time instead of dragging a run, likely a regression from the explicit path edge connections or toolbar changes.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Dragging lays a full path run with a preview again, in every rotation
- [ ] #2 A browser test drags a multi-tile run and checks every tile is placed and connected
<!-- AC:END -->
