---
id: TASK-153
title: Builder says no connection to the station while the track already meets it
status: To Do
assignee: []
created_date: '2026-11-18 17:36'
labels:
  - coaster
  - interface
dependencies: []
priority: medium
ordinal: 150000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while building the new demo (TASK-140). Steps: in the default park, start a coaster, build a full circuit and place the last piece so that its end meets the station entry (here OneTileStraight ending at the station). The Build tab then says 'The track meets the station. Close the circuit with C', while the Finish coaster section under it says 'No nearby connection to the station was found.' and Test coaster stays unavailable. Pressing Close circuit then completes the circuit. The two messages contradict each other; the second one appears to come from the automatic connection search, which looks for a joining path rather than noticing the track is already joined. Expected: when the end meets the station, the Finish section says the same as the Build tab (press Close circuit) or closes automatically.
<!-- SECTION:DESCRIPTION:END -->
