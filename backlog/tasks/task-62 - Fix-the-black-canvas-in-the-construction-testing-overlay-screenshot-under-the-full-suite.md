---
id: TASK-62
title: >-
  Fix the black canvas in the construction testing-overlay screenshot under the
  full suite
status: To Do
assignee: []
created_date: '2026-11-03 19:49'
updated_date: '2026-11-04 04:33'
labels:
  - test
  - rendering
dependencies: []
priority: high
ordinal: 62000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
make ci at c3247c8 (2026-11-03) failed one test: construction.spec.ts 'builds a closed coaster and runs a train around it' took the coaster-testing-overlay screenshot of an entirely black canvas (87% of pixels differ). The same test passes alone in 47 s. Suspects: WebGL context loss under software rendering with four workers, a screenshot taken between frames without a drawn frame, or a render exception after testing starts. Root-cause under load (repeat the spec with parallel workers, log webglcontextlost and page errors) and fix; do not just raise timeouts.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The cause is identified with evidence
- [ ] #2 The test passes in three consecutive full make ci runs
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
2026-11-03: The original black canvas cause remains unconfirmed. Test mode requests preserveDrawingBuffer; ready is set after render, but previously could become true with a lost WebGL context. The renderer had no context-loss handling. A forced WEBGL_lose_context regression failed before the fix and passes after GPU resources are recreated on restoration and readiness is invalidated until redraw.

Four-worker, three-repeat load run: 15 passed, 6 failed; construction stopped at a preview screenshot mismatch in all three repetitions and wooden twister stopped at a missing choice with stale generated WASM. A fresh-WASM diagnostic captured the two preceding construction screenshots rather than asserting them: all three reached the overlay with a rendered canvas, no context-loss/restoration events or page errors, and a 135-pixel marker mismatch. All three wooden tests stopped with chain enabled and Place piece disabled on a descent. Console errors were favicon.ico 404s. Logs and artifacts: /tmp/task62-repro/run.log and /tmp/task62-repro/overlay.log. These diagnostics do not establish the original cause or a passing repeated spec.

Focused render-persistence browser tests passed 2/2; TypeScript typecheck, web production build to /tmp, and 199 web unit tests passed. Full make ci was explicitly not run. Reconcile the concurrent chain-lift changes and screenshot differences, rerun the unchanged repeated slow tests under load with lifecycle instrumentation, and investigate any reproduced black canvas before closing this ticket.

2026-11-03 night: clean full make ci runs at f631a95 and b8dfac1 (two of the three needed; both under the new resource budget).
<!-- SECTION:NOTES:END -->
