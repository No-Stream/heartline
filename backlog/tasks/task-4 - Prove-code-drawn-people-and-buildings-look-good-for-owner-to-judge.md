---
id: TASK-4
title: 'Prove code-drawn people and buildings look good, for owner to judge'
status: Done
assignee: []
created_date: '2026-11-03 01:52'
updated_date: '2026-11-03 16:26'
labels:
  - art
  - spike
milestone: m-0
dependencies:
  - TASK-1
documentation:
  - docs/superpowers/specs/2026-11-02-theme-park-sim-design.md
priority: high
ordinal: 4000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Owner is confident vector art can look good. Build the pipeline from vector drawings to cached sprite textures and draw a first set in the art direction of docs/mockups/art-playful-v2.html. Guest and coaster work continue alongside this. Design section 9.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Pipeline turns vector drawings into sprite atlases, and replacing a drawing needs no code changes
- [x] #2 Walking people in at least 8 varied looks, adults and children, in four rotations
- [x] #3 At least 4 buildings or stalls and a set of scenery items in four rotations
- [x] #4 Owner has reviewed a demo page and approved the look or requested changes
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Art proof committed in 0bb52df. Waiting for the owner's review of the look (last criterion).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Owner approved the quality, asked for something more artsy and less childlike, and chose print-modern (a blend of riso and mid-century) leaning slightly toward mid-century. It is now the default theme.
<!-- SECTION:FINAL_SUMMARY:END -->
