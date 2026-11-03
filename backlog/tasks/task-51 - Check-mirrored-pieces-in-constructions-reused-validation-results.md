---
id: TASK-51
title: Check mirrored pieces in construction's reused validation results
status: In Progress
assignee: []
created_date: '2026-11-03 16:47'
updated_date: '2026-11-03 18:14'
labels:
  - refactor
  - coaster
  - bug
milestone: m-4
dependencies: []
documentation:
  - docs/reviews/2026-11-03-track-physics-code.md
priority: high
ordinal: 51000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Track review finding 5: construction reuses an earlier choice's validation result when geometry matches, but the comparison ignores the mirrored flag, so a mirrored corkscrew or heartline roll near an asymmetric obstacle may get the wrong result. Also from the simulation review: facility placement checks clearance against the demo train's envelope instead of the player's model (harmless today because all models share one envelope).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A regression test with an asymmetric obstacle compares reused and individual validation for mirrored pieces, and the key includes orientation if it fails; facility placement uses the player's model envelope
- [ ] #2 Behaviour is unchanged unless stated, and the relevant tests pass while editing, with make ci green at the end
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Mirrored hypothesis confirmed and fixed in this commit. Remaining: place_facility_at in crates/sim-wasm/src/lib.rs should check demo pieces with the demo train and construction pieces plus segments with construction.model().train; apply when the bridge is free.

Facility placement now checks demo circuit pieces with the demo train configuration and construction pieces plus disconnected segments with the construction model train configuration. Added a bridge regression test using a deliberately wider model train; current coaster models share an envelope, so a production-model-only test cannot distinguish them without a custom test model. The test exercises the separation helper with distinct configurations and verifies both track sets use their assigned envelope. Targeted theme-park-sim-wasm tests passed; make ci was not run per task scope.
<!-- SECTION:NOTES:END -->
