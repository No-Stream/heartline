---
id: TASK-105
title: Show an approximate g-force and speed overlay while building
status: Done
assignee: []
created_date: '2026-11-04 17:00'
updated_date: '2026-11-05 06:43'
labels: []
dependencies:
  - TASK-95
priority: high
ordinal: 105000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Owner request 2026-11-04. While building a coaster, show approximate speed and g-forces along the track built so far (colour along the track and a readout for the next piece), simulated from the station or lift with the current train, even before the circuit is closed. Updates as pieces are placed. Approximate is fine; it should make building more fun and help avoid rollbacks and painful forces.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Speed and vertical/lateral g shown along the unfinished track while building
- [ ] #2 Updates after each placement fast enough to feel live
- [ ] #3 Browser test checks the overlay appears and changes
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
2026-11-04 owner: wants force overlays while building, like the existing test-run track overlay, e.g. to see 'this camelback is pulling -2.5 g, that won't work'. Approximate is fine. Reuse the existing overlay metrics (speed, vertical, lateral, longitudinal, airtime) on the unfinished track, and flag forces past the model's comfortable limits.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
While building, a simulated estimate from the station colours the unfinished track by limit closeness (worst of vertical, lateral, longitudinal g as a share of the model's comfortable limit: blue, amber, crimson with outline), labels past-limit spots and recent peaks with weak/good/great/too much/dangerous ratings (FORCE_RATING_BANDS in web/src/force-overlay.ts), shows all values on hover, reports where the train stalls, and shows each launch's maximum speed and peak force. Single-metric overlays remain selectable. 4-19 ms per update on the player circuits; within 0.15 g of the full test run. Gates: lint, npm test, construction-estimate, construction-controls and audio specs, make journeys 8/8.
<!-- SECTION:FINAL_SUMMARY:END -->
