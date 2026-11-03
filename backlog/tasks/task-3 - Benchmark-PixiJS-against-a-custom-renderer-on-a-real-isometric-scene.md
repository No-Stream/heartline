---
id: TASK-3
title: Benchmark PixiJS against a custom renderer on a real isometric scene
status: Done
assignee: []
created_date: '2026-11-03 01:52'
updated_date: '2026-11-03 02:56'
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
- [x] #1 Both options draw a scene with 10,000 animated people, terrain and overlapping track
- [x] #2 Frame times are recorded on WebGPU and WebGL2, and the result and choice are written to docs
- [x] #3 Chosen approach draws track passing over and under other track and people with no visible depth errors
- [x] #4 On a browser without WebGPU, the game runs on WebGL2 with no action needed
- [x] #5 Current WebGPU support is rechecked, including Firefox on Linux
<!-- AC:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
On the owner's RTX 5090, custom WebGL2 took 2.6 ms per frame for 10,000 people against about 45 ms for PixiJS. Chose the custom renderer, WebGL2 path only for now. Results in docs/renderer-benchmark.md.
<!-- SECTION:FINAL_SUMMARY:END -->
