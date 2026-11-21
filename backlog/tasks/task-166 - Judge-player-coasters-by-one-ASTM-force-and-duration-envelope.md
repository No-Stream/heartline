---
id: TASK-166
title: Judge player coasters by one ASTM force-and-duration envelope
status: In Progress
assignee: []
created_date: '2026-11-21 16:05'
updated_date: '2026-11-21 21:25'
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
<!-- SECTION:NOTES:END -->
