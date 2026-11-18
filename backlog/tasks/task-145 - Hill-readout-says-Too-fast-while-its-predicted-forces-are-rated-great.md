---
id: TASK-145
title: Hill readout says Too fast while its predicted forces are rated great
status: To Do
assignee: []
created_date: '2026-11-18 13:05'
updated_date: '2026-11-18 16:45'
labels:
  - coaster
  - interface
dependencies: []
priority: high
ordinal: 144000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Owner playtest 2026-11-18: a Floater 10 x 105 ft hill at 42 mph entry shows Rated for 20-33 mph with a TOO FAST tag, while the same readout predicts -1.1 g (great airtime) and 3.5 g (great). The tag contradicts the forces. Find what the rated window encodes beyond the readout (full-train worst car, intended feel band, design margins, duration rules) and make the readout either show the limiting value or describe the actual consequence (e.g. 'feels like an ejector', 'back car exceeds ...') instead of a bare Too fast.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
2026-11-18: likely cause is the floater cap in the rated-window generator (force-rule branch, crates/sim/examples/measure_force_windows.rs): a floater's maximum entry speed ends where any allowed default train rates stronger than Good airtime, so -1.1 g reads Great on the airtime scale but exceeds the floater feel, and the readout only says Too fast. Start after force-rule merges.
<!-- SECTION:NOTES:END -->
