---
id: TASK-45
title: Open each tool's main action as soon as the tool is selected
status: Done
assignee: []
created_date: '2026-11-03 16:40'
updated_date: '2026-11-03 18:01'
labels:
  - interface
milestone: m-1
dependencies: []
priority: high
ordinal: 45000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Owner report: for tools like paths and landscaping, the button has to be clicked again after opening the tool before it works, which feels janky.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Selecting Paths, Landscape, Scenery or Facilities immediately activates its default mode (build paths, raise tiles, place scenery, place facility) with the ghost preview showing
- [x] #2 Switching tools still ends the previous tool's mode cleanly
- [x] #3 Browser tests cover selecting each tool and using it without an extra click
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Toolbar selection now starts each tool through the same activation handler as its panel action. Targeted Playwright coverage confirms map use without an extra click and verifies old modes are cancelled on switching. npm run e2e -- web/e2e/toolbar.spec.ts web/e2e/path-panel-layout.spec.ts --workers=1 passed.
<!-- SECTION:NOTES:END -->
