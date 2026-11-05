---
id: TASK-111
title: Group inversions by heartlined and classic in the builder
status: Done
assignee: []
created_date: '2026-11-04 20:37'
updated_date: '2026-11-05 12:09'
labels: []
dependencies:
  - TASK-95
priority: medium
ordinal: 111000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Owner request 2026-11-04: heartlined (modern) and non-heartlined (classic, Arrow-style) inversions feel very different, so the builder should separate them. Today the distinction is only a '(not heartlined)' / '(heartlined)' suffix in piece names (crates/sim/src/track/catalog.rs). Add a real heartlined property to inversion pieces and elements, export it in the element report, show inversions in the Build tab's Track elements as two groups (for example 'Modern inversions (heartlined)' and 'Classic inversions (not heartlined)'), and drop the suffix from names. Starts after TASK-95 because that branch reworks the inversion pieces in catalog.rs.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Inversion pieces carry a heartlined property exported to the UI
- [ ] #2 Track elements shows heartlined and classic inversions as separate groups
- [ ] #3 Piece names no longer carry the suffix; browser tests updated
<!-- AC:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Inversion pieces and elements carry a nullable heartlining property exported through the piece and element reports; the Build tab shows Modern inversions (heartlined) and Classic inversions (not heartlined) as separate groups, and the name suffixes are gone. Replays and the player driver use wire ids, so they needed no changes. Gates on current master: lint, report types, npm test 374, full feature project 180/180, make journeys 9/9.
<!-- SECTION:FINAL_SUMMARY:END -->
