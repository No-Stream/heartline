---
id: TASK-28
title: Add sound
status: In Progress
assignee: []
created_date: '2026-11-03 01:52'
updated_date: '2026-11-04 20:30'
labels:
  - audio
milestone: m-3
dependencies:
  - TASK-16
documentation:
  - docs/superpowers/specs/2026-11-02-theme-park-sim-design.md
priority: high
ordinal: 28000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Owner decision 2026-11-04: generate sounds from scratch in the browser (Web Audio) first; free-licensed recordings only to augment where generated sound falls short. Proposed set: lift chain clacks, wheel and rail noise following train speed, crowd murmur scaled by nearby guests, screams on drops and inversions triggered by rider forces, short build feedback sounds, ambient park sounds by time of day. Needs a design plan before implementation.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Owner has agreed the audio direction
- [ ] #2 Rides, crowds and effects have sound, and the player can adjust or mute it
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Owner asked for sound in the 2026-11-03 playtest ('sound please').

Owner approved all four plan decisions on 2026-11-04 (fast-forward silences one-off sounds at x8/x16; master volume and mute only; hybrid track mixes steel roar with half the wooden rumble; silent in automated tests). Slightly imperfect sound at high game speeds is fine. Plan: docs/superpowers/plans/2026-11-04-generated-sound.md. Starts after TASK-95 because the first work item adds train forces to snapshots.

2026-11-04: plan work items 2 and 3 merged (master volume and mute in the game bar, key M; audio engine core with offline test harness). Tool names in the game bar now hide below 1120 px instead of 1020 px to make room for the mute button; the volume slider hides below 1180 px. Remaining items start after TASK-95.
<!-- SECTION:NOTES:END -->
