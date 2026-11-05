---
id: TASK-120
title: Accept keyboard building input while the construction editor is busy
status: To Do
assignee: []
created_date: '2026-11-05 04:22'
labels:
  - construction
  - input
dependencies: []
priority: medium
ordinal: 120000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Journeys on the force-limited inversions branch showed that keyboard shortcuts (h for chain lift, b to place) are silently ignored while the construction editor is busy validating, so a fast player loses keypresses. The journeys now wait for aria-busy=false. Queue or apply the input once the editor is ready instead of dropping it.
<!-- SECTION:DESCRIPTION:END -->
