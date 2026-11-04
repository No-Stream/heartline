---
id: TASK-65
title: Add the Timber Tumbler player journey
status: In Progress
assignee: []
created_date: '2026-11-03 20:05'
updated_date: '2026-11-04 07:17'
labels:
  - test
dependencies: []
priority: high
ordinal: 65000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Plan: docs/superpowers/plans/2026-11-03-flexible-track-and-gameplay-tests.md, work item 3. Full gameplay loop with real input; fails today at the curved-lift step.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
2026-11-03: the journey runs through path, food, drink, restroom, model choice, station and chain climb, then fails as expected (test.fail) with 'no climbing turn offered while the chain lift climbs' (29 s). Still to write once TASK-67/68/69 land: the rest of the circuit, testing report, prices, opening, riders paying, guest memory and confetti; and the circuit in circuits.txt with chain directives the Rust reader understands.

2026-11-03: the journey now turns the chain on with H (asserted via data-chain) and climbs with plain climb pieces. Fragility to fix: in live mode the journey waits for gameTest.ready, which flips while the game runs; one run timed out waiting for it at startup and the next two passed in 30 s. Prefer waiting on a monotonic readiness signal such as the first rendered tick.

2026-11-03 later: the startup readiness timeout has a likely cause, fixed in 0356dd8: parkBounds threw on a new park before its first facilities message and killed the render loop. Watch whether journey startup is now reliable before changing the readiness wait.

2026-11-03: Incomplete. Scripted the full Timber Tumbler loop, including a shared chain-directive circuit, UI ride test and report checks, station access, prices, queue, opening, revenue, rider selection, memory and confetti assertions. The station is 4 m above ground; one extra chain climb and one return descent make the circuit close. Focused Rust closure test and TypeScript typecheck pass; a separate scratch physics test completed a lap using the game physics configuration. Live diagnostics reached a completed report with ratings and no rollback, opened the ride, observed a queued guest board, and verified coaster income and park revenue increased. Rider selection remains red: near-position scans failed twice at 5.2 min; rendered-bounds scanning timed out after 3000 ticks at 6.9 min; recentering then failed after 5.5 min because the selected rider had exited. No product bug is established, so no proposed bug ticket or expected-failure annotation was added. The 180-second test limit is restored; no complete run or three consecutive runs under three minutes were achieved. Follow-up: instrument rendered rider bounds, pick results, and clicked IDs across frames while following a queued rider; determine whether occlusion or input timing prevents selection, then validate guest memory and crest confetti. Profile construction, terrain targeting, and tick waits to reduce total live runtime, and run the finished journey three times under three minutes.

2026-11-03: Incomplete. Added phase timings and shortened the shared circuit from 45 to 41 pieces; the Rust player circuit closure test passes, and a scratch real-physics lap completed without recovery. The browser constructed and tested the shortened circuit, but the 180 s run timed out while setting the entrance price. Its phases were setup 6.3 s, construction 67.0 s, ride test 17.6 s, terrain 27.8 s, and access paths 52.0 s. The original measured setup 12.2 s, construction 123.6 s, and ride test 18.5 s before timing out during terrain. Diagnostic runs with a temporary 360 s limit reached boarding but rider selection remains unverified: a sampled rider pick became a train click as the train moved, and subsequent bounded camera rotation attempts failed to select the rider. No game bug is established. The 180 s limit is restored. Guest panel, selected guest ride memory, and post-opening crest confetti are not verified, and there are no passing runs under three minutes. Next investigate selection by recording rider pose, rendered bounds, pick result, input event coordinates, and tick/frame at the actual click; arrange selection during stationary boarding. Reduce access path work and construction/render cost before repeating three consecutive 180 s runs. TypeScript typecheck and make build passed; no make ci was run.

2026-11-03 lead: split the journey. Committed the build journey (park setup, a 41-piece circuit with curved chain lift, 60-degree section, crest, drop, banked turns and airtime hill, close, ride test with ratings and no stall): three consecutive passes at 1.6 min. The operate phase (terrain for access, queue and exit paths, entrance and ride prices, opening, queue and boarding, rider selection, ride memory, crest confetti after opening) is saved in docs/handoffs/2026-11-03-timber-tumbler-operate-phase.ts.txt. Next: make it a second journey that starts from a replayed session of the finished build (web/e2e/sessions) instead of rebuilding, and solve rider selection (the click lands on the moving train; consider whether players can click riders on moving trains at all, which may be a real usability gap).

2026-11-03 night: Codex job task-mw1ib6nc-duiu3w (gpt-6.1-terra, medium) is building the operate journey from a recorded build session; brief in the lead's scratchpad (copy of the plan: test hook to replay a session into the live worker, web/e2e/sessions/timber-tumbler-built.json, a second @journey spec, riders selected while queued or boarding, never on a moving train). Owner question pending: should players be able to click riders on moving trains?

2026-11-03 night: owner decided players do not need to click riders on moving trains; select riders while they queue or board.

2026-11-03 night: committed 8aabc55, the operate journey from a recorded build (2.5 min of 3; replay loop shared in web/src/session-replay-worker.ts). Codex job task-mw1k7m7v-7rmuci is making the recorded build reproducible (RECORD_JOURNEY_SESSIONS=1 on the build journey) and cutting the operate journey to about 2 min. Close TASK-65 when that lands with three runs under 2 min and a clean full gate.

2026-11-03 night: done. Operate journey about 2.1 min (b8dfac1); built session regenerates with RECORD_JOURNEY_SESSIONS=1; full make ci green at b8dfac1.

2026-11-03 night: the operate journey timed out in a full gate that ran while two Codex jobs compiled on the same 8 cores (selecting a queued guest took too long); it passes alone. It has the least margin of the journeys.

2026-11-03 night: reopened. The operate journey timed out in three full gates run alongside other agents' builds (phases about 1.7x slower under load). Codex job task-mw1saiyz-j09xty in worktree theme-park-sim-worktrees/journey-speed is profiling replay setup (29 s for 7,408 ticks in frozen mode) and terrain flattening (23 s), target about 75 s per run, no limit increase.
<!-- SECTION:NOTES:END -->
