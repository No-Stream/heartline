---
id: TASK-149
title: >-
  Agent design tools: headless builder session, estimate-versus-test check, park
  description file
status: In Progress
assignee: []
created_date: '2026-11-18 17:03'
updated_date: '2026-11-18 23:21'
updated_date: '2026-11-19 00:17'
labels:
  - tooling
dependencies: []
priority: high
ordinal: 148000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Owner request 2026-11-18: tools that let agents design and QA coasters and parks properly and cheaply, using the same builder core as the game. Three layers: a native headless builder session with the builder panel's actions and estimates; an agreement check that replays any saved ride through the real UI and compares on-screen estimate, headless estimate and measured forces; a park description file and park report, with one realistic showcase coaster per model. Plan: docs/superpowers/plans/2026-11-18-agent-design-tools.md (being written). Related: TASK-146, TASK-109, TASK-140.
<!-- SECTION:DESCRIPTION:END -->

## Work item 1 results (2026-11-18)

The ride-file schema, typed construction estimates, native save/load error helpers, and builder session are implemented on `design-tools`. The session exposes start, estimates, select, place, chain, launch speed, undo, delete, cars per train, train count, colours, price, finish, close, test, automatic blocks and ride-file replay. Replay does not insert automatic blocks. Native invalid-load testing confirmed that the wasm `JsValue` error path aborts off wasm; the native helpers return string errors.

Acceptance is incomplete:

- Ride-file parse and round-trip, strict unknown fields, model and step diagnostics, and deterministic replay pass. The existing animated versus headless test report comparison passes.
- Undo after place does not restore `state_hash`: before 51132564572711369, after 14619315595286438851. `Construction::compute_state_hash` includes the monotonic `next_id`, which undo does not restore. The track is restored. Resolve the plan's hash requirement against the piece-identity invariant before changing core behavior; the exact assertion remains enabled.
- The Timber Tumbler benchmark comparison fails: expected digest 13268569719681531691, actual 13244395020988907760. The first remaining differing call is 565, `full: choices_with_preview`; the printed prefixes match. Two fix attempts were exhausted, so investigate the complete differing values and worker/session cache sequencing before changing the test. The comparison stops at Timber, so the other two circuits are not verified.
- `make check-report-types` passes (1 test).

Verification: `make build` passed; the targeted ride-file suite passed 3 tests, the existing report parity test passed 1, and native train-count/launch refusal tests passed 2. `make test-rust` failed: all 672 sim library tests passed (3 filtered), the integration suites reached before wasm passed, and the wasm suite had 162 passed and the two acceptance failures above. `make lint` failed on new Clippy issues; after correcting them, both its workspace and threaded Clippy commands passed separately, as did TypeScript typechecking. `make test-web` passed 456 application tests and 9 art tests. Full `make check` was not run because its recipe starts Playwright, contrary to the owner's no-browser instruction; `make build`, the web tests and typechecking covered its non-browser components except its particular Rust subset (the broader Rust gate above ran instead).

## Follow-up: item 1 fixes and item 2 in progress (2026-11-18)

The three-circuit digest test now passes unchanged (1 test, three circuits). The first divergence was a one-f64-step difference in Timber Tumbler's choice entry speed: the worker and benchmark JSON-parse the force preview before passing its last speed to choices, while the session previously passed the in-memory value. The session now JSON-round-trips that speed. The undo test passes (1 test): it compares the full serialized construction and exact track with only `next_id` excluded, and asserts that the ID counter remains advanced. Plan sections 10 and 12 now describe that invariant.

Item 2 remains incomplete and uncommitted. Five ride files and the temporary circuits.txt equivalence test are present; that test passed (1 test). A two-car per-piece peak accessor regression passed (1 test). The CLI, report, agreement evaluator and export acceptance test are in progress. The first session replay test run had 466 passes and one failure because the wrapper did not yet exist; session export has not been verified. The package gate remains red: after two attempts to fix ownership of the map output text, compilation still fails with E0505 in `crates/design-tools/src/commands.rs`, where `footprint` borrows `text` before the reply moves it. Stopped as required by the two-attempt rule. A subsequent session should make the footprint own its strings or serialize it before moving text, then complete and verify the remaining item 2 acceptance criteria.

The requested final gates (`make test-rust`, `make lint`, `make test-web`, `make check`) were not run in this follow-up because the stop condition was reached. No release replay timing, save-load acceptance result, golden-text results, or successful exported-session parse is available. Park build and park report are scheduled as later work items 8 and 9. No stale test was removed or disabled; the undo assertion was updated as directed.

CLI review identified two additional unverified risks for the follow-up: undoing a bridge group that contains multiple pieces may not remove the corresponding ride steps correctly, and exported session launch-speed indices may be wrong after an element step expands into multiple pieces. Verify these against the bridge and cover them with tests before treating the CLI as complete.

## Work item 2 follow-up (2026-11-18)

The ride-file format now has an additive `autoBlocks` step, applied in order. A multiple-train count is restored after all steps, when automatic block analysis has established capacity; a single train uses the bridge default. The exported session records the actual block-analysis advances and physical piece IDs, including after expanded elements. The CLI preserves grouped undo, targets launch speeds by physical placement, and reports palette-matched estimates with force ratings. Native save export/load is covered by comparing the loaded state hash, track, and train layout with a ride replay.

The previous placeholder goldens were replaced with the actual choices, estimate, place and state output; estimate now includes the force ratings required by the plan. The report golden uses the original short unpowered circuit and records the simulated stop at 56.4 m. The finish test was stale: finish search successfully adds four pieces on that fixture, and the assertion now checks that result. The samples heading assertion now matches the front-car, one-metre output. No tests were disabled.

A release `state` on 70 half-tile straights measured 4.13 s cold including snapshot creation and 0.17 s with a matching snapshot, with identical output. The cold replay exceeds the 4 s target. Snapshot identity uses exact ride bytes, the executable fingerprint and version; a corrupt matching snapshot fails loudly. The snapshot preserves the builder speed cache and session-export metadata. The first attempted timing used 70 longer straights and ran outside the world, so it is not a valid measurement.

Verification boundary: `make test-rust` was interrupted after more than 15 minutes in the enabled `close_and_auto_blocks_accept_a_completed_open_ride` full-search test. Before interruption, 6 library tests and 12 of 13 command coverage tests passed. `make lint` failed on three Clippy findings; they were fixed, and targeted design-tools Clippy and workspace formatting checks passed afterward. `make test-web` was interrupted after more than 10 minutes in the synchronous session export using Timber Tumbler and full automatic-block search, before Vitest reported counts. `make check` built successfully, passed generated report types (1 test) and its Rust subset (228 tests), then was interrupted after about 15 minutes in the same Vitest export; typechecking subsequently passed standalone. The real-worker browser replay test has not run.

The remaining blocker is the cost of exhaustive automatic-block placement and throughput analysis. The 16-piece block fixture also exceeded four minutes in a focused debug run; the full gate exceeded 15 minutes. Existing block tests apply the best result early, but this plan explicitly requires waiting for completion, so no test or search was shortened. Investigate and measure a correctness-preserving search improvement, or agree on a changed automatic-block contract, then run the four gates and verify the real-worker replay. Cold 70-piece replay also remains above its 4 s target despite the warm snapshot meeting it.

The exported-session fixture `session_export_replays_element_expansion_before_assigning_launch_speed` used station coordinates `(241, 249)`, which placed the final track endpoint outside the 512-by-512 world when session export queried its ground height. The fixture now starts at `(100, 100)`; the focused export test passes. The old slow `autoBlocks` replay assertions were replaced with explicit `blockSections` command assertions.

## Explicit block results follow-up (2026-11-18)

The CLI spends at most 1,000 block-analysis work steps and saves the installed brake-run identities, train capacity and completion flags. The focused close, search, save and fresh replay test passed in 1.98 s in debug. Replay installs the recorded result without search; a Rust test compares block section details and state hashes, and the real-worker browser replay passed (1 test). A boundary alone can identify multiple brake runs, so each saved section also records its brake start and piece indices. Invalid recorded layouts fail without changing the park. The old automatic-search replay and recorded-advance tests were replaced; the existing block correctness tests remain enabled.

The 70-half-tile release measurement was 4.29 s cold including snapshot write and 0.17 s warm, with identical output. The cold target is under 4 s and remains unmet. Repeated support-render updates during append are a candidate for a separately verified optimization. The command coverage tests, export tests, save/load test and goldens passed in the Rust gate; `check` on Timber Tumbler passed expectations and reported native agreement failure as informational. Park build and report remain scheduled for work items 8 and 9.

The single requested `make test-rust` run passed 948 tests across 22 suites; `make test-web` passed 468 web and 9 art tests. `make check` passed build, generated types (1 test), its Rust subset (228 tests), 468 web tests and 9 art tests, then failed on a missing browser-test recording type. After fixing it, standalone typechecking and the real-worker test passed; the gate was not rerun. `make lint` failed on one collapsible conditional; after fixing it, targeted Clippy for the affected packages and formatting passed, as did standalone typechecking. Its full workspace and threaded Clippy commands were not rerun. No timeouts or retries were added.

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
2026-11-18: plan items 1-2 merged to master (a4aeac4, pushed): ride files, headless builder session, design command-line tool, explicit block layouts. Items 3 (native agreement table), 4 (browser agreement spec), 5 (one code path for worker and tool) and 8 (park description loader) started in parallel in worktrees dt-agreement, dt-browser-agreement, dt-one-path and dt-park.
## Native agreement table work (2026-11-18)

The native report now prints placement, finished-track, measured front-car and per-axis worst-car force peaks and entry speeds, with individual check 2 and check 3 failure reasons. JSON includes the same values. Check 2 uses the finished preview versus the front car; check 3 uses the placement preview versus the envelope across cars. The station is excluded because it is not a placed piece. Expanded elements are split into physical pieces using the candidate's samples. Piece ranges are half-open except at the final endpoint; finished-preview entry speed is interpolated at the piece boundary without extrapolation. Missing data is reported as a failed comparison. Agreement failures remain informational and `check` says that they exit zero until work item 6.

The full catalog tables and their failure details are in TASK-146 and plan section 9. Release `tools/design check content/rides/*.ride.json` took 33.02 s and exited zero: expectations passed on all rides, agreement passed on Velocity Viper and failed on the other four. All 227 pieces passed check 2; 17 failed check 3. The separate demo-ride comparison remains pending its branch. The three synthetic tests existed at the starting commit; further axis and piece-boundary coverage and catalog integration were added without disabling them.

The requested short straight-and-hill end-to-end fixture remains failing. Two straight-and-hill prefixes could not be closed by finish search (`No nearby connection to the station was found`); a direct four-turn loop with an AirtimeHill substituted for a straight also failed to close (`The track must connect back to its station.`). The test remains enabled in `crates/design-tools/tests/rides.rs`. Stop after the two failed attempts on the finish-search approach; a follow-up should construct a verified closed short ride with a hill from the geometry and then rerun the gates. The existing catalog integration and Timber agreement command exercise the full native path, but do not satisfy the short-fixture requirement.

Verification: `make build` passed. The requested workspace Rust command ran every binary and failed only `design-tools/tests/rides.rs` (2 passed, 1 failed, none ignored); the failing test is the short fixture described above. The other binaries passed, including 680 simulation library tests and 169 wasm library tests. `make lint` passed. `make check` passed generated types (1), its Rust subset (228), web tests (468), art tests (9), and fast browser tests (43). `make test-web` passed 468 web and 9 art tests. No existing test was disabled or deleted. The incomplete work remains uncommitted; the workspace Rust gate must pass before it is ready to land.

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
2026-11-18 lead, using tools/design to build the short agreement fixture (Launch 14 m/s, AirtimeHill, two small turns, finish, close; agreement passes on all 9 pieces). Friction for the agent guide (item 10): every call prints dozens of ts-rs 'failed to parse serde attribute' build warnings before the reply; there is no 'help' ride command (only the plan lists commands); 'finish' brings the track back to the station but a separate 'close' is still needed, and the state after finish says 'No nearby connection to the station was found' while 'can test: true' (same family as TASK-153).
<!-- SECTION:NOTES:END -->
