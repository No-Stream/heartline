---
id: TASK-36
title: Build the path building tool
status: To Do
assignee: []
created_date: '2026-11-03 06:15'
labels:
  - world
  - interface
milestone: m-1
dependencies: []
priority: high
ordinal: 36000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The owner named path building as a priority. Players need RCT2-style path placement: drag to lay paths, slopes that follow terrain, stairs or ramps between levels, queue paths (used by the boarding ticket), path styles per theme, and deletion. Design sections 6 and 7.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Players lay paths by dragging, with a ghost preview, and paths follow terrain slopes
- [ ] #2 Paths connect across levels with sloped sections, including into tunnels
- [ ] #3 Queue path pieces exist and connect to ride and facility entrances
- [ ] #4 Deleting paths under guests moves them sensibly, and nothing freezes
- [ ] #5 A browser test lays a path network through the UI and guests walk it
<!-- AC:END -->
