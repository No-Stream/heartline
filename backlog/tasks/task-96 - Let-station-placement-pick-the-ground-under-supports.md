---
id: TASK-96
title: Let station placement pick the ground under supports
status: To Do
assignee: []
created_date: '2026-11-04 11:50'
labels:
  - interface
  - bug
dependencies: []
priority: medium
ordinal: 96000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
From TASK-88 (model-accurate supports): the station tool picks whatever is drawn under the cursor, so wider wooden bents can block clicks on the ground beneath them. Station (and probably facility) placement should use a ground-only pick. Browser tests currently work around it with visibleTilePoint in web/e2e/player/driver.ts.
<!-- SECTION:DESCRIPTION:END -->
