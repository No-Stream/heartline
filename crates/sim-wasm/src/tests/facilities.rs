use super::WasmSimulation;

#[test]
fn ride_snapshots_expose_idle_cycle_state() {
    let simulation = WasmSimulation::new(42, 0, String::from("demo"));
    let snapshot = simulation.facilities_json();
    let reports: Vec<serde_json::Value> = serde_json::from_str(&snapshot).unwrap();
    assert!(!reports.is_empty());
    assert!(
        reports
            .iter()
            .all(|report| report["ride_state"] == "idle" && report["ride_progress"] == 0.0)
    );
}

#[test]
fn track_construction_rejects_existing_building_footprints() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    simulation.construction_start(440, 508, 5, 0);
    assert!(!simulation.construction_active());
    assert!(!simulation.construction_message().is_empty());
}

#[test]
fn placement_reports_errors_and_exports_and_removes_buildings() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    assert!(
        simulation
            .facilities_json()
            .contains("\"kind\":\"entrance\"")
    );
    assert!(!simulation.place_facility("unknown", 240, 254, 2));
    assert!(!simulation.facility_message().is_empty());
    assert!(simulation.place_facility("carousel", 240, 254, 2));
    assert!(simulation.facility_message().is_empty());
    let snapshot = simulation.facilities_json();
    assert!(snapshot.contains("\"kind\":\"carousel\""));
    assert!(snapshot.contains("\"x\":240,\"y\":254,\"z\":8,\"rotation\":2"));
    assert!(!simulation.place_facility("restroom", 240, 254, 2));
    assert!(!simulation.facility_message().is_empty());
    let id = simulation
        .facilities
        .buildings()
        .iter()
        .find(|building| building.position.x == 240)
        .unwrap()
        .id;
    assert!(simulation.remove_facility(id));
    assert!(
        !simulation
            .facilities
            .buildings()
            .iter()
            .any(|building| building.id == id)
    );
}

#[test]
fn facility_snapshot_has_structured_wire_fields() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    assert!(simulation.place_facility("carousel", 240, 254, 2));
    let reports: Vec<serde_json::Value> =
        serde_json::from_str(&simulation.facilities_json()).unwrap();
    let carousel = reports
        .iter()
        .find(|report| report["kind"] == "carousel" && report["x"] == 240)
        .unwrap();
    assert_eq!(carousel["x"], 240);
    assert_eq!(carousel["y"], 254);
    assert_eq!(carousel["z"], 8);
    assert_eq!(carousel["rotation"], 2);
    assert_eq!(carousel["ride_state"], "idle");
    assert_eq!(carousel["ride_progress"], 0.0);
    assert!(carousel["id"].is_u64());
    assert!(carousel["price"].is_u64());
}
