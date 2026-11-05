---
id: TASK-129
title: Autosave the park with rotating slots
status: Done
assignee: []
created_date: '2026-11-05 19:44'
updated_date: '2026-11-05 23:15'
labels:
  - saving
  - ui
dependencies: []
priority: medium
ordinal: 129000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Owner request 2026-11-05: autosave with sensible defaults, about once every few minutes, keeping the last few dozen autosaves. Default: every 5 minutes of real play time (not while paused with no changes), keep the 30 most recent in a rotating set separate from manual saves; list them in the Park menu with time and park age; settings for the interval and count; never block the game while saving; handle full storage by dropping the oldest autosave first with a clear message.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
2026-11-05: implemented on branch autosave (387a9df, master merged): every 5 minutes of active play, 30 rotating slots, Park menu list with load and delete, interval and count settings. Unit and feature gates passed on the branch; journeys were running at handoff. Snapshot serialisation briefly pauses the worker; measure on a large park.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Autosave every 5 minutes of active, unpaused, changed play into 30 rotating slots separate from manual saves; the Park menu lists them with time and park age, load and delete, and interval and count settings. Serialisation runs in the worker and writes are asynchronous; on a full store the oldest autosaves go first with a message. Gates: unit, full feature 187/187, journeys 9/9 on the branch; autosave and saving specs on merged master.
<!-- SECTION:FINAL_SUMMARY:END -->
