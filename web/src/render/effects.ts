/**
 * Effects that follow the time of day: ground shadows, night lights and their glow, fireworks, crest confetti and the sky.
 * Every function is a pure function of its inputs and a tick, so the same tick always draws the same frame.
 */
import { projectWorld, type Camera, type Viewport } from "../camera";
import { carouselPoint, facilityBuildings, facilityFootprint, ferrisWheelPoint, rideAngle, type Facility } from "../facilities";
import { hexColor, type Backdrop, type Lighting, type Rgb, type Sun } from "./daylight";
import { SHAPE_PAGES, type Atlas, type GeometryQuad, type Vertex, type WorldSprite } from "./scene";

/** A soft additive light, in canvas pixels, drawn into the low-resolution glow layer. */
export interface Glow { x: number; y: number; radius: number; color: Rgb; intensity: number }
export interface CrestBurst { x: number; y: number; z: number; tick: number }
export interface CrestTracker { climbing: boolean; climbStartTick: number; climbStartZ: number }
/** The area fireworks are launched over, in tiles; `ground` is the height of the park in tiles. */
export interface ParkBounds { minX: number; minY: number; maxX: number; maxY: number; ground: number }

const INK = {
  plum: hexColor("#3a2e58"), coral: hexColor("#dc5c3e"), mustard: hexColor("#e9ab38"), teal: hexColor("#227571"), slate: hexColor("#3f61a6"), paper: hexColor("#f2eadb"),
};
/** Brighter versions of the inks for light, which is added rather than printed. */
const LIGHT_INK = {
  lamp: hexColor("#ffe2ad"), sign: hexColor("#ffd59a"),
  warm: hexColor("#ffc46b"), mustard: hexColor("#ffd27a"), paper: hexColor("#fff1d6"), coral: hexColor("#ff8a6a"), teal: hexColor("#6cc7b8"), slate: hexColor("#8fa8ec"),
};

/** A repeatable pseudo-random number in [0, 1) for a pair of integers. */
function hash(first: number, second: number): number {
  let value = Math.imul(first | 0, 0x27d4eb2d) ^ Math.imul(second | 0, 0x165667b1);
  value = Math.imul(value ^ (value >>> 15), 0x85ebca6b);
  value = Math.imul(value ^ (value >>> 13), 0xc2b2ae35);
  return ((value ^ (value >>> 16)) >>> 0) / 4294967296;
}

function project(camera: Camera, viewport: Viewport, x: number, y: number, z: number): Vertex {
  const screen = projectWorld(camera, viewport, { x, y, z });
  return { ...screen, depth: [x + y, x - y, -x - y, -x + y][camera.rotation] + z, height: z * 4, u: 0, v: 0 };
}

function onScreen(viewport: Viewport, point: { x: number; y: number }, margin: number): boolean {
  return point.x > -margin && point.y > -margin && point.x < viewport.width + margin && point.y < viewport.height + margin;
}

/** A shape quad centred on a canvas point, turned by `angle`, with u and v spanning the unit square. */
function screenQuad(centre: { x: number; y: number }, halfWidth: number, halfHeight: number, angle: number, depth: number, height: number, page: number, color: Rgb, alpha: number, kind: string, id: string, light: number): GeometryQuad {
  const cos = Math.cos(angle), sin = Math.sin(angle);
  const corners = [[-1, -1], [1, -1], [1, 1], [-1, 1]] as const;
  const vertices = corners.map(([across, down]) => ({
    x: centre.x + across * halfWidth * cos - down * halfHeight * sin,
    y: centre.y + across * halfWidth * sin + down * halfHeight * cos,
    depth, height, u: (across + 1) / 2, v: (down + 1) / 2,
  })) as GeometryQuad["vertices"];
  return { vertices, page, color: [color[0], color[1], color[2], alpha], kind, id, tileX: 0, tileY: 0, light };
}

/** Returns null for a park with no facilities or track yet, such as a new park before its first facilities message. */
export function parkBounds(facilities: readonly Facility[], rails: readonly Float32Array[]): ParkBounds | null {
  const points: [number, number, number][] = facilities.map((facility) => [facility.x + 1, facility.y + 1, facility.z / 4]);
  for (const data of rails) for (let offset = 0; offset < data.length; offset += 19 * 16) points.push([data[offset + 1], data[offset + 2], data[offset + 3]]);
  if (!points.length) return null;
  return {
    minX: Math.min(...points.map(([x]) => x)), maxX: Math.max(...points.map(([x]) => x)),
    minY: Math.min(...points.map(([, y]) => y)), maxY: Math.max(...points.map(([, y]) => y)),
    ground: Math.min(...points.map(([, , z]) => z)),
  };
}

const SHADOW_DEPTH_BIAS = 0.0015;
const facilityHeights: Record<Facility["kind"], number> = { entrance: 2, food_stall: 1.3, drink_stall: 1.3, restroom: 1.2, souvenir: 1.6, carousel: 1.9, ferris_wheel: 2.6 };

/** Soft ground shadows that buildings and rides cast away from the sun, longer when it is low. */
export function buildShadows(facilities: readonly Facility[], lighting: Lighting, sun: Sun, camera: Camera, viewport: Viewport): GeometryQuad[] {
  if (lighting.shadowStrength <= 0.005 || sun.elevation <= 0.05) return [];
  const along = sun.shadowDirection;
  const across = { x: -along.y, y: along.x };
  return facilities.map((facility) => {
    const { width, height } = facilityFootprint(facility);
    const length = Math.min(4, facilityHeights[facility.kind] / Math.tan(sun.elevation));
    const reach = 0.45 * (Math.abs(width * along.x) + Math.abs(height * along.y)) + length / 2;
    const breadth = 0.45 * (Math.abs(width * across.x) + Math.abs(height * across.y));
    const centreX = facility.x + width / 2 + along.x * length / 2, centreY = facility.y + height / 2 + along.y * length / 2;
    const ground = facility.z / 4;
    const vertices = ([[-1, -1], [1, -1], [1, 1], [-1, 1]] as const).map(([u, v]) => {
      const vertex = project(camera, viewport, centreX + along.x * u * reach + across.x * v * breadth, centreY + along.y * u * reach + across.y * v * breadth, ground);
      return { ...vertex, depth: vertex.depth + SHADOW_DEPTH_BIAS, u: (u + 1) / 2, v: (v + 1) / 2 };
    }) as GeometryQuad["vertices"];
    return { vertices, page: SHAPE_PAGES.softDisc, color: [...INK.plum, lighting.shadowStrength] as const, kind: "shadow", id: `shadow-${facility.id}`, tileX: facility.x, tileY: facility.y };
  });
}

export interface LightSources {
  facilities: readonly Facility[];
  /** Rail segments as the simulation sends them: 19 floats each, starting with the piece index and the start position. */
  rails: readonly Float32Array[];
  sprites: readonly WorldSprite[];
  atlas: Atlas;
  scale: string;
}

const TRACK_BULB_SPACING = 3;
/** How far a path lamp lights the ground, in tiles. */
const LAMP_POOL_RADIUS = 2.3;
const POOLS_FROM_LIGHTS_ON = 0.6;
const CHASE_TICKS = 8;
const trackColors = [LIGHT_INK.mustard, LIGHT_INK.paper, LIGHT_INK.coral];
const rideColors = [LIGHT_INK.mustard, LIGHT_INK.coral, LIGHT_INK.paper, LIGHT_INK.teal];

/** A soft disc of light on the ground, as a lamp throws it, drawn into the pool layer that relights the world. */
function groundPool(camera: Camera, viewport: Viewport, x: number, y: number, z: number, radius: number, color: Rgb, intensity: number, id: string): GeometryQuad {
  const vertices = ([[-1, -1], [1, -1], [1, 1], [-1, 1]] as const).map(([across, down]) => ({ ...project(camera, viewport, x + across * radius, y + down * radius, z), u: (across + 1) / 2, v: (down + 1) / 2 })) as GeometryQuad["vertices"];
  return { vertices, page: SHAPE_PAGES.softDisc, color: [color[0], color[1], color[2], intensity], kind: "pool", id, tileX: Math.floor(x), tileY: Math.floor(y), light: 1 };
}

export interface NightLights {
  /** Bulbs, as depth-tested world geometry. */
  bulbs: GeometryQuad[];
  /** Soft halos, added on top of the finished picture. */
  glows: Glow[];
  /** Lamp light on the ground and on lit fronts, drawn into the pool layer; see `lightInPool`. */
  pools: GeometryQuad[];
}

/**
 * Bulbs on rides and track, lit stalls, lamp pools and lamp heads, faded in with `lightsOn`. Bulbs are world geometry,
 * pools relight the world beneath them, and glows are drawn blurred on top.
 */
export function buildLights(sources: LightSources, lighting: Lighting, tick: number, camera: Camera, viewport: Viewport): NightLights {
  const bulbs: GeometryQuad[] = [];
  const glows: Glow[] = [];
  const pools: GeometryQuad[] = [];
  const on = lighting.lightsOn;
  if (on <= 0.01) return { bulbs, glows, pools };
  const zoom = camera.zoom;
  // Pools only show once the land is dark enough for lamplight to matter; at dusk the lamps glow but light nothing yet.
  const poolLight = on * Math.min(1, Math.max(0, (on - POOLS_FROM_LIGHTS_ON) / (1 - POOLS_FROM_LIGHTS_ON)));
  /** A pool is kept only when some of it can reach the screen; `reach` is its radius in tiles. */
  const pool = (x: number, y: number, z: number, reach: number, color: Rgb, intensity: number, id: string): void => {
    if (poolLight > 0 && onScreen(viewport, project(camera, viewport, x, y, z), reach * 64 * zoom)) pools.push(groundPool(camera, viewport, x, y, z, reach, color, intensity * poolLight, id));
  };
  const bulbRadius = Math.max(1.2, 1.7 * zoom);
  const bulb = (x: number, y: number, z: number, color: Rgb, id: string, glowRadius: number, glowIntensity: number): void => {
    const point = project(camera, viewport, x, y, z);
    if (!onScreen(viewport, point, glowRadius)) return;
    bulbs.push(screenQuad(point, bulbRadius, bulbRadius, 0, point.depth + 0.02, point.height, SHAPE_PAGES.disc, color, on, "light", id, 1));
    glows.push({ x: point.x, y: point.y, radius: glowRadius, color, intensity: glowIntensity * on });
  };

  const chase = Math.floor(tick / CHASE_TICKS);
  for (const data of sources.rails) {
    for (let segment = 0; segment * 19 < data.length; segment += TRACK_BULB_SPACING) {
      const offset = segment * 19;
      const color = trackColors[(segment / TRACK_BULB_SPACING + chase) % trackColors.length];
      bulb(data[offset + 1] + data[offset + 7] * 0.07, data[offset + 2] + data[offset + 8] * 0.07, data[offset + 3] + data[offset + 9] * 0.07, color, "track-light", 9 * zoom, 0.3);
    }
  }

  for (const facility of sources.facilities) {
    const id = `facility-light-${facility.id}`;
    const turned = rideAngle(facility);
    if (facility.kind === "ferris_wheel") {
      for (let index = 0; index < 16; index++) bulb(...ferrisWheelPoint(facility, index * Math.PI / 8 + turned, 0.9), rideColors[index % rideColors.length], id, 10 * zoom, 0.35);
      const hub = project(camera, viewport, ...ferrisWheelPoint(facility, 0, 0));
      glows.push({ x: hub.x, y: hub.y, radius: 60 * zoom, color: LIGHT_INK.warm, intensity: 0.22 * on });
      pool(facility.x + 1, facility.y + 1, facility.z / 4, 2.2, LIGHT_INK.lamp, 0.9, id);
    } else if (facility.kind === "carousel") {
      for (let index = 0; index < 12; index++) bulb(...carouselPoint(facility, index * Math.PI / 6 + turned, 1.5, 1), rideColors[index % rideColors.length], id, 10 * zoom, 0.35);
      const inside = project(camera, viewport, ...carouselPoint(facility, 0, 0.9, 0));
      glows.push({ x: inside.x, y: inside.y, radius: 55 * zoom, color: LIGHT_INK.warm, intensity: 0.45 * on });
      pool(facility.x + 1, facility.y + 1, facility.z / 4, 2.4, LIGHT_INK.lamp, 1, id);
    } else if (facilityBuildings[facility.kind]) {
      // Windows light up on the walls that face the camera, where depth grows towards the viewer.
      const { width, height } = facilityFootprint(facility);
      const centre = { x: facility.x + width / 2, y: facility.y + height / 2 };
      // A lit shop throws light from its counter and windows over its front and the ground around it.
      pool(centre.x, centre.y, facility.z / 4, Math.max(width, height) / 2 + 1.3, LIGHT_INK.sign, 1, id);
      for (const [normalX, normalY] of [[1, 0], [-1, 0], [0, 1], [0, -1]] as const) {
        const facing = [normalX + normalY, normalX - normalY, -normalX - normalY, -normalX + normalY][camera.rotation];
        if (facing <= 0) continue;
        const wall = project(camera, viewport, centre.x + normalX * width / 2, centre.y + normalY * height / 2, facility.z / 4 + 0.45);
        if (onScreen(viewport, wall, 30 * zoom)) glows.push({ x: wall.x, y: wall.y, radius: 20 * zoom, color: LIGHT_INK.warm, intensity: 0.5 * on });
      }
    }
  }

  for (const sprite of sources.sprites) {
    if (!sprite.sprite.startsWith("scenery/lamp")) continue;
    const frame = sources.atlas.frames[`${sprite.sprite}/${sprite.view ?? "any"}/${sprite.animation ?? "static"}/${sprite.frame ?? 0}`];
    if (!frame) throw new Error(`Missing atlas frame for lamp ${sprite.id}`);
    const anchor = projectWorld(camera, viewport, { x: sprite.x, y: sprite.y, z: sprite.height / 4 });
    const head = { x: anchor.x, y: anchor.y - (frame.anchor.y - 8) * zoom * (sprite.scale ?? 1) };
    if (onScreen(viewport, head, 40 * zoom)) glows.push({ ...head, radius: 36 * zoom, color: LIGHT_INK.warm, intensity: 0.65 * on });
    pool(sprite.x, sprite.y, sprite.height / 4, LAMP_POOL_RADIUS, LIGHT_INK.lamp, 1, `lamp-pool-${sprite.id}`);
  }
  return { bulbs, glows, pools };
}

const FIREWORK_SLOT_TICKS = 120;
const FIREWORK_RISE_TICKS = 40;
const FIREWORK_BURST_TICKS = 80;
const FIREWORK_SPARKS = 64;
const SPARK_SPEED = 0.34;
const SPARK_DRAG_TICKS = 16;
const SPARK_GRAVITY = 0.0007;
const shellColors = [LIGHT_INK.mustard, LIGHT_INK.coral, LIGHT_INK.teal, LIGHT_INK.slate, LIGHT_INK.paper];

/** A line between two canvas points, drawn as a thin soft-ended streak. */
function streak(from: { x: number; y: number }, to: { x: number; y: number }, width: number, color: Rgb, alpha: number, id: string): GeometryQuad {
  const length = Math.hypot(to.x - from.x, to.y - from.y);
  return screenQuad({ x: (from.x + to.x) / 2, y: (from.y + to.y) / 2 }, Math.max(width, length / 2 + width), width, Math.atan2(to.y - from.y, to.x - from.x), 0, 0, SHAPE_PAGES.softDisc, color, alpha, "firework", id, 1);
}

/** Fireworks over the park on dark nights: at most one shell starts in each four-second slot. Sparks are drawn additively. */
export function buildFireworks(bounds: ParkBounds | null, lighting: Lighting, tick: number, camera: Camera, viewport: Viewport): { sparks: GeometryQuad[]; glows: Glow[] } {
  const sparks: GeometryQuad[] = [];
  const glows: Glow[] = [];
  const darkness = Math.min(1, Math.max(0, (lighting.stars - 0.5) * 2));
  if (bounds === null || darkness <= 0) return { sparks, glows };
  const zoom = camera.zoom;
  const width = Math.max(1.1, 1.5 * zoom);
  const currentSlot = Math.floor(tick / FIREWORK_SLOT_TICKS);
  for (const slot of [currentSlot - 1, currentSlot]) {
    if (hash(slot, 1) > 0.7) continue;
    const age = tick - (slot * FIREWORK_SLOT_TICKS + Math.floor(hash(slot, 2) * 30));
    if (age < 0 || age >= FIREWORK_RISE_TICKS + FIREWORK_BURST_TICKS) continue;
    const id = `firework-${slot}`;
    const x = bounds.minX + hash(slot, 3) * (bounds.maxX - bounds.minX);
    const y = bounds.minY + hash(slot, 4) * (bounds.maxY - bounds.minY);
    const burstHeight = bounds.ground + 10 + 4 * hash(slot, 5);
    const colors = [shellColors[Math.floor(hash(slot, 6) * shellColors.length)], shellColors[Math.floor(hash(slot, 7) * shellColors.length)]];
    const shellHeight = (time: number): number => bounds.ground + (burstHeight - bounds.ground) * (1 - (1 - Math.max(0, time) / FIREWORK_RISE_TICKS) ** 2);
    if (age < FIREWORK_RISE_TICKS) {
      const head = project(camera, viewport, x, y, shellHeight(age));
      sparks.push(streak(project(camera, viewport, x, y, shellHeight(age - 6)), head, width, LIGHT_INK.paper, darkness * 0.8, id));
      glows.push({ x: head.x, y: head.y, radius: 12 * zoom, color: LIGHT_INK.warm, intensity: 0.4 * darkness });
      continue;
    }
    const burstAge = age - FIREWORK_RISE_TICKS;
    const fade = (1 - burstAge / FIREWORK_BURST_TICKS) ** 1.4;
    const centre = project(camera, viewport, x, y, burstHeight);
    glows.push({ x: centre.x, y: centre.y, radius: 170 * zoom, color: colors[0], intensity: darkness * 0.8 * Math.exp(-burstAge / 10) });
    const positionAt = (spark: number, time: number) => {
      const rise = 2 * hash(slot * 131 + spark, 8) - 1;
      const turn = 2 * Math.PI * hash(slot * 131 + spark, 9);
      const travel = SPARK_SPEED * (0.85 + 0.15 * hash(slot * 131 + spark, 10)) * SPARK_DRAG_TICKS * (1 - Math.exp(-Math.max(0, time) / SPARK_DRAG_TICKS));
      const flat = Math.sqrt(1 - rise * rise);
      return project(camera, viewport, x + flat * Math.cos(turn) * travel, y + flat * Math.sin(turn) * travel, burstHeight + rise * travel - SPARK_GRAVITY * time * time);
    };
    for (let spark = 0; spark < FIREWORK_SPARKS; spark++) {
      // Late sparks crackle: each one flickers off now and then.
      if (burstAge > 45 && hash(spark * 7919 + slot, Math.floor(burstAge / 3)) < 0.35) continue;
      const color = colors[spark % 2];
      const head = positionAt(spark, burstAge);
      sparks.push(streak(positionAt(spark, burstAge - 5), head, width, color, darkness * fade, id));
      sparks.push(streak(positionAt(spark, burstAge - 12), positionAt(spark, burstAge - 5), width * 0.7, color, darkness * fade * 0.35, id));
      if (spark % 6 === 0) glows.push({ x: head.x, y: head.y, radius: 16 * zoom, color, intensity: 0.3 * darkness * fade });
    }
  }
  return { sparks, glows };
}

/** A chain lift climbs steadily for several seconds; hills climbed on momentum are over much sooner. */
const CREST_MINIMUM_CLIMB_TICKS = 120;
const CREST_MINIMUM_RISE = 2;
export function createCrestTracker(): CrestTracker {
  return { climbing: false, climbStartTick: 0, climbStartZ: 0 };
}

/** Watches the lead car (car 0) of a train and reports a burst when it tops a climb long and high enough to be a lift hill. */
export function detectCrest(tracker: CrestTracker, poses: Float32Array, tick: number): { tracker: CrestTracker; burst?: CrestBurst } {
  if (poses.length % 11) throw new Error("Invalid train snapshot");
  let lead = -1;
  for (let offset = 0; offset < poses.length; offset += 11) if (poses[offset] === 0) lead = offset;
  if (lead < 0) return { tracker: createCrestTracker() };
  const [x, y, z, climb] = [poses[lead + 1], poses[lead + 2], poses[lead + 3], poses[lead + 6]];
  if (!tracker.climbing) return { tracker: climb > 0.1 ? { climbing: true, climbStartTick: tick, climbStartZ: z } : tracker };
  if (climb > 0.02) return { tracker };
  const liftHill = tick - tracker.climbStartTick >= CREST_MINIMUM_CLIMB_TICKS && z - tracker.climbStartZ >= CREST_MINIMUM_RISE;
  return { tracker: createCrestTracker(), ...(liftHill ? { burst: { x, y, z, tick } } : {}) };
}

export const CONFETTI_LIFE_TICKS = 90;
const CONFETTI_PIECES = 70;
const CONFETTI_DRAG_TICKS = 14;
const confettiColors = [INK.coral, INK.mustard, INK.teal, INK.slate, INK.paper];

/** Paper confetti thrown up from a crest, fluttering and tumbling as it falls. */
export function buildConfetti(bursts: readonly CrestBurst[], tick: number, camera: Camera, viewport: Viewport): GeometryQuad[] {
  const pieces: GeometryQuad[] = [];
  const zoom = camera.zoom;
  for (const burst of bursts) {
    const age = tick - burst.tick;
    if (age < 0 || age >= CONFETTI_LIFE_TICKS) continue;
    const fade = Math.min(1, (CONFETTI_LIFE_TICKS - age) / 20);
    const spread = CONFETTI_DRAG_TICKS * (1 - Math.exp(-age / CONFETTI_DRAG_TICKS));
    for (let piece = 0; piece < CONFETTI_PIECES; piece++) {
      const random = (salt: number): number => hash(burst.tick * 977 + piece, salt);
      const flutter = 0.08 * Math.sin(age * 0.25 + 6.28 * random(1));
      const point = projectWorld(camera, viewport, {
        x: burst.x + (random(2) - 0.5) * 0.22 * spread + flutter,
        y: burst.y + (random(3) - 0.5) * 0.22 * spread - flutter,
        z: burst.z + 0.3 + (0.05 + 0.07 * random(4)) * spread - 0.016 * age,
      });
      const tumble = Math.abs(Math.cos(age * 0.3 + random(5) * 6));
      const angle = random(6) * 6.28 + age * 0.2 * (random(7) > 0.5 ? 1 : -1);
      pieces.push(screenQuad(point, Math.max(1.2, 4.2 * zoom * (0.3 + 0.7 * tumble)), Math.max(1, 2.4 * zoom), angle, 0, 0, SHAPE_PAGES.flat, confettiColors[piece % confettiColors.length], fade, "confetti", "confetti", 0.6));
    }
  }
  return pieces;
}

function atlasQuad(atlas: Atlas, scale: string, key: string, centre: { x: number; y: number }, width: number, alpha: number, id: string, light: number): GeometryQuad {
  const frame = atlas.frames[key];
  const placement = frame?.placements[scale];
  if (!placement) throw new Error(`Missing atlas frame ${key}`);
  const page = atlas.pages[scale][placement.page];
  const height = width * frame.height / frame.width;
  const quad = screenQuad(centre, width / 2, height / 2, 0, 0, 0, placement.page, [1, 1, 1], alpha, "sky", id, light);
  for (const vertex of quad.vertices) {
    vertex.u = (placement.x + vertex.u * placement.width) / page.width;
    vertex.v = (placement.y + vertex.v * placement.height) / page.height;
  }
  return quad;
}

/**
 * The sun, moon and clouds in the sky beyond the park. `horizonY` is where the sky meets the paper below, in canvas pixels;
 * everything here sits behind the world, so it only shows past the park's far edges.
 */
export function buildSky(lighting: Lighting, backdrop: Backdrop, phase: number, horizonY: number, camera: Camera, viewport: Viewport, atlas: Atlas, scale: string): GeometryQuad[] {
  const skyBottom = Math.min(horizonY, viewport.height);
  if (skyBottom < viewport.height * 0.1) return [];
  const quads: GeometryQuad[] = [];
  const { width, height } = viewport;
  const arcHeight = (progress: number): number => skyBottom * (0.85 - 0.65 * Math.sin(Math.PI * progress));
  const dayProgress = (phase - 0.23) / 0.54;
  if (dayProgress > 0 && dayProgress < 1) {
    quads.push(atlasQuad(atlas, scale, "sky/sun/any/static/0", { x: width * (0.12 + 0.76 * dayProgress), y: arcHeight(dayProgress) }, height * 0.14, 1, "sun", 1));
  }
  const nightProgress = ((phase - 0.77 + 1) % 1) / 0.46;
  if (nightProgress > 0 && nightProgress < 1) {
    const moon = { x: width * (0.12 + 0.76 * nightProgress), y: arcHeight(nightProgress) };
    const radius = height * 0.035;
    // Matches the sky shader's gradient, so the disc that cuts the crescent disappears into the sky.
    const skyAtMoon = [0, 1, 2].map((channel) => lighting.skyTop[channel] + (lighting.skyHorizon[channel] - lighting.skyTop[channel]) * Math.max(0, Math.min(1, moon.y / skyBottom)) ** 1.6) as unknown as Rgb;
    quads.push(screenQuad(moon, radius, radius, 0, 0, 0, SHAPE_PAGES.disc, INK.paper, 1, "sky", "moon", 1));
    quads.push(screenQuad({ x: moon.x + radius * 0.45, y: moon.y - radius * 0.25 }, radius * 0.88, radius * 0.88, 0, 0, 0, SHAPE_PAGES.disc, skyAtMoon, 1, "sky", "moon-shadow", 1));
  }
  const clouds = [["sky/cloud_big", 0.08, 0.6, 0.2], ["sky/cloud_small", 0.45, 0.35, 0.32], ["sky/cloud_big", 0.72, 0.45, 0.42]] as const;
  for (const [sprite, offset, size, level] of clouds) {
    const x = ((offset + phase * 3) % 1.3) * (width + height * 0.6) - height * 0.3;
    quads.push(atlasQuad(atlas, scale, `${sprite}/any/static/0`, { x, y: skyBottom * level }, height * 0.22 * size, backdrop.cloudOpacity, "cloud", 0));
  }
  return quads;
}
