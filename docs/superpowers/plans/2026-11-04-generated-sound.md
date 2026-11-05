# Generated sound

> **For agentic workers:** Each work item below becomes one Backlog ticket under TASK-28. Pick one up, write its test-first steps if it needs them, and carry it out with superpowers:subagent-driven-development or superpowers:executing-plans. Items are ordered. Respect the dependencies listed on each. Vitest runs in Node without a DOM, so anything that touches `AudioContext` or `document` is tested in Playwright; keep the mapping from game state to sound parameters in pure functions so Vitest can cover it.

**Goal:** Give the park sound that follows what the player sees: trains roar and clack, riders scream on drops, crowds murmur, the day has birds and the night has crickets, and building gives short audible feedback. Everything is synthesised in the browser with the Web Audio API; no recordings in the first version. The player can set the volume and mute, and the choice survives a reload.

**Architecture:** The audio engine lives on the main thread beside the renderer, in `web/src/audio/`. It reads the same snapshots the renderer draws and the same camera, so it needs almost nothing new from the worker: one small per-train record (speed, piece kind, vertical and lateral g) is added to the existing `snapshot` message. A pure mapping layer turns a snapshot, the camera and the time of day into a `SoundFrame` (a plain object of per-emitter parameters and one-shot events); the engine applies a `SoundFrame` to a fixed graph of native Web Audio nodes. The engine takes a `BaseAudioContext`, so the same code renders into an `OfflineAudioContext` in tests. Voices are capped and allocated by audibility, so many trains and thousands of guests cost the same as a few.

**Tech stack:** TypeScript, Web Audio API (native nodes only: `OscillatorNode`, `AudioBufferSourceNode`, `BiquadFilterNode`, `GainNode`, `StereoPannerNode`, `DynamicsCompressorNode`), Rust and wasm-bindgen for the per-train record, Vitest, Playwright.

**Spec:** `docs/superpowers/specs/2026-11-02-theme-park-sim-design.md` section 12 (audio was left open). Ticket TASK-28. Related: TASK-95 (force-limited inversions, heart-based rider forces on branch `inversion-forces`), TASK-105 (g-force and speed overlay while building), TASK-59 (per-frame scene cost).

## What exists today

- The worker sends a `snapshot` message about 30 times a second (`web/src/worker.ts` `wake`), one per wakeup. At game speed ×N it runs N ticks per wakeup, so the snapshot rate stays about 30 Hz and each snapshot covers N ticks. The message carries guests (`guests`, 7 floats each: x, y, height, heading x and y, distance, moving), `trainPoses` for the demo coaster and `constructionTrainPoses` for the built coaster (11 floats per car: global car index, position in tiles, tangent, up, occupied seat mask; `BRIDGE_LAYOUTS.TRAIN_POSE` in `web/src/bridge-layouts.ts`). The built coaster can run several trains under block operation; the global car index is `train_id × cars + car` (`rider_car_index` in `crates/sim-wasm/src/lib.rs`).
- `main.ts` keeps `camera` (`centerX`, `centerY`, `zoom` 0.5–4 in CSS pixels, `rotation` 0–3), `viewport`, `dayPhase` (0 at midnight, 0.5 at noon, daytime 0.25–0.75), `daySpeed`, the game speed from `dayClock.cycleGameSpeed()`, and the interpolated `renderFrame` from `RenderClock`. `projectWorld` in `web/src/camera.ts` maps a world point to canvas pixels.
- Crest detection already exists: `detectCrest` in `web/src/render/effects.ts` produces `crestBursts` from train poses, used for confetti. Builder events exist: `constructionEvent` in `web/src/construction-effects.ts` recognises a placed piece and a closed circuit from consecutive `ConstructionState`s; refusals arrive as `ConstructionState.message`; path, facility, scenery and terrain tools receive result messages with `success` flags. `ConstructionState.track[].kind` is `"Track" | "Lift" | "Launch" | "Brakes" | "Station"` for the built coaster, but the demo coaster's piece kinds never reach the client.
- Physics: `Train` in `crates/sim/src/physics/mod.rs` has `speed()`, `distance()`, and `rider_g(piece, local, speed, acceleration)` computes vertical, lateral and longitudinal g for one car. `circuit.locate(distance)` gives the piece index and local distance. The `inversion-forces` branch moves `rider_g` to the heart line; the accessor below calls whichever `rider_g` exists.
- Persisted preferences follow `web/src/units-setting.ts` and `web/src/theme.ts`: a `localStorage` key `theme-park-sim.<name>`, reads and writes wrapped so only `DOMException` is swallowed, a button in the game bar (`web/index.html`), an `InputAction` dispatched through `main.ts` `dispatch`, and the setting object applying the change. Key bindings live in `builderShortcuts` in `web/src/input/bindings.ts`; `m` is unused.
- Tests: `npm test` runs Vitest in Node (no DOM). Browser tests run through `npm run e2e` with Chrome, `@fast` ones in `make check`, with `window.gameTest` hooks (`web/src/game-test.ts`) as the test surface. Vite bundles the worker through `new URL(..., import.meta.url)`; whether it bundles an `AudioWorklet` module the same way has not been verified, which is one reason the first version uses native nodes only.
- No audio code exists anywhere in `web/src`.

## Design

### Data the engine needs, and how it gets it

The engine runs once per rendered frame from `frame()` in `main.ts`, after `composeScene`, with:

- `renderCurrent` (the snapshot the frame draws) and `renderFrame.interpolation`; the engine uses the snapshot's positions without interpolation, since parameter smoothing in the audio graph hides 33 ms steps;
- `camera`, `viewport`, `dayPhase`, the game speed, `world.coasterTrain` and `construction?.trainLayout` (for the material), `crestBursts`;
- events collected since the last frame: builder events from `constructionEffects.observe`, tool failures from the result messages, and the audio setting.

One addition to the worker protocol: `trainStates: Float32Array` and `constructionTrainStates: Float32Array` on the `snapshot` message, with a new `BRIDGE_LAYOUTS.TRAIN_STATE = ["train_index", "speed", "piece_kind", "vertical_g", "lateral_g", "occupied"]` record per train, where `piece_kind` is 0 Track, 1 Lift, 2 Launch, 3 Brakes, 4 Station for the piece under the lead car, the g values are the lead car's rider forces at the moment of the snapshot, and `occupied` is 1 when any seat is taken. That is six floats per train, next to the 11 floats the poses already send per car, so the protocol does not grow in any way that matters. The train index matches the `train_id` encoded in the pose car index, so the engine can pair a state with its lead car's position (the pose with `car_index % cars === 0`). The ride overlay work (TASK-105) and the force work (TASK-95) may add the same numbers; if a per-train accessor lands first, reuse it and keep the wire layout above.

Nothing else is added. Guest positions are already in the snapshot; the crowd voice counts them. Piece kinds for the built coaster are in `ConstructionState.track` but the engine uses the per-train `piece_kind` for both coasters, so it does not need them.

### Engine layout

```
web/src/audio/
  engine.ts        createAudioEngine(context, options): the node graph, voice pools, scheduler, apply(frame)
  mapping.ts       soundFrame(input): pure; snapshot + camera + day + events -> SoundFrame
  voices.ts        pure voice allocation and level-of-detail (which trains get a voice)
  synth/           one file per sound family, each exporting a factory that builds its nodes on a BaseAudioContext
    noise.ts       shared looped noise buffers (white, pink) made once per context
    train.ts       roar (steel and wooden), chain clack and motor, launch, brake, station
    screams.ts     rider scream groups
    crowd.ts       murmur
    ambience.ts    wind, birds, crickets
    ui.ts          place, undo, error, finish
web/src/sound-setting.ts   the game bar control and persisted preference
```

The master chain is `voices -> master GainNode -> DynamicsCompressorNode -> destination`. The master gain holds the player's volume (perceptual curve: `gain = volume²`), and mute sets it to 0 and suspends the context. Every continuous parameter is set with `setTargetAtTime(value, now, 0.05)` so a 30 Hz update never clicks; one-shots are scheduled on the context clock with a 150 ms lookahead from the frame's `context.currentTime`.

`SoundFrame` is a plain object:

```ts
interface SoundFrame {
  trains: TrainSound[];           // at most MAX_TRAIN_VOICES, sorted by audibility
  screams: ScreamEvent[];         // new this frame
  crowd: { level: number; pan: number };
  ambience: { day: number; night: number; distance: number };
  ui: UiEvent[];                  // "place" | "undo" | "error" | "finish"
  eventsMuted: boolean;           // true above game speed ×4
}
interface TrainSound {
  train: number; pan: number; audibility: number; speed: number; material: "steel" | "wood" | "hybrid";
  onLift: boolean; clackRate: number; launch: boolean; brake: number; depart: boolean; arrive: boolean;
}
```

`mapping.ts` builds it from `MappingInput` and the previous frame's per-train memory (last piece kind, last speed, last g, scream cooldown), returning both the frame and the next memory, the same shape as `detectCrest` returns its tracker. All thresholds live in one `SOUND_TUNING` constant object at the top of `mapping.ts` so playtest tuning is one diff.

### Spatialisation

The camera is isometric, so sound is positioned in screen space. For a world point `p`, `projectWorld(camera, viewport, p)` gives canvas pixels. Then:

- `pan = clamp((x − width/2) / (width/2), −1, 1) × 0.8` into a `StereoPannerNode`;
- `audibility = zoomGain × distanceGain`, with `distanceGain = 1 / (1 + (d / R)²)` where `d` is the distance from the screen centre in CSS pixels divided by `camera.zoom` (so it is a world distance) and `R` is 220 world pixels; off-screen sources keep attenuating smoothly rather than cutting;
- `zoomGain = clamp(camera.zoom / 2, 0.35, 1.2)`: close zoom brings detail up, far zoom brings it down; ambience uses the inverse so the park sounds distant when zoomed out;
- a per-voice `BiquadFilterNode` lowpass whose cutoff falls with `d` (8 kHz on screen, down to 1.2 kHz far off screen) stands in for air absorption.

Rotation needs no special handling, since `projectWorld` already turns with the camera. No `PannerNode` or HRTF: stereo pan plus distance gain is enough for a 2D view and is much cheaper.

### Voice limiting and level of detail

- Trains: at most `MAX_TRAIN_VOICES = 4` train voices (roar, clack, motor, launch, brake and station nodes per voice, built once and reused). `voices.ts` ranks trains by audibility each frame and assigns voices to the top four; a train below `0.02` audibility gets none and its voice is released (gain ramped to 0, nodes kept). Assignment is sticky: a train keeps its voice while its audibility is within 10% of a challenger, so voices do not flap between two trains at the screen edge.
- Screams: a pool of `MAX_SCREAM_GROUPS = 3` groups; a new scream takes a free group or the oldest one. One scream per train per `SCREAM_COOLDOWN = 2.5 s`.
- Crowd: one voice, driven by counts, never per guest. The guest scan (position, audibility weight, sum) runs every fourth frame; at 10,000 guests it is one pass over 70,000 floats, well under a millisecond.
- Ambience: one voice. UI: a pool of four one-shot slots.
- The live node count is bounded (about 80) and does not depend on park size.

### Game speed and hidden tab

Sound follows what the player sees: a train at 2 m/s on a lift at ×4 crosses four times the chain per second, so the clack rate is `speed × gameSpeed / CHAIN_LINK_SPACING`. Above ×4 that becomes a buzz and screams pile up, so at ×8 and ×16 `eventsMuted` is true: clacks, launches, brakes, screams and station chimes are suppressed, the roar is held at a low fixed level, and the crowd and ambience beds continue. This keeps fast-forward quiet rather than frantic.

On `document.visibilitychange` to hidden, the engine ramps the master gain to 0 over 50 ms and calls `context.suspend()`; on visible it resumes and ramps back. `requestAnimationFrame` stops in a hidden tab anyway, so no frames are applied while hidden, and the 150 ms scheduling lookahead means nothing queued plays on return.

### Autoplay unlock

The `AudioContext` is created lazily on the first `pointerdown` or `keydown` on `document` (capture phase, listener removed after it fires), followed by `context.resume()`. Until then the engine is a no-op that still accepts frames. If the context exists but is `suspended` (Chrome creates it suspended without a gesture), the same listener resumes it. The sound button shows a "sound starts after your first click" title while locked. Test mode (`?test=1`) never creates a context unless a test calls `gameTest.audio.unlock()`; frozen screenshots must stay silent and deterministic.

### Master volume and mute

`web/src/sound-setting.ts`, modelled on `units-setting.ts`: a button with a speaker icon (`aria-label="Sound"`, `aria-pressed` true when on, `data-sound="on" | "off"`) and a `<input type="range" min="0" max="100">` beside it (`aria-label="Volume"`), both in a new `<div data-sound-setting>` in the game bar after the units setting. Key `m` toggles mute. Preference stored under `theme-park-sim.sound` as JSON `{ "volume": 0.7, "muted": false }`; a `parseStoredSound(text)` function validates it and is unit tested. New `InputAction`s: `{ type: "sound-mute-toggle" }` and `{ type: "sound-volume"; volume: number }`. `main.ts` applies them through `soundSetting.setMuted` / `setVolume`, which store and then call `audio.setVolume(volume, muted)`. Default 0.7, unmuted.

### Synthesis recipes

All levels below are linear gains before the master; the compressor (threshold −12 dB, ratio 3) catches overlaps. Each family exports `create<Family>Voice(context, destination)` returning an object with `apply(parameters, atTime)` and `trigger(event, atTime)` methods and a `release()`.

**Shared noise** (`synth/noise.ts`): two 2 s `AudioBuffer`s made once per context: white noise from a seeded xorshift generator (seeded, so offline renders repeat), and pink noise from the same white noise through the Paul Kellet three-pole filter in JS. Every noise source is an `AudioBufferSourceNode` with `loop = true` on one of these buffers.

**Wheel and rail roar** (`synth/train.ts`): pink noise loop → bandpass (centre `200 + 40·speed` Hz, Q 0.7) → lowpass (cutoff `600 + 150·speed` Hz) → gain `audibility × roarLevel(speed)` with `roarLevel = 0.5 × min(1, (speed / 30)^1.5)` and 0 below 0.5 m/s. Material variants from the train layout model: `velocity_viper` steel; `wooden_twister` wood; `hybrid` (Timber Tumbler, steel track on a wooden structure) steel roar plus the wooden rumble at half depth.
- Steel adds a thin whine: sawtooth oscillator at `180 + 25·speed` Hz → highpass 1.5 kHz → gain 0.02 × roarLevel.
- Wood adds a rumble path: the same pink loop → resonant lowpass 120 Hz Q 4 → gain 0.6 × roarLevel, and a tie-clatter tremolo: a triangle LFO at `speed / TIE_SPACING` Hz (tie spacing 1.2 m, so 8 Hz at 10 m/s) into a gain whose depth is 0.35.

**Chain lift** (`synth/train.ts`): while `onLift` and `speed > 0.1`, clacks at `clackRate = speed × gameSpeed / 0.4` per second (about 5 Hz at 2 m/s). Each clack is a 30 ms slice of the white buffer (`AudioBufferSourceNode` with `start(t, offset, 0.03)`, a different offset each time) → bandpass 1.8 kHz Q 2 → gain envelope (1 ms attack to 0.25 × audibility, exponential decay to 0.001 over 25 ms). The scheduler keeps `nextClackTime` per voice and fills up to 150 ms ahead each frame. A motor hum plays under it: two triangle oscillators at 50 and 100 Hz → lowpass 300 Hz → gain 0.05 × audibility, ramped in over 300 ms when the train enters the lift and out when it leaves.

**Launch** (`synth/train.ts`): on the frame the lead car's piece kind becomes Launch with speed rising, one shot: white loop → bandpass sweeping 300 → 3000 Hz over 1.2 s (Q 1) → gain envelope (80 ms attack to 0.35 × audibility, hold 400 ms, 600 ms release), plus a sine sweeping 80 → 400 Hz over the same time at gain 0.15 (the linear motor hum rising).

**Brakes** (`synth/train.ts`): on entering Brakes or Station with speed above 3 m/s, a hiss whose gain follows deceleration: white loop → highpass 2.5 kHz → gain `0.3 × audibility × clamp(−Δspeed / 8, 0, 1)` updated each frame while the train slows, released over 300 ms when it stops or leaves the piece, capped at 1.5 s; plus a faint squeal: sine 2.2 kHz with a 6 Hz vibrato of 30 Hz, gain 0.03 × the same factor.

**Station** (`synth/train.ts`): `arrive` (speed reaches 0 on a Station piece) plays a pneumatic release: pink loop → lowpass 1.5 kHz → gain envelope 0.2 × audibility with a 400 ms exponential decay. `depart` (speed rises from 0 on a Station piece) plays a two-note dispatch chime: sines at 880 Hz then 1175 Hz, 120 ms each, 10 ms attack, 150 ms release, gain 0.12 × audibility. Rider chatter in the station is deferred.

**Rider screams** (`synth/screams.ts`): a scream group is `n = 3 + occupiedSeats / 4` (capped at 6) detuned voices. Each voice: sawtooth oscillator → two bandpass formants in parallel (700 Hz and 1200 Hz, Q 5, the vowel "aah") → gain. Pitch glides from 500 to 650 Hz over the first 300 ms with a 5.5 Hz vibrato of 20 Hz; voices are detuned ±40 cents and start within 80 ms of each other. Envelope: 60 ms attack to `0.18 × audibility / n` per voice, hold 0.6–1.2 s (longer for larger g), 300 ms release. A breath layer adds pink loop → bandpass 2 kHz → gain 0.1 of the voice level. Triggers in `mapping.ts`, only when `occupied` is 1 and the cooldown has passed, on a rising edge of any of: vertical g below 0.2 (airtime), vertical g above 3, lateral g magnitude above 1.2, or a `crestBurst` for this coaster this frame (the drop after a lift crest). Trigger intensity (0–1) scales hold length and group size.

**Crowd murmur** (`synth/crowd.ts`): pink loop → bandpass 300 Hz Q 0.8 → bandpass 1200 Hz Q 0.8 (in series, a voice-band hump) with a sine LFO at 0.2 Hz moving both centres ±15%, → gain; plus a babble layer: four triangle oscillators at 160–260 Hz whose frequencies wander on independent 0.1–0.3 Hz LFOs (±30 Hz), through the same formant pair at a quarter of the level. `level = 0.25 × min(1, log1p(weightedCount) / log1p(200))` where `weightedCount` is the sum over guests of `distanceGain` from their projected position; `pan` is the audibility-weighted centroid's pan. Riding guests are excluded (their sound is the scream). 

**Ambience** (`synth/ambience.ts`): a wind bed (pink loop → lowpass 400 Hz → gain 0.08 with a 0.1 Hz LFO of ±40%), present day and night at `distance` weight (louder when zoomed out: `0.6 + 0.4 × (1 − zoomGain)`). Day layer: birds, a chirp every 1–4 s (seeded random): a sine oscillator gliding 2.5 → 3.5 kHz over 80 ms, repeated 2–4 times 120 ms apart, gain 0.06 with 5 ms attack and 60 ms release, panned randomly. Night layer: crickets, a square oscillator at 4.2 kHz → bandpass 4.2 kHz Q 8 → a gain gated by a 25 Hz square LFO, itself gated on for 1.5 s and off for 0.8 s, gain 0.04. `day = smoothstep(0.22, 0.28, phase) × (1 − smoothstep(0.72, 0.78, phase))`, `night = 1 − day`.

**Build and interface sounds** (`synth/ui.ts`), not spatialised, gain independent of zoom:
- place: a sine at 1200 Hz gliding to 900 Hz over 40 ms, gain 0.15, plus a sine at 150 Hz for 80 ms at 0.2 (the thunk);
- undo: the same tick reversed, 900 → 1200 Hz, 60 ms, gain 0.1, no thunk;
- error: two square notes at 220 Hz (90 ms) and 180 Hz (120 ms) with a 20 ms gap through a lowpass 1 kHz, gain 0.12;
- finish (circuit closed or Finish coaster confirmed): a sine arpeggio C5 E5 G5 C6, 90 ms each with 200 ms decays, gain 0.12, aligned with the confetti.
Triggers from `main.ts`: `constructionEvent` "placed" and "closed"; `undo` when `next.track.length < previous.track.length`; `error` when a `ConstructionState` arrives with a non-empty `message` that changed while `track` did not, or a path, facility, scenery or terrain result message with `success === false` (or `placed === false` with a message for facilities). Each UI event is debounced to one per 60 ms.

### Keeping CPU low

- Native nodes only, no `AudioWorklet`, no `ScriptProcessorNode`. The audio thread runs a fixed graph of about 80 nodes, which is a small fraction of one core on any machine that runs the renderer.
- Main-thread work per frame is one pure `soundFrame` call (train loop over at most a handful of trains, guest scan every fourth frame) plus `setTargetAtTime` calls for the changed parameters. Target: under 0.3 ms per frame at 10,000 guests, measured in the performance check below.
- Nothing is allocated per frame except the `SoundFrame` object and its small arrays; voices and buffers are built once.
- Muted or hidden: the context is suspended, so the audio thread does no work.
- No reverb, no convolution, no HRTF.

### Testing strategy

- **Vitest (pure, Node):** `mapping.test.ts` (audibility and pan from camera, roar level by speed, clack rate by speed and game speed, launch and brake and station edges from piece kind and speed changes, scream triggers and cooldown, crowd count and centroid, day and night weights, the ×8 and ×16 policy), `voices.test.ts` (allocation, cap, stickiness, release), `sound-setting.test.ts` (`parseStoredSound` accepts valid JSON and rejects junk), `noise.test.ts` (seeded buffers repeat and are zero-mean within tolerance; pink has more energy below 200 Hz than white, checked with a small DFT in the test), and the `trainStates` parsing in `world-snapshot.test.ts` or a new test.
- **Rust:** a test in `crates/sim-wasm/src/tests/` that the demo coaster's train state reports kind Lift and speed equal to `lift_speed` while on the lift, kind Track with vertical g above 1 in the first valley, and that the record count equals the train count under block operation with two trains.
- **Playwright, offline render (`web/e2e/audio.spec.ts`, `@fast`):** `gameTest.audio.renderOffline(script)` builds the engine on an `OfflineAudioContext` (48 kHz, stereo, up to 4 s), applies a scripted list of `SoundFrame`s at given times, renders, and returns per-channel peak, RMS, and RMS in a few bands (below 200 Hz, 200–2000 Hz, above 2 kHz) computed in the page. Assertions, each with a wide margin so they test behaviour and not exact levels: silence when every frame is empty (peak below 1e-4); roar RMS at 25 m/s at least 4× the RMS at 5 m/s; a lift frame at 2 m/s for 2 s produces 9–11 peaks above a threshold in the 1–3 kHz band envelope (clack detection by counting envelope rises); a wooden roar has more energy below 200 Hz than a steel roar at the same speed; a scream event raises the 200–2000 Hz band over the following 0.5 s; a `place` event produces a peak within 50 ms of its time; muted output is silent; peak never exceeds 1.0 and no sample is NaN.
- **Playwright, live (`web/e2e/audio.spec.ts`, `@fast`):** load `/`, assert `gameTest.audio.state()` is `"locked"`, click the canvas, assert it becomes `"running"`, wait until the demo train moves and assert `gameTest.audio.activeVoices().trains >= 1` and that the console has no errors or warnings from the engine; press `m`, assert `"suspended"` and `data-sound="off"`; reload and assert the button still shows off and the stored JSON matches; set the volume slider and reload. One `@slow` check runs with `?park=demo` at ×16 for 10 s and asserts no console errors and that `activeVoices().screams` stays 0.
- **Performance:** extend `web/bench` or the `?timing=1` path to report the engine's `soundFrame` time; the acceptance in the last work item uses it.

Chrome's headless mode has a fake audio output, so `AudioContext.resume()` works after a click and `OfflineAudioContext` renders without hardware; no launch flags are needed. If a flag turns out to be needed, add `--autoplay-policy=no-user-gesture-required` to `launchOptions.args` in `playwright.config.ts` and keep the unlock test on the real click path.

## Work items

1. **Per-train state in snapshots.**
   - Goal: `coaster_train_states()` and `construction_train_states()` on `WasmSimulation` with the `TRAIN_STATE` record; `Train` gains a `last_acceleration` field set in `step_block_brake_impl` (not hashed; it is derived from hashed state) and a `lead_car_forces(&self, circuit) -> GForces` method that calls `rider_g`; the worker transfers the two arrays with the snapshot; `protocol.ts`, `bridge-layouts.ts` and the snapshot validation accept them; `gameTest.trainStates()` exposes them.
   - Files: `crates/sim/src/physics/mod.rs`, `crates/sim-wasm/src/guests.rs`, `crates/sim-wasm/src/construction.rs`, `crates/sim-wasm/src/lib.rs`, `crates/sim-wasm/src/tests/world.rs`, `web/src/worker.ts`, `web/src/protocol.ts`, `web/src/bridge-layouts.ts`, `web/src/game-test.ts`, `web/src/main.ts`.
   - Acceptance: the Rust test above passes; `worker-world.spec.ts` and the session replays pass unchanged (the arrays are extra fields, no hash change); `make check` green.
   - Tests first: the Rust test and a Vitest for the record validation.
   - Model: Codex Luna. Dependencies: none. Coordinate with the `inversion-forces` branch: merge whichever lands second over the other's `Train` changes.
2. **Sound setting and persisted preference.**
   - Goal: `web/src/sound-setting.ts` with the button, slider, key `m`, `parseStoredSound`, the two `InputAction`s, the game bar markup and CSS, and `main.ts` wiring that calls a stub `setVolume(volume, muted)` until item 3 lands.
   - Files: `web/src/sound-setting.ts`, `web/src/sound-setting.test.ts`, `web/src/input/actions.ts`, `web/src/input/bindings.ts`, `web/index.html`, `web/src/toolbar.css` (or `app.css`), `web/src/main.ts`, `web/e2e/toolbar.spec.ts` or a new `sound-setting.spec.ts`.
   - Acceptance: the control renders in the game bar in both themes; `m` toggles it; the preference survives a reload in the browser test; `parseStoredSound` rejects malformed storage.
   - Model: Codex Luna. Dependencies: none.
3. **Engine core and test harness.**
   - Goal: `engine.ts` with lazy creation and unlock, master chain, volume and mute, visibility handling, the scheduler clock with lookahead, voice pools with `voices.ts` allocation, `noise.ts`, the `SoundFrame` type with empty implementations of each family (so applying a frame is a no-op that exercises the plumbing), and `gameTest.audio` with `state()`, `unlock()`, `activeVoices()` and `renderOffline(script)`. `main.ts` builds `MappingInput` each frame and calls the engine.
   - Files: `web/src/audio/engine.ts`, `web/src/audio/voices.ts`, `web/src/audio/synth/noise.ts`, `web/src/audio/mapping.ts` (types and the spatialisation functions only), their Vitest files, `web/src/game-test.ts`, `web/src/main.ts`, `web/e2e/audio.spec.ts`.
   - Acceptance: live test: locked, then running after a click, suspended on `m`, no console errors; offline render of empty frames is silent; `voices.test.ts` and `noise.test.ts` pass; `make check` green and the frozen screenshot specs unchanged.
   - Model: Codex Terra. Dependencies: 2.
4. **Mapping from game state to sound.**
   - Goal: the full `soundFrame` in `mapping.ts` with `SOUND_TUNING`, per-train memory and edge detection, scream triggers, crowd count and centroid, ambience weights, the game speed policy, and `main.ts` feeding builder and tool events into it.
   - Files: `web/src/audio/mapping.ts`, `web/src/audio/mapping.test.ts`, `web/src/main.ts`.
   - Acceptance: `mapping.test.ts` covers every row in the testing strategy; a frame for the demo park with the camera on the train yields one train entry with audibility above 0.5; a frame at ×16 has `eventsMuted` true and no scream or clack output.
   - Model: Codex Luna. Dependencies: 1, 3.
5. **Train sounds.**
   - Goal: `synth/train.ts` with the roar for the three materials, the chain clack and motor, launch, brake and station recipes, wired to the train voices.
   - Files: `web/src/audio/synth/train.ts`, `web/src/audio/engine.ts`, `web/e2e/audio.spec.ts`.
   - Acceptance: the offline assertions for roar by speed, wood against steel, clack count, launch (band above 2 kHz rises over 1 s after the event), brake (band above 2 kHz present for a slowing train) and station chime (two tonal peaks) pass; live test shows a train voice while the demo train runs; peak never exceeds 1.0.
   - Model: Codex Terra. Dependencies: 4.
6. **Rider screams.**
   - Goal: `synth/screams.ts` and the scream pool.
   - Files: `web/src/audio/synth/screams.ts`, `web/src/audio/engine.ts`, `web/e2e/audio.spec.ts`.
   - Acceptance: the offline scream assertion passes; a scripted frame sequence with two trains dropping at once uses two groups and a third drop within the cooldown of the first train uses none; live on the demo park with riders aboard, `activeVoices().screams` reaches 1 within one lap.
   - Model: Codex Terra. Dependencies: 5.
7. **Crowd and ambience.**
   - Goal: `synth/crowd.ts` and `synth/ambience.ts`.
   - Files: `web/src/audio/synth/crowd.ts`, `web/src/audio/synth/ambience.ts`, `web/src/audio/engine.ts`, `web/e2e/audio.spec.ts`.
   - Acceptance: offline: crowd RMS at level 1 is at least 10× the RMS at level 0; day frames have energy above 2 kHz that night frames lack and night frames have the 4.2 kHz cricket band; live: `gameTest.setTimeOfDay(0)` then `(0.5)` changes `activeVoices().ambience` from `"night"` to `"day"`.
   - Model: Codex Luna. Dependencies: 4.
8. **Build and interface sounds.**
   - Goal: `synth/ui.ts` and the triggers for place, undo, error and finish, including tool failure messages.
   - Files: `web/src/audio/synth/ui.ts`, `web/src/audio/engine.ts`, `web/src/main.ts`, `web/e2e/audio.spec.ts`.
   - Acceptance: offline: each of the four events produces a peak within 50 ms of its time and the error sound has its energy below 1 kHz; live: in the Timber Tumbler build flow (reuse the `player` driver), placing a piece sets `gameTest.audio.lastUiEvent()` to `"place"`, Backspace to `"undo"`, and a refused placement to `"error"`.
   - Model: Codex Luna. Dependencies: 4.
9. **Integration check, performance and docs.**
   - Goal: the ×16 `@slow` check, the `soundFrame` timing in the benchmark or `?timing=1` output, a listening pass by the owner with `SOUND_TUNING` adjusted from the notes, `docs/status.md` updated (sound moves from "not started" to what works, plus the testing note), and the TASK-28 ticket closed with the deferred list below copied into follow-up tickets.
   - Acceptance: `make ci` green; `soundFrame` under 0.3 ms per frame with 10,000 guests in the guest benchmark park; the owner has heard it and the tuning diff is committed.
   - Model: Claude Haiku for the docs and checks, the owner for listening. Dependencies: 5, 6, 7, 8.

## Deferred

- CC0 recordings where synthesis falls short (likely candidates after listening: screams and birds). The engine's one-shot slots can play an `AudioBuffer` from a decoded file with the same envelope and spatialisation, so adding them later does not change the architecture.
- `AudioWorklet` DSP (for example a better rail resonance model), once Vite's worklet bundling is verified.
- Flat ride sounds (carousel organ, ferris wheel motor), stall and shop sounds, footsteps, water, fireworks bangs (the fireworks are a pure function of the tick in `render/effects.ts`, so bangs are a cheap later addition), rider chatter in the station, tunnel muffling, reverb, music.
- Per-car sounds and a true 3D panner.
- A separate volume per family (rides, crowd, ambience, interface) in a settings panel; the first version has one master volume and mute.
- Touch-specific unlock handling beyond `pointerdown`, which already covers touch.

## Risks

- **Synthesised screams sound silly.** Formant sawtooth screams are the weakest recipe here. The scream work item is separate and late so the rest ships without it; if listening says no, the fallback is a CC0 scream set through the same slots.
- **Level balance only works in a real browser.** The offline tests check behaviour (ratios, presence, timing), not taste; the owner's listening pass in the last item sets the levels. Keep all levels in `SOUND_TUNING`.
- **Per-frame cost at 10,000 guests.** The guest scan is the only loop over park size; every fourth frame and a single pass keep it small, and the last item measures it.
- **Snapshot timing versus audio clock.** Snapshots arrive about every 33 ms but `RenderClock` can run a frame a few ticks behind; one-shots are scheduled on the audio clock from the frame they are detected in, so they trail the drawn train by at most a frame, which is not audible.
- **Edge detection across game speeds.** At ×4 a 33 ms snapshot covers four ticks (0.13 s of simulation), so a short brake run could be missed; the mapping tests cover a Brakes-to-Track transition within one snapshot by also reading the speed drop.
- **Headless audio.** If `resume()` never reaches `running` in headless Chrome, the unlock test is adjusted to accept `running` or `suspended` after a click only after the flag above has been tried and the behaviour recorded in `docs/status.md`.
- **Conflicts with the force branch.** Both touch `Train`; the item 1 change is a field and a method, small enough to merge either way.

## Owner decisions to make

1. Fast-forward policy: at ×8 and ×16 the plan silences clacks, screams, launches and brakes and keeps a quiet roar, crowd and ambience. The alternative is to keep everything and accept a buzz.
2. One master volume and mute in the game bar (key `m`), with per-family volumes deferred. If you want ride, crowd and interface volumes separately now, item 2 grows into a small settings panel.
3. Material for Timber Tumbler (`hybrid`): steel roar with half the wooden rumble, as proposed, or fully steel.
4. Whether frozen test mode and screenshots stay silent always (proposed), or whether a test flag can unlock sound in frozen mode too.
