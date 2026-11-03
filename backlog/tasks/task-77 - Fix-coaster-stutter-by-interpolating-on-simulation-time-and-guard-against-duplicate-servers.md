---
id: TASK-77
title: >-
  Fix coaster stutter by interpolating on simulation time and guard against
  duplicate servers
status: Done
assignee: []
created_date: '2026-11-03 20:05'
updated_date: '2026-11-03 20:22'
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

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented a buffered 30 Hz simulation-time render clock with a three-tick delay and bounded 2% rate correction; duplicate or older live snapshots are ignored. Live zero-tick wakeups no longer send snapshots; frozen mode and the initial snapshot remain intact. Added strict development port and disabled Playwright server reuse. Render-clock Vitest: 5 passed; typecheck and make build passed; focused frozen game smoke passed. Scratch measurement: before 119/442 zero-tick sends, after 0/315; the scratch script reconstructs the old interpolation, so its movement figures do not measure the new renderer. Software WebGL was heavily loaded, with median frame intervals 367 ms before and 350 ms after. No acceptance criteria were defined to tick.
<!-- SECTION:NOTES:END -->
