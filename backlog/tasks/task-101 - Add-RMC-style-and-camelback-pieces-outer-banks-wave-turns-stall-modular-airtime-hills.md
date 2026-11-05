---
id: TASK-101
title: >-
  Add RMC-style and camelback pieces: outer banks, wave turns, stall, modular
  airtime hills
status: To Do
assignee: []
created_date: '2026-11-04 17:00'
updated_date: '2026-11-05 13:43'
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

2026-11-04 owner: make elements modular with a strong default; players can tweak an element, and every element shows its predicted forces and speeds before placing. Fable's six plan questions take their defaults (200 deg/s roll cap, drop the constant-lean banked hill, 110 deg outer bank, one stall, skip the step-up under-flip, upright hills only on Briar Blitz).

2026-11-04 owner: rate every force moment as weak / good / great / too much, not just the g value. Airtime example (vertical g at the heart): about 0 g weak ('an airtime hill offering 0 g is a little sad'); -0.3 to -0.8 g good (floater); -1.0 to -1.3 g great ('-1 g or a hair more is great'); past the model limit (about -1.5 g) too much; about -2 g dangerous. Same idea for positive vertical and lateral. Use the bands in scoring (great airtime raises excitement, too much raises intensity and nausea), in the building overlay labels (TASK-105, bands table in TypeScript), and as the airtime hill's default targets (TASK-101).

2026-11-04 owner: wants force-vector-design style building. Elements are sized for the incoming speed by default: when the player picks a loop or camelback, the builder preselects the grid size whose design speed matches the predicted entry speed (from the TASK-105 construction estimate), so a loop gives strong but reasonable positives and an ejector camelback gives 'great' airtime (about -1.1 g) by default. The preview shows the expected forces with their rating band, and says so when no size fits. Feel is the primary hill setting; height and length remain tweakable. Do not name real rides in the UI.

2026-11-05: plan tasks 1-2 merged to master. 14 straight hills (ejector, floater, speed, wooden ejector) with measured rated windows; deferred: 4 x 32 m steel ejector (2.1 g longitudinal), 8 x 64 and 16 x 80 m wooden ejectors, 2 x 38 m speed hill (solver does not converge). Hill tables are f32 deltas, wasm +180 KB.

2026-11-05: plan task 3 merged. 22 outer-banked hills (own family: floater-level vertical plus outward lateral, rated within all force limits), four zero-g rolls and two stalls (peak roll rates 178-192 deg/s; the stall's inverted hold is 10% of its roll window to stay under 200 deg/s; the fast zero-g roll passes longitudinal by 0.002 g). Next: task 4 (wave turn and rated overbank turn).
<!-- SECTION:NOTES:END -->
