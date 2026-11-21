---
id: TASK-162
title: Explain Briar Blitz's 7 g lateral and 4.4 g vertical in the design check
status: To Do
assignee: []
created_date: '2026-11-21 05:39'
labels: []
dependencies: []
ordinal: 158000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found during TASK-160 (2026-11-20): tools/design check content/rides/briar-blitz.ride.json reports rated-margin failures on BankLeft, UnbankLeft and TopHat, positive vertical 4.34-4.41 g against a 3.75 g headroom limit, and lateral 7.09-7.26 g against 1.25 g. The check still exits 0 because designMargins is opt-in. 7 g lateral is not plausible for a ride that tests fine in the game, so first establish whether these are real ride forces or a measurement artifact (e.g. bank and unbank transitions in the force evaluation), then either fix the measurement or the ride. The browser-agreement fast circuit fixture also shows 1.59 g lateral against 1.25 g. Until both are resolved, check cannot make rated-window and force-headroom failures fatal for every ride (TASK-160 item 3).
<!-- SECTION:DESCRIPTION:END -->
