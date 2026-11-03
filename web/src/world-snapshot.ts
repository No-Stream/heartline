export interface WorldSnapshotSource {
  world_dimensions(): Uint16Array;
  world_surfaces(): Int32Array;
  world_paths(): Int32Array;
  world_path_metadata(): Int32Array;
  world_carved(): Int32Array;
}

export interface WorldSnapshot {
  readonly type: "world";
  readonly dimensions: Uint16Array;
  readonly surfaces: Int32Array;
  readonly paths: Int32Array;
  readonly pathMetadata: Int32Array;
  readonly carved: Int32Array;
}

export interface SetSurfaceMessage {
  readonly type: "set-surface";
  readonly x: number;
  readonly y: number;
  readonly corners: Int16Array;
  readonly water?: number;
}

export function validateSurfaceEdit(edit: SetSurfaceMessage, dimensions: Uint16Array): void {
  if (edit.type !== "set-surface" || !Number.isInteger(edit.x) || !Number.isInteger(edit.y) ||
      edit.x < 0 || edit.y < 0 || edit.x >= dimensions[0] || edit.y >= dimensions[1] ||
      !(edit.corners instanceof Int16Array) || edit.corners.length !== 4 ||
      (edit.water !== undefined && (!Number.isInteger(edit.water) || edit.water < -32768 || edit.water > 32767))) {
    throw new RangeError("Invalid terrain surface edit");
  }
}

export function captureWorldSnapshot(source: WorldSnapshotSource): {
  message: WorldSnapshot;
  transfer: ArrayBuffer[];
} {
  const dimensions = source.world_dimensions();
  const surfaces = source.world_surfaces();
  const paths = source.world_paths();
  const pathMetadata = source.world_path_metadata();
  const carved = source.world_carved();
  return {
    message: { type: "world", dimensions, surfaces, paths, pathMetadata, carved },
    transfer: [dimensions.buffer, surfaces.buffer, paths.buffer, pathMetadata.buffer, carved.buffer] as ArrayBuffer[],
  };
}
