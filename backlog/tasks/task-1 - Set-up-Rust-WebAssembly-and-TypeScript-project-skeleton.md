---
id: TASK-1
title: 'Set up Rust, WebAssembly and TypeScript project skeleton'
status: To Do
assignee: []
created_date: '2026-11-03 01:51'
labels:
  - infra
milestone: m-0
dependencies: []
documentation:
  - docs/superpowers/specs/2026-11-02-theme-park-sim-design.md
priority: high
ordinal: 1000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Install Rust and build a walking skeleton: a Rust simulation in a web worker sends state to a TypeScript page that draws it. Design section 10.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A single command builds a static site that runs a ticking Rust simulation in a worker and draws its state
- [ ] #2 Simulation crate also builds and tests natively
- [ ] #3 Make targets exist for build, test, lint and a local server
- [ ] #4 After the tab is hidden for several minutes, the game resumes smoothly with no burst of catch-up ticks
<!-- AC:END -->
