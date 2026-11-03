# Project status

Last updated 2026-11-03. The design is in `docs/superpowers/specs/2026-11-02-theme-park-sim-design.md`, the roadmap in `docs/superpowers/plans/2026-11-02-theme-park-sim-roadmap.md`, and the live task list in Backlog.md (`backlog task list --plain`).

## What works

The game runs in the browser from `make build` and `make serve`, with a deterministic Rust simulation in a web worker and a custom instanced WebGL2 renderer. A toolbar switches between Watch, Coaster, Paths, Landscape, Facilities and Scenery, each with one panel in the print-modern art style. The world is a layered isometric map (512 by 512 supported) with sculpted terrain, water, tunnels, caverns and an underground view. Paths are laid by dragging, follow slopes, store explicit edge connections and include queue paths that belong to a specific ride. Facilities cover the park entrance, food and drink stalls, restrooms, a souvenir shop, a carousel and a ferris wheel, with prices and revenue tracked. Scenery comes in Garden and Woodland sets, including lamp posts that light up at night.

Coasters are built piece by piece in the builder panel, with ink icons for each piece, steppers, keyboard shortcuts, a camera that follows the open end, and a test run. Three models exist, each with its own piece list and a playful manufacturer name: Timber Tumbler by Rocky Road Construction (inspired by RMC hybrids), Velocity Viper by Impulse Engineering (Intamin launch coasters) and Briar Blitz by Great Coaster Workshop (GCI wooden twisters, with a steel-structure loop allowed and purely wooden loops refused). Track pieces are curves with continuous curvature across joins and mirrored right-hand versions, and every banked turn leans into its curve. Physics runs at 200 Hz with rider-frame g-forces, rollback recovery and launches. Test runs give excitement, intensity and nausea ratings, friendly notes, a speed and g-force ribbon on the track, and proximity events (near-misses, headchoppers, dueling, tunnels, terrain hugging). Automatic block sections let several trains run without collisions.

Guests are individuals with correlated traits, groups and relationships, needs, personal knowledge of the park, utility-based decisions, bounded memories and thoughts. They walk shared flow fields across levels, queue, board coasters in groups, feel their car's forces and remember the ride. Clicking any guest, including a rider, opens their details. Sprites match each guest's age and appearance from a cast of 18 designed people, including Woody the thoosie. The day-night cycle lasts 24 real minutes, with a print-style colour grade, night lights, fireworks, crest confetti and a sky.

## Performance so far

On the owner's RTX 5090, the renderer drew 10,000 animated people with terrain and track in about 2.6 ms of GPU work per frame. In the browser worker on one thread, 10,000 guests take about 7 ms per simulation tick; natively on 4 threads, about 2 ms, with identical results at any thread count. The full-scale performance check with a realistic large park on real hardware has not been done yet (ticket TASK-26).

## Known gaps and open items

- The owner's first playtest (2026-11-03) found: no way back to level from a slope on Velocity Viper (TASK-42, fix in progress); tools need a second click before they work (TASK-45); the path tool places one tile at a time (TASK-46); buildings drawn cut off (TASK-47); food facilities refused with "missing access" (TASK-48); and the park resets when the dev server reloads (TASK-49, after saving exists). TASK-46 and TASK-48 are probably regressions from the explicit path edge connections.
- The owner asked for a dark interface for OLED screens (TASK-44) and broader, more realistic coaster vocabularies with unique signatures and manufacturer styles (TASK-43; direction recorded in section 3 of the design doc).
- Owner review is still pending for the night effects in a dense park, and the owner's decision on Woody's shirt (keep the lift hill and loop, or one bold loop).
- Four coaster browser tests time out when the whole suite runs in parallel; a root-cause fix is in progress. The scenery and block sections tickets close once the gate is green.
- The full gate takes about 7 minutes; triage into fast and slow tiers is TASK-41, with a per-test plan in the rendering, art and tests review.
- Four code organisation reviews are in `docs/reviews/`, turned into refactor tickets TASK-50 to TASK-61 in the Code health milestone. The biggest are moving the park tick and cross-object rules out of the WebAssembly bridge into a `Park` type (TASK-54), a typed worker protocol (TASK-57), and restructuring `main.ts` around a shared tool interface (TASK-58).
- Not started: saving and loading, the full-scale performance check, public deployment, sound, scenarios, touch input and a desktop wrapper.
- Simplifications worth knowing: sandbox admission is free and ride loading is instant; building and ride clearance assume heights of 4 m and 12 m; automatic block placement searches generated brake positions within a budget, and throughput excludes loading time.
