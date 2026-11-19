---
id: TASK-136
title: Rate bank-in and bank-out transitions
status: To Do
assignee: []
created_date: '2026-11-06 18:37'
labels: []
dependencies: []
priority: medium
ordinal: 136000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found by TASK-126 (docs/reviews/task-126-rated-windows.md on branch force-rule): a six-car Briar Blitz train entering a banked straight at 30 m/s gets -2.4 g sideways on the bank-in transition, which has no rated window, so nothing warns or limits it. Give roll transitions (bank-in, bank-out, roll-ins to banked turns) rated windows from the same filtered force harness and roll-rate rule, so the builder's speed default and the overlay treat them like other rated pieces.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

2026-11-18: TASK-126 adds model-specific rated windows for BankLeft, BankRight, UnbankLeft and UnbankRight, including their gentle-slope variants, and applies the existing roll-rate rule to these transitions and the longer rollouts. The connected measurement uses a model-allowed established-bank approach for bank-out transitions and rollouts. This delivers the core measurement and builder integration for this task; final verification follows the TASK-126 scan and gates.
