# Theme park sim: design

Date: 2026-11-02. Status: reviewed by the owner, whose changes are included.

Each decision below is labeled **Owner** (the owner said it) or **Designer's call** (proposed during design and open to rejection). Sections 11 and 12 list what is left to the builder and what is unverified.

## 1. Intent

This is a free, for-fun theme park and roller coaster game in the spirit of RollerCoaster Tycoon 2 (RCT2). It keeps the 2D isometric look and piece-by-piece coaster building, and it simulates every guest as an individual. The owner enjoys open-ended creative play: coming up with a coaster concept, stacking coasters so they interact, finding fun layouts and threading track underground. Delight is the main goal, and realism in physics and guest behavior serves that delight.

Later 3D park builders lost two things the owner valued in RCT2. Their fully free spline editors gave so much flexibility that building became less delightful than snapping pieces together, and their 3D art lost RCT2's charm. RCT2 also has limits worth lifting, including too few coaster types, too few track pieces and too little flexibility. This game aims for the balance between those: a bigger vocabulary of pieces, each coaster model feeling like its real counterpart, and a stylized art direction that uses modern effects freely.

There is no plan to make money, and the owner does not want to pay a hosting bill. Success means the game is delightful to build in, runs with a large and realistically sized park, and anyone can play it by opening a link.

## 2. Goals, non-goals and anti-goals

Goals (**Owner** unless noted):

- Piece-based coaster construction on a grid, with many more pieces than RCT2.
- Many coaster models, each based on a real one, with limits inspired by that real coaster.
- Realistic coaster physics, kept subordinate to fun.
- Every guest modeled individually, with traits, needs and relationships, and thoughts readable by clicking on them.
- Flexible terrain, tunnels, underground sections and theming.
- At least RCT2-sized parks with thousands of guests, and preferably larger.
- Pure sandbox first, with scenarios added later.
- A whimsical, upbeat stylized look with flashy effects and a day-night cycle.
- Coasters that interact with each other in ways riders notice, such as near-misses and dueling (**Designer's call**, from the owner's enjoyment of stacking coasters).

Non-goals for the first versions (**Designer's call** unless noted):

- Multiplayer, accounts, servers or cloud saves.
- Mobile support. The owner finds it interesting but harder than mouse and keyboard. It is out of scope for now, but input code must not assume a mouse, so touch can be added later.
- A native desktop app. A desktop wrapper is possible later if the web version proves limiting (**Owner**: acceptable if it easily supports Windows, Linux and macOS).
- Mod support.

Anti-goals (**Owner**), which would make the game a failure even if it worked technically:

- Punishing the player. There are no crashes, and no worrying about disasters.
- Maintenance chores, including ride breakdowns.
- A fully free spline editor as the building experience.
- Generic realistic 3D art, or a melancholy mood. The World of Goo reference is about playful whimsy and should not be directly imitated.
- Fussy manual block zone setup.
- Offering track that is not physically plausible. See section 4.

## 3. Coaster construction

The player builds with pieces snapped to a grid, one at a time, with the same rhythm as RCT2 (**Owner**). Pieces are more varied than RCT2's: more turn radii, bank angles and slope steps, half-tile offsets, and signature elements per model (the detailed list is **Designer's call**, and the catalog is the builder's to grow).

Internally, track is more flexible than the pieces shown to the player (**Owner**). Each piece is defined by a continuous 3D curve plus a roll profile, with its entry and exit states on a grid of allowed positions, headings, slopes and banks. Physics, rendering and support generation all use the curve and never rely on hand-made per-piece art (**Designer's call**). This means new pieces are data, and hidden flexibility can be exposed later without changing the simulation.

Each model's limits are a data definition: allowed pieces, minimum radii, inversion rules, lift or launch types, supports, train configurations and materials (**Designer's call**). A piece is offered only when it is plausible for that model, so the game never needs a warning label. For example, a purely wooden vertical loop is not offered, but a wooden coaster with a steel-structure loop is, in the spirit of Son of Beast (**Owner**).

Pieces that are real but beyond what a model's real-world examples do should appear as normal choices, and the model card can note them as "probably physically possible" in its description (**Owner** for the principle, **Designer's call** for the wording and placement).

Supports generate automatically from the curve and terrain, and they are styled by model (**Designer's call**). The player can route track through tunnels, along cliffs and over other rides (**Owner**).

Clearance is checked against the real swept shape of the train around the curve, with a generous tolerance so that tight near-misses remain possible (**Designer's call**). Piece data carries what this check needs, and the near-miss and headchopper events in section 4 share it. If a generated support would land on a path, ride or buried tunnel, it reroutes, and the piece is refused only when no route exists (**Designer's call**).

### Initial coaster models

Launch with three models that have very different building feel, using real traits as the source of limits (**Owner** named these three, **Designer's call** to start with them). The builder should verify the details of each model against real rides when writing the data.

In the game, manufacturers and models never use real names, for trademark reasons. They get playful names that hint at their real inspiration, in the way the GTA series names its vehicles, such as "Rocky Road Construction" for RMC (**Owner**). The table below uses real names only as design references.

| Model | Real-world basis | What makes it distinct |
|---|---|---|
| Steel-track hybrid | RMC I-Box: all-steel box track that can be built on a wooden structure (Steel Vengeance has a chain lift, 4 inversions and 24-rider trains) | Extreme banking and inversions, and the steep drops associated with the type |
| Multi-launch steel | Intamin LSM launch coasters: Taron (2 launches, 117 km/h, no inversions, 16 riders per train) and VelociCoaster (2 launches, 113 km/h, 4 inversions, 24 riders per train) | Launches can be placed mid-course, and terrain hugging suits them |
| Wooden twister | GCI: wooden structure and articulated Millennium Flyer trains. Mystic Timbers is a chain-lift example with no inversions | Tight, twisting layouts through dense structure. Steel-structure inversions are allowed as a plausible extension |

## 4. Physics and feedback

Trains are simulated along the track curve using real forces at a fixed time step: gravity, rolling and air drag, lifts, launches and brakes (**Designer's call**, within **Owner**'s realistic but fun-first rule). The simulation records g-forces, speed and airtime along the track. The player sees these in testing as a friendly overlay along the track, and riders feel them through the guest simulation.

Failures are never disasters (**Owner**). Trains do not crash. A train that would not reach the next section shows up in testing as a friendly note, and a train that stalls in play simply rolls back and is recovered without penalty (rollback handling is **Designer's call**). Forces that would be brutal in real life show up as low rider comfort and ride ratings, and this may include more nausea (**Designer's call**).

Physics is advisory. Ride ratings and guest enjoyment reward good design, but the game never blocks a plausible build because it would be uncomfortable (**Designer's call**).

Rides interact through the shared world. Tracked proximity events (near-misses, headchoppers, dueling sections, tunnels and terrain hugging) feed the ride rating and the riders' memories (**Designer's call**).

## 5. Block sections and operations

Block sections are automatic (**Owner** wanted them delightful and not fussy, the design below is **Designer's call**). The game finds suitable positions, places blocks when the player asks and shows how many trains the layout can run. Experts can move blocks by hand. Trains that would otherwise collide wait, so a poor block layout costs only capacity.

No maintenance or breakdowns exist (**Owner**). Staff are out of scope for the first version. If they appear later they should add flavor and must not become a chore (**Designer's call**).

## 6. Guests

Every guest is an individual whose full state exists at all times (**Owner** for individual modeling, **Designer's call** for the structure). Statistical models generate each guest's traits at spawn, including taste for intensity, favorite ride types, how much they care about theming, spending level and preferred themes (**Owner**). Guests have the usual needs, such as hunger, thirst, restrooms and nausea, and relationships such as parent and child or spouses (**Owner**).

Groups walk and decide together. A family might split because a child is too short for a coaster (**Designer's call**). Guests choose rides using what they have personally seen, experienced and heard from their group, with no access to a global list of rides (**Designer's call**). They keep memories of rides, which feed their thoughts and opinions.

Thoughts are visible only when the player clicks a guest, in a details panel. Speech bubbles do not appear in the park (**Owner**). Guests walk and mill around like people, and do not hop (**Owner**). The panel shows the guest's name, relationships, thoughts, traits, needs and memories (**Designer's call** for the exact contents).

### Visits and time

A visit should be realistic in how much a guest does, about as many rides and stops as a real day at a park, even if that spans several day-night cycles (**Owner**). A day-night cycle lasts about 20 to 30 real minutes, because that pace is more fun (**Owner**). Coasters run at real speed, so a realistic visit takes more than one cycle.

To make this work, visit length is measured in what a guest does and is not tied to the clock (**Designer's call**). The park never closes. Guests arrive and leave continuously, and they keep going through the night, so an individual guest may see two or three sunsets in one visit. Night changes mood and lighting and can lightly shift preferences, such as favoring lit rides and fireworks, but it does not empty the park. Guests may return for later visits with their memories, so memory data must stay compact and bounded (**Designer's call**).

Guest numbers follow how appealing the park is, and sandbox also has a slider to override them so the player can fill a park at any time (**Owner**).

### Walking and crowds

Paths stay on the tile grid, and wide plazas are adjacent path tiles (**Designer's call**). Queues are built by the player from path pieces, as in RCT2 (**Owner**). Guests avoid each other only loosely, enough to look like a crowd without the cost of full collision avoidance for 10,000 guests (**Designer's call**). The guest benchmark must include this avoidance, so it measures the real workload.

Guest simulation should stay understandable. Exact trait distributions and need rates are for the builder to tune. The bar is that clicking any guest finds something coherent and often entertaining, and that park-wide crowd behavior responds visibly to how the park is built.

## 7. World and theming

The world is an isometric grid with sculpted terrain, water, paths, scenery and buildings (**Owner**). Terrain supports hills, cliffs, tunnels and caverns, and guests as well as track can go underground, including paths, stations and shops in carved spaces (**Owner**). The terrain data model must be layered from the start, and pathfinding must handle several levels, even if caverns ship after surface terrain (**Designer's call**, because a plain height map would be expensive to replace). An underground view mode is needed so buried work remains visible and editable (**Designer's call**).

The first playable park includes a park entrance, food stalls, restrooms and a small set of flat rides such as a carousel and a ferris wheel (**Owner**). Flat rides need their own simple framework outside the track curve system (**Designer's call**). Four rotated camera views and smooth zoom are expected, as in RCT2 plus modern zoom (**Designer's call**).

Theming is a first-class building activity with sets of scenery and building pieces (**Owner**). The first theme sets are for the builder to choose, and they should be few and charming. Guests' theme preferences and theming sensitivity connect scenery to enjoyment.

## 8. Progression

The game starts as pure sandbox, with scenarios added later (**Owner**). Sandbox has no money pressure, so guest spending can start as simulated behavior with light economic effect (**Designer's call**). The simulation should still track money and prices, because scenarios will need them.

## 9. Art direction

The look is 2D isometric and stylized, whimsical and upbeat (**Owner**). It sits between the "wonky storybook" and "odd little world" mockups, which means hand-inked outlines, slightly bent geometry and playful shapes, and has no melancholy. People look like small people. The reference is `docs/mockups/art-playful-v2.html`, which the owner reacted to positively. It shows mood and is not a final art standard or a code base. Open it through a local web server, as it was written as a fragment for the brainstorming page.

After reviewing the first art proof, the owner found the original bright "candy" look too childlike and fairy-tale, and asked for something more artsy and grown-up while staying whimsical (**Owner**). From four themes (candy, riso, pen-and-wash, mid-century) and a blend, they chose **print-modern**: a riso-style print look with a few overprinted inks and a quiet hand-inked line, leaning toward mid-century illustration with geometric trees, clear lit and shaded planes, flat colour and readable adult proportions (**Owner**). They asked for it to lean slightly closer to mid-century with a little more colour. Candy-era details such as lollipop trees, smiling suns, polka dots and hearts do not belong in it. The cast includes a "thoosie", a coaster enthusiast in a coaster T-shirt, cargo shorts and a glasses strap, as an inside joke (**Owner**). Themes are configuration in `web/art/generators/themes/`, so the other looks remain available.

Details from that mockup that the builder should carry forward (**Designer's call** except where noted): deep plum ink, candy colors, curly lollipop trees, faint line shimmer as in hand-drawn animation, and paper grain. The owner specifically enjoyed the day-night cycle and effects, so those are required: moving sun, lamps and ride lights, fireworks, and particle effects such as confetti at a coaster's crest (**Owner**).

Modern techniques are welcome, since the look need not be limited by what isometric games did 20 years ago (**Owner**). Effects must not make dense parks hard to read (**Designer's call**).

Art is produced mainly by code. Track, supports and structures are generated from their curves, and scenery, people and buildings come from procedural drawing code rendered into cached sprite textures (**Designer's call**). The owner is confident this can look good, and AI-generated vector drawing is the expected way to author these pieces (**Owner**). It is still the largest art effort in the project, so the builder should produce an early proof of people and buildings for the owner to judge by eye, while coaster and guest work continue alongside it. No fallback art source is planned. The sprite pipeline should take any vector drawing as input, so a drawing can be replaced without code changes (**Designer's call**).

## 10. Technical architecture

All of this section is **Designer's call**, except that it is a web app first and needs no paid hosting (**Owner**). Rust was discussed with the owner, who is happy with it.

**Hosting.** The game is static files, run entirely on the player's machine, with no backend. Saves live in browser storage, and players can export them to a file. Cloudflare Pages and Netlify are free and can set custom headers, which matters only if threads are added later (see below).

**Simulation.** The simulation is Rust compiled to WebAssembly and runs in a dedicated web worker at a fixed tick rate. It is deterministic given a seed and input sequence, which supports testing and replay. Guest data is stored in compact arrays. Walking runs for every guest every tick, expensive decisions are spread across ticks, and crowds share pathfinding data such as flow fields. No guest is ever simplified or faked to save time.

**Threads.** The simulation is designed for multiple threads from the start, because one thread may not scale to the park sizes we want (**Owner**). Each tick is made of explicit phases that read shared state and write to per-partition output, and results merge in a fixed order, so the outcome is identical at any thread count (**Designer's call**). The simulation also builds as native Rust, where it runs on two or more threads as proof that the design works. A test checks that one-thread and multi-thread runs of the same seed and inputs give identical state, and the benchmarks report how speed changes with thread count.

The browser build starts on one thread (**Owner**), which needs no special headers and stays on stable Rust. Multi-threaded Rust in WebAssembly still requires pinned nightly Rust, rebuilding the standard library and cross-origin isolation headers (COOP and COEP), and GitHub Pages cannot set those headers without a service-worker workaround. Threads move to the browser when benchmarks show the game needs them or the toolchain becomes easier, and this should not require restructuring the simulation. If that happens, hosting should use Cloudflare Pages or Netlify. SIMD via `simd128` can be used where it helps.

**Rendering.** The renderer runs on the main thread and draws from state snapshots transferred by the worker, interpolating between ticks. WebGPU support is still incomplete on Linux (Chrome covers only some GPUs by default) and in Firefox on Linux (Nightly only as of May 2026), so WebGL2 is required. The renderer benchmark (docs/renderer-benchmark.md) settled the choice: on the owner's RTX 5090, a thin custom instanced renderer drew 10,000 animated people with terrain and track in 2.6 ms per frame on WebGL2, against about 45 ms for PixiJS, which spent its time sorting sprites on the CPU. The game therefore uses the custom renderer on WebGL2 only. WebGPU can be added later if it brings a measured benefit (**Designer's call**, made after the benchmark).

**Depth sorting.** Isometric depth sorting with long overlapping track is hard and easy to get subtly wrong. The sketch mockups draw back to front using a simple depth key and show visible errors. The renderer needs a real solution, likely writing depth values into a depth buffer per sprite, and this should be part of the early renderer benchmark.

**Frontend.** TypeScript handles the renderer and game interface. It talks to the worker through a small typed message protocol. The interface's visual design should match the art direction.

**Testing.** Development follows test-driven practice. Physics has tests against energy conservation and known ride values, the simulation has deterministic replay tests and performance benchmarks with thousands of guests, and piece definitions have automated checks that entries and exits connect. End-to-end browser tests build a small coaster and check that a train completes a circuit. Screenshot tests catch rendering regressions.

**Performance targets.**

- 10,000 guests with smooth play (60 frames per second on the owner's desktop GPU, and at least 30 on a mid-range laptop).
- Largest map at least 512 by 512 tiles, which is four times the area of RCT2's 256 by 256 limit. OpenRCT2 goes to 999 usable tiles, which is a reference point and not a requirement.
- RCT2's total limit was 10,000 entities shared between guests and vehicles, so this target goes well beyond RCT2 guest counts in practice.

These numbers are starting targets. They should be tested from the first prototype and revised with the owner if evidence says so.

## 11. Suggested order of work

The builder owns the plan, but the riskiest things should be proved first:

1. Toolchain and walking skeleton: Rust to WebAssembly in a worker, renderer on screen and a deployable static build.
2. Rendering and art proof: renderer benchmark with depth sorting, plus code-drawn people and buildings the owner can judge.
3. Coaster core: track pieces as data, physics, building tools for one model and a train completing a circuit.
4. Guest core: thousands of walking guests on paths with needs, decisions, groups, queues and a click-in panel, plus the entrance, stalls, restrooms and flat rides they use, benchmarked at scale.
5. World: layered terrain, tunnels, underground paths and view, scenery and day-night cycle. The layered data model itself must be in place before the guest core, since pathfinding depends on it.
6. The other two coaster models, automatic block sections and proximity events.
7. Save and load, public deployment and polish.
8. Later: scenarios, more models, money pressure in scenarios, then possibly a desktop wrapper and touch input.

Left to the builder: the piece catalog, tick rate, trait distributions, the exact guest panel layout, theme set choices, sound (see section 12) and code structure.

## 12. Open items and unverified facts

- **Audio.** Audio has not been discussed. The designer's assumption is that sound matters to delight, and that simple generated or freely licensed sound can follow the core game.
- **WebGPU status.** The WebGPU details above come from a status page last edited around May 2026. Firefox on Linux may have shipped since. Check again before implementation.
- **COOP and COEP headers.** Header behavior has not been tested on any host. It matters only if threads are added.
- **Renderer numbers.** The PixiJS numbers are the project's own published benchmark, which does not state which renderer backend it used.
- **GCI norms.** General GCI lift and inversion norms were not confirmed beyond specific rides. Check them when writing model data.
