---
id: TASK-83
title: 'Let a banked track go straight into a drop, including twisted and barrel drops'
status: Done
assignee: []
created_date: '2026-11-04 00:28'
updated_date: '2026-11-04 10:57'
labels:
  - coaster
dependencies: []
priority: medium
ordinal: 83000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Owner request 2026-11-03: allow leaving a bank directly into a drop (banked slope transitions), and add RMC-style signature drops: a twisted drop (the track rolls while pitching down) and a barrel-roll drop. Needs pieces whose entry is banked and whose exit is sloped, with continuous curvature and roll; fits TASK-43's signature elements, so coordinate with it.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
2026-11-04: Codex job task-mw1z6m4f-34wppt in worktree theme-park-sim-worktrees/track-batch (with the other two of TASK-80, TASK-81, TASK-83).

2026-11-04: merged 5fad6b4. Lap tests use frictionless physics; forces sampled at 25 m/s (60-degree transitions) and 20 m/s (banked drops).
<!-- SECTION:NOTES:END -->
