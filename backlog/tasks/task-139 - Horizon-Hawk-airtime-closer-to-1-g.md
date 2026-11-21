---
id: TASK-139
title: Horizon Hawk airtime closer to -1 g
status: Done
assignee: []
created_date: '2026-11-07 01:33'
updated_date: '2026-11-21 21:25'
labels: []
dependencies: []
priority: low
ordinal: 139000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Horizon Hawk (merged 2026-11-06) is designed for -1 g camelback crests, but its reference circuit gives -0.6 to -0.9 g because every hill is entered 1.5 m/s below the top of its window, and it needs nine trims. Owner likes about -1 g. Rework the layout (hills entered nearer their window tops once TASK-126's smoothed windows land, fewer trims) and consider a smaller margin for hills.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Horizon Hawk was rebuilt with shaped elements and one two-piece mid-course brake run. The current layout records sustained airtime of -0.94/-0.91 g under construction-test/default physics and passes the documented ride, force-headroom, and rated-margin checks. Master commit 6661fe4; measurements recorded in TASK-156.
<!-- SECTION:FINAL_SUMMARY:END -->
