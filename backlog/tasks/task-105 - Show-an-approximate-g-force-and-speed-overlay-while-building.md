---
id: TASK-105
title: Show an approximate g-force and speed overlay while building
status: In Progress
assignee: []
created_date: '2026-11-04 17:00'
updated_date: '2026-11-05 04:27'
labels: []
dependencies:
  - TASK-95
priority: high
ordinal: 105000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Owner request 2026-11-04. While building a coaster, show approximate speed and g-forces along the track built so far (colour along the track and a readout for the next piece), simulated from the station or lift with the current train, even before the circuit is closed. Updates as pieces are placed. Approximate is fine; it should make building more fun and help avoid rollbacks and painful forces.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Speed and vertical/lateral g shown along the unfinished track while building
- [ ] #2 Updates after each placement fast enough to feel live
- [ ] #3 Browser test checks the overlay appears and changes
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
2026-11-04 owner: wants force overlays while building, like the existing test-run track overlay, e.g. to see 'this camelback is pulling -2.5 g, that won't work'. Approximate is fine. Reuse the existing overlay metrics (speed, vertical, lateral, longitudinal, airtime) on the unfinished track, and flag forces past the model's comfortable limits.
<!-- SECTION:NOTES:END -->
