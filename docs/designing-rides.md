# Designing rides with the design tool

This guide is for agents and people who design coasters with `tools/design`, the command-line builder that drives the same builder session as the game. It covers the design loop, the force rules the owner wants, how to read `tools/design check`, and the mistakes that cost the most time while Horizon Hawk was rebuilt on 2026-11-20. Every command and output below was run on master at `6661fe4`. Where the tool cannot do something yet, the guide says so.

## How the tool works

A ride lives in a JSON ride file, usually `content/rides/<name>.ride.json`. The file is the source of truth: it holds the model, station, train size, an ordered list of steps (chain toggles, classic pieces, shaped elements, close) and an optional `expect` block of targets. Every `tools/design ride <file> ...` call replays the file into a fresh builder session, runs the commands you give it, and writes the file back if anything changed. A cache file `<file>.snapshot` sits next to the ride (it is gitignored) and is rebuilt whenever the ride or the tool binary changes. `tools/design` runs the simulation as a native Rust binary. The game runs the same code compiled to WebAssembly, and players build rides in the browser's builder panel, which this guide calls the visible builder. Native agreement, part of `check`, compares the builder's per-piece estimates with the measured test inside the native binary; the browser agreement test builds the ride through the visible builder and compares its results with the native ones. `tools/design` runs `make design-tools` before every call, which rebuilds the release binary only when the source changed; a build failure prints the log path under `target/`.

You can chain several commands in one call; each command name starts a new command. A call is all or nothing: if any command fails, the tool prints the error and the state at the failure, exits 1, and writes none of that call's changes. With no commands on the line, the tool reads commands from standard input, one per line. Add `--json` anywhere on the line for machine-readable output; it shows exact values where the text output rounds. Text prints shaped-element forces to two decimals, as the browser does (a strong floater that delivers -0.850314 g prints as "-0.85 g"), and most other values to one decimal.

```sh
tools/design help                                   # command list (there is no per-command --help)
tools/design ride my.ride.json state                # where the open end is
tools/design ride my.ride.json place SteepUp place SteepUp   # two commands, one call
tools/design check content/rides/horizon-hawk.ride.json      # the acceptance check
```

| Command | What it does |
| --- | --- |
| `new --model <key> --station x,y,z,heading [--cars <n>]` | Start a ride file. `--cars` defaults to 6. |
| `state` | Open end position, height, heading, offset to the station, predicted end speed. |
| `choices [group]` | Classic pieces offered at the open end, with rated windows and refusals. Groups: bank, element, hill, inversion, shapes, slope, special, straight, turn. |
| `options shapes` | Shaped elements offered at the open end and the model's shaped-element limits. |
| `estimate <id> ...` | Builder force estimate for one or more pieces or shaped elements without placing them. |
| `probe <id>` | Preview one piece or shaped element: entry speed, forces, end tile, or the refusal. |
| `place <id> [--matched] [--launch-speed <m/s>] [--note <text>]` | Place a classic piece. |
| `place hill feel=<floater\|strong-floater\|ejector> [g=<target>] [height=<m>]` | Place a shaped airtime hill. |
| `place turn <left\|right> feel=banked [radius=<m>]` | Place a shaped fully banked turn. |
| `place loop <left\|right> [g=<peak>] [top=<floaty\|neutral\|pressed-in>]` | Place a shaped vertical loop sized to entry speed and peak vertical g (steel-structure on `wooden_twister`). |
| `place zero-g-roll <left\|right> [g=<peak>]` | Place a shaped heartlined zero-g roll sized to entry speed and peak vertical g. |
| `place raven-turn <left\|right> [g=<peak>] [top=<floaty\|neutral\|pressed-in>]` | Place a shaped single-piece Raven turn (climbing half-loop + offset half-roll) that reverses heading 180°. |
| `chain on\|off`, `launch-speed <n> <m/s>`, `cars <n>`, `trains <n>`, `auto-blocks` | Lift, launch, train and block settings. |
| `undo [count]`, `truncate <steps>` | Remove placements. |
| `finish`, `finish apply <n>`, `close` | List ranked ways back to the station; place one and close the circuit; close a circuit that already reaches the station. |
| `test`, `report [--samples]`, `agreement`, `map` | Measure a closed ride; draw the footprint. |
| `export --save <path>` / `--session <path>` | Write a park save or a replay session. |

## The design loop

### Start a ride

Pick a model key and a station. Model keys are `wooden_twister`, `hybrid`, `velocity_viper`, `family_coaster` and `hyper`. The station is given in tiles (one tile is 4 m) plus a height in metres and a heading. All current content rides except the demo and the agreement fixture use `241,249,z,north`; Horizon Hawk uses `z=10`, the others `z=12`. The ride's `name` is taken from the file name ("demo hyper" for `demo-hyper.ride.json`), so edit it in the JSON if you want a proper name.

```console
$ tools/design ride demo-hyper.ride.json new --model hyper --station 241,249,10,north --cars 8
Pieces: 0
Open end: tile (241.0, 253.0), height 10.0 m, heading North, slope Level, bank Level
Height above ground: 2 steps; offset to station entry: 16.0 m (x +0.0, y -4.0 tiles, height +0.0 m)
Closure approach: remaining offset: 4.0 tiles (16.0 m); heading mismatch: 0 degrees
Chain: false; predicted end speed: 2.0 m/s (reachedEnd)
Can close: false; can finish: false (No finish found: no route of up to 6 pieces reaches the station entry without a collision; the end is 16 m from the station entry (x +0.0, y -4.0 tiles, height +0.0 m) with a 0 degree heading mismatch, and the train reaches it at 2.0 m/s. Bring the end closer to the station entry, line it up with the station's heading, or clear the way.); can test: false
```

Three values in the state output matter most while building. "Offset to station entry" and "heading mismatch" tell you how far the open end is from where the circuit must close. "Predicted end speed" is the builder's estimate of the speed at the open end, which decides which pieces and shaped elements fit next. "Can finish" is the result of the same search `finish` runs, with its reason when no finish exists, so it tells you after each placement whether a checked way back to the station exists; near the station the search adds up to a few seconds to each command. "Can close" is true only when the open end already sits at the station entry; after the circuit is closed both flags are false and the reason says the circuit is already closed.

### Look before you place

`choices` lists classic pieces with their rated speed window and the margin of the predicted entry speed from the nearest window edge. A negative margin means the entry speed is outside the window. This margin is the plain distance to the edge; the acceptance rule in `check` is stricter (see the force rules section). The `refusal` column explains why a piece cannot go here, for example "Supports cannot be placed here."

```console
$ tools/design ride demo-hyper.ride.json estimate WideLevelToSixtyDown LevelToGentleDown
piece                      entry m/s window      margin  candidate preview vertical g (rating)     lateral g (rating)   long g (rating) status
WideLevelToSixtyDown            5.0 1.0-9.5       +4.0 1.0 (weak) / -0.8 (good)                   0.0 (fine)         1.1 (strong)       reachedEnd
LevelToGentleDown               5.0 —                — 1.0 (weak) / 0.4 (weak)                    0.0 (fine)         0.4 (fine)         reachedEnd
```

`probe` works for both classic ids and shaped ids. For a shaped id, both `probe` and `estimate` print the solved shape: design speed, target and delivered force, and size. Shaped ids have the form `shape:hill:<floater|strongFloater|ejector>:<g or default>:<auto or fixed-<metres>>`, `shape:turn:<left|right>:fullyBanked:<auto or fixed-<metres>>`, `shape:loop:<left|right>:<peak_g or default>:<floaty|neutral|pressedIn>`, `shape:zeroGRoll:<left|right>:<peak_g or default>`, and `shape:ravenTurn:<left|right>:<peak_g or default>:<floaty|neutral|pressedIn>`; `options shapes` prints the automatic ones supported by the model.

```console
$ tools/design ride demo-hyper.ride.json options shapes
Model hyper limits: vertical -1.50 to +4.00 g, lateral up to 1.20 g; speed 22 to 36 m/s (4–8 cars); default hill targets floater -0.50 g, strong floater -0.85 g, ejector -1.20 g; turn radius 8.0 to 256.0 m, bank up to 75.0 degrees.
id                                   name                     design m/s delivered g size                 refusal
shape:hill:floater:default:auto      floater hill                   34.6       -0.50 24.5x198.0 m
shape:hill:strongFloater:default:auto strong floater hill            34.6       -0.85 24.3x186.0 m
shape:hill:ejector:default:auto      ejector hill                   34.6       -1.20 24.6x178.0 m
shape:turn:left:fullyBanked:auto     left fully banked turn         34.6        0.07 radius 82.0 m
shape:turn:right:fullyBanked:auto    right fully banked turn        34.6        0.07 radius 82.0 m

$ tools/design ride demo-hyper.ride.json probe shape:turn:left:fullyBanked:fixed-32
Shape refusal: {"availableHeadroomG":-2.149668693537394,"kind":"infeasibleTarget","targetG":2.9}
```

For turns, `options shapes` shows only the lateral force in its "delivered g" column. Use `probe` or `estimate` to see the vertical force, which is the number the owner cares about in a banked turn. Refusals give no feasible size range yet, so to find a radius that works, probe a few sizes.

### Place classic pieces

`place <id>` places a catalog piece and prints the builder's estimate and the new state. `chain on` and `chain off` mark lift sections, and `--launch-speed` sets a launch piece's target speed. For pieces with speed-matched variants, such as the speed turns, `--matched` places the variant whose rated window fits the current entry speed:

```console
$ tools/design ride demo-hyper.ride.json place SpeedTurnLarge28Left --matched
Placed SpeedTurnLarge32Left (speed matched from SpeedTurnLarge28Left)
Estimate: 33.2 m/s entry, v+ 3.32 g, v- 1.00 g, lateral 0.03 g, longitudinal 0.32 g
```

Here the turn entered at 33.2 m/s, 1.8 m/s inside the 29-35 m/s window. That passes the builder but leaves only +0.30 m/s under the 1.5 m/s rule in `check`, so a classic turn this close to its window edge is fragile. A shaped banked turn avoids the problem, because the solver shapes it for its entry speed.

### Place shaped elements by feel

Shaped elements are solved for the entry speed and the actual train: you ask for a feel and the tool sizes the element so that the train delivers it. The hill feels are `floater` (default -0.5 g), `strong-floater` (-0.85 g) and `ejector` (-1.2 g on steel, -0.8 g on wood); `g=` overrides the target and must be negative, and `height=` fixes the height instead of letting the solver pick it. Turns have one feel, `banked`, which means fully banked with the lateral force driven close to zero; `radius=` fixes the radius. There is no `g=` option for turns. A radius the solver cannot shape within the force limits is refused with `infeasibleTarget`, as the 32 m probe above shows.

Inverting coaster models also support shaped inversions sized to entry speed, train length, and peak positive vertical g: `place loop` on `velocity_viper`, `hybrid`, and `wooden_twister` (which builds a steel-structure vertical loop on wooden track), plus `place zero-g-roll` and `place raven-turn` on `velocity_viper` and `hybrid`. Vertical loops and Raven turns accept an optional peak vertical force (`g=3.5`, `3.8`, `4.0`, `4.2`, or `4.5` in the browser stepper) and a top feel (`top=floaty` at 0.3 g, `top=neutral` at 1.0 g, or `top=pressed-in` at 1.5 g). Zero-g rolls accept an optional peak vertical force (`g=3.0` through `4.5`) and calibrate crest airtime near 0 g while keeping axial roll rate at or below 200°/s. Pushing peak vertical g past a model's typical target (for example, 4.2 g on Briar Blitz or 4.5 g on Timber Tumbler) stays allowed up to the global ASTM comfort envelope and shows the amber feel note.

A Raven turn's top feel is a target for the whole train. The solver measures each car's seat force over the curved crest and shapes the crest radius so that the average over the cars lands within 0.35 g of the requested feel (it aims for 0.25 g, to leave room for the small difference between its preview and a full ride test). Cars cross the crest at different speeds, so single cars spread around that average: on a 3-car Velocity Viper at 30 m/s a Neutral top reads 0.81 to 0.93 g by car, while an 8-car Hybrid at 26 m/s reads 0.68 to 1.84 g. Only the force limits bound that spread. The readout and `estimate` print the train average. When the average cannot reach the feel without the crest breaking a force limit, the turn is refused with `topFeelOutOfReach`, which reports the average and the lowest and highest car. That happens mostly for Pressed-in tops on slow or long trains, where a tight crest pulls the rear cars past the longitudinal limit: at 22 m/s on every train tested, at 26 m/s on most, and on the 8-car Hybrid up to 30 m/s. A refusal with `infeasibleTarget` on an inversion always means the peak force, and the browser calls it the peak target. Every top feel stays selectable in the browser stepper; a feel the train cannot reach shows the refusal in the readout and cannot be placed.

```console
$ tools/design ride demo-hyper.ride.json place turn left feel=banked radius=68 place hill feel=strong-floater g=-0.85 height=24
Placed shape:turn:left:fullyBanked:fixed-68
Estimate: 34.6 m/s entry, Shaped left fully banked turn: 34.6 m/s for 8 cars; delivered vertical 3.38 g and lateral 0.11 g; radius 68.0 m, bank 71.5 degrees
...
Placed shape:hill:strongFloater:-0.85:fixed-24
Estimate: 34.0 m/s entry, Shaped hill: strongFloater at 34.0 m/s for 8 cars; target -0.85 g, delivered -0.85 g; height 24.1 m, span 180.0 m
```

Shaped elements are supported on all five coaster models, each driven by its own `ShapedProfile` in `crates/sim/src/track/models.rs`. The entry must be level, unbanked and straight; outside a model's train-length or speed range the tool refuses with `unsupportedScope`, `belowMinimumSpeed` or `unsupportedConnector`.

| Model | Cars | Entry speed | Typical hill feels | Default targets (`floater` / `strong-floater` / `ejector`) | Auto turn vertical / max bank | Finish turn ceiling |
| --- | --- | --- | --- | --- | --- | --- |
| `hyper` (Horizon Hawk) | 4–8 | 22–44 m/s | `floater`, `strong-floater` | -0.50 / -0.85 / -1.20 g | 2.9 g / 75° | 3.5 g vert, 0.6 g lat |
| `velocity_viper` (Velocity Viper) | 3–6 | 20–36 m/s | `floater`, `strong-floater`, `ejector` | -0.50 / -0.85 / -1.05 g | 3.2 g / 75° | 3.5 g vert, 0.6 g lat |
| `hybrid` (Timber Tumbler) | 4–8 | 20–36 m/s | `floater`, `strong-floater`, `ejector` | -0.50 / -0.85 / -1.05 g | 3.1 g / 75° | 3.5 g vert, 0.6 g lat |
| `wooden_twister` (Briar Blitz) | 4–12 | 18–30 m/s | `floater`, `strong-floater` | -0.50 / -0.60 / -0.80 g | 1.9 g / 60° | 3.5 g vert, 0.6 g lat |
| `family_coaster` (Daisy Dash) | 4–10 | 14–22 m/s | `floater` | -0.25 / -0.85 / -1.20 g | 1.9 g / 60° | 2.75 g vert, 0.6 g lat |

**Typical feel defaults versus global ASTM flexibility.** Each model's typical hill feels and automatic banked turns stay inside that model's design force targets with 0.25 g headroom so showcase rides pass `designMargins: true`. Players can still push past a model's typical feel (for example, true ejector on a hyper coaster, strong floater or ejector on Daisy Dash, or tighter fixed-radius manual turns) all the way up to the global ASTM F2291 comfort envelope. When a shaped element exceeds the model's typical feel while staying inside the comfort envelope, the builder shows a mild amber note ("Within the comfort envelope, stronger than a typical `<model>`.") with no score penalty.

Pick values the visible builder can reach. The browser's strength stepper moves in 0.1 g steps from the feel's default, so on `hyper` a strong floater can be -0.85, -0.95 or -1.05 g but not -1.0 g. Height and radius steppers move in 2 m steps, so use even whole metres or `auto`. The CLI accepts any value, and a ride that uses an unreachable one passes `check` but fails the browser agreement test (see the traps below).

### Undo and truncate

`undo [count]` removes the last placements, counting each classic piece or shaped element as one. `truncate <steps>` keeps the first N ride steps and replays them; it counts every serialized step, including `chain` toggles and `close`, so count the `steps` array in the file before you use it. There is no command to replace an element in the middle of a ride. To change an early choice, truncate to just before it, place the new element, and place the rest again (the regeneration recipe below does this in one go).

### Close the circuit

`close` succeeds only when the open end sits exactly at the station entry, at the station's height and heading, and refuses a circuit that is already closed. `finish` searches for routes of up to six pieces that bring the end there; it only starts within 120 m of the station and never places shaped hills, lifts or launches. On every coaster model it also places fully banked shaped turns with fixed even radii, solved for their entry speed, kept under that model's finish ceiling (2.75 g vertical on `family_coaster`, 3.5 g vertical on the other four models, and 0.6 g lateral on all models), and ending at least 8 m before the station entry; `finish apply` saves them as shaped steps like `place turn`. It lists the routes it found, best first, each with its arrival speed, the energy its brakes remove, its force headroom, its rated-window margin and whether the closed ride passes the rated-window and force-headroom rules of `check`. `finish apply <n>` places route `n` and closes the circuit in one step, so do not run `close` after it.

```console
$ tools/design ride hawk-end.ride.json truncate 41     # Horizon Hawk minus its last two straights and close
Kept 41 ride step(s).
...
Closure approach: remaining offset: 2.0 tiles (8.0 m); heading mismatch: 0 degrees
Chain: false; predicted end speed: 22.5 m/s (reachedEnd)
Can close: false; can finish: true; can test: false
$ tools/design ride hawk-end.ride.json close
close circuit: The track must connect back to its station.
$ tools/design ride hawk-end.ride.json finish
Found 1 way(s) to finish the circuit, best first (construction-test physics; 'finish apply <n>' places one):
1. OneTileStraight, OneTileStraight (2 pieces): arrives at 22.5 m/s; brakes remove 0 kJ; force headroom +0.92 g; rated margin none rated; ride check pass
...
$ tools/design ride hawk-end.ride.json finish apply 1
Placed finish option 1 (2 piece(s)) and closed the circuit.
...
```

When the search fails, the reason says why: the end is too far away, no route of up to six pieces gets there without a collision, or every route that gets there breaks a force or rated-window rule, with the worst case for each. In practice, you bring the end within a few tiles, near station height and heading, with your own pieces, and let `finish` add the last pieces.

### Test and report

`test` and `report` need a closed circuit; on an open ride they stop with "rated entry margins require a closed circuit", so you cannot measure forces on a partial layout yet. `test` prints scores, statistics and the per-piece table. `report` adds the per-car force table, rated-window margins, the shaped element table, per-element sustained forces, brake run speeds and the mid-course brake summary. `report --samples` adds a one-metre trace of the front car, which is long. Pieces in these tables are numbered from 1, starting with the first piece after the station.

Two lines in the report's statistics summarise the force picture:

```text
peak forces: vertical +3.63 g at LevelToGentleUp (piece 27, car 0); vertical -1.14 g at GentleUpToLevel (piece 30, car 0); lateral 0.44 g at ShapedTurnRight (piece 39, car 7); ...
sustained by any car: highest vertical 3.31 g at ShapedTurnLeft (piece 9, car 0) held 0.5 s; lowest vertical -0.94 g at ShapedHill (piece 16, car 7) held 0.3 s
```

The last two report lines show the brake runs:

```text
# run class piece_indices default_entry_m/s default_exit_m/s default_removed_m/s construction_entry_m/s construction_exit_m/s construction_removed_m/s
  1 mid-course [25, 26]         27.15 23.28 3.87 27.37 23.54 3.83
Mid-course brake runs: 1; estimated lift energy removed by trims from lead-car samples: 14.6% (...)
```

`agreement` compares the builder's per-piece estimates with the measured forces; `check` runs the same comparison. `map` draws the footprint in 8 m cells with `S` for the station, plus a height profile per piece.

### Targets and the check

The `expect` block holds the ride's targets. No command writes it; add it to the JSON by hand after `new`. These are Horizon Hawk's:

```json
"expect": {
  "designMargins": true,
  "excitement": [6, 8],
  "sustainedMaxVerticalG": [3, 3.5],
  "sustainedMinVerticalG": [-1.2, -0.8],
  "maxLateralG": 0.6,
  "inversions": 0,
  "airtimeMomentsAtLeast": 6,
  "withinComfortableForceLimits": true,
  "launches": 0,
  "biggestDropAtLeast": 50,
  "maxSpeedAtLeast": 30,
  "features": { "piecePrefixesAtLeast": { "ShapedHill": 4, "ShapedTurn": 7, "WideLevelToSixtyDown": 1 } }
}
```

Other keys are `inversionsAtLeast`, `invertedSeconds`, and under `features`: `piecePrefixGroupsAtLeast`, `heartlinedInversionsAtLeast`, `piecePrefixesAfter`, `orderedSteps` and `adjacentSteps`. Unknown keys are rejected when the file is parsed. Shaped steps count as `ShapedHill`, `ShapedTurnLeft` or `ShapedTurnRight` for the prefix features. Do not loosen `expect` to make a ride pass; if a target looks wrong, raise it with the owner.

`tools/design check <file> ...` tests the ride twice. Default physics starts the train at rest with a 3 m/s lift; construction-test physics, which the builder's estimates use, starts at 2 m/s with a 5 m/s lift. For each configuration, it prints rated-window margins, force headroom and every expectation, then any native agreement failures, and finally runs a fast Node/WASM browser-path agreement replay (which also flags stale shaped geometry that the current solver would no longer produce). It exits 1 when an expectation fails under either configuration or agreement fails. Rated-window and force-headroom failures make it exit 1 only when `designMargins` is `true`; without it they are printed and ignored, so set it on every new ride. A failing check still prints its whole report on standard output; standard error gets only a short summary naming each failing ride and the checks it failed, for example `demo hyper: expectations, default physics expectations`. The Horizon Hawk check takes about 25 seconds.

```console
$ tools/design check content/rides/horizon-hawk.ride.json
Horizon Hawk: expectations pass; native agreement pass
Rated entry-speed margin (default physics, lift 3.0 m/s, initial speed 0.0 m/s):
# piece car entry_m/s window_m/s rule_margin_m/s status
  7 WideLevelToSixtyDown car 0 3.00 1.00 to 9.47 +0.50 PASS
  8 WideSixtyDownToLevel car 7 23.96 2.00 to 26.76 +1.30 PASS
...
Force headroom (0.25 g inside each model limit):
# physics metric observed_g boundary_g remaining_g status
default physics maximum positive vertical g 3.58 3.75 +0.17 PASS
...
construction-test physics maximum forward g 1.17 1.25 +0.08 PASS
  expectation excitement expected 6.00 to 8.00, observed 7.42 (pass)
  expectation sustained maximum vertical g expected 3.00 to 3.50, observed 3.31 (pass)
  expectation sustained minimum vertical g expected -1.20 to -0.80, observed -0.94 (pass)
  expectation maximum lateral g expected at most 0.60, observed 0.44 (pass)
...
  default physics expectation excitement expected 6.00 to 8.00, observed 6.71 (pass)
```

The "sustained maximum vertical g" and "sustained minimum vertical g" lines check `sustainedMaxVerticalG` and `sustainedMinVerticalG`. Expect lower numbers under default physics: the slower lift gives a slower train, so Horizon Hawk scores 6.71 there against 7.42 in the construction test, and its floaters deliver -0.91 g against -0.94 g. Leave room for both.

### Export

`export --save <path>` writes a park save with the ride built; the game loads it through the Park menu's "Import park file". `export --session <path>` writes a replay session. I exported both for Horizon Hawk (a 2.3 MB `.park` and a 17 KB session) but did not load them in the browser for this guide.

## Force rules and owner targets

The owner's targets for steel coasters are below; the second column says how to express each one in `expect` or where to read it.

| Rule | Where it is checked |
| --- | --- |
| Sustained positives 3.0 to 3.5 g on steel; about 2.5 g on wood | `sustainedMaxVerticalG: [3, 3.5]` on steel; no wooden ride sets this target yet |
| Brief positive peaks up to 4.5 g are fine | Not an expectation; capped by force headroom, which is lower than 4.5 g on most models |
| Airtime about -1 g | `sustainedMinVerticalG: [-1.2, -0.8]` |
| Lateral at most 0.6 g | `maxLateralG: 0.6` (brief peak, not sustained) |
| 0.25 g force headroom inside the model limits | Force headroom table; gating with `designMargins: true` |
| Rated-speed windows: 1.5 m/s from both edges of windows at least 3 m/s wide, within 0.3 m/s of the centre of narrower windows | Rated entry-speed margin table; gating with `designMargins: true` |
| At most one mid-course brake run, removing little energy | Not checked; read the brake lines in `report` |

**Sustained versus peak.** A sustained value is the strongest force that any car holds without a break: positive vertical force held for at least 0.5 s, negative vertical force held for at least 0.3 s. A peak is the single most extreme sample from any car. Horizon Hawk's sustained maximum is 3.31 g while its peak is 3.63 g, at the start of the return climb. `sustainedMaxVerticalG` and `sustainedMinVerticalG` use sustained values; `maxLateralG`, force headroom and `withinComfortableForceLimits` use peaks. A ride can therefore meet the 3.0 to 3.5 g pullout target and still fail headroom on a brief spike, and a short spike above 3.5 g does not fail the pullout target.

**Force headroom.** Each model has comfortable force limits, and `check` requires every peak to stay 0.25 g inside them, under both physics configurations. Longitudinal force is limited to 1.5 g on all models, so forward and braking peaks must stay under 1.25 g.

| Model | Vertical limits | Lateral limit | Peaks must stay within |
| --- | --- | --- | --- |
| `hyper` | -1.5 to +4.0 g | 1.2 g | -1.25 to +3.75 g, lateral 0.95 g |
| `hybrid` | -1.5 to +4.5 g | 1.5 g | -1.25 to +4.25 g, lateral 1.25 g |
| `velocity_viper` | -1.5 to +5.0 g | 1.5 g | -1.25 to +4.75 g, lateral 1.25 g |
| `wooden_twister` | -1.0 to +4.0 g | 1.5 g | -0.75 to +3.75 g, lateral 1.25 g |
| `family_coaster` | -0.6 to +3.0 g | 1.0 g | -0.35 to +2.75 g, lateral 0.75 g |

So "brief peaks up to 4.5 g" holds only on `velocity_viper`; on `hyper` and `wooden_twister` the check stops brief peaks at 3.75 g.

**Rated-speed windows.** Classic pieces such as speed turns, classic hills and steep transitions have a rated window of entry speeds. In `check`, a window at least 3 m/s wide needs an entry speed 1.5 m/s or more from both edges, and a narrower window needs an entry within 0.3 m/s of its centre. The check uses the worst car under each physics configuration, except on crests fed directly by a lift, where only the front car counts (the output says "Lift-fed crests check the front car only"). The `rule_margin_m/s` column is positive when the rule passes. Shaped elements have no rated window; the solver shapes them for their entry speed instead.

**Brake runs.** A mid-course brake run is any run of adjacent `Brakes` pieces that does not end right before the station. Keep at most one, and make it take off little speed: Horizon Hawk's two-piece run takes 27.2 m/s down to 23.3 m/s (3.9 m/s, about 15% of the lift's energy). The layout before it had a five-piece run that removed 11 m/s and 37% of the lift's energy, which is the brake-heavy design the owner asked to remove. If a ride needs a big brake run, the elements before it are too small for the speed; use bigger or stronger shaped elements to spend the energy.

## Reading check failures

**Expectation failure.** The line names the target and the value, for example `expectation sustained maximum vertical g expected 3.40 to 3.50, observed 3.31 (fail)`. A line prefixed "default physics" failed only in the slower configuration. Find the element responsible in `report`'s "Measured per-element forces" table, which gives each element's worst-car sustained vertical and peak lateral force under both configurations. Low pullouts call for tighter shaped turns or a stronger hill; floaters that are too weak call for a stronger `g=` target, remembering that default physics delivers a little less than the construction test.

**Rated speed margin failure.** The message reads `rated speed margin failure: piece 8 WideSixtyDownToLevel rated margin under default physics is -0.20 m/s` (format from `crates/design-tools/src/report.rs`). The train enters a classic piece too fast or too slow for its rated window. For a turn, try `--matched` to pick the variant for this speed, or replace it with a shaped banked turn. Trimming speed with a brake run in front of it is the last resort, because it moves the energy problem elsewhere: in one Horizon Hawk attempt, a brake moved in front of `SpeedTurnLarge28Left` left the turn only 1.3 m/s inside its 25-31 m/s window, below the 1.5 m/s rule.

**Force headroom failure.** The message reads `force headroom failure: construction-test physics maximum positive vertical g observed 3.81 g against 3.75 g boundary (headroom -0.06 g)`. Find the piece in the report's "peak forces" line or the per-car table. A common cause is a terrain-following piece at speed: one Horizon Hawk draft ended on `TerrainFall` (shallow descent) at 27.5 m/s and measured +5.27/-3.91 g. Classic transitions are built for low speeds; at hyper speeds use gentle slopes (`LevelToGentleDown`, `OneTileGentleDown`, `LongGentleDownToLevel`) or slow the train first. If the train stops before completing a lap, every headroom row reads "unavailable: train stopped..." and fails.

**Native agreement failure.** Lines of the form `piece <n> <check> <metric>: <reason>` mean that the builder's estimate and the measured test disagree for that piece. That is a tool bug, not something to design around; report it with the ride file.

**Stale shaped geometry.** Each shaped step in the file has an `intent` (what you asked for: feel, target and size) and a `spec` (the solved shape as a compact recipe, ending in `geometry_digest`). Edit `intent` only through the tool; never edit `spec`. If the code that rebuilds geometry from a recipe changes, loading the ride fails with "resolved geometry digest mismatch: stored ..., regenerated ...". If the solver changes so that the same intent would now give a different shape, the native check still passes on master, because it replays the saved `spec` blocks. The Rust test `every_content_ride_shaped_intent_resolves_to_its_saved_geometry_digest` in `crates/sim/tests/shaped_elements.rs` catches it with "... differs from re-solved digest ...; regenerate the ride with tools/design", and the browser agreement test fails, because the browser solves every shape again. Either way, regenerate the ride (see "A ride file goes stale when the solver changes" below).

**Station height.** `close` fails with "The track must connect back to its station." when the end is off by a tile, a height step or a heading. Check the "height +x m" part of the offset line and the heading mismatch. The browser test also needs the station's absolute height: the visible builder has a minimum station height above ground, and the agreement test lowers the terrain under the station so the ride file's `z` can be kept. Do not change `z` to suit the browser.

## Traps that cost hours

**Closing the circuit at station height and heading.** Long shaped elements move the open end tens of metres at a time, and the finish search only looks within 120 m of the station and adds at most six plain pieces. During the Horizon Hawk rebuild, an end 5.4 tiles away with a 180-degree heading mismatch could not be finished, and after a turn brought it within 1.2 tiles, a 90-degree mismatch still blocked `close`. Plan the return from the start: decide which way the last element must point, and watch the offset and heading-mismatch lines after every placement. Bring the train back to station height early, with gentle slopes, and finish with straights pointing at the station. Horizon Hawk ends with a 28 m left turn, a one-tile straight, a 36 m right turn and two one-tile straights.

**Brake runs and the station.** A `Brakes` piece is four tiles (16 m) long. In one Horizon Hawk attempt, a brake placed after the last turn left only three tiles before the station, and the descent that had to follow then collided with the station track and was refused. If a brake run is needed, place it where the layout has room, well before the station approach.

**A ride file goes stale when the solver changes.** Never edit `spec` blocks by hand and never copy specs between rides. When the solver changes, rebuild the ride from its own steps in a fresh session, so that every shape is solved again at its replayed entry speed. This script turns a ride file into commands (it does not handle `blockSections` steps or `--note` text):

```python
import json, sys

ride = json.load(open(sys.argv[1]))
station = ride["station"]
print(f"new --model {ride['model']} --station {station['x']},{station['y']},{station['z']},{station['heading']} --cars {ride['carsPerTrain']}")
feel_tokens = {"Floater": "floater", "StrongFloater": "strong-floater", "Ejector": "ejector"}
for step in ride["steps"]:
    if "chain" in step:
        print("chain on" if step["chain"] else "chain off")
    elif "place" in step:
        launch = f" --launch-speed {step['launchSpeed']}" if "launchSpeed" in step else ""
        print(f"place {step['place']}{launch}")
    elif "hill" in step.get("intent", {}):
        hill = step["intent"]["hill"]
        size = "" if hill["size"] == "Auto" else f" height={hill['size']['Height']}"
        print(f"place hill feel={feel_tokens[hill['feel']]} g={hill['strength_g']}{size}")
    elif "turn" in step.get("intent", {}):
        turn = step["intent"]["turn"]
        size = "" if turn["size"] == "Auto" else f" radius={turn['size']['Radius']}"
        print(f"place turn {turn['hand'].lower()} feel=banked{size}")
    elif "close" in step:
        print("close")
    else:
        raise SystemExit(f"step not handled: {step}")
```

```sh
python3 ride-commands.py content/rides/horizon-hawk.ride.json | tools/design ride /tmp/hawk-regenerated.ride.json
```

The new file has the regenerated steps but a name taken from its file name and no `expect` block. Copy `name` and `expect` across from the old file, replace the old file, and run `check`. Run against Horizon Hawk on `6661fe4`, this reproduces the committed steps exactly. After regenerating a content ride, also update the pinned Horizon Hawk digests and builder hashes that tests record; the TASK-156 notes list the ones that changed last time.

**Hill strength must be reachable in the browser.** The stepper rule is in "Place shaped elements by feel" above. An early Horizon Hawk draft used a -1.0 g strong floater and a 12.5 m hill, neither of which the browser can select: from the -0.85 g default it reaches -0.95 or -1.05 g. A later run also timed out because a stepper rounding bug (since fixed) made -0.95 g unreachable. The browser agreement test now fails at once with "Visible strength stepper cannot select ... in 0.1 g steps" or "only selects 2 m increments". Choose reachable values from the start.

**Native and browser must agree.** `tools/design check` proves the ride works natively. The browser agreement test then builds the same ride through the real UI, piece by piece, and requires the browser's estimates and measured samples to match the native ones within 1e-6. That is the final check for a content ride:

```sh
PLAYWRIGHT_PORT=4781 make ride-agreement RIDE=horizon-hawk   # one ride; each ride has a 6-minute limit
PLAYWRIGHT_PORT=4781 make ride-agreement                     # all rides, about 20 minutes
```

It runs a browser, so take the shared browser lock in turn with other agents and run it once, at the end. During the Horizon Hawk rebuild on 2026-11-20 it found two real differences that the native check could not: a saved turn solved by an older solver, and a hill whose delivered force differed by 0.000013 g between native and WebAssembly builds.

**Sustained versus peak targets.** The ride file's `sustainedMaxVerticalG` is a sustained target. An earlier version of the browser test compared it with the report's brief peak (3.63 g against a 3.5 g ceiling) and failed a ride that met its target with 3.31 g sustained.

## Worked example: Horizon Hawk

Horizon Hawk is a hyper coaster with an 8-car train. Its layout is a 74 m chain lift, a 64 m drop at 60 degrees, a run of shaped strong floaters and banked turns, one short brake run, and a gentle return to the station. You can follow it from an empty file; each command block below ran on `6661fe4`, and the first twelve steps match the committed ride exactly.

```sh
tools/design ride demo-hyper.ride.json new --model hyper --station 241,249,10,north --cars 8
tools/design ride demo-hyper.ride.json chain on place LevelToGentleUp place GentleToSteepUp \
  place SteepUp place SteepUp place SteepToGentleUp place GentleUpToLevel chain off
tools/design ride demo-hyper.ride.json estimate WideLevelToSixtyDown
tools/design ride demo-hyper.ride.json place WideLevelToSixtyDown place WideSixtyDownToLevel
```

The lift crest feeds `WideLevelToSixtyDown` at 5 m/s, inside its 1.0-9.5 m/s window, and the pullout delivers 3.16 g to the front car. The state then shows the train level at 12 m, heading north at 34.6 m/s, which is inside the shaped-element range of 22 to 36 m/s. From here, every element is shaped:

```sh
tools/design ride demo-hyper.ride.json options shapes
tools/design ride demo-hyper.ride.json probe shape:turn:left:fullyBanked:fixed-68
tools/design ride demo-hyper.ride.json place turn left feel=banked radius=68 place hill feel=strong-floater g=-0.85 height=24
```

The 68 m turn delivers 3.4 g vertical with 0.1 g lateral, and the 24 m strong floater delivers -0.85 g. The automatic turn at this speed would be 82 m; the tighter fixed radius raises the pullout into the 3.0-3.5 g band. The committed ride continues with two 64 m left turns, a 12 m strong floater at -0.95 g, 64 m and 60 m right turns, another 12 m floater at -0.95 g, two and a half tiles of straight, two 56 m left turns, a 10 m floater at -0.95 g, and 54 m right and left turns. The floaters after the first use -0.95 g because the train is slower by then and a stronger target keeps their delivered force near -0.94 g.

The ending is where the earlier attempts failed. After the last banked turn, the train is still doing 27 m/s, so two `Brakes` pieces take it to 23.3 m/s. `LevelToGentleUp`, two `OneTileGentleUp` and `GentleUpToLevel` climb to 24 m, and `LevelToGentleDown`, `OneTileGentleDown` and `LongGentleDownToLevel` come back down to 10 m, the station's height. Three automatically sized banked turns (38, 38 and 36 m), a 28 m left turn, a straight, a 36 m right turn and two straights then line the train up with the station, and `close` ends the ride. The result has 44 steps and 41 pieces.

The check passes under both physics configurations. In the construction test and under default physics respectively, excitement is 7.42 and 6.71, sustained maximum vertical force is 3.31 and 3.28 g, sustained airtime is -0.94 and -0.91 g, and peak lateral force is 0.44 and 0.14 g, with 7 airtime moments in both. The tightest margins are construction-test forward force (0.08 g of headroom, on the first drop) and the `WideSixtyDownToLevel` rated window (+0.99 m/s). The focused browser agreement test passed for this ride before it merged (TASK-156 notes); I did not rerun it for this guide.

## Giga drops on the hyper

A first drop of 90 m or more ends in `GigaSixtyDownToLevel`, which is solved for 3.5 g at a 43.5 m/s bottom speed and drops about 44 m by itself. Sky Harrier (`content/rides/sky-harrier.ride.json`) uses a 104 m crest, `WideLevelToSixtyDown`, two `SixtyDown` and the giga pullout: a 94 m drop that the eight-car train leaves at 42.5 m/s with 3.25 to 3.33 g in the pullout. The rear car reaches the pullout about 4.7 m/s faster than the front car, so its rated-window margin (about +0.3 m/s in the construction test) is the one to watch. `WideSixtyDownToLevel` stays the pullout for Horizon Hawk-sized drops of about 60 m.

## Where to look next

- Command code: `crates/design-tools/src/commands.rs` (parsing, `check`), `report.rs` (margins and headroom), `brake_report.rs`.
- Shaped elements: `crates/sim/src/track/shaped.rs`, `crates/sim/src/track/models.rs` (`ShapedProfile`), the plans `docs/superpowers/plans/2026-11-19-shaped-elements.md` and `docs/superpowers/plans/2026-11-21-global-limits-and-shaped-elements.md`, and TASK-156 / TASK-167.
- Design tool plan and friction notes: `docs/superpowers/plans/2026-11-18-agent-design-tools.md` and TASK-149.
- Browser agreement test: `web/e2e/rides/ride-agreement.ride.spec.ts`, which replays ride files through `web/e2e/player/place-circuit.ts`.
