---
id: TASK-41
title: Triage slow tests into fast and slow tiers
status: To Do
assignee: []
created_date: '2026-11-03 16:26'
updated_date: '2026-11-03 19:04'
labels:
  - infra
  - performance
milestone: m-2
dependencies: []
documentation:
  - docs/reviews/2026-11-03-rendering-art-tests.md
priority: high
ordinal: 41000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The full gate takes about 7 minutes: Playwright about 4 minutes with 4 workers (slowest: wooden twister 49 s, daylight confetti 35 s, guest appearances 29 s, path rebuild 22 s), Rust suites about 50 s each and wasm about 46 s. Agents running it in parallel starve the machine and cause timeouts. Inputs: the rendering, art and tests review in docs/reviews/.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A fast tier (make check) runs in under 90 seconds and covers each feature at least once
- [x] #2 Slow end-to-end flows run in a separate tier (make ci still runs everything) and none waits on wall-clock time
- [x] #3 Each slow test's purpose is kept, with cheaper fixtures or checks moved to unit tests where possible
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
The rendering, art and tests review has a slow-test triage section per test, plus shared fixture setup, atlas reuse and deterministic waits; the browser review proposes a shared e2e driver and circuits module.

Fast check passed in 71.34 s and 71.02 s with warm artifacts; an intermediate concurrent run failed when the shared preview server disappeared, so check now defaults to a separate port. The full default sim suite passed in 51.78 s; the threaded configuration repeats it and adds six tests, and both configurations remain in the full gate. Confetti, guest appearances and automatic block sections passed focused browser runs; both art screenshot pipelines passed. The construction preview screenshot differs after the concurrent dark-theme control was added; its baseline is unchanged and requires reconciliation by that work. Full CI was not run per instruction, and slow-flow preservation remains unchecked pending the full gate.

Added the eight art Vitest tests to make check (standalone run passed in 0.99 s). A subsequent check stopped after 33.66 s at the concurrent Park/WebAssembly refactor with E0502/E0503 borrow errors before the tests; the updated fast tier needs a passing timed run once that refactor compiles. The earlier 71.34 s and 71.02 s measurements predate the added art tests.

Added a focused ParkGuests test for a removed and rebuilt path followed by a food-queue visit; omitting the rebuild made it fail and restoring it passed. The retained full browser path-rebuild scenario passed in 23.2 s. Art screenshots now wait for the actual rendered camera rotation and zoom; both screenshot pipelines and typechecking passed. A follow-up WebAssembly check still failed in the concurrent Park refactor, now with missing path/terrain error helpers and CommandError conversions, so the expanded make check remains unmeasured end to end.

Expanded make check passed three times at 103.42 s, 107.17 s, and 82.45 s; each run rebuilt Rust while the shared Park refactor was changing, and the first two exceeded the 90 s target. Each passed 142 Rust tests, 184 application Vitest tests, eight art tests, typecheck, and eight Playwright tests. The wooden twister browser build and lap passed in 50.6 s against the existing browser build. Criterion 1 remains unchecked because the measured time is not consistently below 90 s.

Slow-flow audit found the original assertions retained; existing Rust and TypeScript tests cover wooden-twister geometry/laps, launch physics, slopes and appearance selection, and the new small Rust fixture covers rebuilt access followed by a food visit. Focused full browser checks passed for wooden twister (50.6 s), construction (48.0 s, with the concurrently updated baseline), path rebuild and food visit (23.2 s), confetti, guest appearances and automatic block sections. The entire slow suite and make ci were not run per instruction.

The construction browser flow passed in 48.0 s with the current concurrently updated baseline. The full block-operation physics test passed in 12.71 s alone; it remains in make test/ci, while make check now excludes that long case and still runs other block physics tests. The first measurement of this revised selection stopped after 28.24 s at compilation errors in the concurrent track refactor (unresolved TrackState, catalog and related names); its wall time remains unverified.

The focused multi-launch browser flow passed in 14.2 s against the existing browser build. The revised fast Rust selection lists 141 tests from the previously built binary; the excluded long block-operation test remains in the full suite. Two targeted WebAssembly checks still failed in the concurrent Park refactor on facility_intersects_track being test-only and a missing SupportSearchExhausted variant, so no new end-to-end check timing is available.
<!-- SECTION:NOTES:END -->
