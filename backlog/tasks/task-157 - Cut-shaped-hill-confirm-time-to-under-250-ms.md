---
id: TASK-157
title: Cut shaped hill confirm time to under 250 ms
status: To Do
assignee: []
created_date: '2026-11-20 08:07'
labels:
  - shaped-elements
dependencies: []
ordinal: 154000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Cold Node/WASM bench on shaped-elements (2026-11-20): fully banked turns confirm in 25-54 ms p95, but shaped hills take 200-250 ms (4 cars), 230-485 ms (6 cars) and 320-804 ms (8 cars, worst at 34-36 m/s). The steps track the number of simulator previews in the secant (each about 100-200 ms for long trains on tall hills). Today's classic placement p95 is 164-191 ms. Options: a better analytic first guess (fit a per-model correction from the bench data) so most hills need one or two previews; a shorter preview run-out; showing a 'shaping' state while confirming. Browse latency (one preview) was not measured separately; measure it first.
<!-- SECTION:DESCRIPTION:END -->
