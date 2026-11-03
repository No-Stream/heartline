---
id: TASK-32
title: Match guest sprites to their simulated appearance
status: Done
assignee: []
created_date: '2026-11-03 03:38'
updated_date: '2026-11-03 16:26'
labels:
  - art
  - guests
milestone: m-1
dependencies:
  - TASK-13
priority: medium
ordinal: 32000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Guest walking picks person sprites without knowing which sprite matches a guest's simulated age, skin tone, hair, clothes and body shape, because the atlas has no appearance metadata. Clicking a child should never show an adult. Design sections 6 and 9.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Person drawings carry appearance metadata in atlas.json (age group, skin, hair, clothes, body shape)
- [x] #2 The renderer picks a sprite consistent with each guest's traits, and children always draw as children
- [x] #3 A test checks the mapping for a generated population
<!-- AC:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
atlas.json carries appearance metadata per person; the renderer matches age group first, then skin tone, deterministically per guest. Spread is tested within each age group.
<!-- SECTION:FINAL_SUMMARY:END -->
