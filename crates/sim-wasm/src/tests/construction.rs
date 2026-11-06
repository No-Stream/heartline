use std::collections::{HashSet, VecDeque};

use super::WasmSimulation;

#[test]
fn candidate_force_preview_reports_selected_track_without_placing_it() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("new"));
    assert_eq!(
        simulation.construction_candidate_force_preview_json(),
        "null"
    );
    simulation.construction_start(100, 100, 20, 0);
    simulation.construction_select("Straight");
    let before = simulation.construction_track();
    let preview: serde_json::Value =
        serde_json::from_str(&simulation.construction_candidate_force_preview_json()).unwrap();
    assert_eq!(preview["status"], "reachedEnd");
    assert_eq!(preview["outerBank"], false);
    assert!(preview["entrySpeed"].as_f64().unwrap() > 0.0);
    assert!(preview["peaks"]["maximumVerticalG"].as_f64().is_some());
    assert_eq!(preview["limits"]["maximumVerticalG"], 4.5);
    assert_eq!(simulation.construction_track(), before);
}

#[test]
fn candidate_force_preview_identifies_outer_banks_for_lateral_readout() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("new"));
    simulation.construction_start(100, 100, 20, 0);
    simulation.construction_select("EjectorHill12x64OuterLeft");

    let preview: serde_json::Value =
        serde_json::from_str(&simulation.construction_candidate_force_preview_json()).unwrap();

    assert_eq!(preview["outerBank"], true);
    assert!(preview["peaks"]["minimumVerticalG"].as_f64().is_some());
    assert!(
        preview["peaks"]["maximumAbsoluteLateralG"]
            .as_f64()
            .is_some()
    );
}

#[test]
fn candidate_force_preview_does_not_invent_peaks_beyond_a_stall() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("new"));
    simulation.construction_start(100, 100, 20, 0);
    simulation.construction_select("LevelToGentleUp");
    assert!(simulation.construction_can_append());
    simulation.construction_append();
    for _ in 0..4 {
        simulation.construction_select("GentleUp");
        assert!(simulation.construction_can_append());
        simulation.construction_append();
    }
    simulation.construction_select("GentleUp");
    let preview: serde_json::Value =
        serde_json::from_str(&simulation.construction_candidate_force_preview_json()).unwrap();
    assert_eq!(preview["status"], "stalled");
    assert!(preview["entrySpeed"].is_null());
    assert!(preview["peaks"].is_null());
}

#[test]
fn diagonal_support_bounds_do_not_fill_its_bounding_rectangle() {
    let simulation = super::WasmSimulation::new(42, 0, String::from("demo"));
    let piece = simulation
        .demo_coaster()
        .expect("demo park")
        .circuit
        .pieces()[0]
        .clone();
    let point = piece.sample(piece.length() / 2.0).position;
    let segment = super::SupportSegment {
        start: point + super::Vec3::new(-40.0, 0.0, 0.0),
        end: point + super::Vec3::new(0.0, 40.0, 0.0),
    };
    let baseline = theme_park_sim::track::proximity::scan_proximity_with_supports(
        &simulation.world,
        std::slice::from_ref(&piece),
        &simulation
            .demo_coaster()
            .expect("demo park")
            .train_configuration,
        &[],
        &[],
        &[],
    );
    let diagonal = theme_park_sim::track::proximity::scan_proximity_with_supports(
        &simulation.world,
        &[piece],
        &simulation
            .demo_coaster()
            .expect("demo park")
            .train_configuration,
        &[],
        &[],
        &[theme_park_sim::track::proximity::OwnedSupportSegment {
            piece_index: None,
            segment,
        }],
    );
    assert_eq!(baseline.events_for_run(&[]), diagonal.events_for_run(&[]));
}

#[test]
fn construction_force_preview_reports_placed_track_and_last_piece_peaks() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("new"));
    assert_eq!(simulation.construction_force_preview_json(), "null");

    simulation.construction_start(100, 100, 20, 0);
    let station_preview: serde_json::Value =
        serde_json::from_str(&simulation.construction_force_preview_json()).unwrap();
    assert!(station_preview["lastPlaced"].is_null());

    simulation.construction_set_chain(true);
    simulation.construction_select("LevelToGentleUp");
    assert!(simulation.construction_can_append());
    simulation.construction_append();

    let preview: serde_json::Value =
        serde_json::from_str(&simulation.construction_force_preview_json()).unwrap();
    let samples = preview["samples"].as_array().unwrap();
    assert!(!samples.is_empty());
    assert!(samples.iter().all(|sample| {
        sample["x"].as_f64().is_some()
            && sample["y"].as_f64().is_some()
            && sample["z"].as_f64().is_some()
    }));
    assert!(samples.windows(2).all(|pair| {
        pair[1]["distance"].as_f64().unwrap() - pair[0]["distance"].as_f64().unwrap() <= 1.001
    }));
    assert_eq!(preview["limits"]["maximumVerticalG"], 4.5);
    assert!(preview["lastPlaced"]["maximumSpeed"].as_f64().is_some());
    assert!(preview["lastPlaced"]["minimumVerticalG"].as_f64().is_some());
    assert!(preview["lastPlaced"]["maximumVerticalG"].as_f64().is_some());
    assert!(
        preview["lastPlaced"]["maximumAbsoluteLateralG"]
            .as_f64()
            .is_some()
    );
    assert!(
        preview["lastPlaced"]["maximumAbsoluteLongitudinalG"]
            .as_f64()
            .is_some()
    );
}

#[test]
fn construction_force_preview_reports_launch_capability_and_estimated_force() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    simulation.construction_select("model:velocity_viper");
    simulation.construction_start(200, 200, 20, 0);
    simulation.construction_select("LaunchShort");
    simulation.construction_append();
    assert!(simulation.construction_set_launch_intensity(1, 12.0));

    let preview: serde_json::Value =
        serde_json::from_str(&simulation.construction_force_preview_json()).unwrap();
    let launch = &preview["launches"][0];
    let track: serde_json::Value = serde_json::from_str(&simulation.construction_track()).unwrap();
    assert_eq!(launch["pieceIndex"], 1);
    assert_eq!(launch["maximumSpeed"], track[1]["launchMaximumSpeed"]);
    assert!(launch["peakLongitudinalG"].as_f64().unwrap() > 0.0);
    let launch_start = simulation.construction.as_ref().unwrap().pieces()[0].length();
    let peak = preview["samples"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|sample| sample["distance"].as_f64().unwrap() >= launch_start)
        .map(|sample| sample["longitudinalG"].as_f64().unwrap().abs())
        .fold(0.0_f64, f64::max);
    assert_eq!(launch["peakLongitudinalG"].as_f64().unwrap(), peak);
    assert!(simulation.construction_set_launch_intensity(1, 14.0));
    let stronger: serde_json::Value =
        serde_json::from_str(&simulation.construction_force_preview_json()).unwrap();
    assert!(
        stronger["launches"][0]["peakLongitudinalG"]
            .as_f64()
            .unwrap()
            > peak
    );
}

const RETURN_PIECES: [theme_park_sim::track::PieceId; 9] = [
    theme_park_sim::track::PieceId::Straight,
    theme_park_sim::track::PieceId::HalfTileStraight,
    theme_park_sim::track::PieceId::OneTileStraight,
    theme_park_sim::track::PieceId::TurnSmallStandalone,
    theme_park_sim::track::PieceId::TurnSmallRightStandalone,
    theme_park_sim::track::PieceId::TurnLargeStandalone,
    theme_park_sim::track::PieceId::TurnLargeRightStandalone,
    theme_park_sim::track::PieceId::OffsetLeft,
    theme_park_sim::track::PieceId::OffsetRight,
];

fn state_key(state: theme_park_sim::track::TrackState) -> (i32, i32, i32, u8, u8, u8, u8, u8) {
    (
        state.x,
        state.y,
        state.z,
        state.heading as u8,
        state.slope as u8,
        state.bank as u8,
        state.horizontal_curvature as u8,
        state.vertical_curvature as u8,
    )
}

fn return_path(
    start: theme_park_sim::track::TrackState,
    target: theme_park_sim::track::TrackState,
    model: &theme_park_sim::track::CoasterModel,
) -> Vec<theme_park_sim::track::PieceId> {
    let templates = theme_park_sim::track::catalog();
    let mut queue = VecDeque::from([(start, Vec::new())]);
    let mut visited = HashSet::from([state_key(start)]);
    while let Some((state, path)) = queue.pop_front() {
        if state == target {
            return path;
        }
        if path.len() >= 80 {
            continue;
        }
        for id in RETURN_PIECES {
            let template = templates.iter().find(|piece| piece.id == id).unwrap();
            if !model.allows_piece(template) {
                continue;
            }
            let Some(next_piece) = template.at(state) else {
                continue;
            };
            let next_state = next_piece.exit;
            if next_state.x.abs() > 500 || next_state.y.abs() > 500 || next_state.z.abs() > 300 {
                continue;
            }
            if !visited.insert(state_key(next_state)) {
                continue;
            }
            let mut next_path = path.clone();
            next_path.push(id);
            queue.push_back((next_state, next_path));
        }
    }
    panic!("no bounded return path from {start:?} to {target:?}");
}

fn append_return_path(simulation: &mut WasmSimulation) {
    let construction = simulation.construction.as_ref().unwrap();
    let route = return_path(
        construction.end(),
        construction.start(),
        construction.model(),
    );
    for id in route {
        simulation.construction_set_chain(false);
        simulation.construction_select(crate::construction::wire_pieceid(id));
        simulation.construction_append();
        assert!(
            simulation.construction_message().is_empty(),
            "{id:?}: {}",
            simulation.construction_message()
        );
    }
    simulation.construction_close();
    assert!(simulation.construction_message().is_empty());
    assert!(simulation.construction.as_ref().unwrap().is_closed());
}

fn append_lift_drop_fixture(simulation: &mut WasmSimulation) {
    simulation.construction_start(100, 100, 40, 0);
    for (piece, chain) in [
        ("LevelToGentleUp", true),
        ("GentleUp", true),
        ("GentleUp", true),
        ("GentleUp", true),
        ("GentleUp", true),
        ("GentleUp", true),
        ("GentleUpToLevel", true),
        ("TurnLargeEntry", true),
        ("TurnLargeExit", false),
        ("OneTileStraight", false),
        ("OffsetLeft", false),
        ("TurnLargeEntry", false),
        ("TurnLargeExit", false),
        ("LevelToGentleDown", false),
        ("GentleDown", false),
        ("GentleDown", false),
        ("GentleDown", false),
        ("GentleDown", false),
        ("GentleDown", false),
        ("GentleDownToLevel", false),
        ("Loop", false),
        ("OffsetRight", false),
        ("TurnLargeEntry", false),
        ("TurnLargeExit", false),
    ] {
        simulation.construction_set_chain(chain);
        simulation.construction_select(piece);
        simulation.construction_append();
        assert!(
            simulation.construction_message().is_empty(),
            "{piece}: {}",
            simulation.construction_message()
        );
    }
    append_return_path(simulation);
}

#[test]
fn wire_piece_names_round_trip_without_catalog_lookup() {
    for piece in theme_park_sim::track::catalog() {
        let wire_name = crate::construction::wire_pieceid(piece.id);
        assert_eq!(
            crate::construction::pieceid_from_wire(wire_name),
            Some(piece.id)
        );
    }
    assert_eq!(
        crate::construction::pieceid_from_wire("unknown-piece"),
        None
    );
}

#[test]
fn station_height_used_by_browser_can_start_construction() {
    let mut simulation = WasmSimulation::new(2026, 12, String::from("demo"));
    simulation.construction_start(100, 100, 40, 0);
    let obstacles = simulation.facility_obstacles();
    let world = &simulation.park.world;
    let construction = simulation.park.construction.as_mut().unwrap();
    construction.set_chain(true);
    for piece_id in [
        theme_park_sim::track::PieceId::LevelToGentleUp,
        theme_park_sim::track::PieceId::GentleUp,
        theme_park_sim::track::PieceId::GentleUp,
        theme_park_sim::track::PieceId::GentleUp,
        theme_park_sim::track::PieceId::GentleUp,
        theme_park_sim::track::PieceId::GentleUp,
        theme_park_sim::track::PieceId::GentleUpToLevel,
        theme_park_sim::track::PieceId::Straight,
    ] {
        construction.append(piece_id, world, &obstacles).unwrap();
    }
    simulation.construction_set_chain(false);
    let choices = simulation.construction_choices();
    let elements = simulation.construction_elements();

    assert!(
        simulation.construction_active(),
        "{}",
        simulation.construction_message()
    );
    assert_eq!(
        simulation
            .construction
            .as_ref()
            .expect("station construction should be active")
            .pieces()
            .len(),
        9
    );
    assert!(!choices.is_empty());
    assert!(!elements.is_empty());
    assert!(simulation.construction_message().is_empty());
}

fn tunnel_construction(surface_height: i16, track_height: i32) -> WasmSimulation {
    let mut simulation = WasmSimulation::new(42, 0, String::from("new"));
    simulation.world = theme_park_sim::world::World::new(
        128,
        128,
        theme_park_sim::world::Surface {
            corners: [surface_height; 4],
            water: None,
        },
    );
    simulation.park.construction = Some(theme_park_sim::track::construction::Construction::new(
        super::TrackState {
            x: 50,
            y: 50,
            z: track_height,
            ..super::TrackState::origin()
        },
        super::hybrid_model(),
    ));
    simulation
}

#[test]
fn browser_route_can_append_gentle_down_after_transition() {
    let mut simulation = WasmSimulation::new(2027, 12, String::from("new"));
    assert!(simulation.terrain_apply("flatten", vec![66, 63, 9, 0, 0, 20, 0, 0]));

    simulation.construction_start(100, 100, 6, 0);
    assert!(
        simulation.construction_active(),
        "{}",
        simulation.construction_message()
    );
    simulation.construction_set_chain(true);
    for piece in [
        "LevelToGentleUp",
        "GentleUp",
        "GentleUp",
        "GentleUp",
        "GentleUp",
        "GentleUp",
        "GentleUpToLevel",
        "Straight",
    ] {
        simulation.construction_select(piece);
        assert!(
            simulation.construction_can_append(),
            "{piece}: {}",
            simulation.construction_preview_reason()
        );
        simulation.construction_append();
        for _ in 0..8 {
            simulation.tick();
        }
        assert!(
            simulation.construction_message().is_empty(),
            "{piece}: {}",
            simulation.construction_message()
        );
    }
    simulation.construction_set_chain(false);
    for piece in [
        "OneTileStraight",
        "OneTileStraight",
        "OffsetLeft",
        "TurnLargeEntry",
        "TurnLargeExit",
        "LevelToGentleDown",
    ] {
        simulation.construction_select(piece);
        assert!(
            simulation.construction_can_append(),
            "{piece}: {}",
            simulation.construction_preview_reason()
        );
        simulation.construction_append();
        for _ in 0..8 {
            simulation.tick();
        }
        assert!(
            simulation.construction_message().is_empty(),
            "{piece}: {}",
            simulation.construction_message()
        );
    }

    let transition_exit = simulation.construction.as_ref().unwrap().end();
    assert_eq!(transition_exit.x, 180, "{transition_exit:?}");
    assert_eq!(transition_exit.y, 133, "{transition_exit:?}");
    assert_eq!(transition_exit.z, 28, "{transition_exit:?}");

    let descending_choice = simulation
        .construction
        .as_ref()
        .unwrap()
        .choices(&simulation.world, &[])
        .into_iter()
        .find(|choice| choice.id == theme_park_sim::track::PieceId::GentleDown)
        .unwrap();
    assert_eq!(descending_choice.result, Ok(()), "{descending_choice:?}");
    simulation.construction_select("GentleDown");
    assert!(
        simulation.construction_can_append(),
        "{}",
        simulation.construction_preview_reason()
    );
    simulation.construction_append();
    for _ in 0..8 {
        simulation.tick();
    }
    assert!(simulation.construction_message().is_empty());

    let mut descents_will_tunnel = descending_choice.would_tunnel;
    for (piece, piece_id) in [
        ("GentleDown", theme_park_sim::track::PieceId::GentleDown),
        ("GentleDown", theme_park_sim::track::PieceId::GentleDown),
        ("GentleDown", theme_park_sim::track::PieceId::GentleDown),
        ("GentleDown", theme_park_sim::track::PieceId::GentleDown),
        (
            "GentleDownToLevel",
            theme_park_sim::track::PieceId::GentleDownToLevel,
        ),
    ] {
        let choice = simulation
            .construction
            .as_ref()
            .unwrap()
            .choices(&simulation.world, &[])
            .into_iter()
            .find(|choice| choice.id == piece_id)
            .unwrap();
        assert_eq!(
            choice.result,
            Ok(()),
            "{piece} from {:?}: {choice:?}",
            simulation.construction.as_ref().unwrap().end(),
        );
        descents_will_tunnel |= choice.would_tunnel;

        let terrain_before_preview = simulation.world_state_hash();
        simulation.construction_select(piece);
        assert_eq!(simulation.construction_selected(), piece);
        assert_eq!(simulation.world_state_hash(), terrain_before_preview);
        assert!(
            simulation.construction_message().is_empty(),
            "{piece} preview supports: {}; choice={choice:?}",
            simulation.construction_message(),
        );
        assert!(
            simulation.construction_can_append(),
            "{piece}: {}",
            simulation.construction_preview_reason()
        );
        if piece == "GentleDownToLevel" {
            let _ = simulation.construction_choices();
            let _ = simulation.construction_elements();
            let _ = simulation.construction_track();
            let _ = simulation.construction_rails();
            let _ = simulation.construction_ties();
            let _ = simulation.construction_supports();
            let _ = simulation.construction_placed_rails();
            let _ = simulation.construction_rail_distances();
            let _ = simulation.construction_block_candidates();
            let _ = simulation.construction_placed_ties();
            let _ = simulation.construction_placed_supports();
            let _ = simulation.construction_block_report_json();
            let _ = simulation.construction_coaster_price();
            let _ = simulation.construction_coaster_income();
            let _ = simulation.construction_colors();
            let _ = simulation.construction_train_layout();
            let _ = simulation.construction_finish_preview_active();
            let _ = simulation.construction_can_close();
            let _ = simulation.construction_can_finish();
            let _ = simulation.construction_finish_reason();
            let _ = simulation.construction_test_report_json();
            let _ = simulation.construction_can_test();
            let _ = simulation.construction_is_open();
            let _ = simulation.construction_station_positions();
            let _ = simulation.construction_train_count();
        }
        if choice.would_tunnel {
            assert!(
                !simulation.construction_supports().is_empty(),
                "{piece} tunnel preview must include its support geometry"
            );
        }
        let terrain_before_append = simulation.world_state_hash();
        simulation.construction_append();
        for _ in 0..8 {
            simulation.tick();
        }
        if choice.would_tunnel {
            assert_ne!(simulation.world_state_hash(), terrain_before_append);
        }
    }
    assert!(
        descents_will_tunnel,
        "the downhill run should preview a tunnel"
    );
}

#[test]
fn finish_preview_does_not_change_track_and_confirmation_closes_it() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    simulation.construction_start(200, 200, 20, 0);
    for piece in [
        "TurnSmallStandalone",
        "TurnSmallStandalone",
        "Straight",
        "TurnSmallStandalone",
    ] {
        simulation.construction_select(piece);
        simulation.construction_append();
        assert!(simulation.construction_message().is_empty());
    }
    let before = simulation.construction_track();
    simulation.construction_finish_search_start();
    while !simulation.construction_finish_search_advance() {}
    simulation.construction_select("Straight");
    assert!(simulation.construction_can_finish());
    simulation.construction_finish_preview();
    assert!(
        simulation.construction_finish_preview_active(),
        "{}",
        simulation.construction_message()
    );
    assert_eq!(simulation.construction_track(), before);
    assert!(!simulation.construction_rails().is_empty());
    simulation.construction_finish_confirm();
    assert!(!simulation.construction_finish_preview_active());
    assert!(simulation.construction.as_ref().unwrap().is_closed());
    assert_ne!(simulation.construction_track(), before);
}

#[test]
fn finish_search_reports_when_the_station_is_missing() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    simulation.construction_start(200, 200, 20, 0);
    simulation.construction_delete(0);
    simulation.construction_finish_search_start();
    assert!(simulation.construction_finish_search_advance());
    assert_eq!(
        simulation.construction_finish_reason(),
        "Place a station before finishing the coaster."
    );
}

#[test]
fn finish_preview_is_cancelled_by_track_edits() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    simulation.construction_start(200, 200, 20, 0);
    for piece in [
        "TurnSmallStandalone",
        "TurnSmallStandalone",
        "Straight",
        "TurnSmallStandalone",
    ] {
        simulation.construction_select(piece);
        simulation.construction_append();
    }
    simulation.construction_finish_search_start();
    while !simulation.construction_finish_search_advance() {}
    simulation.construction_finish_preview();
    assert!(simulation.construction_finish_preview_active());
    simulation.construction_undo();
    let before = simulation.construction_track();
    simulation.construction_finish_confirm();
    assert_eq!(simulation.construction_track(), before);
    assert!(!simulation.construction.as_ref().unwrap().is_closed());
}

#[test]
fn changing_cars_per_train_invalidates_and_rebuilds_finish_preview() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    simulation.construction_start(200, 200, 20, 0);
    for piece in [
        "TurnSmallStandalone",
        "TurnSmallStandalone",
        "Straight",
        "TurnSmallStandalone",
    ] {
        simulation.construction_select(piece);
        simulation.construction_append();
        assert!(simulation.construction_message().is_empty());
    }
    simulation.construction_finish_search_start();
    while !simulation.construction_finish_search_advance() {}
    assert!(simulation.construction_can_finish());
    simulation.construction_finish_preview();
    assert!(simulation.construction_finish_preview_active());
    assert!(simulation.construction_set_cars_per_train(4));
    assert!(!simulation.construction_finish_preview_active());
    assert!(!simulation.construction_can_finish());
    simulation.construction_finish_search_start();
    while !simulation.construction_finish_search_advance() {}
    assert!(simulation.construction_can_finish());
    simulation.construction_finish_preview();
    simulation.construction_finish_confirm();
    assert!(
        simulation.construction.as_ref().unwrap().is_closed(),
        "{}",
        simulation.construction_message()
    );
}

#[test]
fn piece_groups_follow_track_geometry_instead_of_piece_names() {
    let catalog = super::catalog();
    let expected_groups = [
        (super::PieceId::Straight, "straight"),
        (super::PieceId::TurnSmallStandalone, "turn"),
        (super::PieceId::GentleUp, "slope"),
        (super::PieceId::BankLeft, "bank"),
        (super::PieceId::Loop, "inversion"),
        (super::PieceId::Station, "special"),
        (super::PieceId::Brakes, "special"),
        (super::PieceId::Launch, "special"),
        (super::PieceId::TerrainRise, "special"),
        (super::PieceId::TerrainFall, "special"),
        (super::PieceId::ShortOffsetLeft, "straight"),
        (super::PieceId::ShortOffsetRight, "straight"),
        (super::PieceId::BankedStraightLeft, "straight"),
        (super::PieceId::BankedStraightRight, "straight"),
        (super::PieceId::RolloutLeft, "bank"),
        (super::PieceId::RolloutRight, "bank"),
    ];

    for (id, expected_group) in expected_groups {
        let piece = catalog.iter().find(|piece| piece.id == id).unwrap();
        assert_eq!(super::piece_group(piece), expected_group, "{id:?}");
    }
    for piece in catalog.iter().filter(|piece| piece.family.is_some()) {
        let expected = if piece.inversion.is_some() {
            "inversion"
        } else if piece.entry.heading != piece.exit.heading {
            "turn"
        } else {
            "special"
        };
        assert_eq!(super::piece_group(piece), expected, "{:?}", piece.id);
    }

    for (id, expected_name) in [
        (super::PieceId::BankedStraightLeft, "Left-banked straight"),
        (super::PieceId::BankedStraightRight, "Right-banked straight"),
        (super::PieceId::RolloutLeft, "Roll out from left bank"),
        (super::PieceId::RolloutRight, "Roll out from right bank"),
    ] {
        let piece = catalog.iter().find(|piece| piece.id == id).unwrap();
        assert_eq!(piece.name, expected_name, "{id:?}");
    }

    let mut turn_named_straight = catalog
        .iter()
        .find(|piece| piece.id == super::PieceId::Straight)
        .unwrap()
        .clone();
    turn_named_straight.id = super::PieceId::TurnSmallStandalone;
    assert_eq!(super::piece_group(&turn_named_straight), "straight");

    let mut terrain_named_straight = catalog
        .iter()
        .find(|piece| piece.id == super::PieceId::Straight)
        .unwrap()
        .clone();
    terrain_named_straight.exit.z = 1;
    assert_eq!(super::piece_group(&terrain_named_straight), "special");

    let mut climbing_turn = catalog
        .iter()
        .find(|piece| piece.id == super::PieceId::TurnSmallStandalone)
        .unwrap()
        .clone();
    climbing_turn.entry.slope = theme_park_sim::track::Slope::UpGentle;
    climbing_turn.exit.slope = theme_park_sim::track::Slope::UpGentle;
    assert_eq!(super::piece_group(&climbing_turn), "turn");
}

#[test]
fn elements_preview_the_full_sequence_append_atomically_and_undo_together() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    simulation.construction_start(200, 200, 20, 0);
    let before = simulation.construction_track();

    assert!(
        serde_json::from_str::<serde_json::Value>(&simulation.construction_elements())
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .any(|element| element["id"] == "element:climb-60")
    );
    simulation.construction_select("element:climb-60");

    assert_eq!(simulation.construction_selected(), "element:climb-60");
    assert!(
        simulation.construction_can_append(),
        "{}",
        simulation.construction_message()
    );
    let preview = simulation.construction_preview();
    assert!(!preview.is_empty());
    assert!(!simulation.construction_rails().is_empty());

    simulation.construction_append();
    assert!(simulation.construction_message().is_empty());
    assert_eq!(
        track_rows(&simulation.construction_track()).len(),
        track_rows(&before).len() + preview.len()
    );

    simulation.construction_undo();
    assert_eq!(simulation.construction_track(), before);
}

#[test]
fn construction_selects_the_sixty_degree_continuation_after_steep_to_sixty_transition() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    simulation.construction_start(200, 200, 20, 0);
    simulation.construction_set_chain(true);
    simulation.construction_select("element:climb-45");
    assert!(
        simulation.construction_can_append(),
        "{}",
        simulation.construction_message()
    );
    simulation.construction_append();

    simulation.construction_select("SteepToSixtyUp");
    assert!(
        simulation.construction_can_append(),
        "{}",
        simulation.construction_message()
    );
    simulation.construction_append();

    assert_eq!(simulation.construction_selected(), "SixtyUp");
}

#[test]
fn construction_falls_back_to_the_first_valid_piece_without_a_straight_continuation() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    simulation.construction_start(200, 200, 20, 0);
    simulation.construction_select("TurnSmallEntry");
    assert!(
        simulation.construction_can_append(),
        "{}",
        simulation.construction_message()
    );
    simulation.construction_append();

    let choices: serde_json::Value =
        serde_json::from_str(&simulation.construction_choices()).unwrap();
    let choices = choices.as_array().unwrap();
    let first_valid = choices
        .iter()
        .find(|choice| choice["refusal"].as_str().unwrap().is_empty())
        .unwrap()["id"]
        .as_str()
        .unwrap();

    assert_eq!(simulation.construction_selected(), first_valid);
    let construction = simulation.park.construction.as_ref().unwrap();
    let end = construction.end();
    assert_eq!((end.x, end.y, end.z), (216, 208, 20));
    let candidates =
        construction.available_choices(&simulation.park.world, &simulation.facility_obstacles());
    assert!(!candidates.iter().any(|(piece, choice)| {
        choice.result.is_ok()
            && piece.inversion.is_none()
            && matches!(piece.kind, super::PieceKind::Track | super::PieceKind::Lift)
            && super::same_track_orientation(piece.entry, construction.end())
            && super::same_track_orientation(piece.exit, construction.end())
    }));
    assert_eq!(
        simulation.selected,
        super::default_construction_piece_from_choices(&candidates, construction.end())
    );
}

#[test]
fn chain_mode_marks_every_piece_in_an_element_as_a_lift() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    simulation.construction_start(200, 200, 20, 0);
    simulation.construction_set_chain(true);
    simulation.construction_select("element:small-left-banked-turn");

    assert!(
        simulation.construction_can_append(),
        "{}",
        simulation.construction_message()
    );
    let preview = simulation.construction_preview();
    assert_eq!(preview.len(), 3);
    assert!(
        preview
            .iter()
            .all(|piece| piece.kind == super::PieceKind::Lift)
    );

    simulation.construction_append();
    assert_eq!(
        track_rows(&simulation.construction_track())
            .iter()
            .skip(1)
            .map(|row| row["kind"].as_str().unwrap().to_owned())
            .collect::<Vec<_>>(),
        ["Lift", "Lift", "Lift"]
    );
    simulation.construction_undo();
    assert_eq!(track_rows(&simulation.construction_track()).len(), 1);
}

#[test]
fn coaster_ticket_price_defaults_to_six_and_can_be_changed_to_zero() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));

    assert_eq!(simulation.construction_coaster_price(), 6);
    simulation.construction_set_coaster_price(0);

    assert_eq!(simulation.construction_coaster_price(), 0);
    assert_eq!(simulation.construction_coaster_income(), 0);
}

#[test]
fn coaster_cannot_open_without_a_complete_circuit() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    assert!(!simulation.construction_set_open(true));
    simulation.construction_start(200, 200, 20, 0);
    assert!(!simulation.construction_set_open(true));
    assert!(!simulation.construction_is_open());
    assert!(simulation.construction_riders().is_empty());
}

#[test]
fn coaster_requires_station_paths_before_opening() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    simulation.construction_start(200, 200, 20, 0);
    for piece in [
        "TurnSmallStandalone",
        "TurnSmallStandalone",
        "Straight",
        "TurnSmallStandalone",
        "TurnSmallStandalone",
    ] {
        simulation.construction_select(piece);
        simulation.construction_append();
    }
    assert!(simulation.construction_can_close());
    simulation.construction_close();
    assert!(!simulation.construction_set_open(true));
    assert!(!simulation.construction_is_open());
}

#[test]
fn model_selection_controls_new_construction_and_launch_choices() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    simulation.construction_select("model:velocity_viper");
    simulation.construction_start(200, 200, 20, 0);
    assert_eq!(
        simulation.construction.as_ref().unwrap().model().name,
        "Velocity Viper"
    );
    assert!(
        simulation
            .construction_choices()
            .contains("\"id\":\"LaunchShort\"")
    );
    simulation.construction_select("LaunchShort");
    assert!(simulation.construction_can_append());
    let construction = simulation.construction.as_ref().unwrap();
    let obstacles =
        super::track_support_obstacles(construction.pieces(), &construction.model().train);
    let preview = simulation.construction_preview();
    let expected = super::render_preview_supports(
        &preview[0],
        &obstacles,
        &simulation.world,
        super::SupportStyle::SteelColumns,
    )
    .unwrap();
    assert_eq!(
        simulation.construction_supports(),
        simulation.encode_construction_geometry(expected, 7)
    );
    simulation.construction_append();
    assert!(simulation.construction_track().contains("Launch"));
    simulation.construction_select("model:hybrid");
    simulation.construction_start(200, 200, 20, 0);
    assert_eq!(
        simulation.construction.as_ref().unwrap().model().name,
        super::hybrid_model().name
    );
}

#[test]
fn launch_intensity_changes_hash_and_replays_to_the_same_track_state() {
    fn built_viper() -> WasmSimulation {
        let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
        simulation.construction_select("model:velocity_viper");
        simulation.construction_start(200, 200, 20, 0);
        simulation.construction_select("LaunchShort");
        simulation.construction_append();
        assert!(simulation.construction_message().is_empty());
        simulation
    }

    let mut simulation = built_viper();
    let before = simulation.state_hash();
    assert!(simulation.construction_set_launch_intensity(1, 12.0));
    assert_ne!(simulation.state_hash(), before);
    assert!(
        simulation
            .construction_track()
            .contains("\"launchTargetSpeed\":12.0")
    );
    let replay_hash = simulation.state_hash();
    assert!(!simulation.construction_set_launch_intensity(1, 15.4));

    let mut replay = built_viper();
    assert!(replay.construction_set_launch_intensity(1, 12.0));
    assert_eq!(replay.state_hash(), replay_hash);
}

#[test]
fn rail_distances_follow_placed_rail_intervals_across_pieces() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    assert!(simulation.construction_rail_distances().is_empty());
    simulation.construction_start(200, 200, 20, 0);
    simulation.construction_select("Straight");
    simulation.construction_append();
    let construction = simulation.construction.as_ref().unwrap();
    let rails = simulation.construction_placed_rails();
    let distances = simulation.construction_rail_distances();
    assert_eq!(distances.len(), rails.len() / 19);
    let mut expected = Vec::new();
    let mut start = 0.0;
    for piece in construction.pieces() {
        let intervals = (piece.length() / 2.0).ceil() as usize;
        for index in 0..intervals {
            expected.push((start + piece.length() * index as f64 / intervals as f64) as f32);
            expected.push((start + piece.length() * (index + 1) as f64 / intervals as f64) as f32);
        }
        start += piece.length();
    }
    assert_eq!(distances, expected);
}

#[test]
fn wooden_twister_geometry_distinguishes_track_and_steel_loop_supports() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    simulation.construction_select("model:wooden_twister");
    simulation.construction_start(200, 200, 20, 0);
    let rails = simulation.construction_placed_rails();
    assert!(!rails.is_empty());
    assert!(
        rails
            .chunks_exact(19)
            .all(|record| record[0] == 1_000_000.0)
    );
    let ties = simulation.construction_placed_ties();
    assert!(!ties.is_empty());
    assert!(ties.chunks_exact(13).all(|record| record[0] == 1_000_000.0));
    let supports = simulation.construction_placed_supports();
    assert!(!supports.is_empty());
    assert!(supports.chunks_exact(7).all(|record| record[0] == 0.0));

    let loop_piece = super::catalog()
        .iter()
        .find(|piece| piece.id == super::PieceId::SteelLoop)
        .unwrap()
        .at(super::TrackState {
            x: 200,
            y: 200,
            z: 20,
            ..super::TrackState::origin()
        })
        .unwrap();
    let supports = super::render_preview_supports(
        &loop_piece,
        &[],
        &simulation.world,
        super::SupportStyle::DenseWoodenLattice,
    )
    .unwrap();
    assert!(!supports.is_empty());
    assert!(supports.chunks_exact(7).all(|record| record[0] == -1.0));
    let id = 42;
    let model = super::wooden_twister_model();
    let supports = super::render_cached_supports(
        &[(&id, &loop_piece)],
        &simulation.world,
        &model.train,
        model.support_style,
        &[],
        &mut std::collections::HashMap::new(),
    )
    .unwrap();
    assert!(!supports.is_empty());
    assert!(supports.chunks_exact(7).all(|record| record[0] == -43.0));
}

#[test]
fn geometry_encodes_material_and_preserves_piece_identity() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    simulation.construction_start(200, 200, 20, 0);
    let wooden = simulation.construction_placed_rails();
    assert!(wooden.chunks_exact(19).all(|record| record[0] == 0.0));
    simulation.construction_select("model:velocity_viper");
    simulation.construction_start(200, 200, 20, 0);
    simulation.construction_select("LaunchShort");
    simulation.construction_append();
    for (geometry, stride) in [
        (simulation.construction_placed_rails(), 19),
        (simulation.construction_placed_ties(), 13),
        (simulation.construction_placed_supports(), 7),
        (simulation.construction_rails(), 19),
        (simulation.construction_ties(), 13),
        (simulation.construction_supports(), 7),
    ] {
        assert!(!geometry.is_empty());
        assert!(
            geometry
                .chunks_exact(stride)
                .all(|record| record[0] < 0.0 && -record[0] - 1.0 >= 0.0)
        );
    }
    let ids: std::collections::HashSet<_> = simulation
        .construction_placed_rails()
        .chunks_exact(19)
        .map(|record| (-record[0] - 1.0) as usize)
        .collect();
    assert_eq!(ids, [0, 1].into_iter().collect());
}

#[test]
fn construction_commands_validate_and_preview_without_mutating_on_failure() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    simulation.construction_start(200, 200, 20, 0);
    assert!(simulation.construction_active());
    simulation.construction_select("Straight");
    assert!(simulation.construction_can_append());
    assert!(!simulation.construction_rails().is_empty());
    let construction = simulation.construction.as_ref().unwrap();
    let obstacles =
        super::track_support_obstacles(construction.pieces(), &construction.model().train);
    let preview = simulation.construction_preview();
    let expected_supports = super::render_preview_supports(
        &preview[0],
        &obstacles,
        &simulation.world,
        super::SupportStyle::WoodenLattice,
    )
    .unwrap();
    assert_eq!(simulation.construction_supports(), expected_supports);
    simulation.construction_append();
    assert_eq!(
        simulation.construction_track(),
        "[{\"id\":0,\"label\":\"Station\",\"name\":\"Station\",\"group\":\"special\",\"slope\":\"Level\",\"kind\":\"Station\",\"launchTargetSpeed\":null,\"launchMinimumSpeed\":null,\"launchMaximumSpeed\":null},{\"id\":1,\"label\":\"Straight\",\"name\":\"Straight\",\"group\":\"straight\",\"slope\":\"Level\",\"kind\":\"Track\",\"launchTargetSpeed\":null,\"launchMinimumSpeed\":null,\"launchMaximumSpeed\":null}]"
    );
    assert!(!simulation.construction_placed_rails().is_empty());
    simulation.construction_select("GentleUp");
    simulation.construction_append();
    assert_eq!(
        simulation.construction_track(),
        "[{\"id\":0,\"label\":\"Station\",\"name\":\"Station\",\"group\":\"special\",\"slope\":\"Level\",\"kind\":\"Station\",\"launchTargetSpeed\":null,\"launchMinimumSpeed\":null,\"launchMaximumSpeed\":null},{\"id\":1,\"label\":\"Straight\",\"name\":\"Straight\",\"group\":\"straight\",\"slope\":\"Level\",\"kind\":\"Track\",\"launchTargetSpeed\":null,\"launchMinimumSpeed\":null,\"launchMaximumSpeed\":null}]"
    );
    assert!(!simulation.construction_message().is_empty());
    simulation.construction_undo();
    assert_eq!(
        simulation.construction_track(),
        "[{\"id\":0,\"label\":\"Station\",\"name\":\"Station\",\"group\":\"special\",\"slope\":\"Level\",\"kind\":\"Station\",\"launchTargetSpeed\":null,\"launchMinimumSpeed\":null,\"launchMaximumSpeed\":null}]"
    );
}

#[test]
fn deleting_middle_piece_preserves_and_identifies_retained_track() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    simulation.construction_start(200, 200, 20, 0);
    for _ in 0..2 {
        simulation.construction_select("Straight");
        simulation.construction_append();
        assert!(simulation.construction_message().is_empty());
    }
    simulation.construction_delete(1);
    assert_eq!(
        simulation.construction_track(),
        "[{\"id\":0,\"label\":\"Station\",\"name\":\"Station\",\"group\":\"special\",\"slope\":\"Level\",\"kind\":\"Station\",\"launchTargetSpeed\":null,\"launchMinimumSpeed\":null,\"launchMaximumSpeed\":null},{\"id\":2,\"label\":\"Straight\",\"name\":\"Straight\",\"group\":\"straight\",\"slope\":\"Level\",\"kind\":\"Track\",\"launchTargetSpeed\":null,\"launchMinimumSpeed\":null,\"launchMaximumSpeed\":null}]"
    );
    let rails = simulation.construction_placed_rails();
    assert!(rails.chunks_exact(19).any(|rail| rail[0] == 2.0));
    simulation.construction_delete(2);
    assert_eq!(
        simulation.construction_track(),
        "[{\"id\":0,\"label\":\"Station\",\"name\":\"Station\",\"group\":\"special\",\"slope\":\"Level\",\"kind\":\"Station\",\"launchTargetSpeed\":null,\"launchMinimumSpeed\":null,\"launchMaximumSpeed\":null}]"
    );
}

#[test]
fn placing_track_into_a_hill_carves_it_and_undo_restores_terrain() {
    let mut simulation = tunnel_construction(20, 10);
    simulation.construction_select("LevelToGentleDown");
    let piece = simulation
        .construction_preview()
        .into_iter()
        .next()
        .expect("selected track has preview geometry");
    let tunnel_plan = theme_park_sim::track::clearance::tunnel_plan(
        &simulation.world,
        &piece,
        &simulation.construction.as_ref().unwrap().model().train,
    )
    .expect("selected piece has a tunnel plan");
    let midpoint = piece.sample(piece.length() / 2.0).position;
    assert!(
        tunnel_plan.would_tunnel(),
        "tunnel plan: {tunnel_plan:?}; midpoint: {midpoint:?}; tile surface: {:?}",
        simulation
            .world
            .surface((midpoint.x / 4.0) as u16, (midpoint.y / 4.0) as u16)
    );

    let terrain_before_piece = simulation.world_state_hash();
    let mut expected_world = simulation.world.clone();
    for cell in &tunnel_plan.carve_cells {
        expected_world.carve(cell.x, cell.y, cell.height, cell.height + 1);
    }
    assert!(
        simulation.construction_can_append(),
        "{}",
        simulation.construction_preview_reason()
    );
    simulation.construction_append();

    assert_eq!(simulation.construction.as_ref().unwrap().pieces().len(), 1);
    assert_eq!(simulation.world_state_hash(), expected_world.state_hash());
    assert_ne!(simulation.world_state_hash(), terrain_before_piece);
    simulation.construction_undo();
    assert!(
        simulation
            .construction
            .as_ref()
            .unwrap()
            .pieces()
            .is_empty()
    );
    assert_eq!(simulation.world_state_hash(), terrain_before_piece);
}

#[test]
fn deleting_a_middle_piece_restores_only_cells_no_remaining_piece_needs() {
    let mut simulation = tunnel_construction(20, 10);
    let terrain_before_pieces = simulation.world.clone();
    for (index, piece) in ["LevelToGentleDown", "GentleDown", "GentleDownToLevel"]
        .into_iter()
        .enumerate()
    {
        simulation.construction_select(piece);
        simulation.construction_append();
        assert_eq!(
            simulation.construction.as_ref().unwrap().pieces().len(),
            index + 1
        );
    }

    simulation.construction_delete(1);

    let construction = simulation.construction.as_ref().unwrap();
    let retained: Vec<_> = construction
        .piece_ids()
        .iter()
        .zip(construction.pieces())
        .chain(
            construction
                .disconnected_segments()
                .iter()
                .flat_map(|segment| segment.ids.iter().zip(&segment.pieces)),
        )
        .map(|(_, piece)| piece)
        .collect();
    let mut expected_world = terrain_before_pieces.clone();
    let train = construction.model().train.clone();
    let cells: std::collections::BTreeSet<_> = retained
        .iter()
        .flat_map(|piece| {
            theme_park_sim::track::clearance::tunnel_plan(&terrain_before_pieces, piece, &train)
                .expect("retained piece has a tunnel plan")
                .carve_cells
        })
        .collect();
    for cell in cells {
        expected_world.carve(cell.x, cell.y, cell.height, cell.height + 1);
    }
    assert_eq!(simulation.world_state_hash(), expected_world.state_hash());
}

#[test]
fn undo_preserves_space_the_player_carved_by_hand() {
    let mut simulation = tunnel_construction(20, 10);
    simulation.construction_select("LevelToGentleDown");
    let piece = simulation
        .construction_preview()
        .into_iter()
        .next()
        .expect("selected track has preview geometry");
    let train = simulation
        .construction
        .as_ref()
        .unwrap()
        .model()
        .train
        .clone();
    let manual_cell =
        theme_park_sim::track::clearance::tunnel_plan(&simulation.world, &piece, &train)
            .expect("track has a tunnel plan")
            .carve_cells
            .into_iter()
            .next()
            .expect("hill intersects the selected track");
    assert!(simulation.terrain_apply(
        "carve",
        vec![
            i32::from(manual_cell.x),
            i32::from(manual_cell.y),
            1,
            0,
            0,
            0,
            i32::from(manual_cell.height),
            i32::from(manual_cell.height + 1),
        ]
    ));
    let terrain_with_manual_cavern = simulation.world_state_hash();

    simulation.construction_append();
    assert_eq!(simulation.construction.as_ref().unwrap().pieces().len(), 1);
    simulation.construction_undo();

    assert_eq!(simulation.world_state_hash(), terrain_with_manual_cavern);
}

#[test]
fn tunnelled_element_undo_restores_its_whole_sequence() {
    let mut simulation = tunnel_construction(20, 10);
    let terrain_before_element = simulation.world_state_hash();
    simulation.construction_select("element:descend-gentle");
    assert_eq!(simulation.world_state_hash(), terrain_before_element);
    let element_geometry = simulation.construction_preview();
    assert!(!element_geometry.is_empty());
    assert!(
        simulation.construction_message().is_empty(),
        "element preview supports: {}",
        simulation.construction_message()
    );
    assert!(!simulation.construction_supports().is_empty());

    assert!(
        simulation.construction_can_append(),
        "{}",
        simulation.construction_preview_reason()
    );
    simulation.construction_append();

    assert_eq!(
        simulation.construction.as_ref().unwrap().pieces().len(),
        element_geometry.len()
    );
    assert_ne!(simulation.world_state_hash(), terrain_before_element);
    simulation.construction_undo();
    assert_eq!(simulation.world_state_hash(), terrain_before_element);
}

#[test]
fn terrain_change_does_not_panic_when_rendering_supports() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    simulation.construction_start(200, 200, 20, 0);
    assert!(simulation.construction_active());
    assert!(!simulation.construction_placed_supports().is_empty());
    for y in 95..105 {
        for x in 95..110 {
            simulation.set_world_surface(x, y, vec![100; 4], None);
        }
    }
    let supports = simulation.construction_placed_supports();
    assert!(supports.is_empty());
    assert_eq!(
        simulation.construction_message(),
        "Could not route supports for the placed track."
    );
}

#[test]
fn placed_support_cache_matches_full_render_after_track_changes() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    simulation.construction_start(200, 200, 20, 0);
    for _ in 0..4 {
        simulation.construction_select("Straight");
        simulation.construction_append();
        assert!(simulation.construction_message().is_empty());
    }
    for deleted in [None, Some(2), Some(4)] {
        if let Some(id) = deleted {
            simulation.construction_delete(id);
        }
        let construction = simulation.construction.as_ref().unwrap();
        let entries: Vec<_> = construction
            .piece_ids()
            .iter()
            .zip(construction.pieces())
            .chain(
                construction
                    .disconnected_segments()
                    .iter()
                    .flat_map(|segment| segment.ids.iter().zip(&segment.pieces)),
            )
            .collect();
        let pieces: Vec<_> = entries.iter().map(|(_, piece)| (*piece).clone()).collect();
        let mut expected = super::try_render_supports(&pieces, &simulation.world).unwrap();
        for record in expected.chunks_exact_mut(7) {
            record[0] = *entries[record[0] as usize].0 as f32;
        }
        let entry_count = entries.len();
        assert_eq!(simulation.park.support_render_cache.len(), entry_count);
        assert_eq!(simulation.construction_placed_supports(), expected);
        assert_eq!(simulation.park.support_render_cache.len(), entry_count);
        assert_eq!(
            simulation.park.refresh_construction_supports().unwrap(),
            expected
        );
        assert_eq!(simulation.park.support_render_cache.len(), entry_count);
        assert_eq!(simulation.construction_placed_supports(), expected);
    }
}

#[test]
fn deleting_distant_track_reuses_cached_supports() {
    let simulation = WasmSimulation::new(42, 0, String::from("demo"));
    let template = super::catalog()
        .iter()
        .find(|piece| piece.id == super::PieceId::Straight)
        .unwrap();
    let first = template
        .at(super::TrackState {
            x: 100,
            y: 100,
            z: 20,
            ..super::TrackState::origin()
        })
        .unwrap();
    let second = template
        .at(super::TrackState {
            x: 200,
            y: 200,
            z: 20,
            ..super::TrackState::origin()
        })
        .unwrap();
    let first_id = 1;
    let second_id = 2;
    let mut cache = std::collections::HashMap::new();
    let train = super::hybrid_model().train;
    let initial = super::render_cached_supports(
        &[(&first_id, &first), (&second_id, &second)],
        &simulation.world,
        &train,
        super::SupportStyle::WoodenLattice,
        &[],
        &mut cache,
    )
    .unwrap();
    let mut expected_initial =
        super::try_render_supports(&[first.clone(), second], &simulation.world).unwrap();
    for record in expected_initial.chunks_exact_mut(7) {
        record[0] += first_id as f32;
    }
    assert_eq!(initial, expected_initial);
    let segments = cache[&first_id].segments.as_ref().unwrap();
    assert!(!segments.is_empty());
    let original_allocation = segments.as_ptr();
    let cached = super::render_cached_supports(
        &[(&first_id, &first)],
        &simulation.world,
        &train,
        super::SupportStyle::WoodenLattice,
        &[],
        &mut cache,
    )
    .unwrap();
    assert_eq!(
        cache[&first_id].segments.as_ref().unwrap().as_ptr(),
        original_allocation
    );
    let mut expected = super::try_render_supports(&[first], &simulation.world).unwrap();
    for record in expected.chunks_exact_mut(7) {
        record[0] = first_id as f32;
    }
    assert_eq!(cached, expected);
}

#[test]
fn rendered_supports_clear_other_tracks_train_envelope() {
    let world = super::World::new(
        128,
        128,
        super::Surface {
            corners: [0; 4],
            water: None,
        },
    );
    let template = super::catalog()
        .iter()
        .find(|piece| piece.id == super::PieceId::Straight)
        .unwrap();
    let lower = template
        .at(super::TrackState {
            x: 50,
            y: 48,
            z: 3,
            ..super::TrackState::origin()
        })
        .unwrap();
    let upper = template
        .at(super::TrackState {
            x: 54,
            y: 46,
            z: 5,
            heading: super::Heading::North,
            ..super::TrackState::origin()
        })
        .unwrap();
    let obstacles = theme_park_sim::track::construction::track_support_obstacles(
        std::slice::from_ref(&lower),
        &super::hybrid_model().train,
    );
    let expected = super::generate_wooden_lattice(&upper, &world, &obstacles).unwrap();
    let mut expected_records = Vec::new();
    for segment in expected {
        if segment.start != segment.end {
            expected_records.push(1.0);
            super::append_vector(&mut expected_records, segment.start / 4.0);
            super::append_vector(&mut expected_records, segment.end / 4.0);
        }
    }
    let mut cache = std::collections::HashMap::new();
    let actual = super::render_cached_supports(
        &[(&0, &lower), (&1, &upper)],
        &world,
        &super::hybrid_model().train,
        super::SupportStyle::WoodenLattice,
        &[],
        &mut cache,
    )
    .unwrap();
    let upper_records: Vec<_> = actual
        .chunks_exact(7)
        .filter(|record| record[0] == 1.0)
        .flatten()
        .copied()
        .collect();
    assert_eq!(upper_records, expected_records);
    let preview = super::render_preview_supports(
        &upper,
        &obstacles,
        &world,
        super::SupportStyle::WoodenLattice,
    )
    .unwrap();
    let mut expected_preview = expected_records;
    for record in expected_preview.chunks_exact_mut(7) {
        record[0] = 0.0;
    }
    assert_eq!(preview, expected_preview);
}

#[test]
fn chain_mode_controls_placed_piece_and_refusals() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    simulation.construction_start(100, 100, 40, 0);
    assert!(!simulation.construction_chain());
    simulation.construction_set_chain(true);
    assert!(simulation.construction_chain());
    simulation.construction_select("Loop");
    assert!(!simulation.construction_can_append());
    assert_eq!(
        simulation.construction_preview_reason(),
        "No chain lift on this piece"
    );
    simulation.construction_select("LevelToGentleUp");
    simulation.construction_append();
    assert!(simulation.construction_message().is_empty());
    assert_eq!(
        simulation
            .construction
            .as_ref()
            .unwrap()
            .pieces()
            .last()
            .unwrap()
            .kind,
        super::PieceKind::Lift
    );
    simulation.construction_set_chain(false);
    assert!(!simulation.construction_chain());
}

#[test]
fn coaster_colours_start_from_the_model_and_change_the_state_hash() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    assert_eq!(simulation.construction_colors(), "null");
    assert_eq!(
        simulation.coaster_colors(),
        "{\"track\":\"coral\",\"supports\":\"timber\",\"trains\":\"sage\"}"
    );
    for (model, colors) in [
        (
            "model:hybrid",
            "{\"track\":\"coral\",\"supports\":\"timber\",\"trains\":\"sage\"}",
        ),
        (
            "model:velocity_viper",
            "{\"track\":\"teal\",\"supports\":\"cream\",\"trains\":\"coral\"}",
        ),
        (
            "model:wooden_twister",
            "{\"track\":\"timber\",\"supports\":\"timber\",\"trains\":\"slate\"}",
        ),
    ] {
        simulation.construction_select(model);
        simulation.construction_start(100, 100, 40, 0);
        assert_eq!(simulation.construction_colors(), colors, "{model}");
    }

    let before = simulation.state_hash();
    simulation.construction_set_colors("plum", "cream", "mustard");
    assert_eq!(
        simulation.construction_colors(),
        "{\"track\":\"plum\",\"supports\":\"cream\",\"trains\":\"mustard\"}"
    );
    assert!(simulation.construction_message().is_empty());
    let repainted = simulation.state_hash();
    assert_ne!(repainted, before);

    simulation.construction_set_colors("plum", "#ffffff", "mustard");
    assert_eq!(
        simulation.construction_colors(),
        "{\"track\":\"plum\",\"supports\":\"cream\",\"trains\":\"mustard\"}"
    );
    assert_eq!(simulation.construction_message(), "Unknown coaster colour");

    simulation.construction_start(120, 120, 40, 0);
    assert_eq!(
        simulation.construction_colors(),
        "{\"track\":\"timber\",\"supports\":\"timber\",\"trains\":\"slate\"}"
    );
}

#[test]
fn train_layouts_name_each_model_and_its_train_shape() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    assert_eq!(simulation.construction_train_layout(), "null");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&simulation.coaster_train_layout()).unwrap(),
        serde_json::json!({"model": "hybrid", "carsPerTrain": 6, "rowsPerCar": 2, "seatsPerRow": 2, "lengthMetres": 18.0})
    );
    for (model, key, cars, rows) in [
        ("model:hybrid", "hybrid", 6, 2),
        ("model:velocity_viper", "velocity_viper", 6, 2),
        ("model:wooden_twister", "wooden_twister", 12, 1),
    ] {
        simulation.construction_select(model);
        simulation.construction_start(100, 100, 40, 0);
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&simulation.construction_train_layout())
                .unwrap(),
            serde_json::json!({"model": key, "carsPerTrain": cars, "rowsPerCar": rows, "seatsPerRow": 2, "lengthMetres": 18.0}),
            "{model}"
        );
    }
}

#[test]
fn coaster_colours_replay_to_the_same_state() {
    let paint = |colors: [&str; 3]| {
        let mut simulation = WasmSimulation::new(7, 0, String::from("new"));
        simulation.construction_start(100, 100, 40, 0);
        simulation.construction_set_colors(colors[0], colors[1], colors[2]);
        simulation.tick();
        simulation.state_hash()
    };
    assert_eq!(
        paint(["sky", "charcoal", "rose"]),
        paint(["sky", "charcoal", "rose"])
    );
    assert_ne!(
        paint(["sky", "charcoal", "rose"]),
        paint(["sky", "charcoal", "walnut"])
    );
}

#[test]
fn lift_drop_circuit_completes_lap() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    append_lift_drop_fixture(&mut simulation);
    assert!(!simulation.park.support_render_cache.is_empty());
    simulation.construction_test();
    for _ in 0..9000 {
        simulation.tick();
        if simulation.construction_message() == "Train completed a lap." {
            assert!(!simulation.park.support_render_cache.is_empty());
            assert!(!simulation.park.proximity_supports().is_empty());
            let report: serde_json::Value =
                serde_json::from_str(&simulation.construction_test_report_json()).unwrap();
            let statistics = &report["statistics"];
            assert!(statistics["durationSeconds"].as_f64().unwrap() > 0.0);
            assert!(statistics["maxSpeed"].as_f64().unwrap() > 0.0);
            assert!(statistics["maxHeight"].as_f64().unwrap() > 0.0);
            assert!(statistics["biggestDrop"].as_f64().unwrap() > 0.0);
            assert!(statistics["inversions"].as_u64().unwrap() > 0);
            assert!(statistics["invertedSeconds"].as_f64().unwrap() > 0.0);
            return;
        }
        if simulation.construction_outcome.is_some() {
            break;
        }
    }
    let (circuit, train) = simulation.construction_train.as_ref().unwrap();
    let mut start = 0.0;
    let mut piece = "unknown";
    for candidate in circuit.pieces() {
        if train.distance() < start + candidate.length() {
            piece = candidate.name;
            break;
        }
        start += candidate.length();
    }
    panic!(
        "{}: distance={}, speed={}, length={}, piece={piece}",
        simulation.construction_message(),
        train.distance(),
        train.speed(),
        circuit.length()
    );
}

#[test]
fn opening_after_a_completed_test_clears_the_report() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    append_lift_drop_fixture(&mut simulation);
    simulation.construction_test();
    for _ in 0..9000 {
        simulation.tick();
        if simulation.construction_message() == "Train completed a lap." {
            let station = &simulation.construction.as_ref().unwrap().pieces()[0];
            let (queue, exit) = super::station_positions(station).unwrap();
            for (position, kind) in [
                (queue, super::PathKind::Queue),
                (exit, super::PathKind::Normal),
            ] {
                simulation.world.set_surface(
                    position.x,
                    position.y,
                    super::Surface {
                        corners: [position.height; 4],
                        water: None,
                    },
                );
                simulation
                    .world
                    .build_path_route(position, position, kind, 0)
                    .unwrap();
            }
            assert!(
                simulation
                    .construction_test_report_json()
                    .contains("\"rating\"")
            );
            assert!(simulation.construction_set_open(true));
            assert_eq!(simulation.construction_test_report_json(), "null");
            return;
        }
        if simulation.construction_outcome.is_some() {
            break;
        }
    }
    let (circuit, train) = simulation.construction_train.as_ref().unwrap();
    let mut start = 0.0;
    let mut piece = "unknown";
    for candidate in circuit.pieces() {
        if train.distance() < start + candidate.length() {
            piece = candidate.name;
            break;
        }
        start += candidate.length();
    }
    panic!(
        "{}: distance={}, speed={}, length={}, piece={piece}",
        simulation.construction_message(),
        train.distance(),
        train.speed(),
        circuit.length()
    );
}

#[test]
fn construction_closes_and_runs_physics_deterministically() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    assert_eq!(simulation.construction_test_report_json(), "null");
    simulation.construction_start(200, 200, 20, 0);
    for piece in [
        "TurnSmallStandalone",
        "TurnSmallStandalone",
        "Straight",
        "TurnSmallStandalone",
        "TurnSmallStandalone",
    ] {
        simulation.construction_select(piece);
        simulation.construction_append();
        assert!(
            simulation.construction_message().is_empty(),
            "{}",
            simulation.construction_message()
        );
    }
    assert!(simulation.construction_can_close());
    simulation.construction_close();
    assert!(simulation.construction_can_test());
    simulation.construction_test();
    assert_eq!(simulation.construction_test_report_json(), "null");
    let initial_poses = simulation.construction_train_poses();
    assert!(initial_poses.chunks_exact(11).all(|car| car[10] == 0.0));
    for _ in 0..9000 {
        simulation.tick();
        if simulation.construction_message() == "Train stopped before completing a lap." {
            break;
        }
    }
    assert_ne!(simulation.construction_train_poses(), initial_poses);
    assert_eq!(
        simulation.construction_message(),
        "Train stopped before completing a lap."
    );
    let report = simulation.construction_test_report_json();
    assert!(report.contains("\"samples\":[{"), "{report}");
    assert!(report.contains("\"longitudinalG\":"), "{report}");
    assert!(report.contains("\"rating\":{"), "{report}");
    assert!(report.contains("\"proximityEvents\":["), "{report}");
    assert!(report.contains("\"notes\":[\""), "{report}");
    let stopped_poses = simulation.construction_train_poses();
    for _ in 0..60 {
        simulation.tick();
    }
    simulation.construction_select("Straight");
    assert_eq!(simulation.construction_train_poses(), stopped_poses);
    assert_eq!(
        simulation.construction_message(),
        "Train stopped before completing a lap."
    );
    simulation.construction_delete(5);
    assert_eq!(simulation.construction_test_report_json(), "null");
    let (_, recovered_train) = simulation.construction_train.as_ref().unwrap();
    assert_eq!(recovered_train.speed(), 0.0);
    let recovered_poses = simulation.construction_train_poses();
    let station = &simulation.construction.as_ref().unwrap().pieces()[0];
    let recovered_state = simulation.construction_train_states();
    assert_eq!(recovered_state, vec![0.0, 0.0, 4.0, 1.0, 0.0, 0.0]);
    assert_eq!(recovered_train.distance(), station.length());
    let config = &simulation.construction.as_ref().unwrap().model().train;
    assert_eq!(recovered_poses.len(), usize::from(config.cars) * 11);
    for (index, car) in recovered_poses.chunks_exact(11).enumerate() {
        let distance = (station.length()
            - index as f64 * config.length_metres / f64::from(config.cars))
        .max(0.0);
        let expected = station.sample(distance).position / 4.0;
        assert_eq!(
            &car[1..4],
            &[expected.x as f32, expected.y as f32, expected.z as f32]
        );
    }
    for _ in 0..60 {
        simulation.tick();
    }
    assert_eq!(simulation.construction_train_poses(), recovered_poses);
    assert!(!simulation.construction_can_test());
    simulation.construction_delete(0);
    assert!(simulation.construction_train_poses().is_empty());
}

#[test]
fn construction_lists_are_structured_and_preserve_selection_ids() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&simulation.construction_track()).unwrap(),
        serde_json::json!([])
    );
    simulation.construction_start(200, 200, 20, 0);
    assert_eq!(simulation.construction_selected(), "Straight");
    let choices: serde_json::Value =
        serde_json::from_str(&simulation.construction_choices()).unwrap();
    assert!(
        choices
            .as_array()
            .unwrap()
            .iter()
            .any(|choice| choice["id"] == "Straight")
    );
    simulation.construction_select("Straight");
    assert_eq!(simulation.construction_selected(), "Straight");
    let track: serde_json::Value = serde_json::from_str(&simulation.construction_track()).unwrap();
    assert_eq!(track[0]["id"], 0);
    assert_eq!(track[0]["label"], "Station");
    assert_eq!(track[0]["kind"], "Station");
    let elements: serde_json::Value =
        serde_json::from_str(&simulation.construction_elements()).unwrap();
    assert!(
        elements
            .as_array()
            .unwrap()
            .iter()
            .any(|element| element["id"] == "element:climb-60")
    );
}

#[test]
fn construction_reports_include_nullable_heartlining_metadata() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("new"));
    simulation.construction_start(200, 200, 20, 0);

    let choices: serde_json::Value =
        serde_json::from_str(&simulation.construction_choices()).unwrap();
    let choices = choices.as_array().unwrap();
    let choice_for = |id: &str| {
        choices
            .iter()
            .find(|choice| choice["id"] == id)
            .unwrap_or_else(|| panic!("missing construction choice {id}"))
    };
    assert_eq!(choice_for("Loop")["heartlined"], false);
    assert_eq!(choice_for("HeartlineRoll")["heartlined"], true);
    assert_eq!(
        choice_for("Straight")["heartlined"],
        serde_json::Value::Null
    );
    assert!(
        choices
            .iter()
            .all(|choice| { choice["heartlined"].is_boolean() || choice["heartlined"].is_null() })
    );

    let elements: serde_json::Value =
        serde_json::from_str(&simulation.construction_elements()).unwrap();
    let elements = elements.as_array().unwrap();
    let element_for = |id: &str| {
        elements
            .iter()
            .find(|element| element["id"] == id)
            .unwrap_or_else(|| panic!("missing construction element {id}"))
    };
    assert_eq!(element_for("element:immelmann-left")["heartlined"], false);
    assert_eq!(
        element_for("element:climb-60")["heartlined"],
        serde_json::Value::Null
    );
    assert!(elements.iter().all(|element| {
        (element["heartlined"].is_boolean() || element["heartlined"].is_null())
            && !element["name"].as_str().unwrap().contains("heartlined")
    }));
}

fn track_rows(json: &str) -> Vec<serde_json::Value> {
    serde_json::from_str(json).unwrap()
}

#[test]
fn catalog_selection_ids_match_recorded_session_names() {
    for piece in super::catalog() {
        assert_eq!(
            crate::construction::wire_pieceid(piece.id),
            format!("{:?}", piece.id)
        );
    }
}

#[test]
fn hill_options_follow_model_and_select_variants_by_parameters() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("new"));
    simulation.construction_start(200, 200, 20, 0);
    let options: Vec<serde_json::Value> =
        serde_json::from_str(&simulation.construction_hill_options()).unwrap();
    let hill = options
        .iter()
        .find(|option| option["id"] == "EjectorHill12x64OuterLeft")
        .unwrap();
    assert_eq!(hill["feel"], "ejector");
    assert_eq!(hill["height"], 12);
    assert_eq!(hill["span"], 64);
    assert_eq!(hill["twist"], "outerLeft");
    assert!(hill["ratedSpeed"]["minimumEntry"].is_number());
    simulation.construction_select("hill:ejector:12x64:outerLeft");
    assert_eq!(
        simulation.construction_selected(),
        "EjectorHill12x64OuterLeft"
    );
    let choices: Vec<serde_json::Value> =
        serde_json::from_str(&simulation.construction_choices()).unwrap();
    let selected = choices
        .iter()
        .find(|choice| choice["id"] == "EjectorHill12x64OuterLeft")
        .unwrap();
    assert!(selected["hillEstimate"]["entrySpeed"].is_number());
    assert!(selected["hillEstimate"]["ratedSpeed"].is_object());
    assert!(selected["hillEstimate"]["limits"]["minimumVerticalG"].is_number());
    simulation.construction_select("model:wooden_twister");
    simulation.construction_start(200, 200, 20, 0);
    let wooden: Vec<serde_json::Value> =
        serde_json::from_str(&simulation.construction_hill_options()).unwrap();
    assert!(!wooden.iter().any(|option| option["feel"] == "zeroGRoll"));
}

#[test]
fn speed_turns_list_the_model_sizes_with_hand_and_design_speed() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("new"));
    assert_eq!(simulation.construction_speed_turns(), "[]");
    simulation.construction_start(200, 200, 20, 0);
    let hybrid: Vec<serde_json::Value> =
        serde_json::from_str(&simulation.construction_speed_turns()).unwrap();
    let summary: Vec<_> = hybrid
        .iter()
        .map(|turn| {
            (
                turn["id"].as_str().unwrap().to_owned(),
                turn["hand"].as_str().unwrap().to_owned(),
                turn["designSpeed"].as_f64().unwrap(),
            )
        })
        .collect();
    let expected = [
        ("SpeedTurnLarge16Left", "left", 16.0),
        ("SpeedTurnLarge16Right", "right", 16.0),
        ("SpeedTurnLarge20Left", "left", 20.0),
        ("SpeedTurnLarge20Right", "right", 20.0),
        ("SpeedTurnLarge24Left", "left", 24.0),
        ("SpeedTurnLarge24Right", "right", 24.0),
    ]
    .map(|(id, hand, speed)| (id.to_owned(), hand.to_owned(), speed));
    assert_eq!(summary, expected);
    simulation.construction_select("model:wooden_twister");
    simulation.construction_start(200, 200, 20, 0);
    let wooden: Vec<serde_json::Value> =
        serde_json::from_str(&simulation.construction_speed_turns()).unwrap();
    assert_eq!(wooden.len(), 4);
    assert!(
        wooden
            .iter()
            .all(|turn| turn["designSpeed"].as_f64().unwrap() < 24.0)
    );
}

#[test]
fn hill_estimate_uses_supplied_preview_status() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("new"));
    simulation.construction_start(200, 200, 20, 0);
    simulation.construction_select("EjectorHill12x64");
    for (status, speed, expected_speed, source) in [
        ("reachedEnd", Some(22.0), Some(22.0), "simulated"),
        ("stalled", None, Some(0.0), "stalled"),
        ("timedOut", None, None, "timedOut"),
    ] {
        let choices: Vec<serde_json::Value> =
            serde_json::from_str(&simulation.construction_choices_with_preview(speed, status))
                .unwrap();
        let estimate = &choices
            .iter()
            .find(|choice| choice["id"] == "EjectorHill12x64")
            .unwrap()["hillEstimate"];
        assert_eq!(estimate["entrySpeed"].as_f64(), expected_speed);
        assert_eq!(estimate["source"], source);
        if status != "reachedEnd" {
            assert!(estimate["minimumVerticalG"].is_null());
        }
    }
}
