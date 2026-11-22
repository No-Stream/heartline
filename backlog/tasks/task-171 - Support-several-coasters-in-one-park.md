---
id: TASK-171
title: Support several coasters in one park
status: To Do
assignee: []
created_date: '2026-11-22 15:55'
labels: []
dependencies: []
priority: high
ordinal: 165000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The simulation, WASM layer, park description loader (crates/sim-wasm/src/park_description.rs rejects a second coaster: 'this build supports one player coaster per park'), saves, ride window, guest boarding, design tools and the starter park all assume one player coaster. Owner 2026-11-22: fix this so a park can hold any number of coasters, each with its own trains, test runs, queue, stats and guests riding it. Prerequisite for the richer starter park (TASK-109).
<!-- SECTION:DESCRIPTION:END -->
