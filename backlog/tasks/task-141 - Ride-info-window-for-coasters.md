---
id: TASK-141
title: Ride info window for coasters
status: In Progress
assignee: []
created_date: '2026-11-18 12:33'
labels: []
dependencies: []
ordinal: 141000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Clicking a coaster in Watch mode (or Ride info in the builder) opens a window with its model, excitement, intensity and nausea, grouped statistics, score findings and a g-force graph over ride time. Reports live in the simulation (demo_report, construction_report), so they survive save and load and clear when the track changes. Branch ride-window.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Clicking the demo coaster or the player coaster opens the window; untested coasters offer a Test button
- [ ] #2 The graph shows vertical, lateral and forward g with the model limit bands and speed, and hovering highlights the matching track point
- [ ] #3 Docker screenshot baseline for the window exists (needs the pinned Playwright image)
<!-- AC:END -->
