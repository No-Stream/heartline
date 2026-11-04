---
id: TASK-97
title: >-
  Find why the operate journey sometimes selects a different guest than it
  clicked
status: To Do
assignee: []
created_date: '2026-11-04 13:55'
labels:
  - test
  - bug
dependencies: []
priority: medium
ordinal: 97000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Seen 2026-11-04 in one make journeys run on the renderer-stages branch: the Timber Tumbler operate journey picked guest-0 before clicking, and the guest panel then showed guest-10; three repeat runs passed. The guest selection step has been the least reliable part of this journey. Instrument the click: pointer-up position, pick result at event time, guest poses and render tick, and compare with the selected id, then fix the cause (input timing, lazy picking, or the journey's selection helper).
<!-- SECTION:DESCRIPTION:END -->
