---
id: TASK-38
title: Combine tool panels into one toolbar
status: Done
assignee: []
created_date: '2026-11-03 10:44'
updated_date: '2026-11-03 11:30'
labels:
  - interface
milestone: m-1
dependencies: []
priority: high
ordinal: 38000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Four tool panels (coaster, landscaping, facilities, paths) are stacked down the page at once, and the facilities panel is unstyled. The owner tries the builder this morning.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A toolbar switches between coaster, paths, landscaping and facilities, showing one panel at a time beside the world
- [x] #2 All panels share the print-modern style, including facilities
- [x] #3 The world view uses the available screen space
<!-- AC:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Toolbar with Watch, Coaster, Paths, Landscape and Facilities; one panel at a time; world fills the window at the right pixel ratio; shared print-modern panel styles; facilities panel rebuilt with thumbnails. Fixed the souvenir shop kind mismatch that stopped it being placed or drawn.
<!-- SECTION:FINAL_SUMMARY:END -->
