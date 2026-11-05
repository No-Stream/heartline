//! Ordered fields in packed WASM bridge arrays. Each array repeats the named record.
//! World positions use tiles unless a field says metres; heights use world height units
//! for world records. Guest IDs and appearances align with guest snapshots. Path
//! metadata aligns with paths; rail distances align with pairs of rails.
//! Construction rail, tie, and support piece indices encode material: steel column
//! support style uses -(index + 1) for all three records. Otherwise wooden track adds
//! 1,000,000 to rail and tie indices only; support indices remain unchanged. Placed
//! geometry uses piece IDs; preview rails and ties use indices. Preview supports use
//! zero as a placeholder. Individual steel supports also use -(index + 1). Demo
//! geometry is untagged.

pub const DIMENSIONS: &[&str] = &["width", "height"];
pub const SURFACE: &[&str] = &[
    "north_west",
    "north_east",
    "south_east",
    "south_west",
    "water_present",
    "water_height",
];
pub const PATH: &[&str] = &["x", "y", "height", "slope"];
pub const PATH_METADATA: &[&str] = &["kind", "style", "edge_mask"];
pub const FACILITY_OPENING: &[&str] = &["x", "y", "height", "direction_mask"];
pub const CARVED_SPAN: &[&str] = &["x", "y", "bottom", "top"];
pub const POSITION: &[&str] = &["x", "y", "height"];
pub const PATH_PREVIEW: &[&str] = &[
    "x",
    "y",
    "height",
    "slope",
    "kind",
    "style",
    "refusal",
    "path_edges",
    "facility_openings",
];
pub const GUEST: &[&str] = &[
    "x",
    "y",
    "height",
    "heading_x",
    "heading_y",
    "distance",
    "moving",
];
pub const GUEST_ID: &[&str] = &["guest_id"];
pub const GUEST_APPEARANCE: &[&str] = &[
    "skin_tone",
    "hair_style",
    "clothing_style",
    "body_shape",
    "age",
];
pub const COASTER_SAMPLE: &[&str] = &[
    "piece_index",
    "x",
    "y",
    "z",
    "lateral_x",
    "lateral_y",
    "lateral_z",
    "up_x",
    "up_y",
    "up_z",
    "distance",
];
pub const RAIL: &[&str] = &[
    "piece_index",
    "start_x",
    "start_y",
    "start_z",
    "start_lateral_x",
    "start_lateral_y",
    "start_lateral_z",
    "start_up_x",
    "start_up_y",
    "start_up_z",
    "end_x",
    "end_y",
    "end_z",
    "end_lateral_x",
    "end_lateral_y",
    "end_lateral_z",
    "end_up_x",
    "end_up_y",
    "end_up_z",
];
pub const TIE: &[&str] = &[
    "piece_index",
    "start_x",
    "start_y",
    "start_z",
    "end_x",
    "end_y",
    "end_z",
    "up_x",
    "up_y",
    "up_z",
    "tangent_x",
    "tangent_y",
    "tangent_z",
];
pub const SUPPORT: &[&str] = &[
    "piece_index",
    "start_x",
    "start_y",
    "start_z",
    "end_x",
    "end_y",
    "end_z",
];
pub const TRAIN_POSE: &[&str] = &[
    "car_index",
    "x",
    "y",
    "z",
    "tangent_x",
    "tangent_y",
    "tangent_z",
    "up_x",
    "up_y",
    "up_z",
    "occupied_seat_mask",
];
pub const TRAIN_STATE: &[&str] = &[
    "train_index",
    "speed",
    "piece_kind",
    "vertical_g",
    "lateral_g",
    "occupied",
];
pub const RIDER: &[&str] = &["car_index", "seat_index", "guest_id"];
pub const RAIL_DISTANCE: &[&str] = &["start_metres", "end_metres"];
pub const BLOCK_CANDIDATE: &[&str] = &["distance_metres"];
