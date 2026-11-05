---
id: TASK-112
title: >-
  Make train physics more realistic: friction, materials, drag, warm-up and
  weather
status: To Do
assignee: []
created_date: '2026-11-04 21:24'
labels: []
dependencies:
  - TASK-95
priority: medium
ordinal: 112000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Owner request 2026-11-04. Today (crates/sim/src/physics/mod.rs) rolling resistance is a flat 0.005 g regardless of wheel load, drag is one constant for the whole train, wood and steel share the same friction, and there is no warm-up, temperature, weather or wind. Banking already converts lateral into vertical force correctly because forces are projected onto the track's up and sideways axes. Proposed: rolling resistance proportional to normal force (so high-g valleys cost more energy); per-model wheel and track material coefficients (polyurethane or nylon wheels on steel or wood, wood track losing more); drag from frontal area, car count and train shape; a warm-up model where trains run slower on cold morning laps and speed up as wheels and bearings warm through the day (and cool when idle); weather and wind (temperature, rain, headwind and tailwind on exposed sections) tied to the day cycle. Needs a short plan with real-world reference values first; changes speeds on existing circuits, so journeys and rated windows must be rechecked.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Rolling resistance scales with normal force; per-model wheel and track materials
- [ ] #2 Drag depends on train frontal area and length
- [ ] #3 Trains warm up through the day and run measurably faster when warm
- [ ] #4 Weather and wind affect speed with plausible magnitudes
- [ ] #5 Existing circuits and journeys still complete; rated windows rechecked
<!-- AC:END -->
