import { mkdtemp, readFile, readdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { expect, test, type Page } from "@playwright/test";
import { build, preview, type PreviewServer } from "vite";
import {
  firstCheckpointMismatch,
  parseReplaySession,
  replayExpectationFailures,
  type ReplaySession,
} from "../src/session-replay";

interface ReplayWorkerOutcome {
  readonly recording: {
    readonly finalTick: number;
    readonly checkpoints: { readonly tick: number; readonly hash: string }[];
  };
  readonly observation: Parameters<typeof replayExpectationFailures>[1];
}

declare global {
  interface Window {
    replayWorkerSession(session: ReplaySession): Promise<ReplayWorkerOutcome>;
  }
}

interface SessionRun {
  readonly path: string;
  readonly session: ReplaySession;
  readonly outcome: ReplayWorkerOutcome;
  readonly mismatch: ReturnType<typeof firstCheckpointMismatch>;
}

let fixtureServer: PreviewServer;
let fixtureOutDir: string;
let fixtureUrl: string;

test.beforeAll(async () => {
  fixtureOutDir = await mkdtemp(join(tmpdir(), "session-replay-e2e-"));
  const config = {
    configFile: false as const,
    root: "web",
    publicDir: false as const,
    build: {
      outDir: fixtureOutDir,
      rollupOptions: { input: resolve("web/e2e/fixtures/session-replay.html") },
    },
  };
  await build(config);
  fixtureServer = await preview({
    ...config,
    preview: { host: "127.0.0.1", port: 0, strictPort: true },
  });
  const address = fixtureServer.httpServer?.address();
  if (!address || typeof address === "string") throw new Error("Vite did not start on a TCP port");
  fixtureUrl = `http://127.0.0.1:${address.port}/e2e/fixtures/session-replay.html`;
});

test.afterAll(async () => {
  if (fixtureServer) {
    await new Promise<void>((resolveClose, reject) => fixtureServer.httpServer.close((error) => error ? reject(error) : resolveClose()));
  }
  if (fixtureOutDir) await rm(fixtureOutDir, { recursive: true });
});

async function replayWithWorker(page: Page, session: ReplaySession): Promise<ReplayWorkerOutcome> {
  return page.evaluate((recordedSession) => window.replayWorkerSession(recordedSession), session);
}

test("replay waits for a finish-coaster search before confirming the connection @fast", async ({ page }) => {
  await page.goto(fixtureUrl);
  const pieces = ["TurnSmallStandalone", "TurnSmallStandalone", "Straight", "TurnSmallStandalone"];
  const commands = [
    { type: "construction-start", x: 100, y: 100, z: 40, heading: 0 },
    ...pieces.flatMap((piece) => [{ type: "construction-select", piece }, { type: "construction-append" }]),
    { type: "construction-finish-preview" },
    { type: "construction-finish-confirm" },
  ].map((command) => ({ tick: 0, command }));
  const session = parseReplaySession({ format: "theme-park-session", version: 1, build: { gitHash: "test", mode: "development" }, seed: "42", park: "demo", guestCount: 0, commands, checkpoints: [], finalTick: 0 });
  const outcome = await replayWithWorker(page, session);
  expect(outcome.observation.constructionPieces).toBeGreaterThan(5);
});

async function replaySessionsDirectory(): Promise<{ readonly path: string; readonly session: ReplaySession }[]> {
  const sessionsDirectory = resolve("web/e2e/sessions");
  const names = (await readdir(sessionsDirectory)).filter((name) => name.endsWith(".json")).sort();
  if (names.length === 0) throw new Error(`No replay session JSON files found in ${sessionsDirectory}`);
  return Promise.all(names.map(async (name) => {
    const path = join(sessionsDirectory, name);
    const session = parseReplaySession(JSON.parse(await readFile(path, "utf8")) as unknown);
    return { path, session };
  }));
}

function checkpointAnnotation(path: string, mismatch: NonNullable<ReturnType<typeof firstCheckpointMismatch>>): string {
  const expected = mismatch.expected ?? "missing";
  const actual = mismatch.actual ?? "missing";
  return `${path}: first state hash difference at tick ${mismatch.tick}; expected ${expected}, replayed ${actual}`;
}

test("replays every exported session against the real worker @fast", async ({ page }) => {
  test.setTimeout(120_000);
  const browserErrors: string[] = [];
  page.on("pageerror", (error) => browserErrors.push(error.message));
  page.on("console", (message) => {
    if (message.type() === "error") browserErrors.push(message.text());
  });
  await page.goto(fixtureUrl);

  const sessions = await replaySessionsDirectory();
  const runs: SessionRun[] = [];
  const errors: string[] = [];
  for (const { path, session } of sessions) {
    try {
      const outcome = await replayWithWorker(page, session);
      const expectationFailures = session.expect ? replayExpectationFailures(session.expect, outcome.observation) : [];
      if (expectationFailures.length) errors.push(`${path}: ${expectationFailures.join("; ")}`);
      if (outcome.recording.finalTick !== session.finalTick) {
        errors.push(`${path}: final tick expected ${session.finalTick}, replay reached ${outcome.recording.finalTick}`);
      }
      const mismatch = firstCheckpointMismatch(session.checkpoints, outcome.recording.checkpoints);
      if (mismatch && process.env.REHASH_SESSIONS !== "1") {
        test.info().annotations.push({ type: "checkpoint-mismatch", description: checkpointAnnotation(path, mismatch) });
        if (session.strict) errors.push(checkpointAnnotation(path, mismatch));
      }
      runs.push({ path, session, outcome, mismatch });
    } catch (error) {
      errors.push(`${path}: ${error instanceof Error ? error.message : String(error)}`);
    }
  }

  expect(browserErrors, "browser page or console errors during replay").toEqual([]);
  expect(errors, "session replay failures").toEqual([]);

  if (process.env.REHASH_SESSIONS === "1") {
    for (const { path, session, outcome } of runs) {
      await writeFile(path, `${JSON.stringify({ ...session, checkpoints: outcome.recording.checkpoints }, null, 2)}\n`);
    }
  }
});

test("reports the first checkpoint after a recorded command changes @fast", async ({ page }) => {
  await page.goto(fixtureUrl);
  const sessions = await replaySessionsDirectory();
  const { session } = sessions.find(({ session: candidate }) => candidate.commands.some(({ command }) => command.type === "path-build")) ?? {};
  if (!session) throw new Error("A recorded path-build session is required for the altered-command control");
  const firstPathBuildIndex = session.commands.findIndex(({ command }) => command.type === "path-build");
  const originalEntry = session.commands[firstPathBuildIndex]!;
  const originalCommand = originalEntry.command;
  const originalEnd = originalCommand.end as { readonly x: number; readonly y: number; readonly z: number };
  const alteredCommands = session.commands.map((entry, index) => index === firstPathBuildIndex
    ? { ...entry, command: { ...entry.command, end: { ...originalEnd, y: originalEnd.y + 1 } } }
    : entry);
  const altered = parseReplaySession({ ...session, commands: alteredCommands });

  const baselineReplay = await replayWithWorker(page, session);
  const alteredReplay = await replayWithWorker(page, altered);
  const mismatch = firstCheckpointMismatch(baselineReplay.recording.checkpoints, alteredReplay.recording.checkpoints);

  expect(mismatch).toBeDefined();
  expect(mismatch?.tick).toBe(300);
  const annotation = checkpointAnnotation("altered-command-control", mismatch!);
  test.info().annotations.push({ type: "checkpoint-mismatch", description: annotation });
  expect(test.info().annotations).toContainEqual({ type: "checkpoint-mismatch", description: annotation });
});
