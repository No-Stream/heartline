---
id: TASK-37
title: Polish the coaster building flow
status: In Progress
assignee: []
created_date: '2026-11-03 06:15'
updated_date: '2026-11-03 07:32'
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
- [ ] #5 Owner feedback from the first try is applied
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Polish committed in 7549274. Waiting for owner feedback from their first try (last criterion). Found and dispatched: no right turns in the catalog, and banked turns leaning outward.
<!-- SECTION:NOTES:END -->
