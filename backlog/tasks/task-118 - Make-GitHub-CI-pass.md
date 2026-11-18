---
id: TASK-118
title: Make GitHub CI pass
status: To Do
assignee: []
created_date: '2026-11-05 00:27'
updated_date: '2026-11-07 01:33'
labels: []
dependencies: []
priority: high
ordinal: 118000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The repo went public as No-Stream/heartline on 2026-11-04. The first CI run (Actions run 41936488470) failed: lint (formatting and Clippy from merged branches; fixed locally in e96260e, not yet pushed) and e2e (12 browser tests on GitHub's runner: timeouts in construction, Immelmann, path panel layout, pause, audio, the operate journey and coaster boarding; screenshot mismatches in dark theme, guest panel and construction; the toolbar fit sweep). The test job passed. The suite was tuned on the owner's machine, so GitHub's slower 4-vCPU software-rendering runner exposes timing assumptions and platform-specific screenshot baselines. Diagnose each failure from the uploaded artifacts: decide per test whether it is a real bug (fix it), a timing assumption (wait on observable state instead of fixed timeouts), or a rendering difference (separate CI baselines generated on the runner, or a tolerance justified per test). Keep the local gate unchanged in strictness.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 CI is green on master on GitHub
- [ ] #2 Each failure explained and fixed at its cause
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
2026-11-05: postcard replaced bincode (RUSTSEC-2025-0141; save version 2, version 1 refused), CI split into smaller jobs, two browser tests use deterministic waits; merged and pushed as 6595b23. GitHub run 42119910401: audit, lint, threaded and flexibility tests, e2e build and smoothness passed; test and the feature and journey e2e jobs were still running at handoff.

2026-11-05: run 42121748977 passed every job except e2e (feature), which hit the 20-minute job limit and was cancelled; the feature project is now sharded into three CI jobs.

2026-11-05: sharded run 42128358630: all jobs green except the feature shards (16 failures: small screenshot differences and runner timeouts). Branch ci-feature investigating (Docker parity likely).

2026-11-06: merged into master (ci-journeys): browser tests run in the pinned Playwright image with npm ci, journeys sharded in the container, world quads drawn as indexed triangles (Docker frames 333 -> 167 ms). Branch gates: Docker journeys 9/9, local journeys 9/9, feature 187/188 before a stale draw-count test fix. Not yet confirmed on GitHub: check the first run after this push.
<!-- SECTION:NOTES:END -->
