# Theme Park Sim Roadmap

> **For agentic workers:** This is a project-level roadmap. Work comes from the Backlog.md tickets (`backlog task list --plain`). When you pick up a ticket, write its detailed test-first plan in `docs/superpowers/plans/`, then use superpowers:subagent-driven-development or superpowers:executing-plans to carry it out.

**Goal:** Build the free browser theme park game described in the design doc, proving the riskiest parts first.

**Architecture:** A deterministic Rust simulation runs in a web worker as WebAssembly, and also builds natively for tests and multi-thread proofs. A TypeScript main thread draws transferred state snapshots with a GPU renderer and hosts the interface. Track geometry is data, and art is vector drawing rendered into cached sprite textures.

**Tech Stack:** Rust, WebAssembly (wasm-bindgen), TypeScript, Vite, a custom instanced WebGL2 renderer (chosen by benchmark), Playwright, static hosting.

**Spec:** `docs/superpowers/specs/2026-11-02-theme-park-sim-design.md`

## Why this is a roadmap

Step-by-step plans for every subsystem now would be invented detail. The renderer benchmark, art proof and guest benchmark will change later decisions, so each ticket gets its detailed plan when it starts. Tickets carry their dependencies and acceptance criteria.

## Global Constraints

- Static files only, with no backend and no paid hosting.
- No crashes, breakdowns or maintenance chores. Failures are never punishing.
- Only physically plausible track is offered for each model.
- No real manufacturer or ride names in the game. Use playful hints such as "Rocky Road Construction" for RMC.
- Guests walk, never hop. Thoughts appear only in the click-in panel, with no speech bubbles in the park.
- Simulation results are identical at any thread count for the same seed and inputs.
- The browser build stays on stable Rust and one thread until a benchmark justifies a change.
- The renderer is the custom instanced WebGL2 renderer, which runs in every target browser.
- Performance targets: 10,000 guests at 60 frames per second on the owner's desktop GPU and at least 30 on a mid-range laptop, with maps of at least 512 by 512 tiles.
- Input code must not assume a mouse.

## Review Focus

These conditions are implied by the spec but easy to miss. Each is an acceptance criterion on the ticket named in parentheses.

- The player deletes a path, ride or track while guests or trains are on it. Guests and riders are moved somewhere sensible, and nothing freezes (world data model, guest walking, track building tool).
- The browser tab is hidden for several minutes. On return the game resumes smoothly, without a burst of catch-up ticks (project skeleton).
- The player's GPU or browser has no WebGPU. The game runs on WebGL2 with no action needed (renderer benchmark).
- A park has guests but no path to a ride, or a coaster has an unfinished circuit. Guests behave sensibly, and the player gets a friendly note (guest walking, testing overlay).
- Browser storage is full or cleared, or a save comes from an older version. The player gets a clear message and keeps their exported file (saving and loading).

## Order of work

Tickets are grouped into four milestones. Dependencies are recorded on the tickets, so independent tickets can run in parallel.

1. **Risk proofs:** project skeleton, multi-thread tick framework, renderer benchmark and art proof.
2. **First playable park:** world, camera, one coaster model, guests, stalls, flat rides and the guest panel.
3. **Full creative toolset:** terrain and underground, theming, day-night cycle and effects, the other two coaster models, automatic block sections, proximity events, saving and the scale check.
4. **Public release and later:** deployment, then scenarios, sound, touch input and a possible desktop wrapper.
