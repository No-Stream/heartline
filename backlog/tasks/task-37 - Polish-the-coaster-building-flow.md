---
id: TASK-37
title: Polish the coaster building flow
status: In Progress
assignee: []
created_date: '2026-11-03 06:15'
updated_date: '2026-11-21 21:24'
labels:
  - coaster
  - interface
milestone: m-1
dependencies:
  - TASK-10
priority: high
ordinal: 37000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Make building coasters feel delightful before and after the owner's first try: station height as a stepper, a styled panel in print-modern, clearer piece groups with small icons of each piece shape, keyboard-first flow, smooth camera follow of the open track end, and hover tooltips for refused pieces.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Station height and piece options use steppers instead of long dropdowns
- [x] #2 Panel styled to print-modern with piece icons drawn from the piece curves
- [x] #3 Keyboard shortcuts cover the whole build loop and are listed in the panel
- [x] #4 The camera follows the open track end while building
- [x] #5 Owner feedback from the first try is applied
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Owner tried it on 2026-11-03. Their feedback is filed as TASK-42 (level-out on Velocity Viper), TASK-43 (broader vocabularies) and TASK-45 (tools opening without a second click); the last criterion closes when those land.

2026-11-03: the remaining dependencies landed (TASK-42 level-out labels, TASK-45 tool activation, chain toggle, climbing turns and 60-degree slopes); close after the owner playtests the builder and TASK-70.

Backlog review 2026-11-21: criteria 1-5 are implemented on master; follow-up tickets for first-try feedback landed. Remaining: owner playtest/sign-off of the builder flow, which the existing notes require before closure; TASK-70 has its own playtest.
<!-- SECTION:NOTES:END -->
