---
id: TASK-3
title: Benchmark PixiJS against a custom renderer on a real isometric scene
status: To Do
assignee: []
created_date: '2026-11-03 01:52'
labels:
  - rendering
  - spike
milestone: m-0
dependencies:
  - TASK-1
documentation:
  - docs/superpowers/specs/2026-11-02-theme-park-sim-design.md
priority: high
ordinal: 3000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Choose the renderer by measurement. PixiJS published numbers do not cover animated, depth-sorted isometric scenes. Include long overlapping track, which is the hard depth sorting case. Design section 10.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Both options draw a scene with 10,000 animated people, terrain and overlapping track
- [ ] #2 Frame times are recorded on WebGPU and WebGL2, and the result and choice are written to docs
- [ ] #3 Chosen approach draws track passing over and under other track and people with no visible depth errors
- [ ] #4 On a browser without WebGPU, the game runs on WebGL2 with no action needed
- [ ] #5 Current WebGPU support is rechecked, including Firefox on Linux
<!-- AC:END -->
