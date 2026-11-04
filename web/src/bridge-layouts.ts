/** Ordered fields in packed WASM bridge arrays. Each array repeats the named record.
 * World positions use tiles unless a field says metres; world record heights use
 * world height units. Guest IDs and appearances align with guest snapshots. Path
 * metadata aligns with paths; rail distances align with pairs of rails.
 * Construction rail, tie, and support piece indices encode material: steel column
 * support style uses -(index + 1) for all three records. Otherwise wooden track adds
 * 1,000,000 to rail and tie indices only; support indices remain unchanged. Placed
 * geometry uses piece IDs; preview rails and ties use indices. Preview supports use
 * zero as a placeholder. Individual steel supports also use -(index + 1). Demo
 * geometry is untagged.
 */
export const BRIDGE_LAYOUTS = {
  DIMENSIONS: ["width", "height"],
  SURFACE: ["north_west", "north_east", "south_east", "south_west", "water_present", "water_height"],
  PATH: ["x", "y", "height", "slope"],
  PATH_METADATA: ["kind", "style", "edge_mask"],
  FACILITY_OPENING: ["x", "y", "height", "direction_mask"],
  CARVED_SPAN: ["x", "y", "bottom", "top"],
  POSITION: ["x", "y", "height"],
  PATH_PREVIEW: ["x", "y", "height", "slope", "kind", "style", "refusal", "path_edges", "facility_openings"],
  GUEST: ["x", "y", "height", "heading_x", "heading_y", "distance", "moving"],
  GUEST_ID: ["guest_id"],
  GUEST_APPEARANCE: ["skin_tone", "hair_style", "clothing_style", "body_shape", "age"],
  COASTER_SAMPLE: ["piece_index", "x", "y", "z", "lateral_x", "lateral_y", "lateral_z", "up_x", "up_y", "up_z", "distance"],
  RAIL: ["piece_index", "start_x", "start_y", "start_z", "start_lateral_x", "start_lateral_y", "start_lateral_z", "start_up_x", "start_up_y", "start_up_z", "end_x", "end_y", "end_z", "end_lateral_x", "end_lateral_y", "end_lateral_z", "end_up_x", "end_up_y", "end_up_z"],
  TIE: ["piece_index", "start_x", "start_y", "start_z", "end_x", "end_y", "end_z", "up_x", "up_y", "up_z", "tangent_x", "tangent_y", "tangent_z"],
  SUPPORT: ["piece_index", "start_x", "start_y", "start_z", "end_x", "end_y", "end_z"],
  TRAIN_POSE: ["car_index", "x", "y", "z", "tangent_x", "tangent_y", "tangent_z", "up_x", "up_y", "up_z", "occupied_seat_mask"],
  RIDER: ["car_index", "seat_index", "guest_id"],
  RAIL_DISTANCE: ["start_metres", "end_metres"],
  BLOCK_CANDIDATE: ["distance_metres"],
} as const;
