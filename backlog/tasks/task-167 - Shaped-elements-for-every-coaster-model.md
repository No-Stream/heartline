---
id: TASK-167
title: Shaped elements for every coaster model
status: In Progress
assignee: []
created_date: '2026-11-21 16:05'
updated_date: '2026-11-22 04:52'
labels: []
dependencies: []
ordinal: 161000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Plan: docs/superpowers/plans/2026-11-21-global-limits-and-shaped-elements.md (Part B). Per-model ShapedProfile data; solver reads only the profile; rollout hyper (zero diff), Velocity Viper, Timber Tumbler, Daisy Dash, Briar Blitz; finish search for every profiled model. Supersedes the Daisy Dash choice in TASK-161.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Carry-forward from TASK-161: Daisy Dash still needs an all-model profile rollout that addresses its measured force-headroom failures, updates the ride file and digest/golden expectations, and verifies both physics configurations; the local attempts in TASK-161 did not find a viable layout. Carry-forward from the remaining staged scope in TASK-156: preserve turn-feel and vertical-transition controls, add named inversions with peak-force intent, and finish the remaining showcase rides. The current profile, matrix, parity, browser-agreement, and finish-search work should include these items explicitly.
<!-- SECTION:NOTES:END -->
