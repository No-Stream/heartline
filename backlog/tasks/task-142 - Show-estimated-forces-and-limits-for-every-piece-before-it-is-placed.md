---
id: TASK-142
title: Show estimated forces and limits for every piece before it is placed
status: To Do
assignee: []
created_date: '2026-11-18 12:49'
labels:
  - coaster
  - interface
dependencies: []
priority: high
ordinal: 141000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Owner playtest 2026-11-18: the force estimate and limits only show after placing. Today construction.ts:631 shows the candidate force preview only for speed-matched elements and turns (and hills have their own readout). Show the predicted forces against the model's limits for whichever piece or element is selected, before placing, including drops, slopes and hills.
<!-- SECTION:DESCRIPTION:END -->

## Implementation notes

The worker already simulated the selected candidate on complete construction updates; the panel restricted its candidate readout to speed-matched pieces and turns. It now covers ordinary pieces as well; hills retain their existing hill inspector. Candidate samples are sent only for the selected preview and rendered with the existing limit-closeness colours. The simulation still uses its existing construction caches and does not simulate every available choice.

Validation: `make check`, `make test-web`, and `make lint` passed. The feature suite passed 186 of 190 tests; two marker-position baselines were updated and their focused reruns passed. The dark workshop screenshot and underground cutaway screenshot failed twice and remain open. The underground image includes the new candidate ribbon and a changed existing force annotation (5.8 g to 5.9 g), which needs investigation before updating its baseline. Native placement-update p95 before/after was 133.8/126.4 ms (Timber Tumbler), 122.0/102.6 ms (Velocity Viper), and 240.7/113.1 ms (Briar Blitz); run-to-run contention limits interpretation.
