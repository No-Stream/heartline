import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, test } from "vitest";
import { checkRideAgreement } from "./fast-agreement";

const repositoryRoot = resolve(fileURLToPath(new URL("../../..", import.meta.url)));
const designBinary = resolve(repositoryRoot, "target/release/heartline-design");
const fixture = (name: string): string => resolve(repositoryRoot, "web/e2e/rides/fixtures", name);

// Needs `make design-tools agreement-wasm`; `make test-agreement` builds both first.
describe("fast browser-path ride agreement", () => {
  test("passes a ride that the browser agreement test passes", async () => {
    const result = await checkRideAgreement(designBinary, resolve(repositoryRoot, "content/rides/browser-agreement-fast.ride.json"));
    expect(result.failures).toEqual([]);
    expect(result.estimateComparisons).toBe(18);
  }, 120_000);

  test("rejects a ride whose shaped geometry was saved by an older solver", async () => {
    // Horizon Hawk at 20483ea: the browser test found 0.0185 g lateral versus 0.0214 g natively after its first turn.
    const result = await checkRideAgreement(designBinary, fixture("horizon-hawk-older-solver.ride.json"));
    expect(result.failures).toHaveLength(1);
    expect(result.failures[0]).toMatch(/stale shaped geometry: ride step 11 at .* saved shaped geometry digest 7a165ba6bd127329 differs from the current solver's 0f412105af8180cd/);
  }, 120_000);

  test("reports the strong-floater delivery mismatch the browser test found at Horizon Hawk step 12", async () => {
    // Horizon Hawk at 035f766. Until native and WASM solve this hill identically, the browser path reports a
    // 1.3e-5 g delivery difference and a different geometry digest. Once they do, the saved hill no longer matches
    // the solver and the stale-geometry check rejects step 12 instead; either way step 12 must be flagged.
    const result = await checkRideAgreement(designBinary, fixture("horizon-hawk-hill-delivery-mismatch.ride.json"));
    expect(result.failures.length).toBeGreaterThan(0);
    expect(result.failures[0]).toMatch(/ride step 12\b/);
  }, 300_000);
});
