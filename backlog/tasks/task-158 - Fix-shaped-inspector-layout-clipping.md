---
id: TASK-158
title: Fix shaped inspector layout clipping
status: To Do
assignee: []
created_date: '2026-11-20 11:02'
labels:
  - shaped-elements
dependencies: []
ordinal: 155000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The shaped-hill inspector (web/e2e/construction.spec.ts-snapshots/shaped-hill-inspector-linux.png) shows the strength stepper's 'g' unit outside the + button at the panel's right edge, and the summary header 'Floater hill · 87 km/h' is cut off. Fix the layout so the unit sits inside the value like the classic steppers, re-record the baseline and look at it.
<!-- SECTION:DESCRIPTION:END -->
