import { readFileSync } from "node:fs";

/** The lines of a named circuit in circuits.txt: piece ids plus `+chain` and `-chain`, without blanks or comments. */
export function readCircuit(name: string): string[] {
  const sections = readFileSync(new URL("./circuits.txt", import.meta.url), "utf8").split(`[${name}]`);
  if (sections.length !== 2) throw new Error(`circuits.txt must contain exactly one [${name}] circuit`);
  return sections[1].split("\n[")[0].split("\n").map((line) => line.trim()).filter((line) => line && !line.startsWith("#"));
}
