import { readFileSync } from "node:fs";

export type RideHeading = "east" | "north" | "west" | "south";

export interface RideStation {
  readonly x: number;
  readonly y: number;
  readonly z: number;
  readonly heading: RideHeading;
}

export interface RidePlaceStep {
  readonly place: string;
  readonly launchSpeed?: number;
  readonly note?: string;
}

export interface RideChainStep {
  readonly chain: boolean;
}

export interface RideCloseStep {
  readonly close: boolean;
}

export interface RideBlockSectionPlacement {
  readonly boundaryMetres: number;
  readonly brakeStartMetres: number;
  readonly brakePieceIndices: readonly number[];
}

export interface RideBlockSectionsStep {
  readonly blockSections: readonly RideBlockSectionPlacement[];
  readonly trainCapacity: number;
  readonly capacitySearchComplete: boolean;
  readonly placementSearchComplete: boolean;
}

export type RideShapeIntent =
  | {
      readonly hill: {
        readonly feel: "Ejector" | "Floater" | "StrongFloater";
        readonly strength_g: number;
        readonly size: "Auto" | { readonly Height: number };
      };
    }
  | {
      readonly turn: {
        readonly hand: "Left" | "Right";
        readonly size: "Auto" | { readonly Radius: number };
      };
    }
  | {
      readonly loop: {
        readonly hand: "Left" | "Right";
        readonly peak_g: number;
        readonly top_feel: "Floaty" | "Neutral" | "PressedIn";
      };
    }
  | {
      readonly zero_g_roll: {
        readonly hand: "Left" | "Right";
        readonly peak_g: number;
      };
    }
  | {
      readonly raven_turn: {
        readonly hand: "Left" | "Right";
        readonly peak_g: number;
        readonly top_feel: "Floaty" | "Neutral" | "PressedIn";
      };
    };

export type RideShapeSpec =
  | {
      readonly kind: "hill";
      readonly design_speed_mps: number;
      readonly cars: number;
      readonly train_length_metres: string;
      readonly target_g: number;
      readonly delivered_g: number;
      readonly height_m: number;
      readonly span_m: number;
      readonly target_height_m: string;
      readonly pull_g: string;
      readonly crest_g: string;
      readonly flank_angle_degrees: string;
      readonly profile_grid_half_tiles: number;
      readonly diagonal: boolean;
      readonly geometry_digest: string;
    }
  | {
      readonly kind: "turn";
      readonly design_speed_mps: number;
      readonly cars: number;
      readonly train_length_metres: string;
      readonly hand: "Left" | "Right";
      readonly vertical_g: number;
      readonly lateral_g: number;
      readonly radius_m: number;
      readonly bank_degrees: number;
      readonly geometry_peak_bank_radians: string;
      readonly geometry_speed_loss_fraction: string;
      readonly geometry_radius_m: string;
      readonly geometry_digest: string;
    };

export interface RideShapeStep {
  readonly intent: RideShapeIntent;
  readonly spec: RideShapeSpec;
}

export type RideStep = RidePlaceStep | RideShapeStep | RideChainStep | RideCloseStep | RideBlockSectionsStep;

export type RideFeatureStep = { readonly place: string } | { readonly placePrefix: string } | { readonly chain: boolean };

export interface RideFeatureExpect {
  readonly piecePrefixesAtLeast?: Readonly<Record<string, number>>;
  readonly piecePrefixGroupsAtLeast?: readonly RidePiecePrefixGroup[];
  readonly heartlinedInversionsAtLeast?: number;
  readonly piecePrefixesAfter?: readonly RidePiecePrefixAfter[];
  readonly orderedSteps?: readonly (readonly RideFeatureStep[])[];
  readonly adjacentSteps?: readonly (readonly RideFeatureStep[])[];
}

export interface RidePiecePrefixGroup {
  readonly prefixes: readonly string[];
  readonly minimum: number;
}

export interface RidePiecePrefixAfter {
  readonly prefix: string;
  readonly after: RideFeatureStep;
}

export interface RideExpect {
  readonly designMargins?: boolean;
  readonly excitement?: readonly [number, number];
  readonly sustainedMaxVerticalG?: readonly [number, number];
  readonly sustainedMinVerticalG?: readonly [number, number];
  readonly maxLateralG?: number;
  readonly inversions?: number;
  readonly airtimeMomentsAtLeast?: number;
  readonly withinComfortableForceLimits?: boolean;
  readonly inversionsAtLeast?: number;
  readonly launches?: number;
  readonly biggestDropAtLeast?: number;
  readonly maxSpeedAtLeast?: number;
  readonly invertedSeconds?: number;
  readonly features?: RideFeatureExpect;
}

export interface RideFile {
  readonly format: "heartline-ride";
  readonly version: 1;
  readonly name: string;
  readonly model: string;
  readonly station: RideStation;
  readonly carsPerTrain: number;
  readonly trains: number;
  readonly steps: readonly RideStep[];
  readonly expect?: RideExpect;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function hasOnlyKeys(value: Record<string, unknown>, keys: readonly string[]): boolean {
  return Object.keys(value).every((key) => keys.includes(key));
}

function isCanonicalU64(value: unknown): value is string {
  if (typeof value !== "string" || !/^(0|[1-9]\d{0,19})$/.test(value)) return false;
  return BigInt(value) <= 18_446_744_073_709_551_615n;
}

function isRideShapeIntent(value: unknown): value is RideShapeIntent {
  if (!isRecord(value)) return false;
  if (isRecord(value.hill)) {
    const hill = value.hill;
    const validSize = hill.size === "Auto"
      || (isRecord(hill.size) && hasOnlyKeys(hill.size, ["Height"]) && typeof hill.size.Height === "number");
    return hasOnlyKeys(value, ["hill"])
      && hasOnlyKeys(hill, ["feel", "strength_g", "size"])
      && (hill.feel === "Ejector" || hill.feel === "Floater" || hill.feel === "StrongFloater")
      && typeof hill.strength_g === "number"
      && validSize;
  }
  if (isRecord(value.turn)) {
    const turn = value.turn;
    const validSize = turn.size === "Auto"
      || (isRecord(turn.size) && hasOnlyKeys(turn.size, ["Radius"]) && typeof turn.size.Radius === "number");
    return hasOnlyKeys(value, ["turn"])
      && hasOnlyKeys(turn, ["hand", "size"])
      && (turn.hand === "Left" || turn.hand === "Right")
      && validSize;
  }
  return false;
}

function isRideShapeSpec(value: unknown): value is RideShapeSpec {
  if (!isRecord(value) || typeof value.kind !== "string") return false;
  if (value.kind === "hill") {
    return hasOnlyKeys(value, ["kind", "design_speed_mps", "cars", "train_length_metres", "target_g", "delivered_g", "height_m", "span_m", "target_height_m", "pull_g", "crest_g", "flank_angle_degrees", "profile_grid_half_tiles", "diagonal", "geometry_digest"])
      && typeof value.design_speed_mps === "number"
      && Number.isInteger(value.cars)
      && typeof value.train_length_metres === "string"
      && typeof value.target_g === "number"
      && typeof value.delivered_g === "number"
      && typeof value.height_m === "number"
      && typeof value.span_m === "number"
      && typeof value.target_height_m === "string"
      && typeof value.pull_g === "string"
      && typeof value.crest_g === "string"
      && typeof value.flank_angle_degrees === "string"
      && Number.isInteger(value.profile_grid_half_tiles)
      && typeof value.diagonal === "boolean"
      && isCanonicalU64(value.geometry_digest);
  }
  if (value.kind === "turn") {
    return hasOnlyKeys(value, ["kind", "design_speed_mps", "cars", "train_length_metres", "hand", "vertical_g", "lateral_g", "radius_m", "bank_degrees", "geometry_peak_bank_radians", "geometry_speed_loss_fraction", "geometry_radius_m", "geometry_digest"])
      && typeof value.design_speed_mps === "number"
      && Number.isInteger(value.cars)
      && typeof value.train_length_metres === "string"
      && (value.hand === "Left" || value.hand === "Right")
      && typeof value.vertical_g === "number"
      && typeof value.lateral_g === "number"
      && typeof value.radius_m === "number"
      && typeof value.bank_degrees === "number"
      && typeof value.geometry_peak_bank_radians === "string"
      && typeof value.geometry_speed_loss_fraction === "string"
      && typeof value.geometry_radius_m === "string"
      && isCanonicalU64(value.geometry_digest);
  }
  return false;
}

function isRideStep(value: unknown): value is RideStep {
  if (!isRecord(value)) return false;
  if (typeof value.place === "string") {
    return hasOnlyKeys(value, ["place", "launchSpeed", "note"])
      && (value.launchSpeed === undefined || typeof value.launchSpeed === "number")
      && (value.note === undefined || typeof value.note === "string");
  }
  if (isRideShapeIntent(value.intent) && isRideShapeSpec(value.spec)) {
    const intentKind = "hill" in value.intent ? "hill" : "turn";
    return intentKind === value.spec.kind && hasOnlyKeys(value, ["intent", "spec"]);
  }
  if (typeof value.chain === "boolean") return hasOnlyKeys(value, ["chain"]);
  if (typeof value.close === "boolean") return hasOnlyKeys(value, ["close"]);
  if (Array.isArray(value.blockSections)) {
    return hasOnlyKeys(value, ["blockSections", "trainCapacity", "capacitySearchComplete", "placementSearchComplete"])
      && value.blockSections.every((section) => isRecord(section)
        && hasOnlyKeys(section, ["boundaryMetres", "brakeStartMetres", "brakePieceIndices"])
        && typeof section.boundaryMetres === "number"
        && typeof section.brakeStartMetres === "number"
        && Array.isArray(section.brakePieceIndices)
        && section.brakePieceIndices.every((index) => Number.isInteger(index)))
      && Number.isInteger(value.trainCapacity)
      && typeof value.capacitySearchComplete === "boolean"
      && typeof value.placementSearchComplete === "boolean";
  }
  return false;
}

function isRideFile(value: unknown): value is RideFile {
  if (!isRecord(value) || !isRecord(value.station)) return false;
  return hasOnlyKeys(value, ["format", "version", "name", "model", "station", "carsPerTrain", "trains", "steps", "expect"])
    && value.format === "heartline-ride"
    && value.version === 1
    && typeof value.name === "string"
    && typeof value.model === "string"
    && hasOnlyKeys(value.station, ["x", "y", "z", "heading"])
    && Number.isInteger(value.station.x)
    && Number.isInteger(value.station.y)
    && Number.isInteger(value.station.z)
    && ["east", "north", "west", "south"].includes(String(value.station.heading))
    && Number.isInteger(value.carsPerTrain)
    && Number.isInteger(value.trains)
    && Array.isArray(value.steps)
    && value.steps.every(isRideStep)
    && (value.expect === undefined || isRideExpect(value.expect));
}

function isRideExpect(value: unknown): value is RideExpect {
  return isRecord(value)
    && (value.designMargins === undefined || typeof value.designMargins === "boolean");
}

/** Read and validate one ride file by its filename stem, such as `timber-tumbler`. */
export function readRide(name: string): RideFile {
  if (!/^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(name)) throw new Error(`Invalid ride filename stem: ${name}`);
  return readRideFile(new URL(`../../../content/rides/${name}.ride.json`, import.meta.url));
}

/** Read and validate a ride file at any path. */
export function readRideFile(path: string | URL): RideFile {
  const parsedRide: unknown = JSON.parse(readFileSync(path, "utf8"));
  if (!isRideFile(parsedRide)) throw new Error(`Invalid ride file ${String(path)}`);
  return parsedRide;
}
