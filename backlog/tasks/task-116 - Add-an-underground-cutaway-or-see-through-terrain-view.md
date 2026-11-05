---
id: TASK-116
title: Add an underground cutaway or see-through terrain view
status: Done
assignee: []
created_date: '2026-11-04 23:33'
updated_date: '2026-11-05 17:58'
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

2026-11-05 owner: cutaway view on by default when relevant, plus a toggle.

2026-11-05: follow-up merged: the coaster builder has an Underground cutaway Auto/Off/On control (remembered); in Auto the cutaway window appears while building track, paths or carves below ground and disappears above ground; Cutaway window is the default style; U and the Landscaping control still work.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Landscaping's underground section gains a Show setting: Whole view cut (default, unchanged) or Cutaway window, a 150 px window drawn cut away (following the track end while building coasters, cut 3 m above it, or the pointer at the cut-level slider in other tools), with buried coaster track shown outside it as a translucent silhouette (slate-blue by day, pale gold at night). Picking inside the window hits the cut world. About 18% slower frames while on; the surface view is unchanged. Alternatives tried: ghosted terrain, x-ray silhouette, cutaway window. Open owner questions: make the cutaway the default while building underground, and put the style switch in the coaster builder.
<!-- SECTION:FINAL_SUMMARY:END -->
