---
id: TASK-156
title: 'Shaped elements: solve element shape for entry speed and feel intent'
status: To Do
assignee: []
created_date: '2026-11-19 16:16'
labels: []
dependencies: []
priority: high
ordinal: 153000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Plan: docs/superpowers/plans/2026-11-19-shaped-elements.md. Owner vision 2026-11-19: ask for an element type plus feel and optional force (floater/ejector hill, banked or strong-lateral turn, loop or Raven turn at 3.8/4/4.5 g); the engine sizes and shapes it for the entry speed and the model's train; classic fixed pieces stay. Root cause of the brake-heavy Horizon Hawk. Stage 1: shaped hills and turns plus a Horizon Hawk with at most one mid-course brake run and sustained airtime -0.8 to -1.2 g.
<!-- SECTION:DESCRIPTION:END -->

## Owner decision and results (2026-11-20)

Owner decision: raise Horizon Hawk's comfortable minimum vertical limit from -1.3 g to -1.5 g, matching the other steel models. Keep the 0.25 g design margin and 1.5 m/s rated-speed margin unchanged. This lets strong floater airtime approach -1 g within the design margin.

Only `coaster_models_use_peak_rider_force_limits` changed its expected hyper limit. Other `-1.3` values in force peak samples, synthetic calibration and agreement fixtures, hybrid airtime-band tests, and generic comfort tests do not encode the hyper model limit and remain unchanged.

`make generate-rated-windows` changed two hyper maximum edges; all other rated windows stayed the same:

| Piece | Old window (m/s) | New window (m/s) | Edge sources |
| --- | --- | --- | --- |
| `WideLevelToSixtyDown` | 1.00–8.29 | 1.00–9.47 | minimum `EnergyMinimum`; maximum `ForceBound` |
| `SteepToSixtyDown` | 3.00–24.66 | 3.00–27.17 | minimum `EnergyMinimum`; maximum `ForceBound` |

`tools/design check content/rides/horizon-hawk.ride.json` passed expectations and native agreement. It reported excitement 6.67, maximum vertical 3.24 g, maximum lateral 0.19 g, 8 airtime moments, and force limits within limits. Its nine rated-window checks passed in both physics configurations. The seeded park report remained identical to its golden (scores 7.19 / 6.32 / 0.84, with the existing airtime target warning still failing). The ride layout was not changed.

Updated generated artifacts: `crates/sim/src/track/rated_windows.generated.rs` for the two force-bounded windows and regeneration date, and `web/src/generated/modelForceLimits.ts` for Horizon Hawk's limit and derived airtime rating bands. The park-report golden did not change, and no screenshots needed re-recording.

Verification: workspace tests passed (1,058 passed, 3 filtered); `make lint`, `make check` (237 Rust, 484 web, 9 art, 44 browser tests), `make ride-agreement` (7 browser tests), `PLAYWRIGHT_PORT=4771 make e2e-feature` (198 tests), and `PLAYWRIGHT_PORT=4772 make journeys` (11 tests) passed. The all-rides agreement run reported one Velocity Viper downstream-track check-3 issue, with zero estimate errors; its test passed.
