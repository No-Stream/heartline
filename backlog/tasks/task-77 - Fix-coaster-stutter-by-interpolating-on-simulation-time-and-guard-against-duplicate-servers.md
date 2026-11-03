---
id: TASK-77
title: >-
  Fix coaster stutter by interpolating on simulation time and guard against
  duplicate servers
status: To Do
assignee: []
created_date: '2026-11-03 20:05'
updated_date: '2026-11-03 20:05'
labels:
  - rendering
  - performance
dependencies: []
priority: high
ordinal: 77000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Stutter diagnosis 2026-11-03: the worker keeps 30 Hz (send interval median 29.7 ms), but main.ts interpolates from snapshot arrival time while each frame costs 35-40 ms of main-thread work (buildScene, appendGeometry, prepare re-encode about 2.6 MB of quads and draw twice for picking), so the train alternates between 0.67x and 1.33x speed. About 27% of worker wakeups run 0 ticks but still post a snapshot. Fix: interpolate on simulation time with a render clock advanced by real time and corrected toward the newest tick, buffering two snapshots; skip posting 0-tick snapshots in worker.ts; make serve --port 5173 --strictPort; Playwright reuseExistingServer false. Reducing per-frame scene rebuild cost is a separate follow-up (TASK-59).
<!-- SECTION:DESCRIPTION:END -->
