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

export type RideStep = RidePlaceStep | RideChainStep | RideCloseStep | RideBlockSectionsStep;

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

function isRideStep(value: unknown): value is RideStep {
  if (!isRecord(value)) return false;
  if (typeof value.place === "string") {
    return hasOnlyKeys(value, ["place", "launchSpeed", "note"])
      && (value.launchSpeed === undefined || typeof value.launchSpeed === "number")
      && (value.note === undefined || typeof value.note === "string");
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
    && (value.expect === undefined || isRecord(value.expect));
}

/** Read and validate one ride file by its filename stem, such as `timber-tumbler`. */
export function readRide(name: string): RideFile {
  if (!/^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(name)) throw new Error(`Invalid ride filename stem: ${name}`);
  const rideJson = readFileSync(new URL(`../../../content/rides/${name}.ride.json`, import.meta.url), "utf8");
  const parsedRide: unknown = JSON.parse(rideJson);
  if (!isRideFile(parsedRide)) throw new Error(`Invalid ride file content/rides/${name}.ride.json`);
  return parsedRide;
}
