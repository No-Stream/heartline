# Finish the circuit (TASK-159)

Goal: from the open end of a coaster, find short piece sequences that bring the train back into the station with the right position, height and heading, a train that reaches the station, every new piece inside its model's force limits (with the 0.25 g headroom `tools/design check` uses) and inside its rated-speed window (with the check's rated margin rule), and no collisions. Return a ranked list, or a clear explanation of why there is none. Agents use it through `tools/design ride <file> finish`; players use it through the builder's Finish coaster action. Both call the same Rust search.

## What exists

`crates/sim/src/track/construction/completion.rs` (TASK-92) already has a sliced beam search used by the worker (4 ms slices), the builder session and the CLI `finish` command. It searches at most five pieces within 80 m of the station, uses geometry only, and stops at the first connection. Two measured problems on Horizon Hawk with its last three pieces removed (end 57 m from the station, 90 degree heading mismatch, 22.8 m/s): the search took 10.6 s natively and found nothing. The time goes into placement validation, because every search node is a construction copy without the validation cache, so each append rebuilds tunnel plans and support obstacles for the whole ride. It also only considers small turns in its heuristic, so the large turn the layout needs was pruned.

A full lap of Horizon Hawk takes 0.66 s natively (25,926 steps of 5 ms with eight cars), so a search that simulates whole laps per candidate cannot answer in about a second. The builder's force preview (`PreviewCache`, construction-test physics: default physics with a 5 m/s lift) already resumes from a snapshot taken where the train crosses the open end, so a completion can be simulated by running only its own pieces.

## Decisions

**Extend the existing search; do not add a second one.** `completion.rs` keeps its `CompletionSearch::advance` API (one validation or one physics evaluation per call), so the worker slicing and builder session keep working.

**Candidates.** Every model-placeable track template without an inversion whose shape is plain, speed-banked, profiled, an airtime hill, an overbank or a wave turn (straights, offsets, slopes, classic turns, speed turns, classic hills), plus `Brakes`. Stations, lifts, launches, rolls and shaped elements are not candidates in this version: a shaped element needs a solve for its entry speed, which would not fit the time budget. Node copies keep the validation cache so each append validates only the new piece. Candidates with the same geometry but different rated windows (for example the three speed-turn sizes) are kept apart.

**Geometry search.** Beam search, at most 6 added pieces, beam width 12, 16 candidates per node, starting when the end is within 120 m of the station entry. Nodes are ranked by the length so far plus an estimate of the remaining length that accounts for heading: the best over up to two classic turns of the straight distance plus 16 m per 45 degrees still to turn, or, once lined up, the distance along the station heading plus 1.5 m per metre of sideways and height offset and a large penalty for overshooting. Nodes are deduplicated by end state, brake count and rated piece count, so a brake run or a speed turn is kept as an alternative to the same geometry without one. A node is dropped when its energy estimate (speed at the open end, height change, rolling resistance, 3 m/s² while on brakes) says the train cannot reach its end.

**Physics check per closing sequence.** Each sequence that closes is simulated with the builder's construction-test physics, resuming from the cached snapshot at the open end (only the new pieces and the train-length runout are simulated). A completion is valid when the train reaches the station, every new piece's worst-car force peaks stay 0.25 g inside the model's comfortable limits, and every car enters every rated new piece with `rated_speed_margin >= 0`. Pieces already placed are not judged.

A test-only variant skips the physics so the older geometry coverage tests on unpowered layouts keep checking that the search finds a known route.

**Stopping.** The search finishes the depth at which the first valid completion appears, so alternatives with the same piece count are ranked together, then stops. It also stops at the depth limit or when the beam empties.

**Ranking.** Fewest pieces, then least energy removed by brakes (kJ, from the simulated speeds entering and leaving each brake piece and the loaded train mass), then the largest worst-case margin (smallest of force headroom in g and rated-speed margin in m/s across the new pieces). At most 5 completions are returned.

**Explaining failure.** When nothing is valid, the result says why and what would need to change: too far from the station (with distance), already closed, no station, disconnected track; no geometric route (with the remaining offset, height difference and heading mismatch); or every route failed physics, with counts per reason and the worst example: too fast for a rated piece (entry speed against window, so brakes or a climb are needed before the return), too slow or stalls (needs more speed or a lower end), or forces over the limit (which axis and by how much).

**CLI.** `tools/design ride <file> finish` lists the ranked completions without changing the ride: pieces, arrival speed, brake energy, the smallest force headroom and rated margin, plus a full closed-lap verification of each listed completion with the same functions `check` uses (rated margins under default and construction-test physics, force headroom under both). `--json` returns the same data. `finish apply <n>` applies completion n, refuses it if its verification fails, and records the pieces as ride steps. This changes `finish`, which used to apply the first connection; the existing tests and the agent guide are updated.

**Browser.** The worker's existing sliced search produces the list. The Finish coaster button previews the best completion; the confirmation area shows "Option i of n" with its pieces, arrival speed and brake energy, and Previous/Next buttons cycle the preview. Confirm places the shown completion. Failure reasons appear in the existing finish reason line.

## Time budget

Target: the browser answer within about a second. Measured natively (release build, the search alone, after the builder's force preview is already warm, which the builder computes anyway):

| Open ride | Search time | Result |
|---|---|---|
| Horizon Hawk without its last 3 steps | 35 ms | 1 option (two one-tile straights) |
| Velocity Viper without its last 4 steps | 183 ms | 2 options |
| Timber Tumbler without its last 5 steps | 152 ms | 1 option |
| Horizon Hawk without its last 4 steps | 3.0 s | no safe finish, too fast for the classic turns |

The failing case is the slow one, because the search runs to its depth limit. The force preview itself takes 0.3 to 0.5 s on these rides. The CLI adds a full closed-lap check of each listed option under both physics configurations, so `finish` takes 2.5 to 4 s per call.

## Tests

Native, test first: a nearly finished classic ride gets a valid completion, Horizon Hawk with its last three pieces removed gets valid completions that pass the check's rated margin and force headroom rules after closing, and an impossible case (end far below the station with too little speed, or a heading the search cannot fix) returns a clear reason. CLI tests for listing, JSON and apply. A Playwright feature test builds a short open ride in the browser, uses Finish coaster, cycles options, confirms, and sees the circuit closed.
