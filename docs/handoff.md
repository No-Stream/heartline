# Handoff

Written 2026-11-03 at the end of the first long session. Read `docs/status.md` for what works and what is open, then this file for in-flight work and the order to resume in. The design is `docs/superpowers/specs/2026-11-02-theme-park-sim-design.md`; tickets are in Backlog.md (`backlog task list --plain`).

## How to work in this repo

Run targeted tests while editing (a crate's cargo tests, a single vitest file, a single Playwright spec) and the full `make ci` only at checkpoints, because the full gate takes about 7 minutes and parallel runs starve the machine. Say this in every subagent brief. Commit by explicit path, never push. Codex jobs started from a session die with that session's `/clear`; Claude subagents do not. The game runs with `make build` and then `npm run dev -- --host 127.0.0.1 --port 5173`; restart the dev server after Vite config changes.

## In flight when this was written

One Codex job was still running: the root-cause fix for four coaster browser tests that time out under the full parallel suite (`web/e2e/block-sections.spec.ts` both tests, `construction.spec.ts` "builds a closed coaster", `coaster-boarding.spec.ts`). Its work so far is committed in the last checkpoint, but it may have left later uncommitted edits in `crates/sim/src/track/blocks/`, `crates/sim-wasm/src/lib.rs`, `web/src/worker.ts` and `web/e2e/block-sections.spec.ts`, possibly including temporary timing instrumentation. The previous lead was that the block placement command throws in the worker and never returns its report, rather than being slow. To resume: check `git status` and `git diff`, remove instrumentation, run the four specs individually, then `make ci` twice. When the gate is green, close TASK-19 (scenery) and TASK-23 (block sections), whose code is committed.

## Next, in order

1. Green the gate (above).
2. Owner playtest bugs, all high priority: tools needing a second click (TASK-45), the path tool placing one tile at a time (TASK-46), buildings drawn cut off (TASK-47), food facilities refused with "missing access" (TASK-48), and why the owner could not find the way back to level on Velocity Viper (TASK-42, pieces exist per a reachability test). TASK-46 and TASK-48 are probably regressions from explicit path edge connections.
3. Dark interface for OLED screens (TASK-44), then the CSS cleanup (TASK-61).
4. Slow test tiers (TASK-41), using the per-test plan in `docs/reviews/2026-11-03-rendering-art-tests.md`.
5. Code health refactors from the four reviews in `docs/reviews/`, TASK-51 to TASK-60, roughly in this order: the facility clearance follow-up in TASK-51, split the WebAssembly bridge (TASK-53), the `Park` type (TASK-54), the typed worker protocol (TASK-57), `main.ts` around a shared tool interface (TASK-58), typed coaster and block state (TASK-55), shared helpers (TASK-52), the track module split (TASK-56), projection and renderer stages (TASK-59), art generators (TASK-60).
6. Broader coaster vocabularies with unique signatures and manufacturer styles (TASK-43), following the direction in section 3 of the design doc.
7. Then saving and loading (TASK-25), the full-scale performance check (TASK-26), and deployment (TASK-27).

## Waiting on the owner

The night effects in a dense park (TASK-20), the builder after its polish (TASK-10, TASK-37), and whether Woody's coaster shirt keeps the lift hill and loop or switches to one bold loop.
