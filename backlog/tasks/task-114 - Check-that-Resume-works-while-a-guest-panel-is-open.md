---
id: TASK-114
title: Check that Resume works while a guest panel is open
status: To Do
assignee: []
created_date: '2026-11-04 23:18'
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
- [ ] #1 Clicking Resume with a guest panel open resumes the game
- [ ] #2 A browser test covers it
<!-- AC:END -->
