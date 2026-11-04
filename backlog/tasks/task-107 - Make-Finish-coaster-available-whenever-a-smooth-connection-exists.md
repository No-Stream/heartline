---
id: TASK-107
title: Make Finish coaster available whenever a smooth connection exists
status: Done
assignee: []
created_date: '2026-11-04 17:00'
updated_date: '2026-11-04 17:54'
labels: []
dependencies: []
priority: high
ordinal: 107000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Owner playtest 2026-11-04: Finish coaster does not light up when it should. Diagnose and fix with a realistic corpus of near-station endings across models and rotations.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Corpus test passes for all cases where a smooth connection exists
- [x] #2 Availability updates after every edit
- [x] #3 Journeys pass
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Search limits: 80 m, up to five pieces, bounded beam; slowest corpus case 3.67 s native release. Not a guarantee every smooth connection is found.
<!-- SECTION:NOTES:END -->
