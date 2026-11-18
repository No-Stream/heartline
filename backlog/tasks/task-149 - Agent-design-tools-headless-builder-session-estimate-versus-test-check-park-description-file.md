---
id: TASK-149
title: >-
  Agent design tools: headless builder session, estimate-versus-test check, park
  description file
status: To Do
assignee: []
created_date: '2026-11-18 17:03'
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
