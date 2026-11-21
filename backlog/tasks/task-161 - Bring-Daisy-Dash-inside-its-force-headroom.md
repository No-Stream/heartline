---
id: TASK-161
title: Bring Daisy Dash inside its force headroom
status: To Do
assignee: []
created_date: '2026-11-21 03:15'
labels: []
dependencies: []
ordinal: 157000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
On master 6661fe4, tools/design check content/rides/daisy-dash.ride.json reports force headroom failures: default physics +2.82/-0.41 g, construction-test physics +2.89/-0.51 g, against 2.75/-0.35 g limits (model limits minus 0.25 g). check exits 0 because the ride file does not set designMargins (see TASK-160). Soften its worst pullout and airtime piece with the design tool, set designMargins: true, re-record its digests and goldens with reasons, and run ride-agreement. Layout work goes to Opus.
<!-- SECTION:DESCRIPTION:END -->
