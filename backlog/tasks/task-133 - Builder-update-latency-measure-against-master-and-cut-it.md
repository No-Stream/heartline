---
id: TASK-133
title: 'Builder update latency: measure against master and cut it'
status: To Do
assignee: []
created_date: '2026-11-06 02:56'
labels: []
dependencies: []
priority: medium
ordinal: 133000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found during TASK-126: placing a piece takes a worker update p95 of 250-280 ms in the browser on the three journey circuits (append call 90-110 ms, force previews 17-55 ms of it; native append p95 79-117 ms without previews). Measure the same on master to see whether it regressed, profile where construction_append spends its time (validation, support solving, completion search), and bring the browser update under about 100 ms p95. Owner wants delightful UX; the builder should feel instant.
<!-- SECTION:DESCRIPTION:END -->
