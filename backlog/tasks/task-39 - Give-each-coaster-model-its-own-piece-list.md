---
id: TASK-39
title: Give each coaster model its own piece list
status: In Progress
assignee: []
created_date: '2026-11-03 11:06'
labels:
  - coaster
milestone: m-2
dependencies: []
priority: medium
ordinal: 39000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Timber Tumbler and Velocity Viper derive their allowed pieces from the whole catalog, so they also offer each other's and the wooden twister's signature pieces. Each model should feel like its real counterpart.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Each model lists its allowed pieces explicitly, based on what its real inspirations build plus clearly plausible extensions
- [ ] #2 Tests check that signature pieces stay with their models (for example fan turns only on the twister, launches only on Velocity Viper)
- [ ] #3 Existing circuits in tests and the demo still build
<!-- AC:END -->
