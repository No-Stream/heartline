---
id: TASK-93
title: Stop reporting a coaster's own supports as near misses at every piece
status: Done
assignee: []
created_date: '2026-11-04 08:49'
updated_date: '2026-11-04 09:44'
labels:
  - coaster
  - bug
dependencies: []
priority: high
ordinal: 93000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Seen 2026-11-04 in the construction spec's testing panel (already in older baselines): the testing report lists 28 'Near-miss with the coaster's supports' events, one at every piece from piece 1, all 'very close'. A coaster's own supports sit directly under its track by design, so the proximity check most likely counts the supports holding up the piece itself (or its neighbours). Real near misses (a support of one section close to another section of track) should still be reported. Probably in crates/sim/src/physics/testing.rs.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
2026-11-04: merged 3dee388 (28 false support near misses down to 8 real ones in the construction spec circuit).
<!-- SECTION:NOTES:END -->
