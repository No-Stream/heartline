use serde_json::{Value, json};
use theme_park_sim::facilities::station_positions;
use theme_park_sim::guests::ActivityKind;
use theme_park_sim::park::CoasterId;
use theme_park_sim::park_guests::ParkGuests;
use theme_park_sim::track::PieceKind;
use theme_park_sim::track::construction::track_support_obstacles;
use theme_park_sim::world::{PathKind, Position, World};

use super::WasmSimulation;

fn coaster_list(simulation: &WasmSimulation) -> Value {
    serde_json::from_str(&simulation.coasters_json()).expect("coaster list is JSON")
}

fn two_coaster_park_description(open_first: bool, open_second: bool) -> Value {
    let mut description: Value = serde_json::from_str(include_str!(
        "../../../../content/parks/horizon-hawk.park.json"
    ))
    .expect("park description is valid JSON");
    let horizon_hawk: Value = serde_json::from_str(include_str!(
        "../../../../content/rides/horizon-hawk.ride.json"
    ))
    .expect("Horizon Hawk ride file is valid JSON");
    let mut daisy_dash: Value = serde_json::from_str(include_str!(
        "../../../../content/rides/daisy-dash.ride.json"
    ))
    .expect("Daisy Dash ride file is valid JSON");
    daisy_dash["station"]["z"] = json!(10);

    description["coasters"] = json!([
        {
            "ride": horizon_hawk,
            "station": {"x": 241, "y": 249, "z": 10, "heading": "north"},
            "open": open_first,
            "price": 6
        },
        {
            "ride": daisy_dash,
            "station": {"x": 340, "y": 300, "z": 10, "heading": "north"},
            "open": open_second,
            "price": 8
        }
    ]);

    let terrain_edits = description["terrain"]["edits"].as_array_mut().unwrap();
    for x in 337..=344 {
        for y in 300..=302 {
            terrain_edits.push(json!({
                "command": "flatten",
                "parameters": [x, y, 1, 0, 0, 10, 0, 0]
            }));
        }
    }
    for x in [338, 342] {
        for corner in [2, 3] {
            terrain_edits.push(json!({
                "command": "lower_corner",
                "parameters": [x, 302, 1, corner, 1, 0, 0, 0]
            }));
        }
    }
    let paths = description["paths"].as_array_mut().unwrap();
    paths.extend([
        json!({
            "kind": "normal",
            "style": 0,
            "route": [[239, 260, 8], [239, 320, 8], [338, 320, 8]]
        }),
        json!({
            "kind": "normal",
            "style": 0,
            "route": [[338, 301, 10], [338, 320, 8], [343, 320, 8]]
        }),
        json!({
            "kind": "queue",
            "style": 0,
            "route": [[342, 301, 10], [342, 320, 8]]
        }),
    ]);
    description
}

fn compact_guest_park_description() -> Value {
    let mut description = two_coaster_park_description(true, true);
    description["facilities"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "kind": "food_stall",
            "x": 334,
            "y": 318,
            "height": 8,
            "rotation": 2,
            "price": 4
        }));

    let offset_x = 160_i64;
    let offset_y = 200_i64;
    let translate_x = |coordinate: &mut Value| {
        *coordinate = json!(coordinate.as_i64().unwrap() - offset_x);
    };
    let translate_y = |coordinate: &mut Value| {
        *coordinate = json!(coordinate.as_i64().unwrap() - offset_y);
    };
    translate_x(&mut description["entrance"]["x"]);
    translate_y(&mut description["entrance"]["y"]);
    for edit in description["terrain"]["edits"].as_array_mut().unwrap() {
        translate_x(&mut edit["parameters"][0]);
        translate_y(&mut edit["parameters"][1]);
    }
    for path in description["paths"].as_array_mut().unwrap() {
        for point in path["route"].as_array_mut().unwrap() {
            translate_x(&mut point[0]);
            translate_y(&mut point[1]);
        }
    }
    for facility in description["facilities"].as_array_mut().unwrap() {
        translate_x(&mut facility["x"]);
        translate_y(&mut facility["y"]);
    }
    for item in description["scenery"].as_array_mut().unwrap() {
        translate_x(&mut item["x"]);
        translate_y(&mut item["y"]);
    }
    for coaster in description["coasters"].as_array_mut().unwrap() {
        translate_x(&mut coaster["station"]["x"]);
        translate_y(&mut coaster["station"]["y"]);
    }
    description
}

fn load_compact_guest_park(seed: u64, guest_count: usize) -> WasmSimulation {
    let description = compact_guest_park_description();
    let description_json = serde_json::to_string(&description).unwrap();
    let mut simulation =
        WasmSimulation::from_park_description(seed, guest_count, &description_json)
            .expect("compact two-coaster park description loads");
    let original_world = simulation.world.clone();
    let dimensions = (256, 192);
    assert!(
        original_world
            .paths()
            .all(|path| { path.position.x < dimensions.0 && path.position.y < dimensions.1 })
    );
    assert!(
        original_world
            .carved_spans()
            .all(|(x, y, _, _)| { x < dimensions.0 && y < dimensions.1 })
    );
    assert!(
        original_world.scenery().all(|scenery| {
            scenery.position.x < dimensions.0 && scenery.position.y < dimensions.1
        })
    );
    assert!(
        simulation
            .automatic_tunnel_owners
            .keys()
            .all(|cell| { cell.x < dimensions.0 && cell.y < dimensions.1 })
    );
    for (coaster_id, coaster) in simulation.coasters() {
        for piece in coaster.construction.pieces() {
            for position in [piece.entry.position(), piece.exit.position()] {
                let tile_x = (position.x / 4.0).floor() as i32;
                let tile_y = (position.y / 4.0).floor() as i32;
                assert!(
                    (0..i32::from(dimensions.0)).contains(&tile_x),
                    "coaster {} track endpoint x={tile_x} outside compact world width {}",
                    coaster_id.0,
                    dimensions.0
                );
                assert!(
                    (0..i32::from(dimensions.1)).contains(&tile_y),
                    "coaster {} track endpoint y={tile_y} outside compact world height {}",
                    coaster_id.0,
                    dimensions.1
                );
            }
            for obstacle in track_support_obstacles(
                coaster.construction.pieces(),
                &coaster.construction.model().train,
            ) {
                assert!(
                    obstacle.min.x >= 0.0
                        && obstacle.max.x <= f64::from(dimensions.0) * 4.0
                        && obstacle.min.y >= 0.0
                        && obstacle.max.y <= f64::from(dimensions.1) * 4.0,
                    "coaster {} support geometry exceeds the compact world bounds",
                    coaster_id.0
                );
            }
        }
    }
    let default_surface = original_world.surface(0, 0);
    let mut compact_world = World::new(dimensions.0, dimensions.1, default_surface);

    for y in 0..dimensions.1 {
        for x in 0..dimensions.0 {
            let surface = original_world.surface(x, y);
            if surface != default_surface {
                compact_world.set_surface(x, y, surface);
            }
        }
    }
    assert_eq!(
        (0..dimensions.1)
            .flat_map(|y| (0..dimensions.0).map(move |x| (x, y)))
            .filter(|&(x, y)| original_world.surface(x, y) != compact_world.surface(x, y))
            .count(),
        0,
        "compact world preserves every surface inside its bounds"
    );
    for (x, y, bottom, top) in original_world.carved_spans() {
        if x < dimensions.0 && y < dimensions.1 {
            compact_world.carve(x, y, bottom, top);
        }
    }
    for scenery in original_world.scenery() {
        compact_world
            .place_scenery(*scenery)
            .expect("compact world preserves scenery");
    }
    for path_description in description["paths"].as_array().unwrap() {
        let kind = match path_description["kind"].as_str().unwrap() {
            "normal" => PathKind::Normal,
            "queue" => PathKind::Queue,
            other => panic!("unknown test path kind {other}"),
        };
        let style = path_description["style"].as_u64().unwrap() as u8;
        let route = path_description["route"].as_array().unwrap();
        for segment in route.windows(2) {
            let position = |coordinates: &Value| Position {
                x: coordinates[0].as_u64().unwrap() as u16,
                y: coordinates[1].as_u64().unwrap() as u16,
                height: coordinates[2].as_i64().unwrap() as i16,
            };
            compact_world
                .build_path_route(position(&segment[0]), position(&segment[1]), kind, style)
                .expect("compact world preserves paths");
        }
    }
    assert_eq!(
        original_world.paths().count(),
        compact_world.paths().count()
    );
    assert_eq!(
        original_world.carved_spans().count(),
        compact_world.carved_spans().count()
    );
    assert_eq!(
        original_world.scenery().count(),
        compact_world.scenery().count()
    );

    let entrance = simulation
        .facilities
        .entrance(&compact_world)
        .expect("compact world retains entrance access");
    let guest_facilities = simulation.facilities.guest_facilities(&compact_world);
    let mut guests = ParkGuests::new(
        seed,
        guest_count,
        &compact_world,
        entrance,
        guest_count.max(1),
    );
    guests.open_with_guests(&guest_facilities);
    simulation.park.world = compact_world;
    simulation.park.simulation = guests;
    simulation.park.rebind_world_after_loading();
    simulation
}

fn has_guest_queued_for(simulation: &WasmSimulation, coaster_id: CoasterId) -> bool {
    let place_id = u32::MAX - coaster_id.0;
    simulation
        .simulation
        .walking()
        .traits()
        .ids()
        .any(|guest_id| {
            let activity = simulation.simulation.behaviors().activity_detail(guest_id);
            activity.kind == ActivityKind::Queuing && activity.place_id == Some(place_id)
        })
}

fn has_moving_occupied_train(simulation: &WasmSimulation, coaster_id: CoasterId) -> bool {
    let Some(coaster) = simulation.coaster(coaster_id) else {
        return false;
    };
    coaster.coaster_operation.as_ref().is_some_and(|operation| {
        operation.trains().any(|(train_id, train)| {
            train.distance() > 0.0
                && coaster
                    .coaster_riders
                    .get(&train_id)
                    .is_some_and(|riders| !riders.is_empty())
        })
    })
}

#[test]
fn coaster_selection_rename_price_and_demolition_use_stable_ids() {
    let mut simulation = WasmSimulation::new(17, 0, String::from("new"));

    simulation.coaster_start(100, 100, 40, 0);
    assert_eq!(simulation.editing_coaster_id().unwrap().0, 0);
    simulation.coaster_start(300, 300, 40, 0);
    assert_eq!(simulation.editing_coaster_id().unwrap().0, 1);

    let coasters = coaster_list(&simulation);
    assert_eq!(coasters.as_array().unwrap().len(), 2);
    assert_eq!(coasters[0]["id"], 0);
    assert_eq!(coasters[1]["id"], 1);
    let ride_info: Value =
        serde_json::from_str(&simulation.coaster_info_json(0)).expect("coaster info is JSON");
    assert_eq!(ride_info["id"], 0);
    assert!(ride_info["name"].is_string());
    assert!(ride_info["model"].is_string());
    assert!(
        !simulation.coaster_test(0).is_empty(),
        "testing an incomplete coaster reports why it cannot run"
    );

    assert!(simulation.coaster_rename(0, String::from("North Loop")));
    assert!(simulation.coaster_set_price(0, 9));
    assert!(simulation.coaster_set_price(1, 11));
    assert!(simulation.coaster_edit(1));
    simulation.construction_start(400, 400, 40, 0);
    assert_eq!(simulation.editing_coaster_id().unwrap().0, 2);
    let after_start = coaster_list(&simulation);
    assert_eq!(after_start.as_array().unwrap().len(), 3);
    assert_eq!(after_start[0]["id"], 0);
    assert_eq!(after_start[1]["id"], 1);
    assert_eq!(after_start[1]["price"], 11);
    assert_eq!(after_start[2]["id"], 2);
    assert_eq!(after_start[2]["price"], 6);

    assert!(simulation.coaster_edit(0));
    assert_eq!(simulation.editing_coaster_id().unwrap().0, 0);
    simulation.coaster_stop_editing();
    assert!(simulation.editing_coaster_id().is_none());

    assert!(simulation.coaster_demolish(2));
    let remaining = coaster_list(&simulation);
    assert_eq!(remaining.as_array().unwrap().len(), 2);
    assert_eq!(remaining[0]["id"], 0);
    assert_eq!(remaining[0]["name"], "North Loop");
    assert_eq!(remaining[0]["price"], 9);
    assert_eq!(remaining[1]["id"], 1);
    assert_eq!(remaining[1]["price"], 11);
}

#[test]
fn new_coaster_start_rejects_a_station_through_an_existing_coaster() {
    let mut description = two_coaster_park_description(true, false);
    let first_coaster = description["coasters"][0].clone();
    description["coasters"] = json!([first_coaster]);
    let mut simulation =
        WasmSimulation::from_park_description(17, 0, &serde_json::to_string(&description).unwrap())
            .expect("single-coaster park loads");
    let existing_coaster = simulation.coaster(CoasterId(0)).unwrap();
    let existing_station = existing_coaster
        .construction
        .pieces()
        .iter()
        .find(|piece| piece.kind == PieceKind::Station)
        .unwrap();
    assert!(
        simulation
            .validate_track_clearance(
                &existing_coaster.construction.model().train,
                existing_station,
                None
            )
            .is_err()
    );

    simulation.coaster_start(482, 498, 5, 2);

    assert_eq!(coaster_list(&simulation).as_array().unwrap().len(), 1);
    assert!(
        !simulation.construction_message().is_empty(),
        "the overlapping coaster station is refused with a builder message"
    );
}

#[test]
fn undo_history_stays_with_the_selected_coaster() {
    let mut simulation = WasmSimulation::new(17, 0, String::from("new"));

    simulation.coaster_start(100, 100, 40, 0);
    simulation.construction_select("Straight");
    simulation.construction_append();
    assert!(simulation.construction_message().is_empty());
    simulation.coaster_start(300, 300, 40, 0);
    simulation.construction_select("Straight");
    simulation.construction_append();
    assert!(simulation.construction_message().is_empty());

    assert!(simulation.coaster_edit(0));
    simulation.construction_undo();
    assert_eq!(
        simulation
            .coaster(CoasterId(0))
            .unwrap()
            .construction
            .pieces()
            .len(),
        1,
        "undo removes the selected coaster's last append"
    );
    assert_eq!(
        simulation
            .coaster(CoasterId(1))
            .unwrap()
            .construction
            .pieces()
            .len(),
        2,
        "undo leaves the other coaster's track unchanged"
    );
}

#[test]
fn block_analysis_spends_its_budget_on_the_edited_coaster_first() {
    let description = two_coaster_park_description(false, false);
    let mut simulation =
        WasmSimulation::from_park_description(17, 0, &serde_json::to_string(&description).unwrap())
            .expect("two-coaster park description loads");

    for coaster_id in [0, 1] {
        assert!(simulation.coaster_edit(coaster_id));
        assert!(simulation.construction_auto_blocks());
    }
    let first_before = simulation
        .coaster(CoasterId(0))
        .unwrap()
        .block_placement_search
        .as_ref()
        .expect("first coaster has pending block analysis")
        .work_completed();
    let second_before = simulation
        .coaster(CoasterId(1))
        .unwrap()
        .block_placement_search
        .as_ref()
        .expect("second coaster has pending block analysis")
        .work_completed();
    assert!(simulation.coaster_edit(0));

    simulation.construction_advance_block_analysis(1);

    let first_after = simulation
        .coaster(CoasterId(0))
        .unwrap()
        .block_placement_search
        .as_ref()
        .expect("first coaster search remains pending")
        .work_completed();
    let second_after = simulation
        .coaster(CoasterId(1))
        .unwrap()
        .block_placement_search
        .as_ref()
        .expect("second coaster search remains pending")
        .work_completed();
    assert_eq!(first_after, first_before + 1);
    assert_eq!(second_after, second_before);
}

#[test]
fn park_description_loads_two_coasters_with_different_models_and_open_flags() {
    let description = two_coaster_park_description(true, false);

    let simulation =
        WasmSimulation::from_park_description(17, 0, &serde_json::to_string(&description).unwrap())
            .expect("two-coaster park description loads");

    let coasters = coaster_list(&simulation);
    assert_eq!(coasters.as_array().unwrap().len(), 2);
    assert_eq!(coasters[0]["id"], 0);
    assert_eq!(coasters[1]["id"], 1);
    assert_ne!(coasters[0]["model"], coasters[1]["model"]);
    assert_eq!(coasters[0]["open"], true);
    assert_eq!(coasters[1]["open"], false);
    let second_construction = &simulation.coaster(CoasterId(1)).unwrap().construction;
    let second_station = second_construction
        .pieces()
        .iter()
        .find(|piece| piece.kind == PieceKind::Station)
        .expect("second coaster station exists");
    let (queue, exit) = station_positions(second_station).expect("station access positions");
    assert_eq!(
        simulation
            .world
            .path_metadata(queue)
            .unwrap_or_else(|| panic!("missing queue path at {queue:?}; exit is {exit:?}"))
            .kind,
        PathKind::Queue
    );
    assert_eq!(
        simulation
            .world
            .path_metadata(exit)
            .unwrap_or_else(|| panic!("missing exit path at {exit:?}; queue is {queue:?}"))
            .kind,
        PathKind::Normal
    );
}

#[test]
fn two_open_coasters_serve_guests_independently_and_save_mid_ride() {
    let mut simulation = load_compact_guest_park(42, 48);
    let mut same_seed = load_compact_guest_park(42, 48);
    let mut saw_riders = [false; 2];
    let mut saw_queue = [false; 2];
    let mut saw_income = [false; 2];
    let mut saw_ratings = [false; 2];

    for tick in 0..20_000 {
        simulation.tick();
        same_seed.tick();
        if (tick + 1) % 500 == 0 {
            assert_eq!(simulation.state_hash(), same_seed.state_hash());
        }

        for index in 0..2 {
            let coaster_id = CoasterId(index);
            let coaster = simulation.coaster(coaster_id).unwrap();
            saw_riders[index as usize] |= coaster
                .coaster_riders
                .values()
                .any(|riders| !riders.is_empty());
            saw_queue[index as usize] |= has_guest_queued_for(&simulation, coaster_id);
            saw_income[index as usize] |= simulation.coaster_income(coaster_id) > 0;
            saw_ratings[index as usize] |= coaster.coaster_ratings.is_some();
        }
        if saw_riders.into_iter().all(|seen| seen)
            && saw_queue.into_iter().all(|seen| seen)
            && saw_income.into_iter().all(|seen| seen)
            && saw_ratings.into_iter().all(|seen| seen)
            && (0..2).any(|index| has_moving_occupied_train(&simulation, CoasterId(index)))
        {
            break;
        }
    }
    assert_eq!(simulation.state_hash(), same_seed.state_hash());

    assert_eq!(saw_riders, [true, true], "guests rode both coasters");
    assert_eq!(saw_queue, [true, true], "guests queued for both coasters");
    assert_eq!(saw_income, [true, true], "both coasters earned income");
    assert_eq!(saw_ratings, [true, true], "both coasters recorded ratings");
    assert!(
        (0..2).any(|index| has_moving_occupied_train(&simulation, CoasterId(index))),
        "save is taken while a train carries guests"
    );
    for coaster_id in 0..2 {
        let coaster = simulation.coaster(CoasterId(coaster_id)).unwrap();
        let operation = coaster
            .coaster_operation
            .as_ref()
            .expect("open coaster has an operation");
        let train_count = operation.trains().count();
        assert!(train_count > 0, "open coaster has an operating train");
        let expected_pose_count =
            train_count * usize::from(coaster.construction.model().train.cars) * 11;
        assert_eq!(
            simulation.coaster_train_states(coaster_id).len(),
            train_count * 6,
            "open coaster renders only its operating trains"
        );
        assert_eq!(
            simulation.coaster_train_poses(coaster_id).len(),
            expected_pose_count,
            "open coaster does not add a second parked train"
        );
    }

    let bytes = simulation.save_state().unwrap();
    let mut restored = WasmSimulation::load_state(&bytes).unwrap();
    assert_eq!(simulation.state_hash(), restored.state_hash());
    for _ in 0..30 {
        simulation.tick();
        restored.tick();
        assert_eq!(simulation.state_hash(), restored.state_hash());
    }

    let second_coaster_before_close = simulation
        .coaster(CoasterId(1))
        .unwrap()
        .coaster_operation
        .as_ref()
        .expect("second coaster is operating")
        .trains()
        .map(|(_, train)| train.distance())
        .collect::<Vec<_>>();
    assert!(simulation.coaster_set_open(0, false));
    assert!(!simulation.coaster(CoasterId(0)).unwrap().coaster_open);
    assert!(simulation.coaster(CoasterId(1)).unwrap().coaster_open);
    let closed_train_poses = simulation.coaster_train_poses(0);
    let closed_train_states = simulation.coaster_train_states(0);
    assert_eq!(
        closed_train_poses.len(),
        usize::from(
            simulation
                .coaster(CoasterId(0))
                .unwrap()
                .construction
                .model()
                .train
                .cars
        ) * 11
    );
    assert_eq!(closed_train_states.len(), 6);
    assert_eq!(closed_train_states[1], 0.0);
    assert_eq!(closed_train_states[2], 4.0);
    assert_eq!(closed_train_states[5], 0.0);
    assert!(
        simulation
            .coaster(CoasterId(1))
            .unwrap()
            .coaster_operation
            .is_some(),
        "closing one coaster leaves the other operation installed"
    );

    let mut queued_for_first_after_reopen = false;
    assert!(simulation.coaster_set_open(0, true));
    for _ in 0..10_000 {
        simulation.tick();
        if has_guest_queued_for(&simulation, CoasterId(0)) {
            queued_for_first_after_reopen = true;
            break;
        }
    }
    assert!(
        queued_for_first_after_reopen,
        "first coaster has a guest queued before demolition"
    );
    let second_track_hash = simulation
        .coaster(CoasterId(1))
        .unwrap()
        .construction
        .state_hash();
    assert!(simulation.coaster_demolish(0));
    assert!(simulation.coaster(CoasterId(0)).is_none());
    assert_eq!(
        simulation
            .coaster(CoasterId(1))
            .unwrap()
            .construction
            .state_hash(),
        second_track_hash,
        "demolishing one coaster leaves the other track unchanged"
    );
    assert!(simulation.coaster(CoasterId(1)).unwrap().coaster_open);
    assert!(
        simulation
            .coaster(CoasterId(1))
            .unwrap()
            .coaster_operation
            .is_some(),
        "demolishing one coaster leaves the other running"
    );
    for _ in 0..1_000 {
        simulation.tick();
        if !has_guest_queued_for(&simulation, CoasterId(0)) {
            break;
        }
    }
    assert!(
        !has_guest_queued_for(&simulation, CoasterId(0)),
        "guests leave the demolished coaster's queue"
    );
    let second_coaster_after_demolition = simulation
        .coaster(CoasterId(1))
        .unwrap()
        .coaster_operation
        .as_ref()
        .unwrap()
        .trains()
        .map(|(_, train)| train.distance())
        .collect::<Vec<_>>();
    assert_ne!(second_coaster_before_close, second_coaster_after_demolition);
}
