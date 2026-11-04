---
id: TASK-92
title: Add a Finish coaster option that connects the track end to the station
status: In Progress
assignee: []
created_date: '2026-11-04 05:12'
updated_date: '2026-11-04 05:24'
labels:
  - coaster
  - construction
dependencies: []
priority: high
ordinal: 92000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Owner request 2026-11-03 night: a construction option that builds the smoothest connection from the current track end to the station when one exists within a short distance; it need not handle ends far away or on the opposite side. Depends on TASK-86 (short pieces and S-bends) for fine corrections. Approach to consider: a bounded search over the model's pieces from the track end to the station entry, scored for smoothness (fewest and gentlest transitions, comfortable lateral and vertical forces), previewed before building.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Codex job task-mw1o97za-acqx9q, worktree theme-park-sim-worktrees/finish-coaster.
<!-- SECTION:NOTES:END -->
