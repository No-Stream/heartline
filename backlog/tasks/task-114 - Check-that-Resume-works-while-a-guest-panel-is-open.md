---
id: TASK-114
title: Check that Resume works while a guest panel is open
status: Done
assignee: []
created_date: '2026-11-04 23:18'
updated_date: '2026-11-05 01:06'
labels: []
dependencies: []
priority: medium
ordinal: 114000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found 2026-11-04 while fixing the operate journey (TASK-97): the diagnosing agent reported that clicking the clock's Resume button after opening a guest panel stalled in two runs, so the journey now resumes with Space instead. Reproduce by pausing, clicking a guest, then clicking Resume; check whether the panel overlaps or captures the click, focus handling, or the pause state; fix the cause and add a browser test that resumes by clicking the button with a guest panel open.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Clicking Resume with a guest panel open resumes the game
- [x] #2 A browser test covers it
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Cause: the guest panel kept its old position after a resize and covered Resume at 1280 px; it now stays inside the canvas.
<!-- SECTION:NOTES:END -->
