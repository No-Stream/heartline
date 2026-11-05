---
id: TASK-116
title: Add an underground cutaway or see-through terrain view
status: In Progress
assignee: []
created_date: '2026-11-04 23:33'
updated_date: '2026-11-05 11:58'
labels: []
dependencies: []
priority: medium
ordinal: 116000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Owner request 2026-11-04: make it easy to see what you are doing underground. Today's underground view clips everything above a chosen level. Add a cutaway or transparent terrain mode (for example terrain drawn see-through or cut away around the cursor and the track being built) so tunnels, caverns and track below ground are visible in context with the surface. Taste-heavy; design with screenshots before implementing.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A cutaway or see-through view shows underground track in context
- [x] #2 Works while building coaster track and paths underground
- [x] #3 Browser test and screenshots
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Underground view has a style select in the Landscaping panel: Whole view cut (the old behaviour, default) or Cutaway window. The window is a 150 CSS px circle drawn cut at a level: around the end of the coaster being built (cut 3 m above the track end) or around the pointer at the chosen cut level in other tools. Coaster track buried outside the window is traced through the ground as a flat silhouette. Prototypes compared: ghosted terrain over buried track, x-ray silhouettes, and a cutaway window; the shipped view combines the window with the silhouettes. Spec: web/e2e/underground-cutaway.spec.ts.
<!-- SECTION:NOTES:END -->
