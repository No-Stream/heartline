---
id: TASK-134
title: Raise Briar Blitz to a 5-7 wooden twister score
status: To Do
assignee: []
created_date: '2026-11-06 06:30'
updated_date: '2026-11-21 14:40'
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
2026-11-06: four design passes; best closed candidate (default 3.57, construction test 4.10; peaks +3.25 / -0.47 / 0.92 sideways / 1.22 g) misses the 1.5 m/s turn margin (1.11 m/s) and the top-hat centre rule (0.36 m/s). Candidates saved in /tmp/briar-final-design-unique.txt, /tmp/briar-turn-adjust.txt, /tmp/briar-trim-later.txt (copied to docs/reviews/briar-candidates/); a journey draft is stashed in the good-circuits-2 worktree ('Briar journey draft for TASK-134'). The Briar acceptance test was removed until the layout lands (c366818). New pieces already on good-circuits: Strong floater wooden 8 x 64 hill, long pullout. Retry after TASK-126 (smoothed windows) and TASK-135 (faster speed turns).

2026-11-21: the TASK-162 redesign (on master since 2d4b675) scores excitement 5.44 under construction-test physics but 4.72 under default physics, so the 5-7 target is met only under one of the two physics configurations; left open for that reason. Forces: sustained maximum vertical 2.97 g (peak 3.44 g), minimum vertical -0.45 to -0.53 g, lateral at most 0.58 g, 9 airtime moments. Against the owner's ride-feel targets the pull-outs are a little above the ~2.5 g wood guidance and the airtime is milder than the -1 g the owner likes, so stronger ejector airtime is the obvious lever for more score. TopHat still fails the narrow-window rated-margin rule (see TASK-162). Revisit after the global envelope (TASK-166) and shaped wooden elements (TASK-167) land.
<!-- SECTION:NOTES:END -->
