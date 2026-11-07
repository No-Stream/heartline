---
id: TASK-26
title: Check full-scale performance and decide on browser threads
status: To Do
assignee: []
created_date: '2026-11-03 01:52'
updated_date: '2026-11-07 01:18'
labels:
  - performance
milestone: m-2
dependencies:
  - TASK-13
  - TASK-19
  - TASK-20
  - TASK-25
documentation:
  - docs/superpowers/specs/2026-11-02-theme-park-sim-design.md
priority: high
ordinal: 26000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Measure a realistic full park against the targets, and use the evidence to decide on threads in the browser. If needed, this covers nightly Rust, COOP and COEP headers and hosting on Cloudflare Pages or Netlify. Design section 10.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A realistic park with 10,000 guests on a 512 by 512 map is measured on the owner's desktop GPU and a mid-range laptop
- [ ] #2 Targets are met, or the shortfall and the fix are reported to the owner
- [ ] #3 The decision on browser threads is written down with its evidence
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
2026-11-06: the renderer now draws world quads as indexed triangles (four vertices per quad, drawElements) instead of instanced draws (branch ci-journeys 2cb41fb), because software WebGL (SwiftShader, used by CI and by players without a GPU) runs each instance as its own draw: frames in the CI container went from 333 to 167 ms. Check on real hardware that the larger vertex upload does not cost frame time at 10,000 guests (the earlier instanced measurement was 2.6 ms GPU per frame on the RTX 5090).
<!-- SECTION:NOTES:END -->
