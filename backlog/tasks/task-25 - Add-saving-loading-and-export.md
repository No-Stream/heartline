---
id: TASK-25
title: 'Add saving, loading and export'
status: To Do
assignee: []
created_date: '2026-11-03 01:52'
labels:
  - infra
milestone: m-2
dependencies:
  - TASK-15
documentation:
  - docs/superpowers/specs/2026-11-02-theme-park-sim-design.md
priority: medium
ordinal: 25000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Parks save in browser storage and can be exported to a file, with no server. Design section 10.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A round-trip test restores identical state, including guests with memories
- [ ] #2 Export and import work with files
- [ ] #3 Full or cleared storage, and saves from older versions, produce clear messages without losing the exported file
<!-- AC:END -->
