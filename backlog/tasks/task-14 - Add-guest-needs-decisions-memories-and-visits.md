---
id: TASK-14
title: 'Add guest needs, decisions, memories and visits'
status: Done
assignee: []
created_date: '2026-11-03 01:52'
updated_date: '2026-11-03 04:07'
labels:
  - guests
  - simulation
milestone: m-1
dependencies:
  - TASK-13
documentation:
  - docs/superpowers/specs/2026-11-02-theme-park-sim-design.md
priority: high
ordinal: 14000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Guests choose using what they have seen, experienced and heard from their group. Visit length is measured in activity, spanning day-night cycles of 20 to 30 minutes. The park never closes. Guest numbers follow park appeal, with a sandbox override slider. Design section 6.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Guests have hunger, thirst, restroom and nausea needs that drive behavior
- [x] #2 A test shows choices depending on personal knowledge rather than a global ride list
- [x] #3 A typical visit contains a realistic number of rides and stops, and spans cycles if needed
- [x] #4 Arrivals follow appeal, and the slider overrides them
- [x] #5 Returning guests keep compact, bounded memories
<!-- AC:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Needs, bounded personal knowledge, utility-based group decisions with height-limit splits, memories and thoughts, continuous appeal-based arrivals with a sandbox override, returning visits. Tuned for about 12 rides and 3 stops per visit; provisional until a real park exists. Groups split by a path edit wait until paths reconnect.
<!-- SECTION:FINAL_SUMMARY:END -->
