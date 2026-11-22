---
id: TASK-158
title: Fix shaped inspector layout clipping
status: Done
assignee: []
created_date: '2026-11-20 11:02'
updated_date: '2026-11-22 16:02'
labels:
  - shaped-elements
dependencies: []
ordinal: 155000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The shaped-hill inspector (web/e2e/construction.spec.ts-snapshots/shaped-hill-inspector-linux.png) shows the strength stepper's 'g' unit outside the + button at the panel's right edge, and the summary header 'Floater hill · 87 km/h' is cut off. Fix the layout so the unit sits inside the value like the classic steppers, re-record the baseline and look at it.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Strength stepper shows its g unit inside the value with no trailing unit outside +
- [x] #2 Shaped inspector stepper columns align cleanly when Auto is hidden
- [x] #3 Summary header (Floater hill · 87 km/h) does not clip at the right edge
- [x] #4 Updated shaped-hill-inspector-linux.png snapshot passes Playwright and visual inspection
<!-- AC:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Removed duplicate trailing stepper__unit 'g' from the shaped strength stepper in web/src/construction.ts, gated the 5th auto CSS grid track in web/src/construction.css on visible [data-shape-auto], added padding-inline-end: 0.2rem on .builder__hill-current to prevent italic overhang clipping, and re-recorded and visually verified shaped-hill-inspector-linux.png.
<!-- SECTION:FINAL_SUMMARY:END -->
