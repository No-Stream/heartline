# Project status

Last updated 2026-11-03. The design is in `docs/superpowers/specs/2026-11-02-theme-park-sim-design.md`, the roadmap in `docs/superpowers/plans/2026-11-02-theme-park-sim-roadmap.md`, and the live task list in Backlog.md (`backlog task list --plain`).

## What works

The game runs in the browser from `make build` and `make serve`, with a deterministic Rust simulation in a web worker and a custom instanced WebGL2 renderer. A toolbar switches between Watch, Coaster, Paths, Landscape, Facilities and Scenery, each with one panel in the print-modern art style. The world is a layered isometric map (512 by 512 supported) with sculpted terrain, water, tunnels, caverns and an underground view. Paths are laid by dragging, follow slopes, store explicit edge connections and include queue paths that belong to a specific ride. Facilities cover the park entrance, food and drink stalls, restrooms, a souvenir shop, a carousel and a ferris wheel, with prices and revenue tracked. Scenery comes in Garden and Woodland sets, including lamp posts that light up at night.

Coasters are built piece by piece in the builder panel, with ink icons for each piece, steppers, keyboard shortcuts, a camera that follows the open end, and a test run. Three models exist, each with its own piece list and a playful manufacturer name: Timber Tumbler by Rocky Road Construction (inspired by RMC hybrids), Velocity Viper by Impulse Engineering (Intamin launch coasters) and Briar Blitz by Great Coaster Workshop (GCI wooden twisters, with a steel-structure loop allowed and purely wooden loops refused). Track pieces are curves with continuous curvature across joins and mirrored right-hand versions, and every banked turn leans into its curve. Physics runs at 200 Hz with rider-frame g-forces, rollback recovery and launches. Test runs give excitement, intensity and nausea ratings, friendly notes, a speed and g-force ribbon on the track, and proximity events (near-misses, headchoppers, dueling, tunnels, terrain hugging). Automatic block sections let several trains run without collisions.

Guests are individuals with correlated traits, groups and relationships, needs, personal knowledge of the park, utility-based decisions, bounded memories and thoughts. They walk shared flow fields across levels, queue, board coasters in groups, feel their car's forces and remember the ride. Clicking any guest, including a rider, opens their details. Sprites match each guest's age and appearance from a cast of 18 designed people, including Woody the thoosie. The day-night cycle lasts 24 real minutes, with a print-style colour grade, night lights, fireworks, crest confetti and a sky.

## Performance so far

On the owner's RTX 5090, the renderer drew 10,000 animated people with terrain and track in about 2.6 ms of GPU work per frame. In the browser worker on one thread, 10,000 guests take about 7 ms per simulation tick; natively on 4 threads, about 2 ms, with identical results at any thread count. The full-scale performance check with a realistic large park on real hardware has not been done yet (ticket TASK-26).

## Known gaps and open items

- Owner review is pending for the coaster building tool, its polish, and the night effects in a dense park.
- Owner decision pending on Woody's shirt: keep the lift hill and loop, or switch to one bold loop.
- Four coaster browser tests time out when the whole suite runs in parallel; a root-cause fix is in progress. The scenery and block sections tickets close once the gate is green.
- The full gate takes about 7 minutes; triage into fast and slow tiers is ticket TASK-41.
- Code organisation reviews from four reviewers are in `docs/reviews/` and will become refactor tickets.
- Not started: saving and loading, the full-scale performance check, public deployment, sound, scenarios, touch input and a desktop wrapper.
- Simplifications worth knowing: sandbox admission is free and ride loading is instant; building and ride clearance assume heights of 4 m and 12 m; automatic block placement searches generated brake positions within a budget, and throughput excludes loading time.
