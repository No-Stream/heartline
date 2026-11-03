---
id: TASK-61
title: Clean up interface CSS
status: To Do
assignee: []
created_date: '2026-11-03 16:47'
updated_date: '2026-11-03 18:28'
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
- [x] #1 App styles live in app.css, !important is gone where avoidable, every panel uses the shared styles, and changed screenshots are reviewed on their own
- [ ] #2 Behaviour is unchanged unless stated, and the relevant tests pass while editing, with make ci green at the end
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Moved app-shell CSS to web/src/app.css and imported it from main.ts. Removed avoidable !important declarations by increasing selector specificity; retained only reduced-motion overrides with comments explaining that they enforce the user motion preference. Scenery uses shared tool-panel controls. Centralized CSS color literals, including the guest panel palette, in print-modern.css tokens; removed TypeScript assignments of raw guest palette values. Verification: make build passed; npm run typecheck passed; 94 requested Playwright tests passed with no screenshot diffs; npm run lint is not defined. make ci was not run per owner instruction, so AC #2 remains unchecked.

Final verification after moving transparent to the shared token: make build passed, npm run typecheck passed, and all 94 requested Playwright tests passed in 10.3 minutes with no screenshot diffs.
<!-- SECTION:NOTES:END -->
