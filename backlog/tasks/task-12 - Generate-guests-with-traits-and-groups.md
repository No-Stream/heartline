---
id: TASK-12
title: Generate guests with traits and groups
status: Done
assignee: []
created_date: '2026-11-03 01:52'
updated_date: '2026-11-03 02:38'
labels:
  - guests
  - simulation
milestone: m-1
dependencies:
  - TASK-2
documentation:
  - docs/superpowers/specs/2026-11-02-theme-park-sim-design.md
priority: high
ordinal: 12000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Statistical models generate taste for intensity, favorite ride types, theming care, spending, preferred themes and relationships. Storage is compact arrays with bounded memories. Design section 6.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Generated populations match intended distributions in tests
- [x] #2 Groups include singles, couples and families with children
- [x] #3 Memory for 10,000 guests is measured and within budget
<!-- AC:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Seeded correlated trait generation, groups with two-way relationships, stable IDs, and an 8-entry memory ring (136 bytes per guest). Executor generalized to any output type. Distribution table in the guests module docs.
<!-- SECTION:FINAL_SUMMARY:END -->
