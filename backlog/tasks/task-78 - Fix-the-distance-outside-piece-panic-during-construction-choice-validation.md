---
id: TASK-78
title: Fix the 'distance outside piece' panic during construction choice validation
status: To Do
assignee: []
created_date: '2026-11-03 20:24'
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
- [ ] #1 A regression test reproduces the panic and passes after the fix
- [ ] #2 Every caller that samples a piece at a computed distance is checked for the same rounding
<!-- AC:END -->
