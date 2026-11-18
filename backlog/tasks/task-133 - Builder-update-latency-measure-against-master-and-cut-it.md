---
id: TASK-133
title: 'Builder update latency: measure against master and cut it'
status: In Progress
assignee: []
created_date: '2026-11-06 02:56'
updated_date: '2026-11-18 19:22'
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

2026-11-18 (branch builder-speed-2): recorded a new baseline at 096b192. Native placement median/p95 in milliseconds: Timber Tumbler 120.6/140.7, Velocity Viper 102.4/129.9, Briar Blitz 101.6/132.7. Chrome full placement state median/p95: 175.7/288.9, 129.6/217.5, 195.9/289.1; selection median/p95: 57.1/143.0, 42.0/108.8, 54.7/143.6. The native output digests were b8236ddc331f3f2b, b0a3412bab1f1a5c, and 4325ef4ea41ca5aa, respectively.

Removed the redundant full state at finish-search start, avoided copying construction caches merely to inspect slope-search nodes, and limited clearance path checks to the tiles overlapping each envelope. Tests cover finish-search update ordering, cached construction copies, and path-query boundaries and ordering. The three native digests still match the baseline. After removing the redundant start state, Chrome full placement median/p95 was 169.7/217.2, 151.2/214.2, 154.2/217.6; selection was 53.2/81.6, 51.7/76.7, 56.0/97.0. Later path-query timings encountered substantial machine contention, so they cannot establish a comparable speedup. The force-rule rewrite was not on master at the time of these measurements.
2026-11-18 (branch builder-speed-2, support sharing): construction choice validations now retain one path-and-scenery obstacle snapshot per cached world identity and revision, and cached support vectors are shared between candidate validations. The native digest baseline is unchanged: Timber Tumbler b8236ddc331f3f2b, Velocity Viper b0a3412bab1f1a5c, Briar Blitz 4325ef4ea41ca5aa. In one native timing run, placement median/p95 was Timber Tumbler 109.9/135.8 ms, Velocity Viper 93.6/118.5 ms, and Briar Blitz 88.6/119.8 ms, compared with the recorded 120.6/140.7, 102.4/129.9, and 101.6/132.7 ms baseline. In one Chrome run, full placement-state median/p95 was 157.9/191.3, 138.4/173.6, and 125.1/163.7 ms, compared with the latest recorded 169.7/217.2, 151.2/214.2, and 154.2/217.6 ms. First-state median/p95 was 56.5/67.4, 50.3/70.2, and 39.3/53.7 ms; selection median/p95 was 52.1/84.1, 38.5/72.8, and 49.7/76.7 ms. These single runs were collected on a shared machine, so contention may affect the timings.

The focused construction cache tests passed 6/6. `make check` passed, including 228 simulation tests, 1 generated-types test, 445 web tests, 9 art tests, typecheck, and 38 fast browser tests. `make test-rust` passed 673 simulation unit tests, 148 WASM unit tests, and 71 integration tests, with 3 tests filtered by the Make target. `make test-web` passed 445 web tests and 9 art tests. `make lint` passed formatting, both Clippy configurations, and typecheck. `make e2e-feature` passed 189/190 tests; its one failure was the dark workshop screenshot, where the received image differs only in the responsive game-bar fit state after the fixed-width tick-counter CSS change. After reviewing the images, that one baseline was refreshed, and the focused screenshot test passed. `make journeys` passed 11/11 tests.

2026-11-18: builder-speed-2 merged (fdc0870). Chrome placement p95 now 164-191 ms on the three circuits, still above the 100 ms target; native digests unchanged. Master gates green.
<!-- SECTION:NOTES:END -->
