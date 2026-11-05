---
id: TASK-124
title: Make the crowd and operating journeys robust to machine load
status: To Do
assignee: []
created_date: '2026-11-05 06:23'
labels:
  - testing
dependencies: []
priority: high
ordinal: 124000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
guest-selection.journey.spec.ts and timber-tumbler-operate.journey.spec.ts fail in full make journeys runs when the machine is busy (zero guests queued for Timber Tumbler; a 180 s timeout) but pass when run alone (2026-11-04, on banked-straights and earlier on build-forces). They wait on wall-clock time for the simulation to fill a queue. Make them wait on simulation progress (ticks or a queue condition via gameTest) instead, without raising timeouts.
<!-- SECTION:DESCRIPTION:END -->
