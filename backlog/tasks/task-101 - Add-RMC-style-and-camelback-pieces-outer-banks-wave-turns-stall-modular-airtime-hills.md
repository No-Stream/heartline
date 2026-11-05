---
id: TASK-101
title: >-
  Add RMC-style and camelback pieces: outer banks, wave turns, stall, modular
  airtime hills
status: To Do
assignee: []
created_date: '2026-11-04 17:00'
updated_date: '2026-11-05 01:41'
labels: []
dependencies:
  - TASK-95
priority: high
ordinal: 101000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Owner request 2026-11-04. Add Rocky Mountain Construction style elements to Timber Tumbler (an RMC-style hybrid) and wherever they fit (Briar Blitz should have them): modular camelback / airtime hills that chain together (various heights and lengths, buildable as segments so a player can compose a hill), outer-banked airtime hills and turns (banked beyond 90 degrees), wave turns, stall (zero-g stall), zero-g roll, step-up under-flip, and similar. Each piece must have a valid rated entry-speed window under the force limits from TASK-95 and must be heartlined where the real element is. Plan with the force framework first.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Modular camelback hill segments that compose into hills of several sizes
- [ ] #2 Outer-banked hill and turn, wave turn, stall and zero-g roll on Timber Tumbler; appropriate ones on Briar Blitz
- [ ] #3 Every new piece has a rated window and a journey or test that builds and rides it
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Owner 2026-11-04: variety in airtime moments is key, especially for RMC-style coasters. Proposed interface: one 'Airtime hill' element with an inspector (feel: ejector / floater / speed hill / stall; height stepper; length; twist: none / banked / outer bank / wave turn; live predicted airtime g and seconds), solved to the target force by the TASK-95 profile solver, chainable into camelback runs (repeat-last-hill key or a run preset). Awaiting owner confirmation of the interface direction.

Owner approved the airtime hill interface on 2026-11-04: one parameterised 'Airtime hill' element (feel: ejector / floater / speed hill / stall; height; length; twist: none / banked / outer bank / wave turn; live predicted airtime g and seconds), solved to the target force by the TASK-95 profile solver, chainable into camelback runs (repeat-last-hill key or run preset).
<!-- SECTION:NOTES:END -->
