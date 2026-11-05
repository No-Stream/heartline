---
id: TASK-126
title: Evaluate rider forces with ASTM-style filtering and duration limits
status: To Do
assignee: []
created_date: '2026-11-05 16:09'
updated_date: '2026-11-05 19:44'
labels:
  - physics
  - research
dependencies: []
priority: medium
ordinal: 126000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Research for TASK-125 (2026-11-05) suggests ASTM F2291 evaluates vertical acceleration after low-pass filtering (about 5 Hz) and treats events under about 200 ms separately (clause 7.1.4.2 per ASTM work-item pages), with brief negative spikes allowed further than sustained -1.5 g (commonly cited about -2.0 g for very short durations; exact curves need the paid standard). The game checks raw per-step forces. Decide with the owner whether to adopt filtered, duration-aware limits for rated windows, scoring and the building overlay; it would change many rated windows, so measure the effect first. Buy or consult F2291-26 Section 7 for exact limits.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
2026-11-05 owner: yes, smooth forces. Example: a 4.5 g moment in a loop is fine if brief, as on a Schwarzkopf. Use filtered, duration-aware limits for rated windows, scoring and the building overlay.
<!-- SECTION:NOTES:END -->
