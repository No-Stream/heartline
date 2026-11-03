---
id: TASK-61
title: Clean up interface CSS
status: To Do
assignee: []
created_date: '2026-11-03 16:47'
labels:
  - refactor
  - interface
milestone: m-4
dependencies: []
documentation:
  - docs/reviews/2026-11-03-browser-code.md
priority: low
ordinal: 61000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Browser review finding on CSS: 28 uses of !important, app-shell CSS inline in index.html, and a scenery panel that doesn't use the shared panel styles. Do this together with or after the dark theme ticket.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 App styles live in app.css, !important is gone where avoidable, every panel uses the shared styles, and changed screenshots are reviewed on their own
- [ ] #2 Behaviour is unchanged unless stated, and the relevant tests pass while editing, with make ci green at the end
<!-- AC:END -->
