---
id: TASK-25
title: 'Add saving, loading and export'
status: Done
assignee: []
created_date: '2026-11-03 01:52'
updated_date: '2026-11-05 15:55'
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

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Parks save as versioned Rust binary snapshots in IndexedDB and export or import as .park files (eight-byte container header with format version; older or foreign files are refused with a clear message). A single Park menu in the game bar holds save, load, export and import with status messages. Round-trip tests restore identical state hashes for 30 further ticks with guests queued and riding, construction in progress, and a hill circuit. A 1,000-guest park saves at about 3.5 MB (about 294 KB gzipped), so saves are not compressed yet. Loading disables session export for that session. Gates on merged master: unit, full feature 184/184, journeys 9/9.
<!-- SECTION:FINAL_SUMMARY:END -->
