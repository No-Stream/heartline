---
id: TASK-1
title: 'Set up Rust, WebAssembly and TypeScript project skeleton'
status: Done
assignee: []
created_date: '2026-11-03 01:51'
updated_date: '2026-11-03 02:05'
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
- [x] #1 A single command builds a static site that runs a ticking Rust simulation in a worker and draws its state
- [x] #2 Simulation crate also builds and tests natively
- [x] #3 Make targets exist for build, test, lint and a local server
- [x] #4 After the tab is hidden for several minutes, the game resumes smoothly with no burst of catch-up ticks
<!-- AC:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Walking skeleton in place. make ci passes from a fresh clone. The GitHub workflow has not run on GitHub yet because the repo has no remote.
<!-- SECTION:FINAL_SUMMARY:END -->
