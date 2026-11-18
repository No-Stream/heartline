---
id: TASK-133
title: 'Builder update latency: measure against master and cut it'
status: In Progress
assignee: []
created_date: '2026-11-06 02:56'
updated_date: '2026-11-07 01:33'
labels: []
dependencies: []
priority: medium
ordinal: 133000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found during TASK-126: placing a piece takes a worker update p95 of 250-280 ms in the browser on the three journey circuits (append call 90-110 ms, force previews 17-55 ms of it; native append p95 79-117 ms without previews). Measure the same on master to see whether it regressed, profile where construction_append spends its time (validation, support solving, completion search), and bring the browser update under about 100 ms p95. Owner wants delightful UX; the builder should feel instant.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
2026-11-06: Timber Tumbler's build journey spends about 94 s of its 180 s limit on construction when run alone, and timed out under load on the fast-turns branch; the builder latency is now close to making that journey load-sensitive.

2026-11-06 (branch builder-speed): profiled with crates/sim-wasm/examples/construction_update_bench.rs (make bench-builder-updates), which replays the worker's calls for every placement on the three journey circuits. Before, a Timber Tumbler placement update took p95 556 ms natively: construction_elements p95 266 ms, construction_append p95 216 ms, construction_select p95 208 ms, the two force previews about 45 ms each. Causes, largest first: (1) the wasm builder cloned the park world before every placement, selection and station, and each clone has a new identity, so the validation, choice and element caches missed and supports were rerouted for the whole track on every placement; (2) Construction::state_hash hashed every piece's arc table on each cache lookup; (3) once any track or candidate tunnelled, every validation cloned and carved the world twice and rerouted supports for every placed piece; (4) element choices revalidated shared prefixes and replanned tunnels; (5) tunnel_plan ran the oriented envelope test before the cheap terrain lookup; (6) tunnel plan and clearance each sampled the same envelopes. All fixed with outputs identical: --digest hashes every call's output on the three circuits and matches the branch point a0e9dcf. Fixing (1) exposed a real bug in the incremental choices path (a newly selected hill variant went to the end of the list); fixed with a regression test. After: placement update p95 Timber Tumbler 129 ms, Velocity Viper 105 ms, Briar Blitz 111 ms natively, of which the two force previews are about 30-40 ms (rewritten on force-rule). Remaining: append about 30-40 ms (validating about 70 piece choices for the new end), element choices about 35 ms on a placement (38 validations). Not measured here but worth a look for big parks: check_clearance_indexed and route_supports scan every path in the world for each sample or call.

2026-11-06 browser (make bench-builder-updates-browser: web/construction-bench/placements.mjs drives the real game worker in Chrome, one exact selection and one placement per piece; baseline is the branch point a0e9dcf on the same harness). Full construction state after a placement, p95: Timber Tumbler 445 -> 261 ms, Velocity Viper 344 -> 197 ms, Briar Blitz 394 -> 253 ms (medians 345 -> 158, 238 -> 127, 283 -> 188). First state after a placement (the partial update that shows the new piece), p95: 240 -> 154, 175 -> 110, 220 -> 159 ms. Selection p95: 234 -> 125, 182 -> 92, 159 -> 137 ms. Wasm in Chrome runs about twice the native time here, so the 100 ms target is not met yet. What remains per placement, natively: two force previews about 30-40 ms (force-rule rewrites these), element choices about 35 ms, validating the piece choices for the new end about 30 ms. Next ideas: send the full state's element choices after the rest of the state (protocol change), share piece samples across one validation's support and clearance passes, and keep the finish search and block analysis chunks short so they do not delay the next action.

2026-11-06: first pass merged (builder-speed): native placement p95 556 -> 129 ms, browser 445 -> 261 ms, outputs identical by digest. Remaining: the two force previews (smaller once force-rule merges), element choices about 35 ms, and scheduleFinishSearch in web/src/worker.ts sending two extra full states per placement. Target browser p95 under 100 ms.
<!-- SECTION:NOTES:END -->
