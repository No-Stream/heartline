# Rendering, art and tests review

Written by a Codex reviewer on 2026-11-03 (read-only; saved from its report). Top opportunities: a shared browser-fixture setup, clearer renderer pass boundaries, smaller art-generator modules, a fast test tier, and a shared projection and depth function.

## Summary

The code has useful feature-local Vitest coverage and focused rendering fixtures, but common work is expensive to navigate and validate. Browser specs repeatedly build and serve fixtures, the renderer relies on implicit state and ordering, and the people generator combines several distinct jobs in one large function. The reported test durations are useful triage inputs; I did not rerun or profile the suite.

Sizes below are rough implementation estimates, including focused verification.

## Findings, ranked by day-to-day impact

1. **Share browser fixture setup and stop rebuilding the atlas inside a test.** [track.spec.ts:13](/web/e2e/track.spec.ts:13) and [renderer-depth.spec.ts:14](/web/e2e/renderer-depth.spec.ts:14) repeat temporary directory, Vite build, preview server, and cleanup code; several other specs do the same. [world.spec.ts:79](/web/e2e/world.spec.ts:79) additionally rasterizes a fresh atlas for one guest screenshot. This spreads lifecycle code across specs and adds setup cost. Introduce a small typed fixture-server helper, then build or serve the guest fixture using the already-generated atlas. Keep one separate atlas pipeline integration check. **Medium: 1–2 days.**

2. **Make renderer stages and their state requirements explicit.** [render/index.ts:234](/web/src/render/index.ts:234) through the draw functions changes framebuffer, viewport, blending, depth testing, and depth writes across mutually dependent helpers. [render/index.ts:297](/web/src/render/index.ts:297) also accepts positional callbacks whose geometry is clipped and appended to the pickable scene, while atmospheric quads enter different stages. The long callback assembly in [main.ts:576](/web/src/main.ts:576) makes that distinction difficult to see. Use a named render-input object with explicit world geometry and atmosphere layers; give each draw stage a clear state setup and separate shader/resource setup from orchestration. Preserve depth, clipping, and picking behavior with the existing browser fixtures. **Large: 2–4 days, independently landable in slices.**

3. **Split the people and building generators by responsibility.** [people.ts:337](/web/art/generators/people.ts:337) begins a roughly 400-line function combining pose, body geometry, clothing, facial features, accessories, and composition; the same file also contains cast data and appearance classification. [buildings.ts:85](/web/art/generators/buildings.ts:85) contains five individual designs alongside shared construction helpers. Extract cast/appearance data, pose and body construction, and face/accessory rendering; split building designs into modules with shared primitives. Preserve generated SVG output and visual checks. **Large: 2–4 days.**

4. **Give developers a fast test tier while retaining the full gate.** [Makefile:21](/Makefile:21) runs both Rust configurations and all Vitest suites; [Makefile:36](/Makefile:36) runs all browser tests. [playwright.config.ts:6](/playwright.config.ts:6) has one four-worker suite. Add a `check` target with representative Rust, TypeScript, and browser smoke coverage, and explicitly tag or select the longer integration and visual cases. Keep `make ci` and the required CI gate comprehensive; measure the new tier before promising a time budget. Avoid increasing workers without measurement: the config documents startup contention. **Medium: 1–2 days.**

5. **Centralize world projection and depth calculation.** The same rotation-dependent depth expression appears in [track/geometry.ts:84](/web/src/render/track/geometry.ts:84), [track/geometry.ts:205](/web/src/render/track/geometry.ts:205), [path-ghost.ts:39](/web/src/render/path-ghost.ts:39), and [effects.ts:34](/web/src/render/effects.ts:34). Changes to coordinate or depth conventions must currently be kept in sync by hand. Extract a small shared projected-vertex function with explicit height units and optional depth bias; retain rotation, picking, and clipping tests. **Small–medium: half a day to a day.**

6. **Protect atlas output and clarify the hand-authored art contract.** [build-atlas.ts:132](/web/art/pipeline/build-atlas.ts:132) accepts an output directory and recursively removes it at line 140. A mistaken argument can therefore delete unrelated files. Restrict cleanup to pipeline-owned files or validate an explicitly designated output directory, while preserving temporary outputs used by tests. Also, the header says drawings can be added without code changes, but [build-atlas.ts:112](/web/art/pipeline/build-atlas.ts:112) requires every person sprite to exist in `CAST`; document or remove that exception. **Small–medium: half a day to a day.**

7. **Remove smaller generator duplication and a latent combination bug.** The restroom and souvenir shop independently construct similar gable roofs at [buildings.ts:276](/web/art/generators/buildings.ts:276) and [buildings.ts:482](/web/art/generators/buildings.ts:482). A focused roof geometry helper would reduce drift while leaving decoration local. Separately, [people.ts:688](/web/art/generators/people.ts:688) can draw framed lenses twice when a design combines a strap with framed glasses: the strap helper also draws them at [people.ts:890](/web/art/generators/people.ts:890). Give one layer ownership and test the combination; the current cast was not shown to use it. **Small: half a day each.**

8. **Check redundant CI build work after a clean-environment experiment.** The test job invokes `make build` before `make test` at [ci.yaml:41](/.github/workflows/ci.yaml:41), and both build and test regenerate art. Assess whether the test job can generate only the prerequisites its tests actually need. Some Vitest files mock generated Wasm imports, so removing the build without checking resolution in a clean checkout would be premature. Keep the Chrome dependency needed by atlas generation. **Small: half a day.**

## Slow-test triage

The times below are the supplied last-run measurements, not new profiling results. Suggested changes should retain the assertions’ intent and be timed after implementation.

- **Wooden twister, 49 s:** [wooden-twister.spec.ts:23](/web/e2e/wooden-twister.spec.ts:23) places 22 pieces through the UI, closes the circuit, and advances a full lap. Keep the complete model-specific build and lap in the slow tier. Put track validity and individual piece combinations in simulation tests, and use a short representative construction interaction for fast browser coverage.
- **Daylight confetti, 35 s:** [daylight.spec.ts:85](/web/e2e/daylight.spec.ts:85) waits for the live simulation’s first crest. Use deterministic test-mode ticks or a controlled crest fixture, and retain a browser assertion that the event reaches the effect. The present test sees one burst at the first crest; it does **not** establish “once per lap” over multiple laps. Add deterministic event-level coverage for that claim.
- **Guest appearances, 29 s:** [world.spec.ts:79](/web/e2e/world.spec.ts:79) rebuilds the atlas and Vite bundle. Its [guest fixture:13](/web/e2e/fixtures/guests.ts:13) also simulates 360 ticks and scans the canvas with repeated GPU picks. Reuse the atlas and fixture server, and use known guest positions or targeted picks. Keep the screenshot as a slow visual check and appearance selection in focused tests.
- **Path rebuild, 22 s:** [paths.spec.ts:9](/web/e2e/paths.spec.ts:9) combines UI rebuilding with potentially 100,000 simulation ticks and repeated per-guest detail requests. Keep a short remove-and-rebuild browser smoke test; cover routing and a subsequent food visit in a deterministic small simulation integration fixture. Retain the full-park scenario in the slow tier until the replacement proves the same consequence.
- **Path-panel layout, 18 s:** [path-panel-layout.spec.ts:4](/web/e2e/path-panel-layout.spec.ts:4) needs a real drag and canvas-position measurement. Use a smaller prepared world or lightweight UI fixture, while retaining the real pointer interaction and resulting path assertion. Move it to slow only if the reduced fixture remains costly.
- **Sloped paths, 17 s:** [paths.spec.ts:80](/web/e2e/paths.spec.ts:80) combines drag construction, slope picking, queue connectivity, and a screenshot. Keep a short browser construction/pick smoke; test connectivity and slope cases directly, and retain the screenshot in the slow visual tier.
- **Multi-launch, 17 s:** [multi-launch-coaster.spec.ts:6](/web/e2e/multi-launch-coaster.spec.ts:6) builds a circuit, runs a lap, checks launch speeds and picks overlays. Retain this end-to-end interaction in the slow tier; use a prepared circuit for a fast UI launch-position check and simulation tests for launch behavior.

For other wall-clock waits, replace the fixed startup delay and manual polling in [block-sections.spec.ts:159](/web/e2e/block-sections.spec.ts:159) with an observable readiness condition that still reports startup errors. The theme screenshot generator also has fixed waits at [themes.ts:53](/web/art/pipeline/themes.ts:53); wait for rendered state instead.

## Independently landable order

1. Protect atlas output and fix the glasses combination.
2. Extract shared browser fixture lifecycle and reuse the generated atlas.
3. Replace deterministic test waits and introduce the fast tier, retaining the full gate.
4. Centralize projection and depth calculations.
5. Split renderer inputs, stateful stages, and resource setup in small changes.
6. Split people and building generators, then extract the repeated roof geometry.
7. Measure clean CI prerequisite changes and the revised test durations.

No tests were run and no files were modified. The missing report file is the remaining deliverable; the read-only mount prevents creating it in this session.
