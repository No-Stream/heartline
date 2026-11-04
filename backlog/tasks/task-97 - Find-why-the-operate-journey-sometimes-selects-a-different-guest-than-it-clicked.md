---
id: TASK-97
title: >-
  Find why the operate journey sometimes selects a different guest than it
  clicked
status: In Progress
assignee: []
created_date: '2026-11-04 13:55'
updated_date: '2026-11-04 20:41'
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

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
2026-11-04: an instrumented investigation could not reproduce the wrong-guest selection; a second failure mode appeared (no stationary queued guest within 45 s). Fix: add a real pause (Space and a clock button) and have the journey select guests while paused, as a player would.
<!-- SECTION:NOTES:END -->
