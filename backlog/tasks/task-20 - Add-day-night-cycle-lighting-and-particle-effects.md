---
id: TASK-20
title: 'Add day-night cycle, lighting and particle effects'
status: In Progress
assignee: []
created_date: '2026-11-03 01:52'
updated_date: '2026-11-03 12:45'
labels:
  - rendering
  - art
milestone: m-2
dependencies:
  - TASK-6
documentation:
  - docs/superpowers/specs/2026-11-02-theme-park-sim-design.md
priority: medium
ordinal: 20000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The owner specifically enjoyed these in the mockup. A cycle takes 20 to 30 minutes, with a moving sun, lamps and ride lights, fireworks and crest confetti. Dense parks must stay readable. Design section 9.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Cycle runs with sun, lamps, ride lights and fireworks, at target frame rate on both WebGPU and WebGL2
- [x] #2 Night is clearly readable, and fireworks look like fireworks
- [ ] #3 Owner has reviewed the effects in a dense park
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Committed in dcf7727: 24-minute day from simulation ticks, print-style colour grade, chasing track lights, lit flat rides and windows, fireworks, crest confetti, sky. CPU cost about +1.6 ms by day and +2.4 ms at night in software rendering. The game places no lamp posts yet, so lamp glow is unused. Waiting for the owner's review of effects in a dense park.
<!-- SECTION:NOTES:END -->
