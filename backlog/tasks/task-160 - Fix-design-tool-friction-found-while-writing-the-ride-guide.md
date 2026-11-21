---
id: TASK-160
title: Fix design tool friction found while writing the ride guide
status: To Do
assignee: []
created_date: '2026-11-21 03:05'
updated_date: '2026-11-21 21:25'
labels: []
dependencies: []
ordinal: 156000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while writing docs/designing-rides.md (2026-11-20): (1) the state line's 'can finish' reads 'Checking for a connection to the station.' even where finish succeeds, and 'can close' stays false after finish although close works; (2) check labels the sustainedMaxVerticalG result 'maximum vertical g' though it checks the sustained value; (3) rated-window and force-headroom failures only make check exit 1 with designMargins: true, and a failing check prints its report on stderr; (4) text output rounds shaped hill values to one decimal (-0.85 g prints -0.8), JSON is exact; (5) help omits new --cars; (6) estimate prints no forces for shaped elements, only probe does; (7) no command writes the expect block. Some overlap with the circuit-finish branch (TASK-159), which changes finish; fix after it merges.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Fixed on branch tool-friction (2026-11-20), each item re-checked against master a17334e after TASK-159:
(1) State now runs the finish search to completion (BuilderSession::search_finish, shared with finish), so 'can finish' is the real result with its reason; 'Checking for a connection' no longer appears in the CLI. Can-close after finish: finish apply now closes the circuit itself, so can close false is correct there. The remaining bug was that close on an already closed circuit succeeded and appended a duplicate close step; BuilderSession::close now refuses with 'The coaster circuit is already closed.' (the browser path, simulation.construction_close, is unchanged). Cost: within 120 m of the station the search adds up to about 3 s to commands that render state.
(2) Label is now 'sustained maximum vertical g' (minimum already said sustained).
(3) Exit rule NOT changed: with rated-window and force-headroom failures gated for every ride, three content rides would fail, not only Daisy Dash: Briar Blitz (rated margins on BankLeft, UnbankLeft, TopHat; positive vertical 4.34-4.41 g against 3.75 g; lateral 7.09-7.26 g against 1.25 g) and the Browser agreement fast circuit fixture (lateral 1.59 g against 1.25 g), plus Daisy Dash (TASK-161). designMargins stays the opt-in; no knownFailures mechanism was added since it only makes sense with the gate. Done: a failing text check prints its report on stdout and only a summary (each failing ride and the gates it failed) on stderr; JSON mode keeps stderr empty as before.
(4) Shaped hill and turn forces print to two decimals in probe, place, estimate and options shapes.
(5) help lists new ... [--cars <count>].
(6) estimate prints the solved shape line for shaped ids, from the existing preview data.
(7) Left as a documented limitation (docs/designing-rides.md, Targets and the check).
docs/designing-rides.md updated, including the Close the circuit section, which still described the pre-TASK-159 finish (80 m, five pieces, run close after finish).

Backlog review 2026-11-21: items 1-6 and the documented output behavior are implemented on master (e94cd57; fixes recorded in the ticket). Keep open because item 7, a command to write the ride expect block, is explicitly still a documented limitation in docs/designing-rides.md.
<!-- SECTION:NOTES:END -->
