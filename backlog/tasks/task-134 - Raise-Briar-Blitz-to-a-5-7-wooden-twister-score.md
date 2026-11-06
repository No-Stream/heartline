---
id: TASK-134
title: Raise Briar Blitz to a 5-7 wooden twister score
status: To Do
assignee: []
created_date: '2026-11-06 06:30'
updated_date: '2026-11-06 15:27'
labels: []
dependencies: []
priority: low
ordinal: 134000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Follow-up to the good-circuits rebuild (docs/superpowers/plans/2026-11-05-good-circuits.md). After three design passes the best Briar Blitz layout that meets every force limit and design margin scores about 4 excitement (target 5-7 for a GCI-like wooden twister); removing trims raised the score but pushed twister lateral force over the limit. Likely needs pieces suited to a twister at 15-20 m/s (tighter speed-matched turns for wood, more airtime hills at those speeds) or the TASK-126 smoothed windows. Re-attempt after TASK-126 lands.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
2026-11-06: four design passes; best closed candidate (default 3.57, construction test 4.10; peaks +3.25 / -0.47 / 0.92 sideways / 1.22 g) misses the 1.5 m/s turn margin (1.11 m/s) and the top-hat centre rule (0.36 m/s). Candidates saved in /tmp/briar-final-design-unique.txt, /tmp/briar-turn-adjust.txt, /tmp/briar-trim-later.txt (copy them somewhere durable before they vanish); a journey draft is stashed in the good-circuits-2 worktree ('Briar journey draft for TASK-134'). The Briar acceptance test was removed until the layout lands (c366818). New pieces already on good-circuits: Strong floater wooden 8 x 64 hill, long pullout. Retry after TASK-126 (smoothed windows) and TASK-135 (faster speed turns).
<!-- SECTION:NOTES:END -->
