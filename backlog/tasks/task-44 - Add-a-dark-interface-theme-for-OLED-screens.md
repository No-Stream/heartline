---
id: TASK-44
title: Add a dark interface theme for OLED screens
status: To Do
assignee: []
created_date: '2026-11-03 16:40'
updated_date: '2026-11-03 19:00'
labels:
  - interface
  - art
milestone: m-1
dependencies: []
priority: high
ordinal: 44000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Owner request (2026-11-03): make the UI darker; the owner and others use OLED displays. Panels, toolbar, page background and the void around the park are currently light paper.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A dark print-modern interface variant (deep ink background, light ink text, same key lines and accents) is the default or follows the system preference, with a toggle
- [x] #2 The area around the park and the sky are dark too, so no large bright areas remain
- [x] #3 Contrast meets readable levels in every panel, checked by eye in screenshots
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Commit b6db8bf. Dark tokens via light-dark() in print-modern.css; theme.ts toggle in the game bar (follows prefers-color-scheme until chosen, stored in localStorage). Renderer void/sky: BACKDROPS and printOnBackdrop in render/daylight.ts, Atmosphere.paper in render/index.ts; light path is numerically identical. Game bar breakpoints widened so Watch-mode items no longer overlap (overlap already existed at 900-1100px). construction-preview baseline regenerated: only the game bar changed. Tests: web/e2e/dark-theme.spec.ts (6), bar-overlap test in toolbar.spec.ts, unit tests in daylight/effects tests. Contrast checked by eye in dark screenshots of every panel.
<!-- SECTION:NOTES:END -->
