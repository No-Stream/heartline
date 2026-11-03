---
id: TASK-11
title: Add coaster testing overlay and friendly notes
status: In Progress
assignee: []
created_date: '2026-11-03 01:52'
updated_date: '2026-11-03 05:46'
labels:
  - coaster
  - interface
milestone: m-1
dependencies:
  - TASK-10
documentation:
  - docs/superpowers/specs/2026-11-02-theme-park-sim-design.md
priority: medium
ordinal: 11000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Show g-forces, speed and airtime along the track. Problems appear as friendly notes and never as failures. Also add ride ratings based on the recorded forces. Design section 4.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Overlay shows forces and speed along track
- [ ] #2 A train that cannot finish, or an unfinished circuit, produces a friendly note
- [ ] #3 Ride rating responds to comfort, and uncomfortable plausible designs are never blocked
<!-- AC:END -->
