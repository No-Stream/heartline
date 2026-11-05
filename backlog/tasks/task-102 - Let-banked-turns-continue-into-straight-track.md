---
id: TASK-102
title: Let banked turns continue into straight track
status: Done
assignee: []
created_date: '2026-11-04 17:00'
updated_date: '2026-11-05 06:23'
labels: []
dependencies:
  - TASK-95
priority: high
ordinal: 102000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Owner playtest 2026-11-04: after a banked turn the player cannot continue with a straight piece, which makes connecting back to the station harder. Add banked straights and bank-to-level straight transitions (rolling out while going straight) for each bank angle, available after any banked turn piece, on all models where they fit.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 After any banked turn a straight continuation is offered (banked straight and roll-out straight)
- [ ] #2 Curvature and roll stay continuous and the pieces are rated
<!-- AC:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Banked straights and roll-out straights are offered after banked track with zero exit curvature (completed banked turns, BankLeft/BankRight) on all three models; mid-turn segments continue through their exit piece. Rated windows scanned over both directions, three slopes, all models and 6/12-car trains (1,488 cases pass, max roll-out lateral 1.20 g). Gates: track, rated physics, player circuits, wasm, build, lint, npm test, piece catalog and construction-controls specs; journeys 6/8 in the full run with the two crowd-timing journeys passing on their own (see TASK-124).
<!-- SECTION:FINAL_SUMMARY:END -->
