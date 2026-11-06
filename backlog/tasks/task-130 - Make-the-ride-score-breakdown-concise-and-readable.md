---
id: TASK-130
title: Make the ride score breakdown concise and readable
status: Done
assignee: []
created_date: '2026-11-05 21:29'
updated_date: '2026-11-05 23:49'
labels:
  - ui
  - scoring
dependencies: []
priority: high
ordinal: 130000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Follow-up to TASK-104/113 (2026-11-05). The score contributions now name the force axis, value, limit and duration, as the owner asked, but the testing panel became a wall of text: each contribution lists every band threshold, weighted seconds, percentages and car numbers in long sentences, and the same ASTM finding appears under excitement, intensity and nausea. The owner wants it clear, self-explanatory and delightful. Design a concise breakdown: one short plain line per factor (for example 'Strong sideways force at the large left turn: 4.8 g for 3.8 s, limit 1.5 g. Intensity +0.6'), the biggest contributors first, small ones folded away, full detail on demand (expand or hover), and each finding shown once with its effect on each score. Also fix the overlap at the top left of the panel where an element covers the first notes paragraph. Taste-heavy: Opus, with screenshots in light and dark themes.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Score breakdown regrouped in web/src/score-findings.ts: one finding per scoring term with its effect on each score, top five shown, the rest under More detail, full numbers expand per row. Location names are buttons that select the spot on the track. The top-left overlap was the track-end height label spilling out of the world canvas; .game-world now clips it. Both scoring-panel baselines re-recorded.
<!-- SECTION:NOTES:END -->
