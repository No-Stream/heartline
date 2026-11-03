import { describe, expect, it } from "vitest";
import type { Camera, Viewport } from "../camera";
import type { Facility } from "../facilities";
import { buildConfetti, buildFireworks, buildLights, buildShadows, buildSky, CONFETTI_LIFE_TICKS, createCrestTracker, detectCrest, parkBounds, type CrestBurst, type CrestTracker } from "./effects";
import { BACKDROPS, lightingAt, sunAt } from "./daylight";
import type { Atlas } from "./scene";

const camera: Camera = { centerX: 20, centerY: 20, zoom: 1, rotation: 0 };
const viewport: Viewport = { width: 1280, height: 800 };
const noon = lightingAt(0.5);
const night = lightingAt(0.95);
const facility = (kind: Facility["kind"], x = 18, y = 18): Facility => ({ id: 1, kind, x, y, z: 8, rotation: 0, price: 0, ride_state: "running", ride_progress: 0.1 });
const frame = (width: number, height: number) => ({ width, height, anchor: { x: width / 2, y: height - 4 }, placements: { "1": { page: 0, x: 0, y: 0, width, height } } });
const atlas: Atlas = {
  pages: { "1": [{ file: "atlas.png", width: 512, height: 512 }] },
  frames: { "sky/sun/any/static/0": frame(224, 224), "sky/cloud_big/any/static/0": frame(160, 84), "sky/cloud_small/any/static/0": frame(90, 40), "scenery/lamp/rot0/static/0": frame(30, 80) },
};
/** One straight rail segment of 19 floats: piece index, start and end frames of position, lateral and up. */
function rails(count: number): Float32Array {
  const data = new Float32Array(count * 19);
  for (let index = 0; index < count; index++) {
    data.set([-1, 15 + index * 0.5, 20, 3, 0, 1, 0, 0, 0, 1, 15 + (index + 1) * 0.5, 20, 3, 0, 1, 0, 0, 0, 1], index * 19);
  }
  return data;
}
/** The lead car of a train at a height, climbing or not. */
const leadCar = (z: number, climbing: number): Float32Array => new Float32Array([0, 10, 10, z, 1, 0, climbing, 0, 0, 1, 15]);

describe("crest confetti", () => {
  function run(heights: readonly number[]): CrestBurst[] {
    let tracker: CrestTracker = createCrestTracker();
    const bursts: CrestBurst[] = [];
    heights.forEach((z, tick) => {
      const next = heights[tick + 1] ?? z;
      const result = detectCrest(tracker, leadCar(z, Math.sign(next - z) * 0.3), tick);
      tracker = result.tracker;
      if (result.burst) bursts.push(result.burst);
    });
    return bursts;
  }

  it("bursts once when a train tops a long lift hill", () => {
    const lift = [...Array.from({ length: 300 }, (_, tick) => 3 + tick * 0.03), ...Array.from({ length: 30 }, (_, tick) => 12 - tick * 0.2)];
    const bursts = run(lift);
    expect(bursts).toHaveLength(1);
    expect(bursts[0].z).toBeGreaterThan(11.9);
    expect(bursts[0].tick).toBeGreaterThan(295);
  });

  it("ignores small hills and tall hills climbed quickly on momentum", () => {
    const bump = [...Array.from({ length: 10 }, (_, tick) => 3 + tick * 0.05), ...Array.from({ length: 10 }, (_, tick) => 3.5 - tick * 0.05)];
    expect(run(bump)).toEqual([]);
    const camelback = [...Array.from({ length: 80 }, (_, tick) => 3 + tick * 0.12), ...Array.from({ length: 30 }, (_, tick) => 12.6 - tick * 0.3)];
    expect(run(camelback)).toEqual([]);
  });

  it("draws the same confetti for the same tick and none once it has settled", () => {
    const burst = { x: 20, y: 20, z: 4, tick: 100 };
    const first = buildConfetti([burst], 120, camera, viewport);
    expect(first.length).toBeGreaterThan(20);
    expect(buildConfetti([burst], 120, camera, viewport)).toEqual(first);
    expect(buildConfetti([burst], 100 + CONFETTI_LIFE_TICKS, camera, viewport)).toEqual([]);
  });
});

describe("fireworks", () => {
  const bounds = parkBounds([facility("food_stall", 10, 10), facility("restroom", 30, 30)], []);

  it("do not fly over an empty park, which a new park is until its first facilities arrive", () => {
    const emptyBounds = parkBounds([], []);
    expect(emptyBounds).toBeNull();
    for (let tick = 0; tick < 2000; tick += 10) expect(buildFireworks(emptyBounds, night, tick, camera, viewport)).toEqual({ sparks: [], glows: [] });
  });

  it("only fly at night, and are the same for the same tick", () => {
    for (let tick = 0; tick < 2000; tick += 37) expect(buildFireworks(bounds, noon, tick, camera, viewport).sparks).toEqual([]);
    const ticksWithFireworks = Array.from({ length: 200 }, (_, index) => index * 10).filter((tick) => buildFireworks(bounds, night, tick, camera, viewport).sparks.length > 0);
    expect(ticksWithFireworks.length).toBeGreaterThan(40);
    const tick = ticksWithFireworks[10];
    expect(buildFireworks(bounds, night, tick, camera, viewport)).toEqual(buildFireworks(bounds, night, tick, camera, viewport));
  });

  it("burst above the park", () => {
    for (let tick = 0; tick < 2000; tick += 10) {
      for (const glow of buildFireworks(bounds, night, tick, camera, viewport).glows) {
        const ground = { x: viewport.width / 2, y: viewport.height / 2 };
        expect(glow.y).toBeLessThan(ground.y + 20 * 32);
        expect(Math.abs(glow.x - ground.x)).toBeLessThan(40 * 32);
      }
    }
  });
});

describe("night lights", () => {
  const input = { facilities: [facility("ferris_wheel"), facility("carousel", 22, 18), facility("food_stall", 18, 22)], rails: [rails(30)], sprites: [{ id: "lamp", sprite: "scenery/lamp", view: "rot0", x: 21, y: 21, height: 8 }], atlas, scale: "1" };

  it("are off at noon", () => {
    expect(buildLights(input, noon, 0, camera, viewport)).toEqual({ bulbs: [], glows: [] });
  });

  it("light the rides, the track, building windows and lamps at night", () => {
    const { bulbs, glows } = buildLights(input, night, 0, camera, viewport);
    expect(bulbs.length).toBeGreaterThan(16 + 12 + 5);
    expect(bulbs.every((bulb) => bulb.kind === "light" && bulb.light === 1)).toBe(true);
    expect(glows.length).toBeGreaterThan(bulbs.length);
    const lampHead = glows.filter((glow) => glow.radius > 30);
    expect(lampHead.length).toBeGreaterThan(0);
  });

  it("chase along the track over time", () => {
    const trackOnly = { ...input, facilities: [], sprites: [] };
    const before = buildLights(trackOnly, night, 0, camera, viewport).bulbs.map((bulb) => bulb.color);
    const after = buildLights(trackOnly, night, 8, camera, viewport).bulbs.map((bulb) => bulb.color);
    expect(after).not.toEqual(before);
  });
});

describe("sun shadows", () => {
  const stall = [facility("food_stall")];
  const centre = (phase: number) => {
    const [shadow] = buildShadows(stall, lightingAt(phase), sunAt(phase), camera, viewport);
    return { x: shadow.vertices.reduce((sum, vertex) => sum + vertex.x, 0) / 4, length: Math.hypot(shadow.vertices[0].x - shadow.vertices[2].x, shadow.vertices[0].y - shadow.vertices[2].y) };
  };

  it("fall on opposite sides in the morning and evening, and are longer when the sun is low", () => {
    expect(Math.sign(centre(0.35).x - viewport.width / 2)).not.toBe(Math.sign(centre(0.65).x - viewport.width / 2));
    expect(centre(0.3).length).toBeGreaterThan(centre(0.5).length);
  });

  it("are gone at night", () => {
    expect(buildShadows(stall, night, sunAt(0.95), camera, viewport)).toEqual([]);
  });
});

describe("sky", () => {
  it("shows the sun by day and the moon and no sun at night", () => {
    const day = buildSky(noon, BACKDROPS.light, 0.5, 700, camera, viewport, atlas, "1");
    expect(day.some((quad) => quad.id === "sun")).toBe(true);
    const nightSky = buildSky(night, BACKDROPS.light, 0.95, 700, camera, viewport, atlas, "1");
    expect(nightSky.some((quad) => quad.id === "sun")).toBe(false);
    expect(nightSky.some((quad) => quad.id === "moon")).toBe(true);
  });

  it("thins the clouds on dark stock", () => {
    const cloudOpacity = (sky: ReturnType<typeof buildSky>): number => sky.find((quad) => quad.id === "cloud")!.color[3];
    expect(cloudOpacity(buildSky(noon, BACKDROPS.dark, 0.5, 700, camera, viewport, atlas, "1"))).toBeLessThan(cloudOpacity(buildSky(noon, BACKDROPS.light, 0.5, 700, camera, viewport, atlas, "1")));
  });

  it("is left out when the park fills the view", () => {
    expect(buildSky(noon, BACKDROPS.light, 0.5, -50, camera, viewport, atlas, "1")).toEqual([]);
  });
});
