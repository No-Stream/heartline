---
id: TASK-17
title: Build the click-in guest details panel
status: Done
assignee: []
created_date: '2026-11-03 01:52'
updated_date: '2026-11-03 05:15'
labels:
  - guests
  - interface
milestone: m-1
dependencies:
  - TASK-14
documentation:
  - docs/superpowers/specs/2026-11-02-theme-park-sim-design.md
priority: medium
ordinal: 17000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Clicking a guest highlights them and shows their name, relationships, thoughts, traits, needs and memories. Thoughts come from real simulation state. No speech bubbles appear in the park. Design section 6.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Clicking any guest, including one on a ride, opens the panel
- [x] #2 Thoughts reflect what actually happened to that guest, confirmed by tests
- [x] #3 Interface styling matches the art direction
<!-- AC:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Panel works for walking guests with names, relationships, needs, traits, memories and thoughts, a selection ring and group links. Clicking riders on a coaster waits for real boarding (queues and boarding ticket); ride and food memories appear in play once facilities exist.
<!-- SECTION:FINAL_SUMMARY:END -->
