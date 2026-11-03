# Browser code review, 2026-11-03

Scope: `web/src` outside `web/src/render`, the main-thread/worker protocol, CSS, and `web/e2e`. Read-only review; nothing was changed.

## Summary

Most of the small modules are in good shape. They are focused, they fail fast, and the pure ones have unit tests: `construction-choices.ts`, `block-panel-state.ts`, `path-tool.ts`, `landscaping-tool.ts`, `input/bindings.ts`, `guest-panel/presentation.ts`. The cost of the fast parallel build has landed in two places. The first is `main.ts`, which has become the place where every feature without a home lives: module-level state, picking maths, a whole station-placement tool, and an `if/else` dispatcher about 150 lines long. The second is the worker protocol, which has no single definition. Each side declares its own idea of the messages, so the compiler cannot catch it when they drift. Beyond that there is moderate duplication across the tool panels and the e2e specs, some dead code, and CSS that fights its own specificity. None of this is urgent, but the first three findings would make adding the next tool noticeably cheaper.

## Findings, ranked by day-to-day payoff

### 1. The worker protocol has no single typed definition (medium)

- `main.ts:476` declares the incoming message type as an inline union of about twelve shapes. Some of them (`CoasterWorldMessage`, `CoasterSnapshot`) are defined only in `main.ts:52-54`, and others are written inline (`facilities`, `guest-detail`, `scenery`).
- `worker.ts:9-22` re-declares `PathPosition` and `PathAction`, which duplicate `PathPosition` and `PathToolCommand` in `path-tool.ts:3-13`. The guest-detail actions are also re-declared there.
- The worker posts through `workerScope.postMessage(message: unknown)` (`worker.ts:29-32`), so nothing checks that `sendConstruction` (`worker.ts:108-118`) actually produces a `ConstructionState`. `JSON.parse(...) as unknown` gets assigned straight into typed fields such as `testingReport` and `blockReport`.
- Some messages are sent and never handled: `ready` (`worker.ts:173`) and `surface-edited` (`worker.ts:317`, which only the e2e worker fixture reads). `construction-delete-mode` is part of `ConstructionAction` (`construction-protocol.ts:10`) but is a UI-only action, and the worker has to skip it explicitly (`worker.ts:303`).
- Test-only commands (`test-advance`, `set-surface`) share the production union and the production handler. The worker learns whether it is in test mode by parsing its own name (`worker.ts:25-28`).

Proposal: add one `web/src/protocol.ts` that exports `MainToWorker` and `WorkerToMain` discriminated unions, built from the existing per-feature types (`ConstructionAction`, `PathToolCommand`, `TerrainEditCommand`, `SceneryCommand`, and so on). The worker would get `post(message: WorkerToMain, transfer?)` and main would get `send(message: MainToWorker)`. Both handlers become a `switch` with an exhaustiveness check. Send the test configuration in an `init` message instead of encoding it in the worker name. Do this first as a types-only change; it should find real mismatches without changing any behaviour.

### 2. `main.ts` does too much and keeps its state in about 40 module-level variables (large, can land in pieces)

- `dispatch()` (`main.ts:318-469`) mixes routing with geometry. It contains corner picking for landscaping (`338-351`), quarter-tile picking for scenery (`355-365`), and block-marker snapping and nudging (`416-429`). None of that code can be unit-tested where it is.
- Station placement is a complete tool living in main: its state (`80-84`), heading choice (`85-105`), ghost geometry (`106-113`) and click handling (`383-412`). Paths, scenery and landscaping each have their own `create*Tool` module.
- Facility placement and removal state (`141-154`, `396-404`) also lives in main. There is no `facility-tool.ts`.
- `inputModeActive()` and `endInputModes()` (`243-253`) list every tool by hand, so each new tool means editing three places.
- `frame()` (`548-621`) puts the whole scene together inline: nine calls to `trackStyle(renderAtlas)`, a guest-panel anchor side effect inside a `.map` (`608-613`), and the atmosphere pass.

Proposal: give each tool a small shared shape, for example `{ handle(action): boolean; active(): boolean; cancel(): void; ghost(camera, viewport, atlas): GeometryQuad[] }`. Move station placement into `station-tool.ts` and facilities into `facility-tool.ts`, each with unit tests like `path-tool.test.ts`. Move the picking helpers into a `world-picking.ts` (next to `path-picking.ts`) with tests. `dispatch` then becomes routing to the active tool, and `endInputModes` becomes a loop. Move the scene assembly out of `frame()` into a `compose-scene.ts` that takes an explicit state object.

### 3. Test-harness concerns are spread through production code (small to medium)

- `gameTest.ready = false` appears 20 times in `main.ts`. If someone forgets it in a new code path, the result is a flaky screenshot test, not a failure that points at the cause.
- `frozen` appears 14 times. Two of those uses change how production messages are handled: facility updates are dropped at `main.ts:499`, and snapshots are held at `main.ts:529`.
- The `gameTest` object (`269-311`) and the timing loop (`624-650`) sit in the middle of the boot sequence.

Proposal: use a single `invalidate()`, or better a revision counter that each state setter bumps, with `ready` meaning "a frame has been drawn at the current revision". Move `gameTest` construction and the timing loop into `game-test.ts` and give them an explicit context. Gather the frozen-mode policy into one place there.

### 4. Tool panels follow three different conventions (medium)

- Callbacks are inconsistent. `createConstructionPanel` and `createLandscapingPanel` take `dispatch`. `createFacilityPanel` takes six positional callbacks (`facility-panel.ts:91-98`). Path and scenery panels take three positional callbacks each.
- Tool state is inconsistent. Path and scenery tools expose `mode`, landscaping exposes `active: boolean`, and facilities and the station have no tool module at all.
- Status text is written in two places. For example, `path-panel.ts:31-34` sets "Drag between tiles to build a path." and `build()` in `path-tool.ts:76-83` sets the same message, which main then copies into the panel again (`main.ts:162`).
- Small helpers have been copied: the `element()` lookup that fails fast (`construction.ts:171`, `facility-panel.ts:128`, while the other panels use `!`); atlas thumbnail drawing (`scenery-panel.ts:23-32` and `facility-panel.ts:82-89`); direction names and rotation steppers (`scenery-panel.ts:20,35`, `facility-panel.ts:17,172`); and the scenery `directional` set (`scenery-tool.ts:16`, `scenery-panel.ts:6`).
- The initial station height of 12 is defined twice (`main.ts:82`, `construction.ts:41`).

Proposal: panels take `dispatch` and emit `InputAction`s, tools own their mode and message, and a panel's status line only displays the tool's message. Add `panel-dom.ts` with `requiredElement`, `atlasThumbnail` and `directionStepper`.

### 5. The e2e specs repeat the same setup (small to medium)

- The same 22-piece circuit, or a close variant, is written out in `construction.spec.ts:29`, `coaster-boarding.spec.ts:19`, `wooden-twister.spec.ts:22` and `block-sections.spec.ts:9`. The station-placement sequence (fill the height, press Tab, click "Place station", then two world clicks) is repeated in five specs.
- `pathAt`, `clickTile`, `dragTiles`, `clickTerrain`, `clickWorld` and `dragWorld` are each defined separately in `paths.spec.ts`, `path-interactions.spec.ts`, `block-sections.spec.ts`, `coaster-boarding.spec.ts`, `facilities.spec.ts` and `landscaping.spec.ts`, with small differences between copies.
- About 15 specs start with the same two lines: `goto("/?test=1")` followed by polling `gameTest.ready`.
- The two worker unit tests each hand-write a fake `WasmSimulation` (`world-snapshot.test.ts:11-41`, `guest-panel/worker.test.ts:5-30`). Both are really tests of `worker.ts`. The second one also covers path and facility handling even though it lives under `guest-panel/`.

Proposal: grow `e2e/game-page.ts` into the shared driver, with `startGame(page, { agents })`, `placeStation`, `buildCircuit(pieces)`, `clickTile`, `dragTiles` and `pathAt`. Put named piece lists in `e2e/circuits.ts`. On the unit side, add a single `fakeSimulation()` and a single `src/worker.test.ts`.

### 6. Dead or misleading code (small)

- `src/renderer.ts` (`drawAgents`, the old canvas-2D renderer) is not imported anywhere.
- `guest-panel/index.ts` is an unused barrel; `main.ts:21` imports `guest-panel/guest-panel` directly.
- `GuestPanel.isOpen()` and `selectedGuestId()` are never called, while `main.ts:127` keeps its own `selectedGuestId`.
- `LandscapingResult` (`landscaping-tool.ts:41`) is an unused alias.
- `block-marker-drag-move` and `block-marker-drag-cancel` are dispatched by `bindings.ts:110,125` but nothing handles them in `dispatch`.
- The coaster model choice travels as `construction-select` with a fake piece id `model:*` (`construction.ts:249`). It should be its own action.
- `bindings.ts:12` says the panels read their key hints from `builderShortcuts` so they cannot drift, but rotate and zoom are hard-coded at `bindings.ts:76-79` and the help text writes `Q E` literally (`construction.ts:168`).
- The keyboard is bound twice, once on the body (`main.ts:118`) and once on the canvas (`bindings.ts:142`). Keys only avoid firing twice because of `defaultPrevented`.

### 7. Small geometry helpers are duplicated (small)

The rotated depth expression `[x + y, x - y, -x - y, -x + y][camera.rotation]` appears in `facilities.ts:54`, `scenery-tool.ts:63`, `landscaping-tool.ts:298` and `guest-panel/selection-ring.ts:24`, and also in three render files. Add `projectVertex(camera, viewport, x, y, heightQuarterTiles)` to `camera.ts`. Placement of ghost builders is also inconsistent: scenery and landscaping ghosts live in their tool modules, while the path ghost lives in `render/path-ghost.ts`. Pick one home for them.

### 8. CSS (small to medium)

- `print-modern.css` is a sensible shared layer. However, `scenery-panel.ts` does not import it, and `scenery-panel.css` styles its own buttons in raw pixels instead of using the `tool-panel` classes, so it already looks different from the other panels.
- `guest-panel.css` has its own token set and hard-coded font stacks.
- About 70 lines of app-shell CSS are inline in `index.html` and depend on tokens defined in `print-modern.css`.
- There are 28 `!important`s (14 in `construction.css`, 5 in `facility-panel.css`), mostly overriding the broad `.tool-panel button` rule.
- The `__actions` grid is identical in `path-panel.css:2-8` and `facility-panel.css`.

Proposal: move the shell into `app.css`. Wrap the base button rules in `:where()` so per-panel rules win without `!important`. Add a shared `.tool-panel__actions` class. Rebuild the scenery panel on the shared classes.

### 9. Optimisations (secondary)

- On every wake at 30 Hz, `wake()` builds `facilities_json()` and `construction_block_report_json()` just to compare them with the last strings (`worker.ts:77-79`).
- `sendConstruction` parses tab-separated rows (`worker.ts:93-94`) and re-parses JSON.
- A revision counter exposed from WebAssembly would remove both the string comparisons and the change keys built with `JSON.stringify` in main (`main.ts:501, 514, 521`).
- `frame()` rebuilds every overlay quad on every frame, even when nothing has changed.

## Proposed order of refactors

Each step can land on its own with `make test`, `make lint` and the e2e suite green.

1. Remove the dead code in finding 6 and give the coaster model its own action. This is mechanical, and the existing tests cover it.
2. Consolidate the e2e helpers and circuits, and add the shared fake simulation and a single `worker.test.ts`. Behaviour stays the same, and later steps get cheaper tests.
3. Add `protocol.ts` as types only, then add the typed `post` and `send` helpers and exhaustive switches. Fix whatever mismatches the compiler finds. Move test configuration into an `init` message.
4. Add `projectVertex`, `panel-dom.ts` and the shared direction and thumbnail helpers.
5. Introduce the revision counter in place of `gameTest.ready = false`, and move the harness into `game-test.ts`.
6. Extract `station-tool.ts` and `facility-tool.ts` with unit tests, and move the picking helpers out of `dispatch`.
7. Add the shared tool interface, route `dispatch` through it, and split scene assembly out of `frame()`. Align the panels on `dispatch`.
8. Clean up the CSS (`app.css`, `:where()`, shared action grid, scenery panel). Screenshot baselines may need to be refreshed on purpose, so review that diff separately.
9. Add revision counters on the WebAssembly side for the worker's change detection, if profiling shows it matters.
