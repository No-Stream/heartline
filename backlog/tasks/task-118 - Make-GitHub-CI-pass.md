---
id: TASK-118
title: Make GitHub CI pass
status: To Do
assignee: []
created_date: '2026-11-05 00:27'
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
