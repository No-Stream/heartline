---
id: TASK-120
title: Accept keyboard building input while the construction editor is busy
status: Done
assignee: []
created_date: '2026-11-05 04:22'
updated_date: '2026-11-05 10:16'
labels:
  - construction
  - input
dependencies: []
priority: medium
ordinal: 120000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Journeys on the force-limited inversions branch showed that keyboard shortcuts (h for chain lift, b to place) are silently ignored while the construction editor is busy validating, so a fast player loses keypresses. The journeys now wait for aria-busy=false. Queue or apply the input once the editor is ready instead of dropping it.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
While the construction editor is busy, keyboard and button actions are queued (up to 32), shown as queued, and applied in order once it is ready; actions that depend on a refused placement, or on an editor that closed, are cancelled. Choosing the already-selected piece stays a no-op. The aria-busy waits were removed from the Velocity Viper journey and the player driver. The Timber Tumbler journey now checks the completed deletion state (fresh estimate, overlay enabled) instead of a transient mid-update state. Gates: lint, npm test 373, full feature project (only the known block-sections:15 failure), make journeys 9/9.
<!-- SECTION:FINAL_SUMMARY:END -->
