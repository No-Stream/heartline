---
id: TASK-166
title: Judge player coasters by one ASTM force-and-duration envelope
status: Done
assignee: []
created_date: '2026-11-21 16:05'
updated_date: '2026-11-22 00:28'
labels: []
dependencies: []
ordinal: 160000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Plan: docs/superpowers/plans/2026-11-21-global-limits-and-shaped-elements.md (Part A). Per-model force limits become DesignForceTargets for sample rides only; scores, bands, warnings, rated window hard edges and the builder's finish search use one loosely ASTM F2291 envelope.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Carry-forward from superseded TASK-145: resolve the floater-hill case where the force readout is rated Great but the rated window says only “Too fast.” The hard window edge should reject unsafe force/energy/clearance cases, while exceeding a model-feel edge should show the planned amber consequence without a contradictory bare too-fast label.

Completed on branch `global-comfort-envelope`:
- Split `ComfortableForceLimits` into `DesignForceTargets` and unified comfort judging into one ASTM F2291-based `EnvelopeTracker` / `EnvelopeEvaluator` (per-level isoacceleration durations, 200 ms minimum event, 1.1x combined-axis ellipses with accurate per-axis attribution, reduced push-pull curve, sustained-lobe reversals, and sustained-weightlessness onset).
- Removed `ExcessiveForces` double penalty from scoring, moved `forceLimitExceeded`, track overlays, ride graphs, and builder hard rated windows to the global envelope, and switched lateral/longitudinal "strong" bands to a global 0.75 g threshold while keeping per-model "great from" thresholds.
- Added two-edge rated windows (`rated_speeds` hard ASTM edge + `feel_rated_speeds` inner model-feel edge) and regenerated `rated_windows.generated.rs` with zero unavailable windows.
- Updated builder readouts to show red "too fast"/"too slow" past the hard edge and amber "stronger than a typical <model>" past the feel edge (resolving the TASK-145 floater-hill contradiction), with no score penalty.
- Parameterized finish search with `FinishForceJudge::{Envelope, DesignTargets}` so the in-game builder judges by the envelope while `tools/design finish` keeps `DesignForceTargets` and headroom checks.
- Bumped save format to `TPSAVE\x06` (`SaveVersion::V6`) with explicit typed error (`SaveLoadError::LegacyForceLimitsUnsupported`) for pre-v6 saves containing serialized physics evaluators.
- Verified with `make lint`, `make test-force-rule`, `make test-rated-windows`, `make test-design-tools`, `make test-rust`, `make bench-finish`, `make check-shaped-turn-arc-parity`, `npm test` (80/80), `npm run test:art`, `fast-agreement`, Playwright feature suite (200/200), `ride-agreement` (8/8), and `journeys` (11/11).
<!-- SECTION:NOTES:END -->
