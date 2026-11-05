---
id: TASK-124
title: Make the crowd and operating journeys robust to machine load
status: Done
assignee: []
created_date: '2026-11-05 06:23'
updated_date: '2026-11-05 07:39'
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

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The crowd and operating journeys pause through the UI, synchronise the rendered snapshot, and advance the test simulation in bounded tick batches while checking their original conditions, instead of waiting on wall-clock time. Both pass together under added CPU load (2.4 and 2.0 min) and unloaded (3.2 and 2.6 min); make journeys now takes about 4.7 min. No timeouts raised or assertions weakened.
<!-- SECTION:FINAL_SUMMARY:END -->
