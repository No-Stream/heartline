---
id: TASK-143
title: 'Show track heights along the built track, at least where height changes'
status: Done
assignee: []
created_date: '2026-11-18 12:49'
updated_date: '2026-11-18 19:22'
labels:
  - coaster
  - interface
dependencies: []
priority: high
ordinal: 142000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Owner playtest 2026-11-18: hard to tell how much height you have. Height markers today only show the station and the open end (web/src/height-markers.ts). Label heights above ground at crests and valleys (and the open end) while building, in the player's units, sparse enough to stay readable.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
The builder identifies track-elevation crests and valleys with at least 3 m of change on each side and labels their height above the local terrain in the selected units. Labels are omitted when they cannot be placed without overlapping another height label. The open-end and station readouts remain available.

Validation: the height Vitest cases and the crest Playwright scenario passed. `make check`, `make test-web`, and `make lint` passed. Two feature screenshots were updated for a one-pixel shift of the combined station/end marker and passed focused verification. Two other feature screenshots remain unresolved; see TASK-142.

2026-11-18: merged to master (cfedbfb) with TASK-142; master gates green.
<!-- SECTION:NOTES:END -->
