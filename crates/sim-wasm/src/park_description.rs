use std::fmt::{Display, Formatter};

use serde::Deserialize;
use theme_park_sim::park_guests::ParkGuests;
use theme_park_sim::track::ride_file::{RideFile, RideStation};

use crate::WasmSimulation;
use crate::builder_session::BuilderSession;

#[derive(Debug)]
pub enum ParkDescriptionError {
    Json(serde_json::Error),
    Invalid(String),
    Entry {
        collection: &'static str,
        index: usize,
        segment: Option<usize>,
        message: String,
    },
}

impl Display for ParkDescriptionError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Json(error) => write!(formatter, "invalid park description JSON: {error}"),
            Self::Invalid(message) => formatter.write_str(message),
            Self::Entry {
                collection,
                index,
                segment,
                message,
            } => match segment {
                Some(segment) => write!(
                    formatter,
                    "{collection}[{index}] segment {segment}: {message}"
                ),
                None => write!(formatter, "{collection}[{index}]: {message}"),
            },
        }
    }
}

impl std::error::Error for ParkDescriptionError {}

impl From<serde_json::Error> for ParkDescriptionError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ParkDescription {
    format: ParkDescriptionFormat,
    version: u32,
    name: String,
    terrain: TerrainDescription,
    entrance: EntranceDescription,
    paths: Vec<PathDescription>,
    facilities: Vec<FacilityDescription>,
    scenery: Vec<SceneryDescription>,
    coasters: Vec<CoasterDescription>,
}

#[derive(Deserialize)]
enum ParkDescriptionFormat {
    #[serde(rename = "heartline-park")]
    HeartlinePark,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TerrainDescription {
    preset: String,
    edits: Vec<TerrainEditDescription>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TerrainEditDescription {
    command: String,
    parameters: [i32; 8],
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EntranceDescription {
    x: u16,
    y: u16,
    height: i16,
    rotation: u8,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PathDescription {
    kind: String,
    style: u8,
    route: Vec<[i32; 3]>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FacilityDescription {
    kind: String,
    x: u16,
    y: u16,
    height: i16,
    rotation: u8,
    #[serde(default)]
    price: Option<u16>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SceneryDescription {
    kind: String,
    x: u16,
    y: u16,
    rotation: u8,
    quarter: u8,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CoasterDescription {
    ride: RideFile,
    station: RideStation,
    open: bool,
    price: u16,
}

impl WasmSimulation {
    pub fn from_park_description(
        seed: u64,
        guest_count: usize,
        json: &str,
    ) -> Result<Self, ParkDescriptionError> {
        let description: ParkDescription = serde_json::from_str(json)?;
        let ParkDescription {
            format: ParkDescriptionFormat::HeartlinePark,
            version,
            name: _name,
            terrain,
            entrance,
            paths,
            facilities,
            scenery,
            coasters,
        } = description;
        if version != 1 {
            return Err(ParkDescriptionError::Invalid(format!(
                "unsupported park description version {version}"
            )));
        }
        if terrain.preset != "flat" {
            return Err(ParkDescriptionError::Invalid(format!(
                "unknown terrain preset {:?}",
                terrain.preset
            )));
        }
        if coasters.len() > 1 {
            return Err(ParkDescriptionError::Invalid(
                "this build supports one player coaster per park".into(),
            ));
        }

        let mut simulation = Self::new(seed, 0, "new".into());
        if !simulation.remove_facility(0) {
            return Err(ParkDescriptionError::Invalid(
                "could not remove the new preset entrance before loading the description".into(),
            ));
        }
        for (x, y) in [(218, 250), (218, 251)] {
            if !simulation.path_remove(x, y, 8) {
                return Err(ParkDescriptionError::Invalid(format!(
                    "could not remove the new preset path at ({x}, {y}, 8) before loading the description"
                )));
            }
        }

        for (index, edit) in terrain.edits.iter().enumerate() {
            if !simulation.terrain_apply(&edit.command, edit.parameters.to_vec()) {
                return Err(entry_error(
                    "terrain.edits",
                    index,
                    simulation.terrain_message(),
                ));
            }
        }

        for (index, item) in scenery.iter().enumerate() {
            if !simulation.place_scenery(&item.kind, item.x, item.y, item.rotation, item.quarter) {
                return Err(entry_error("scenery", index, simulation.scenery_message()));
            }
        }

        let mut coaster_should_open = false;
        for (index, coaster) in coasters.into_iter().enumerate() {
            let mut ride = coaster.ride;
            ride.station = coaster.station;
            let mut session = BuilderSession::new(simulation);
            session
                .replay_ride(&ride)
                .map_err(|error| entry_error("coasters", index, error.to_string()))?;
            session
                .set_price(coaster.price)
                .map_err(|error| entry_error("coasters", index, error.to_string()))?;
            simulation = session.into_simulation();
            coaster_should_open = coaster.open;
        }

        for (index, path) in paths.iter().enumerate() {
            if path.kind == "normal" {
                build_path(&mut simulation, index, path)?;
            } else if path.kind != "queue" {
                return Err(entry_error(
                    "paths",
                    index,
                    format!("unknown path kind {:?}", path.kind),
                ));
            }
        }

        if !simulation.place_facility_at(
            "entrance",
            entrance.x,
            entrance.y,
            entrance.rotation,
            entrance.height,
        ) {
            return Err(entry_error("entrance", 0, simulation.facility_message()));
        }

        for (index, facility) in facilities.iter().enumerate() {
            let previous_count = simulation.facilities.buildings().len();
            if !simulation.place_facility_at(
                &facility.kind,
                facility.x,
                facility.y,
                facility.rotation,
                facility.height,
            ) {
                return Err(entry_error(
                    "facilities",
                    index,
                    simulation.facility_message(),
                ));
            }
            if let Some(price) = facility.price {
                let id = simulation
                    .facilities
                    .buildings()
                    .get(previous_count)
                    .map(|building| building.id)
                    .ok_or_else(|| {
                        entry_error(
                            "facilities",
                            index,
                            "facility bridge succeeded without adding a building".into(),
                        )
                    })?;
                if !simulation.set_facility_price(id, price) {
                    return Err(entry_error(
                        "facilities",
                        index,
                        simulation.facility_message(),
                    ));
                }
            }
        }

        for (index, path) in paths.iter().enumerate() {
            if path.kind == "queue" {
                build_path(&mut simulation, index, path)?;
            }
        }

        if coaster_should_open && !simulation.construction_set_open(true) {
            return Err(entry_error(
                "coasters",
                0,
                simulation.construction_message(),
            ));
        }

        let entrance_path = simulation
            .facilities
            .entrance(&simulation.world)
            .ok_or_else(|| ParkDescriptionError::Entry {
                collection: "entrance",
                index: 0,
                segment: None,
                message: "entrance has no connected normal path".into(),
            })?;
        let mut park_guests = ParkGuests::new(
            seed,
            guest_count,
            &simulation.world,
            entrance_path,
            guest_count.max(1),
        );
        park_guests.open_with_guests(&simulation.facilities.guest_facilities(&simulation.world));
        simulation.park.simulation = park_guests;

        Ok(simulation)
    }
}

#[wasm_bindgen::prelude::wasm_bindgen]
impl WasmSimulation {
    #[wasm_bindgen::prelude::wasm_bindgen(js_name = from_park_description)]
    pub fn from_park_description_wasm(
        seed: u64,
        guest_count: usize,
        json: String,
    ) -> Result<WasmSimulation, wasm_bindgen::JsValue> {
        Self::from_park_description(seed, guest_count, &json)
            .map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))
    }
}

fn build_path(
    simulation: &mut WasmSimulation,
    index: usize,
    path: &PathDescription,
) -> Result<(), ParkDescriptionError> {
    if path.route.len() < 2 {
        return Err(entry_error(
            "paths",
            index,
            "route must contain at least two positions".into(),
        ));
    }
    for (segment_index, segment) in path.route.windows(2).enumerate() {
        let coordinates = segment.iter().flatten().copied().collect();
        if !simulation.path_build(coordinates, &path.kind, path.style) {
            return Err(ParkDescriptionError::Entry {
                collection: "paths",
                index,
                segment: Some(segment_index),
                message: simulation.path_message(),
            });
        }
    }
    Ok(())
}

fn entry_error(collection: &'static str, index: usize, message: String) -> ParkDescriptionError {
    ParkDescriptionError::Entry {
        collection,
        index,
        segment: None,
        message,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use crate::WasmSimulation;

    fn park_description() -> Value {
        json!({
            "format": "heartline-park",
            "version": 1,
            "name": "New park equivalent",
            "terrain": {"preset": "flat", "edits": []},
            "entrance": {"x": 218, "y": 246, "height": 8, "rotation": 2},
            "paths": [{
                "kind": "normal",
                "style": 0,
                "route": [[218, 250, 8], [218, 251, 8]]
            }],
            "facilities": [],
            "scenery": [],
            "coasters": []
        })
    }

    fn ride_file() -> Value {
        json!({
            "format": "heartline-ride",
            "version": 1,
            "name": "Test ride",
            "model": "hybrid",
            "station": {"x": 241, "y": 249, "z": 12, "heading": "north"},
            "carsPerTrain": 6,
            "trains": 1,
            "steps": []
        })
    }

    #[test]
    fn description_rebuilds_the_new_preset_world_through_bridge_calls() {
        let description = park_description();
        let json = serde_json::to_string(&description).unwrap();

        let described = WasmSimulation::from_park_description(17, 0, &json).unwrap();
        let preset = WasmSimulation::new(17, 0, "new".into());

        assert_eq!(described.world_state_hash(), preset.world_state_hash());
    }

    #[test]
    fn refused_facility_reports_its_index_and_bridge_message() {
        let mut description = park_description();
        description["facilities"] = json!([{
            "kind": "food_stall",
            "x": 400,
            "y": 400,
            "height": 8,
            "rotation": 0
        }]);
        let json = serde_json::to_string(&description).unwrap();

        let error = WasmSimulation::from_park_description(17, 0, &json)
            .err()
            .unwrap();

        assert!(error.to_string().contains("facilities[0]"));
        assert!(error.to_string().contains("MissingAccess"));
    }

    #[test]
    fn descriptions_reject_more_than_one_coaster_before_replaying_either() {
        let mut description = park_description();
        let coaster = json!({
            "ride": ride_file(),
            "station": {"x": 241, "y": 249, "z": 12, "heading": "north"},
            "open": false,
            "price": 6
        });
        description["coasters"] = json!([coaster.clone(), coaster]);
        let json = serde_json::to_string(&description).unwrap();

        let error = WasmSimulation::from_park_description(17, 0, &json)
            .err()
            .unwrap();

        assert!(
            error
                .to_string()
                .contains("this build supports one player coaster per park")
        );
    }

    #[test]
    fn path_refusal_names_the_path_and_route_segment() {
        let mut description = park_description();
        description["paths"][0]["route"] = json!([[218, 250, 8], [218, 251, 8], [218, 252, 100]]);
        let json = serde_json::to_string(&description).unwrap();

        let error = WasmSimulation::from_park_description(17, 0, &json)
            .err()
            .unwrap();

        assert!(error.to_string().contains("paths[0] segment 1"));
        assert!(error.to_string().contains("Path"));
    }

    #[test]
    fn unknown_description_fields_are_rejected() {
        let mut description = park_description();
        description["unknown"] = json!(true);
        let json = serde_json::to_string(&description).unwrap();

        let error = WasmSimulation::from_park_description(17, 0, &json)
            .err()
            .unwrap();

        assert!(error.to_string().contains("unknown field"));
    }

    #[test]
    fn station_path_keeps_a_tested_coaster_testable() {
        let mut description: Value = serde_json::from_str(include_str!(
            "../../../content/parks/horizon-hawk.park.json"
        ))
        .unwrap();
        description["coasters"][0]["ride"] = serde_json::from_str(include_str!(
            "../../../content/rides/horizon-hawk.ride.json"
        ))
        .unwrap();
        description["coasters"][0]["open"] = json!(false);
        flatten_original_station_crossing(&mut description);
        description["paths"] = json!([description["paths"][0].clone()]);
        let mut simulation = WasmSimulation::from_park_description(
            17,
            0,
            &serde_json::to_string(&description).unwrap(),
        )
        .unwrap();

        assert_eq!(simulation.ride_test("player"), "");
        let preview = simulation.path_preview(vec![239, 251, 10, 244, 251, 10], "normal", 0);
        assert!(preview.chunks_exact(9).all(|tile| tile[6] == 13));
        assert!(!simulation.path_build(vec![239, 251, 10, 244, 251, 10], "normal", 0));
        assert_eq!(
            simulation.path_message(),
            "Path would block coaster supports"
        );
        assert_eq!(simulation.ride_test("player"), "");
    }

    #[test]
    fn park_description_reports_a_path_that_blocks_coaster_supports() {
        let mut description: Value = serde_json::from_str(include_str!(
            "../../../content/parks/horizon-hawk.park.json"
        ))
        .unwrap();
        description["coasters"][0]["ride"] = serde_json::from_str(include_str!(
            "../../../content/rides/horizon-hawk.ride.json"
        ))
        .unwrap();
        flatten_original_station_crossing(&mut description);
        description["paths"][1]["route"] = json!([[239, 250, 10], [239, 251, 10], [244, 251, 10]]);

        let error = WasmSimulation::from_park_description(
            17,
            0,
            &serde_json::to_string(&description).unwrap(),
        )
        .err()
        .unwrap();
        assert!(error.to_string().contains("paths[1] segment 1"), "{error}");
        assert!(
            error
                .to_string()
                .contains("Path would block coaster supports"),
            "{error}"
        );
    }

    fn flatten_original_station_crossing(description: &mut Value) {
        let edits = description["terrain"]["edits"].as_array_mut().unwrap();
        for x in 239..=244 {
            edits.push(json!({
                "command": "flatten",
                "parameters": [x, 251, 1, 0, 0, 10, 0, 0]
            }));
        }
    }

    #[test]
    fn existing_ride_description_opens_with_guest_paths_and_facility_prices() {
        let mut description: Value = serde_json::from_str(include_str!(
            "../../../content/parks/horizon-hawk.park.json"
        ))
        .unwrap();
        description["coasters"][0]["ride"] = serde_json::from_str(include_str!(
            "../../../content/rides/horizon-hawk.ride.json"
        ))
        .unwrap();

        let simulation = WasmSimulation::from_park_description(
            17,
            48,
            &serde_json::to_string(&description).unwrap(),
        )
        .unwrap();

        assert!(simulation.park.coaster_open);
        assert_eq!(simulation.guest_ids().len(), 48);
        let facilities: Value = serde_json::from_str(&simulation.facilities_json()).unwrap();
        assert!(
            facilities
                .as_array()
                .unwrap()
                .iter()
                .any(|facility| { facility["kind"] == "food_stall" && facility["price"] == 4 })
        );
        assert!(!simulation.scenery_json().contains("[]"));
    }
}
