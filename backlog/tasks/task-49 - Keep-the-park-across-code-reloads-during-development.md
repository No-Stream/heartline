---
id: TASK-49
title: Keep the park across code reloads during development
status: To Do
assignee: []
created_date: '2026-11-03 16:40'
labels:
  - infra
milestone: m-1
dependencies:
  - TASK-25
priority: low
ordinal: 49000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Owner note: the park resets whenever the dev server reloads after code edits. Expected for now; once saving exists, autosave and restore on reload would avoid losing work while testing.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 With saving in place, the dev build autosaves and restores the park after a reload
<!-- AC:END -->
