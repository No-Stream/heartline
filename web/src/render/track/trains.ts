import { projectVertex } from "../projection";
import type { Camera, Viewport } from "../../camera";
import type { CoasterModelKey } from "../../generated/coaster-models";
import type { TrainLayout } from "../../train-layout";
import type { Atlas } from "../scene";
import { add, color, createQuadBuilder, cross, normalize, scale, shade, vector, type TrackPaint, type TrackQuad, type Vec } from "./geometry";

/** Body parts take the train paint; seats a darker shade of it, trim a lighter one, and ink parts the key line. */
type PartPaint = "body" | "seat" | "trim" | "ink";
/** A box in the car's frame, in metres: along the track from the car's (or row's) centre, across it, and above the rails. */
interface CarPart {
  readonly paint: PartPaint;
  readonly along: readonly [number, number];
  /** Sideways offset of the part's centre line from the track's; zero for parts centred on the track. */
  readonly across?: number;
  readonly halfWidth: number;
  readonly up: readonly [number, number];
  /** The front end's narrower cross-section, for a part that tapers forwards like a nose. */
  readonly front?: { readonly halfWidth: number; readonly up: readonly [number, number] };
}
interface TrainLook {
  readonly rowPitchMetres: number;
  readonly seatSpacingMetres: number;
  /** Drawn on every car. */
  readonly car: readonly CarPart[];
  /** Drawn once per row of seats, relative to the row's centre. */
  readonly row: readonly CarPart[];
  /** Drawn on the front car of each train only. */
  readonly lead: readonly CarPart[];
}

const bogie = (halfLength: number): CarPart => ({ paint: "ink", along: [-halfLength, halfLength], halfWidth: 0.45, up: [0.08, 0.3] });
/** The open seating area seen from above: a thin plate in the seat shade lying on the car body's top. */
const interior = (along: readonly [number, number], halfWidth: number, top: number): CarPart => ({ paint: "seat", along, halfWidth, up: [top, top + 0.02] });
/** One seat back per rider, side by side across the car. */
const seatPair = (along: readonly [number, number], up: readonly [number, number]): CarPart[] =>
  [-0.4, 0.4].map((across) => ({ paint: "seat", along, across, halfWidth: 0.3, up }));

/**
 * Each model's cars follow its real inspiration: Rocky Mountain Construction's short, low-sided two-row cars with lap bars;
 * Intamin's sleek launch-coaster cars with high-backed seats and a long pointed lead car; and Great Coaster
 * International's Millennium Flyer, a chain of short single-row cars with tall sides behind a tall, sloping front.
 */
export const TRAIN_LOOKS: Readonly<Record<CoasterModelKey, TrainLook>> = {
  hybrid: {
    rowPitchMetres: 1.12,
    seatSpacingMetres: 0.8,
    car: [bogie(1.0), { paint: "body", along: [-1.3, 1.3], halfWidth: 0.78, up: [0.3, 0.72] }, interior([-1.1, 1.1], 0.62, 0.72)],
    row: [
      ...seatPair([-0.52, -0.3], [0.6, 1.15]),
      { paint: "ink", along: [0.18, 0.26], halfWidth: 0.6, up: [0.84, 0.92] },
    ],
    lead: [{ paint: "body", along: [1.3, 1.85], halfWidth: 0.78, up: [0.3, 0.72], front: { halfWidth: 0.55, up: [0.3, 0.5] } }],
  },
  velocity_viper: {
    rowPitchMetres: 1.12,
    seatSpacingMetres: 0.8,
    car: [
      bogie(1.05),
      { paint: "body", along: [-1.35, 1.35], halfWidth: 0.72, up: [0.26, 0.54] },
      { paint: "trim", along: [-1.35, 1.35], halfWidth: 0.74, up: [0.54, 0.6] },
      interior([-1.15, 1.15], 0.6, 0.6),
    ],
    row: [
      ...seatPair([-0.54, -0.34], [0.5, 1.45]),
      { paint: "ink", along: [0.16, 0.24], halfWidth: 0.55, up: [0.74, 0.82] },
    ],
    lead: [{ paint: "body", along: [1.35, 2.5], halfWidth: 0.72, up: [0.26, 0.6], front: { halfWidth: 0.14, up: [0.26, 0.34] } }],
  },
  wooden_twister: {
    rowPitchMetres: 0,
    seatSpacingMetres: 0.8,
    car: [bogie(0.4), { paint: "body", along: [-0.58, 0.58], halfWidth: 0.74, up: [0.3, 0.98] }, interior([-0.42, 0.36], 0.6, 0.98)],
    row: [
      { paint: "seat", along: [-0.46, -0.3], halfWidth: 0.64, up: [0.8, 1.2] },
      { paint: "ink", along: [0.18, 0.26], halfWidth: 0.58, up: [1.0, 1.07] },
    ],
    lead: [{ paint: "body", along: [0.58, 1.0], halfWidth: 0.74, up: [0.3, 1.35], front: { halfWidth: 0.6, up: [0.3, 0.9] } }],
  },
};

const METRES_PER_TILE = 4;
const RIDER_HEIGHT_TILES = 0.2;

/** Along-track offset of each row's centre from the car's centre, in metres; row 0 is the rearmost. */
function rowOffsetsMetres(layout: TrainLayout): number[] {
  const look = TRAIN_LOOKS[layout.model];
  return Array.from({ length: layout.rowsPerCar }, (_, row) => (row - (layout.rowsPerCar - 1) / 2) * look.rowPitchMetres);
}

export function buildTrainQuads(poses: Float32Array, camera: Camera, viewport: Viewport, paint: TrackPaint, layout: TrainLayout): TrackQuad[] {
  if (poses.length % 11) throw new Error("Invalid train snapshot");
  const look = TRAIN_LOOKS[layout.model];
  const { quads, box, solid } = createQuadBuilder(camera, viewport, paint.ink, paint.lineWeight);
  const body = color(paint.trains);
  const paints: Record<PartPaint, TrackQuad["color"]> = {
    body,
    seat: shade(body, 0.62),
    trim: [body[0] + (1 - body[0]) * 0.55, body[1] + (1 - body[1]) * 0.55, body[2] + (1 - body[2]) * 0.55, 1],
    ink: color(paint.ink),
  };
  const rows = rowOffsetsMetres(layout);
  for (let i = 0; i < poses.length; i += 11) {
    const carId = poses[i];
    const center = vector(poses, i+1), forward = normalize(vector(poses, i+4)), up = normalize(vector(poses, i+7));
    const side = normalize(cross(up, forward));
    const point = (alongMetres: number, acrossMetres: number, upMetres: number): Vec =>
      add(add(add(center, scale(forward, alongMetres / METRES_PER_TILE)), scale(side, acrossMetres / METRES_PER_TILE)), scale(up, upMetres / METRES_PER_TILE));
    const draw = (part: CarPart, offsetMetres: number) => {
      const front = part.front ?? part;
      const ring = (along: number, halfWidth: number, [bottom, top]: readonly [number, number]): Vec[] =>
        [[-1, bottom], [1, bottom], [1, top], [-1, top]].map(([across, height]) => point(along + offsetMetres, (part.across ?? 0) + across * halfWidth, height));
      const corners = [...ring(part.along[0], part.halfWidth, part.up), ...ring(part.along[1], front.halfWidth, front.up)];
      const id = `train-${carId}`;
      if (part.paint === "ink" || part.paint === "trim" || part.up[1] - part.up[0] < 0.05) solid(corners, "train", id, paints[part.paint]);
      else box(corners, "train", id, paints[part.paint]);
    };
    for (const part of look.car) draw(part, 0);
    for (const rowOffset of rows) for (const part of look.row) draw(part, rowOffset);
    if (carId % layout.carsPerTrain === 0) for (const part of look.lead) draw(part, 0);
  }
  return quads;
}

export function buildRiderQuads(poses: Float32Array, camera: Camera, viewport: Viewport, atlas: Atlas, scaleName: string, layout: TrainLayout, occupants?: ReadonlyMap<string, { id: string; sprite: string }>): TrackQuad[] {
  if (poses.length % 11) throw new Error("Invalid train snapshot");
  const riders: TrackQuad[] = [];
  const views = ["se", "sw", "nw", "ne"] as const;
  const cameraDirection: Vec = [[1,1,1], [1,-1,1], [-1,-1,1], [-1,1,1]][camera.rotation] as Vec;
  const rows = rowOffsetsMetres(layout);
  const seatSpacingMetres = TRAIN_LOOKS[layout.model].seatSpacingMetres;
  const seatsPerCar = layout.rowsPerCar * layout.seatsPerRow;
  for (let i = 0; i < poses.length; i += 11) {
    const center = vector(poses, i+1), forward = normalize(vector(poses, i+4)), up = normalize(vector(poses, i+7));
    const side = normalize(cross(up, forward));
    const cameraRight = cross(cameraDirection, up);
    const spriteRight = Math.hypot(...cameraRight) < 1e-8 ? side : normalize(cameraRight);
    const heading = Math.abs(forward[0]) >= Math.abs(forward[1]) ? (forward[0] >= 0 ? 0 : 2) : (forward[1] >= 0 ? 1 : 3);
    for (let seat = 0; seat < seatsPerCar; seat++) if (occupants ? occupants.has(`${poses[i]}-${seat}`) : (poses[i+10] & (1 << seat)) !== 0) {
      const occupant = occupants?.get(`${poses[i]}-${seat}`);
      if (occupants && !occupant) continue;
      const alongMetres = rows[Math.floor(seat / layout.seatsPerRow)];
      const acrossMetres = (seat % layout.seatsPerRow - (layout.seatsPerRow - 1) / 2) * seatSpacingMetres;
      const anchor = add(add(add(center, scale(forward, alongMetres / METRES_PER_TILE)), scale(side, acrossMetres / METRES_PER_TILE)), scale(up, RIDER_HEIGHT_TILES));
      const view = views[(heading + camera.rotation) % 4];
      const sprite = occupant?.sprite ?? `people/${["maya", "otis", "priya", "gus"][seat % 4]}`;
      const frame = atlas.frames[`${sprite}/${view}/idle/0`];
      const placement = frame?.placements[scaleName];
      if (!placement) throw new Error("Atlas is missing rider frame");
      const page = atlas.pages[scaleName][placement.page];
      const factor = 0.012;
      const left = -frame.anchor.x * factor, right = (frame.width - frame.anchor.x) * factor;
      const top = frame.anchor.y * factor, bottom = (frame.anchor.y - frame.height) * factor;
      const points: Vec[] = [[left,top], [right,top], [right,bottom], [left,bottom]].map(([across, vertical]) => add(add(anchor, scale(spriteRight, across)), scale(up, vertical)));
      const uv = [[0,0], [1,0], [1,1], [0,1]];
      const vertices = points.map((point, index) => ({ ...projectVertex(camera, viewport, { x: point[0], y: point[1], z: point[2] }), u: (placement.x + uv[index][0]*placement.width)/page.width, v: (placement.y + uv[index][1]*placement.height)/page.height })) as TrackQuad["vertices"];
      riders.push({ vertices, color: [1,1,1,1], kind: "rider", id: occupant?.id ?? `rider-${poses[i]}-${seat}`, tileX: Math.floor(anchor[0]), tileY: Math.floor(anchor[1]), page: placement.page });
    }
  }
  return riders;
}
