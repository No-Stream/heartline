---
id: TASK-32
title: Match guest sprites to their simulated appearance
status: In Progress
assignee: []
created_date: '2026-11-03 03:38'
updated_date: '2026-11-03 05:15'
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
- [ ] #1 Person drawings carry appearance metadata in atlas.json (age group, skin, hair, clothes, body shape)
- [ ] #2 The renderer picks a sprite consistent with each guest's traits, and children always draw as children
- [ ] #3 A test checks the mapping for a generated population
<!-- AC:END -->
