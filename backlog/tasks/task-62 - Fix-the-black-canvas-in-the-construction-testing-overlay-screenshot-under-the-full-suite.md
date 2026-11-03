---
id: TASK-62
title: >-
  Fix the black canvas in the construction testing-overlay screenshot under the
  full suite
status: To Do
assignee: []
created_date: '2026-11-03 19:49'
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
