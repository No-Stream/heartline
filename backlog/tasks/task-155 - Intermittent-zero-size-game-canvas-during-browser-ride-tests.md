---
id: TASK-155
title: Intermittent zero-size game canvas during browser ride tests
status: To Do
assignee: []
created_date: '2026-11-19 05:31'
labels:
  - bug
  - testing
dependencies: []
ordinal: 152000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
On master 66d3f93 the full gate's browser ride tests (make ride-agreement) failed twice in one run: Horizon Hawk and Timber Tumbler each caught the page error 'Canvas dimensions must be positive' (thrown by web/src/render/index.ts when the canvas backing store is 0 wide or high). fitCanvas in web/src/main.ts sizes the canvas from its layout box through a ResizeObserver, so the game view's layout box was briefly zero. Every estimate and report comparison passed in those tests. Not reproduced in 15 later runs on the same commit (Horizon Hawk 5 alone, Timber Tumbler 4 alone, the full ride project once in order). The failing run happened while the machine was heavily loaded by other agents' Rust and browser gates. The ride test now records the error's stack trace. Next time it appears: read the stack, check the page screenshot for a collapsed game view (a player would see a blank view), and decide whether layout can really give the canvas no area.
<!-- SECTION:DESCRIPTION:END -->
