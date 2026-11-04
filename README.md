# Theme Park Sim

A theme park builder that runs in the browser. You sculpt the land, lay paths and queues, place shops and flat rides, and build roller coasters piece by piece. Thousands of individual guests then walk the park, choose rides, queue in groups and remember how each ride felt.

![The park by day](https://github.com/No-Stream/theme-park-sim/releases/download/readme-images/park-day.png)

![The park at night](https://github.com/No-Stream/theme-park-sim/releases/download/readme-images/park-night.png)

*Screenshots from the work-in-progress build; they will be updated as the game develops.*

## What's in it

- **Coasters built from real track geometry.** Pieces are curves with continuous curvature, so trains ride smoothly. Physics runs at 200 Hz, with g-forces measured where the rider sits, rollbacks, chain lifts, launches with adjustable speed, and automatic block sections for several trains.
- **Three coaster models with their own pieces and limits.** One is a steel-on-wood hybrid in the spirit of RMC, one a launched coaster in the spirit of Intamin's accelerators, and one a wooden twister in the spirit of GCI. Inversions span classic (not heartlined) and modern (heartlined) styles.
- **Test runs with ratings and notes.** Each test gives excitement, intensity and nausea ratings, a speed and g-force overlay on the track, and near-miss events with supports, scenery, terrain and other track.
- **Guests as individuals.** Each guest has traits, a group, needs and a memory of the park. They make their own decisions, walk across levels and tunnels, and ride in the car whose forces they feel.
- **Landscape and building tools.** You can sculpt terrain and water, carve tunnels and caverns, and coaster track tunnels through hills automatically. There are queue paths, scenery sets, a day and night cycle with lamps and fireworks, and light and dark interface themes.
- **A deterministic simulation.** It is written in Rust, compiled to WebAssembly and run in a web worker. Whole play sessions can be exported and replayed exactly.

## Running it

You need Rust (the version is pinned in `rust-toolchain.toml`, with the `wasm32-unknown-unknown` target), Node.js and npm, and Chrome for the art pipeline and browser tests.

```sh
make install   # npm dependencies
make build     # simulation (WebAssembly), sprite atlas and web bundle
make serve     # dev server on http://127.0.0.1:5173
```

Tests: `make check` runs the fast tier, `make journeys` plays the game through the real UI, and `make ci` runs everything.

## Status

The game is in active development and not yet released. Saving and loading, sound, scenarios and a public build are still to come. `docs/status.md` describes what works today.

## License

MIT, see [LICENSE](LICENSE).
