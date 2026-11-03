---
id: TASK-9
title: 'Draw track, supports and trains generated from curves'
status: Done
assignee: []
created_date: '2026-11-03 01:52'
updated_date: '2026-11-03 04:07'
labels:
  - coaster
  - rendering
  - art
milestone: m-1
dependencies:
  - TASK-6
  - TASK-7
documentation:
  - docs/superpowers/specs/2026-11-02-theme-park-sim-design.md
priority: high
ordinal: 9000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Track and supports are generated from the curve data and styled by model, in the art direction. Design sections 3 and 9.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 All pieces in the first model draw correctly in all four views, with screenshot tests
- [x] #2 Supports generate from curve and terrain and reroute around obstacles
- [x] #3 Trains with riders move smoothly along track between simulation ticks
<!-- AC:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Track, supports and trains render as real geometry through a general extra-pass hook in the renderer, sharing depth and picking with sprites; all 55 pieces render and pick in four rotations. Coaster colours currently travel as metadata on one grass drawing, which needs a proper theme style export.
<!-- SECTION:FINAL_SUMMARY:END -->
