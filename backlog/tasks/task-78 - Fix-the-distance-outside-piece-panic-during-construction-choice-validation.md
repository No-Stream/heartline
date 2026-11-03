---
id: TASK-78
title: Fix the 'distance outside piece' panic during construction choice validation
status: Done
assignee: []
created_date: '2026-11-03 20:24'
updated_date: '2026-11-03 21:00'
labels:
  - coaster
  - bug
dependencies: []
priority: high
ordinal: 78000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found by the TASK-63 random walk on 2026-11-03: Briar Blitz at 30 m above ground panics in crates/sim/src/track/curves.rs:514 ('distance outside piece') while validating choices after [Station, TightTwisterLeft, BankLeft, BankedTurnStandalone, LargeLeftBankedTurnEntry, LargeLeftBankedTurnExit, UnbankLeft, SteelLoop, Station, Brakes, Station, LevelToGentleDown, GentleDownToLevel, TurnLargeStandalone, TurnLargeRightStandalone, TurnSmallEntry, TurnSmallExit, OffsetLeft, BankRight, UnbankRight, LiftLevelToGentleUp, LiftGentleUpToLevel, VerticalUpEntry, VerticalUpContinue, VerticalUpExit]. In the browser a panic inside the worker stops all further construction, which matches the owner's 'cannot continue' report. Test first: a regression test that builds this prefix through Construction and calls choices(). Find the root cause (likely a float sample distance slightly beyond piece length, e.g. piece.length() * fraction rounding, in track_support_obstacles or clearance sampling) and fix it at the source rather than clamping blindly; check other callers of sample() for the same pattern.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A regression test reproduces the panic and passes after the fix
- [x] #2 Every caller that samples a piece at a computed distance is checked for the same rounding
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
The support obstacle endpoint was computed as piece length multiplied by a segment position and then divided by the segment count. Floating-point rounding could make the result exceed the arc length and trigger Piece::sample’s bounds assertion during choices validation. Compute the bounded fraction before multiplying by length; the same endpoint-inclusive pattern was corrected in clearance, proximity, and block geometry sampling. The Briar Blitz construction regression reproduced the panic before the fix.

The same fraction-first calculation was applied to WebAssembly rail, coaster geometry, and piece-shape sampling after auditing the bridge callers.
<!-- SECTION:NOTES:END -->
