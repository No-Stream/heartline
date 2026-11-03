---
id: TASK-55
title: Give coasters and block sections typed state
status: To Do
assignee: []
created_date: '2026-11-03 16:47'
labels:
  - refactor
  - coaster
  - simulation
milestone: m-4
dependencies: []
documentation:
  - docs/reviews/2026-11-03-track-physics-code.md
priority: medium
ordinal: 55000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Simulation review step 4 and track review findings 2, 3 and 6: about 25 loose coaster and block fields kept consistent by hand, three booleans encoding which block search is pending, test outcomes detected by matching message text, block search control flow matching error strings, and Train::new checking user input with assertions.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A Coaster type with an Editing, Testing or Operating mode and BlockAnalysis in track::blocks; block search returns typed outcomes; blocks are split into candidates, search and validation; circuit and train construction return typed errors
- [ ] #2 Behaviour is unchanged unless stated, and the relevant tests pass while editing, with make ci green at the end
<!-- AC:END -->
