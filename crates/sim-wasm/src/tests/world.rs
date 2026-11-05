use super::{
    BlockLayout, Position, Surface, WasmSimulation, block_physics_config,
    editable_block_boundaries, theoretical_hourly_riders, track_support_obstacles,
};
use theme_park_sim::facilities::{FacilityKind, ParkFacilities};
use theme_park_sim::guests::GuestId;
use theme_park_sim::park_guests::ParkGuests;
use theme_park_sim::physics::{Circuit, TIME_STEP, demo_circuit};
use theme_park_sim::track::clearance::ClearanceCollision;
use theme_park_sim::track::construction::{Construction, ConstructionError};
use theme_park_sim::track::supports::generate_supports;
use theme_park_sim::track::{PieceId, TrackState, catalog, hybrid_model};
use theme_park_sim::world::{Direction, PathKind, World};

#[test]
fn new_park_starts_flat_with_an_entrance_and_two_path_tiles() {
    let simulation = WasmSimulation::new(17, 12, String::from("new"));

    assert_eq!(simulation.world_dimensions(), vec![512, 512]);
    assert!(
        simulation
            .world_surfaces()
            .chunks_exact(6)
            .all(|surface| surface == [8, 8, 8, 8, 0, 0])
    );
    assert_eq!(
        simulation.world_paths(),
        vec![218, 250, 8, 0, 218, 251, 8, 0]
    );
    assert_eq!(simulation.park.facilities.buildings().len(), 1);
    assert_eq!(
        simulation.park.facilities.buildings()[0].kind,
        FacilityKind::Entrance
    );
    assert!(simulation.demo_coaster().is_none());
    assert!(simulation.coaster_geometry().is_empty());
    assert!(simulation.coaster_rails().is_empty());
    assert!(simulation.coaster_ties().is_empty());
    assert!(simulation.coaster_supports().is_empty());
    assert!(simulation.coaster_train_poses().is_empty());
}

#[test]
fn demo_park_keeps_its_coaster_and_facilities() {
    let simulation = WasmSimulation::new(17, 12, String::from("demo"));

    assert!(simulation.demo_coaster().is_some());
    assert!(simulation.park.facilities.buildings().len() > 1);
    assert!(!simulation.coaster_rails().is_empty());
}

#[test]
fn new_construction_cannot_tunnel_through_the_existing_demo_coaster() {
    let simulation = WasmSimulation::new(17, 0, String::from("demo"));
    let demo_coaster = simulation.demo_coaster().expect("demo coaster exists");
    let station_entry = demo_coaster.circuit.pieces()[0].entry;
    let construction = Construction::new(station_entry, hybrid_model());

    assert_eq!(
        construction.tunnel_plan(
            PieceId::Station,
            &simulation.world,
            &simulation.facility_obstacles(),
        ),
        Err(ConstructionError::Clearance(ClearanceCollision::Scenery))
    );
}

#[test]
fn park_seed_deterministically_initializes_guest_appearances() {
    let first = WasmSimulation::new(17, 12, String::from("new"));
    let same_seed = WasmSimulation::new(17, 12, String::from("new"));
    let different_seed = WasmSimulation::new(18, 12, String::from("new"));

    assert_eq!(first.guest_appearances(), same_seed.guest_appearances());
    assert_ne!(
        first.guest_appearances(),
        different_seed.guest_appearances()
    );
}

fn block_test_circuit() -> Circuit {
    let pieces_catalog = catalog();
    let mut state = TrackState::origin();
    let mut pieces = Vec::new();
    for (id, chain) in [
        (PieceId::Station, false),
        (PieceId::Straight, false),
        (PieceId::Straight, false),
        (PieceId::Straight, true),
        (PieceId::Straight, false),
        (PieceId::Straight, false),
        (PieceId::TurnLargeEntry, false),
        (PieceId::TurnLargeExit, false),
        (PieceId::Straight, false),
        (PieceId::Straight, false),
        (PieceId::Straight, true),
        (PieceId::Straight, false),
        (PieceId::Straight, false),
        (PieceId::Straight, false),
        (PieceId::TurnLargeEntry, false),
        (PieceId::TurnLargeExit, false),
    ] {
        let mut piece = pieces_catalog
            .iter()
            .find(|piece| piece.id == id)
            .unwrap()
            .at(state)
            .unwrap();
        if chain {
            piece.kind = super::PieceKind::Lift;
        }
        state = piece.exit;
        pieces.push(piece);
    }
    Circuit::new(pieces).unwrap()
}

fn unpowered_block_test_circuit() -> Circuit {
    let mut pieces = block_test_circuit().pieces().to_vec();
    for piece in &mut pieces {
        if piece.kind == super::PieceKind::Lift {
            piece.kind = super::PieceKind::Track;
        }
    }
    Circuit::new(pieces).unwrap()
}

fn unpowered_block_physics_config() -> super::PhysicsConfig {
    super::PhysicsConfig {
        rolling_resistance: 0.001,
        ..block_physics_config()
    }
}

fn block_test_boundaries(circuit: &Circuit) -> Vec<f64> {
    [2, 6, 10, 13]
        .into_iter()
        .map(|end| {
            circuit.pieces()[..end]
                .iter()
                .map(|piece| piece.length())
                .sum()
        })
        .collect()
}

fn block_powered_test_circuit() -> Circuit {
    let pieces_catalog = catalog();
    let mut state = TrackState::origin();
    let mut pieces = Vec::new();
    for (id, chain) in [
        (PieceId::Station, false),
        (PieceId::Straight, false),
        (PieceId::Straight, false),
        (PieceId::Straight, true),
        (PieceId::Straight, true),
        (PieceId::Straight, false),
        (PieceId::Straight, false),
        (PieceId::TurnLargeEntry, false),
        (PieceId::TurnLargeExit, false),
        (PieceId::Straight, false),
        (PieceId::Straight, false),
        (PieceId::Straight, true),
        (PieceId::Straight, true),
        (PieceId::Straight, false),
        (PieceId::Straight, false),
        (PieceId::Straight, false),
        (PieceId::TurnLargeEntry, false),
        (PieceId::TurnLargeExit, false),
    ] {
        let mut piece = pieces_catalog
            .iter()
            .find(|piece| piece.id == id)
            .unwrap()
            .at(state)
            .unwrap();
        if chain {
            piece.kind = super::PieceKind::Lift;
        }
        state = piece.exit;
        pieces.push(piece);
    }
    Circuit::new(pieces).unwrap()
}

fn report_number(report: &str, key: &str) -> f64 {
    let marker = format!("\"{key}\":");
    report
        .split_once(&marker)
        .unwrap()
        .1
        .split([',', '}'])
        .next()
        .unwrap()
        .parse()
        .unwrap()
}

fn build_block_test_construction(simulation: &mut WasmSimulation) {
    simulation.construction_start(100, 100, 40, 0);
    for (piece, chain) in [
        ("Straight", false),
        ("Straight", false),
        ("Straight", true),
        ("Straight", false),
        ("Straight", false),
        ("TurnLargeEntry", false),
        ("TurnLargeExit", false),
        ("Straight", false),
        ("Straight", false),
        ("Straight", true),
        ("Straight", false),
        ("Straight", false),
        ("Straight", false),
        ("TurnLargeEntry", false),
        ("TurnLargeExit", false),
    ] {
        simulation.construction_set_chain(chain);
        simulation.construction_select(piece);
        simulation.construction_append();
        assert!(simulation.construction_message().is_empty());
    }
    assert!(simulation.construction_can_close());
    simulation.construction_close();
    assert!(simulation.construction_can_test());
}

fn apply_automatic_blocks_after_finding(simulation: &mut WasmSimulation, minimum_brakes: usize) {
    assert!(simulation.construction_auto_blocks());
    for _ in 0..10_000 {
        if let Some(search) = &simulation.block_placement_search {
            if editable_block_boundaries(search.best()).len() >= minimum_brakes {
                assert!(simulation.construction_apply_best_block_sections());
                return;
            }
            simulation.construction_advance_block_analysis(1_000);
        } else if editable_block_boundaries(simulation.block_layout.as_ref().unwrap()).len()
            >= minimum_brakes
        {
            return;
        } else {
            break;
        }
    }
    panic!("automatic search did not find {minimum_brakes} usable brake runs");
}

fn finish_manual_block_validation(simulation: &mut WasmSimulation) {
    for _ in 0..10_000 {
        if simulation.block_manual_search.is_none() {
            assert!(
                simulation.block_message.is_empty(),
                "{}",
                simulation.block_message
            );
            return;
        }
        simulation.construction_advance_block_analysis(1_000);
    }
    panic!("manual block validation did not finish");
}

fn connect_station_to_park_paths(simulation: &mut WasmSimulation) {
    let construction = simulation.construction.as_ref().expect("coaster exists");
    let station = construction
        .pieces()
        .iter()
        .find(|piece| piece.kind == theme_park_sim::track::PieceKind::Station)
        .expect("station exists");
    let (queue, exit) = super::station_positions(station).expect("station access tiles");
    let network_y = 26;
    let queue_tail_x = 29;
    let exit_tail_x = 4;
    let queue_tail_y = network_y + 1;
    let at = |x, y| Position {
        x,
        y,
        height: queue.height,
    };
    assert_eq!(queue.height, exit.height);
    for x in 4..=queue_tail_x {
        simulation.world.set_surface(
            x,
            network_y,
            Surface {
                corners: [queue.height; 4],
                water: None,
            },
        );
    }
    for y in 4..=network_y {
        simulation.world.set_surface(
            4,
            y,
            Surface {
                corners: [queue.height; 4],
                water: None,
            },
        );
    }
    for x in queue.x.min(queue_tail_x)..=queue.x.max(queue_tail_x) {
        simulation.world.set_surface(
            x,
            queue.y,
            Surface {
                corners: [queue.height; 4],
                water: None,
            },
        );
    }
    for y in queue.y.min(queue_tail_y)..=queue.y.max(queue_tail_y) {
        simulation.world.set_surface(
            queue_tail_x,
            y,
            Surface {
                corners: [queue.height; 4],
                water: None,
            },
        );
    }
    for x in exit.x.min(exit_tail_x)..=exit.x.max(exit_tail_x) {
        simulation.world.set_surface(
            x,
            exit.y,
            Surface {
                corners: [exit.height; 4],
                water: None,
            },
        );
    }
    for (x, corners) in [
        (30, [15, 16, 16, 15]),
        (29, [14, 15, 15, 14]),
        (28, [13, 14, 14, 13]),
        (21, [14, 13, 13, 14]),
        (20, [15, 14, 14, 15]),
        (19, [16, 15, 15, 16]),
    ] {
        simulation.world.set_surface(
            x,
            exit.y,
            Surface {
                corners,
                water: None,
            },
        );
    }
    for x in 22..=27 {
        simulation.world.set_surface(
            x,
            exit.y,
            Surface {
                corners: [13; 4],
                water: None,
            },
        );
    }
    for y in exit.y.min(queue_tail_y)..=exit.y.max(queue_tail_y) {
        simulation.world.set_surface(
            exit_tail_x,
            y,
            Surface {
                corners: [exit.height; 4],
                water: None,
            },
        );
    }
    for y in 0..=3 {
        simulation.world.set_surface(
            4,
            y,
            Surface {
                corners: [queue.height; 4],
                water: None,
            },
        );
    }
    let entrance_position = Position {
        x: 4,
        y: 0,
        height: queue.height,
    };
    simulation
        .world
        .build_path_route(at(4, 4), at(4, network_y), PathKind::Normal, 0)
        .expect("build entrance path");
    simulation
        .world
        .build_path_route(
            at(4, network_y),
            at(queue_tail_x, network_y),
            PathKind::Normal,
            0,
        )
        .expect("build park path");
    let mut facilities = ParkFacilities::new();
    facilities
        .place(
            &simulation.world,
            FacilityKind::Entrance,
            entrance_position,
            Direction::South,
        )
        .expect("place park entrance");
    let entrance = facilities
        .entrance(&simulation.world)
        .expect("park entrance access");
    simulation
        .world
        .build_path_route(queue, at(queue_tail_x, queue.y), PathKind::Queue, 0)
        .expect("build coaster queue to park path");
    simulation
        .world
        .build_path_route(
            at(queue_tail_x, queue.y),
            at(queue_tail_x, queue_tail_y),
            PathKind::Queue,
            0,
        )
        .expect("extend coaster queue to park path");
    simulation
        .world
        .build_path_route(
            at(queue_tail_x, network_y),
            at(queue_tail_x, queue_tail_y),
            PathKind::Queue,
            0,
        )
        .expect("connect coaster queue to park path");
    simulation
        .world
        .build_path_route(exit, at(exit_tail_x, exit.y), PathKind::Normal, 0)
        .expect("build coaster exit path");
    simulation
        .world
        .build_path_route(
            at(exit_tail_x, exit.y),
            at(exit_tail_x, queue_tail_y),
            PathKind::Normal,
            0,
        )
        .expect("extend coaster exit to park path");
    simulation
        .world
        .build_path_route(
            at(exit_tail_x, queue_tail_y),
            at(exit_tail_x, network_y),
            PathKind::Normal,
            0,
        )
        .expect("connect coaster exit to park path");

    simulation.facilities = facilities;
    simulation.simulation = ParkGuests::new(42, 64, &simulation.world, entrance, 64);
    simulation.park.simulation.open_with_guests(
        &simulation
            .park
            .facilities
            .guest_facilities(&simulation.park.world),
    );
    assert_eq!(
        simulation.simulation.walking().path_component(entrance),
        simulation.simulation.walking().path_component(queue),
        "coaster queue must connect to the park entrance"
    );
    assert!(
        simulation.construction_set_open(true),
        "{}",
        simulation.construction_message()
    );
}

#[test]
fn automatic_block_search_reuses_completed_results_until_track_changes() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    build_block_test_construction(&mut simulation);
    let (circuit, train) = simulation.construction_circuit_and_train().unwrap();
    let config = super::PhysicsConfig {
        powered_sections: false,
        ..block_physics_config()
    };
    let mut search = super::BlockPlacementSearch::new(circuit, train, config).unwrap();
    search.advance(2_000);
    assert!(search.is_complete());
    let work = search.work_completed();
    simulation.install_block_layout(search.best().clone(), false);
    simulation.block_placement_search = Some(search);
    simulation.construction_advance_block_analysis(1);
    assert_eq!(
        simulation
            .block_placement_cache
            .as_ref()
            .unwrap()
            .work_completed(),
        work
    );
    assert!(simulation.construction_auto_blocks());
    assert!(simulation.block_placement_search.is_none());
    assert_eq!(
        simulation
            .block_placement_cache
            .as_ref()
            .unwrap()
            .work_completed(),
        work
    );
    simulation.construction_undo();
    assert!(simulation.block_placement_cache.is_none());
}

#[test]
fn repeated_automatic_block_requests_preserve_in_progress_search() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    build_block_test_construction(&mut simulation);
    assert!(simulation.construction_auto_blocks());
    simulation.construction_advance_block_analysis(10);
    let work = simulation
        .block_placement_search
        .as_ref()
        .unwrap()
        .work_completed();
    assert!(work > 0);
    assert!(simulation.construction_auto_blocks());
    assert_eq!(
        simulation
            .block_placement_search
            .as_ref()
            .unwrap()
            .work_completed(),
        work
    );
    assert!(simulation.block_placement_cache.is_none());
    assert!(simulation.construction_apply_best_block_sections());
    assert!(simulation.block_placement_search.is_none());
    assert!(simulation.block_placement_cache.is_none());
    assert!(simulation.construction_auto_blocks());
    assert_eq!(
        simulation
            .block_placement_search
            .as_ref()
            .unwrap()
            .work_completed(),
        0
    );
}

#[test]
fn completed_automatic_search_installs_its_result() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    let circuit = block_test_circuit();
    let train = hybrid_model().train;
    let config = super::PhysicsConfig {
        powered_sections: false,
        ..block_physics_config()
    };
    let mut search = super::BlockPlacementSearch::new(circuit, train, config).unwrap();
    search.advance(2_000);
    assert!(search.is_complete());
    simulation.install_block_layout(search.best().clone(), false);
    simulation.block_placement_search = Some(search);
    simulation.construction_advance_block_analysis(1);
    assert!(simulation.block_placement_search.is_none());
    assert_eq!(simulation.block_layout.as_ref().unwrap().capacity(), 0);
    assert!(simulation.block_layout_is_automatic);
    let report = simulation.construction_block_report_json();
    assert!(
        report.contains("\"placementStatus\":\"complete\""),
        "{report}"
    );
}

#[test]
fn block_candidates_are_sorted_exact_distances_accepted_by_manual_search() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    assert!(simulation.construction_block_candidates().is_empty());
    build_block_test_construction(&mut simulation);
    let (circuit, train) = simulation.construction_circuit_and_train().unwrap();
    let mut expected: Vec<_> =
        theme_park_sim::track::blocks::brake_runs(&circuit, train.length_metres)
            .iter()
            .map(|run| run.boundary_metres)
            .collect();
    expected.sort_by(f64::total_cmp);
    expected.dedup();
    assert!(!expected.is_empty());
    let candidates = simulation.construction_block_candidates();
    assert_eq!(candidates, expected);
    for candidate in candidates {
        assert!(simulation.construction_set_block_sections(vec![candidate]));
    }
}

#[test]
fn manual_block_validation_preserves_layout_and_reports_rejection() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    build_block_test_construction(&mut simulation);
    let (circuit, train) = simulation.construction_circuit_and_train().unwrap();
    let original = BlockLayout::new(&circuit, &train, block_physics_config(), Vec::new()).unwrap();
    simulation.install_block_layout(original, false);
    let old_boundaries = simulation
        .block_layout
        .as_ref()
        .unwrap()
        .boundaries()
        .to_vec();
    let runs = theme_park_sim::track::blocks::brake_runs(&circuit, train.length_metres);
    let pair = runs
        .iter()
        .enumerate()
        .find_map(|(index, first)| {
            runs[index + 1..].iter().find_map(|second| {
                (first.boundary_metres != second.boundary_metres
                    && first
                        .piece_indices
                        .iter()
                        .any(|piece| second.piece_indices.contains(piece)))
                .then_some((first.boundary_metres, second.boundary_metres))
            })
        })
        .expect("fixture has overlapping candidate brakes");
    assert!(simulation.construction_set_block_sections(vec![pair.0, pair.1]));
    assert!(
        simulation
            .construction_block_report_json()
            .contains("\"placementStatus\":\"validating\"")
    );
    assert_eq!(
        simulation.block_layout.as_ref().unwrap().boundaries(),
        old_boundaries
    );
    for _ in 0..1_000 {
        simulation.construction_advance_block_analysis(1_000);
        if simulation.block_manual_search.is_none() {
            break;
        }
    }
    assert!(simulation.block_manual_search.is_none());
    assert_eq!(
        simulation.block_layout.as_ref().unwrap().boundaries(),
        old_boundaries
    );
    assert!(
        simulation.construction_message().contains("overlap"),
        "{}",
        simulation.construction_message()
    );
}

#[test]
fn baseline_report_distinguishes_unverified_from_observed_stall() {
    let circuit = super::demo_circuit();
    let train = hybrid_model().train;
    let config = super::PhysicsConfig {
        powered_sections: false,
        ..Default::default()
    };
    let mut search = super::BlockPlacementSearch::new(circuit, train, config).unwrap();
    assert_eq!(search.best().capacity(), 1);
    assert!(!search.best().capacity_search_complete());
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    simulation.install_block_layout(search.best().clone(), true);
    let report = simulation.construction_block_report_json();
    assert!(
        report.contains("one-train circuit has not yet been verified"),
        "{report}"
    );
    assert!(!report.contains("A train stalled"), "{report}");
    search.advance(2_000);
    assert!(search.is_complete());
    assert_eq!(search.best().capacity(), 0);
    assert!(search.best().capacity_search_complete());
    simulation.block_placement_search = Some(search);
    simulation.construction_advance_block_analysis(0);
    let report = simulation.construction_block_report_json();
    assert!(
        report.contains("A train stalled and could not complete this circuit"),
        "{report}"
    );
    assert!(
        report.contains("\"throughputStatus\":\"unavailable\""),
        "{report}"
    );
    assert!(simulation.block_throughput_estimator.is_none());
}

#[test]
fn applying_an_unfinished_placement_does_not_claim_a_search_limit_was_reached() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    build_block_test_construction(&mut simulation);
    assert!(simulation.construction_auto_blocks());
    assert!(simulation.construction_apply_best_block_sections());
    let report = simulation.construction_block_report_json();
    assert!(
        report.contains("\"placementStatus\":\"incomplete\""),
        "{report}"
    );
    assert!(
        report.contains("Automatic placement is incomplete"),
        "{report}"
    );
    assert!(!report.contains("reached its search limit"), "{report}");
}

#[test]
fn automatic_placement_is_incremental_and_preserves_installed_layout_while_searching() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    build_block_test_construction(&mut simulation);
    let (circuit, train) = simulation.construction_circuit_and_train().unwrap();
    let original = BlockLayout::new(&circuit, &train, block_physics_config(), Vec::new()).unwrap();
    simulation.install_block_layout(original, false);
    let old_boundaries = simulation
        .block_layout
        .as_ref()
        .unwrap()
        .boundaries()
        .to_vec();
    assert!(simulation.construction_auto_blocks());
    let report = simulation.construction_block_report_json();
    assert!(
        report.contains("\"placementStatus\":\"searching\""),
        "{report}"
    );
    assert!(!simulation.construction_advance_block_analysis(1));
    assert_eq!(
        simulation.block_layout.as_ref().unwrap().boundaries(),
        old_boundaries
    );
    let before_tick = simulation
        .block_placement_search
        .as_ref()
        .unwrap()
        .work_completed();
    simulation.tick();
    let after_tick = simulation
        .block_placement_search
        .as_ref()
        .unwrap()
        .work_completed();
    assert_eq!(after_tick, before_tick);
    simulation.construction_advance_block_analysis(200);
    assert!(
        simulation
            .block_placement_search
            .as_ref()
            .unwrap()
            .work_completed()
            > after_tick
    );
    simulation.construction_delete(3);
    assert!(simulation.block_placement_search.is_none());
}

#[test]
fn block_report_marks_throughput_pending_until_incremental_estimate_completes() {
    let circuit = block_test_circuit();
    let train = hybrid_model().train;
    let layout = BlockLayout::new(&circuit, &train, block_physics_config(), Vec::new()).unwrap();
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    simulation.install_block_layout(layout, false);
    let report = simulation.construction_block_report_json();
    assert!(
        report.contains("\"theoreticalHourlyRiders\":null"),
        "{report}"
    );
    assert!(
        report.contains("\"throughputStatus\":\"pending\""),
        "{report}"
    );
    assert!(!simulation.construction_advance_block_analysis(1));
    for _ in 0..1_000 {
        if simulation.construction_advance_block_analysis(1_000) {
            break;
        }
    }
    let report = simulation.construction_block_report_json();
    assert!(
        report.contains("\"throughputStatus\":\"ready\""),
        "{report}"
    );
    assert!(
        report_number(&report, "theoreticalHourlyRiders") > 0.0,
        "{report}"
    );
}

#[test]
fn block_suggestion_does_not_point_across_a_loop_without_a_candidate() {
    let circuit = demo_circuit();
    let mut train = hybrid_model().train;
    train.length_metres = 5.0;
    let runs = theme_park_sim::track::blocks::brake_runs(&circuit, train.length_metres);
    let loop_index = circuit
        .pieces()
        .iter()
        .position(|piece| piece.id == PieceId::Loop)
        .expect("demo circuit contains a loop");
    let loop_start: f64 = circuit.pieces()[..loop_index]
        .iter()
        .map(|piece| piece.length())
        .sum();
    let loop_end = loop_start + circuit.pieces()[loop_index].length();
    let candidate_boundaries: Vec<_> =
        runs.iter()
            .map(|run| run.boundary_metres)
            .fold(Vec::new(), |mut boundaries, boundary| {
                if boundaries.last() != Some(&boundary) {
                    boundaries.push(boundary);
                }
                boundaries
            });
    let before_loop = candidate_boundaries
        .iter()
        .copied()
        .filter(|boundary| *boundary <= loop_start)
        .max_by(f64::total_cmp)
        .expect("brake candidate before loop");
    let after_loop = candidate_boundaries
        .iter()
        .copied()
        .filter(|boundary| *boundary >= loop_end)
        .min_by(f64::total_cmp)
        .expect("brake candidate after loop");
    assert!(
        runs.iter()
            .all(|run| { run.boundary_metres <= before_loop || run.boundary_metres >= after_loop })
    );
    assert!(loop_start >= before_loop && loop_end <= after_loop);

    let validated_circuit = block_test_circuit();
    let validated_train = hybrid_model().train;
    let boundaries = block_test_boundaries(&validated_circuit);
    let layout = BlockLayout::new(
        &validated_circuit,
        &validated_train,
        block_physics_config(),
        boundaries[..3].to_vec(),
    )
    .expect("known three-brake layout validates");
    assert_eq!(editable_block_boundaries(&layout).len(), 3);

    let suggestions = super::block_layout_suggestions(&layout).join(" ");
    assert!(suggestions.contains("automatic placement"), "{suggestions}");
    assert!(!suggestions.contains("largest gap"), "{suggestions}");
    assert!(!suggestions.contains("brake there"), "{suggestions}");
}

#[test]
fn throughput_cache_distinguishes_different_brake_runs_at_the_same_boundary() {
    let mut pieces = block_test_circuit().pieces().to_vec();
    pieces
        .iter_mut()
        .find(|piece| piece.id == PieceId::Straight)
        .unwrap()
        .kind = super::PieceKind::Track;
    let circuit = Circuit::new(pieces).unwrap();
    let train = hybrid_model().train;
    let runs = theme_park_sim::track::blocks::brake_runs(&circuit, train.length_metres);
    let normal = block_physics_config();
    let gentle = super::PhysicsConfig {
        braking_acceleration: 0.5,
        ..normal
    };
    for run in &runs {
        if !runs.iter().any(|other| {
            other.boundary_metres == run.boundary_metres
                && other.piece_indices.len() > run.piece_indices.len()
        }) {
            continue;
        }
        let Ok(first) = BlockLayout::new(&circuit, &train, normal, vec![run.boundary_metres])
        else {
            continue;
        };
        let Ok(second) = BlockLayout::new(&circuit, &train, gentle, vec![run.boundary_metres])
        else {
            continue;
        };
        if first.boundaries() == second.boundaries()
            && first
                .boundaries()
                .iter()
                .enumerate()
                .any(|(index, _)| first.brake_pieces(index) != second.brake_pieces(index))
        {
            assert_ne!(
                super::block_layout_fingerprint(&first),
                super::block_layout_fingerprint(&second)
            );
            return;
        }
    }
    panic!("expected two validated brake lengths at the same boundary");
}

#[test]
fn block_report_distinguishes_native_powered_holds_from_pass_through_boundaries() {
    let circuit = block_powered_test_circuit();
    let train = hybrid_model().train;
    let layout = BlockLayout::new(&circuit, &train, block_physics_config(), Vec::new())
        .expect("powered lift layout validates");
    let powered_index = layout
        .boundaries()
        .iter()
        .enumerate()
        .find_map(|(index, _)| (!layout.powered_hold_pieces(index).is_empty()).then_some(index))
        .expect("layout has a native powered hold");
    assert!(layout.brake_pieces(powered_index).is_empty());

    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    simulation.install_block_layout(layout.clone(), false);
    let report = simulation.construction_block_report_json();
    let held_piece = layout.powered_hold_pieces(powered_index)[0];
    assert!(report.contains("\"brakePieceIndices\":[]"), "{report}");
    assert!(
        report.contains(&format!("\"poweredHoldPieceIndices\":[{held_piece}]")),
        "{report}"
    );
    assert!(
        report.contains("Fixed powered hold at the start of lift or launch track."),
        "{report}"
    );
    assert!(report.contains("Fixed pass-through boundary"), "{report}");
}

#[test]
fn installing_a_native_hold_layout_does_not_adopt_a_legacy_throughput_cache_entry() {
    let circuit = block_powered_test_circuit();
    let train = hybrid_model().train;
    let layout = BlockLayout::new(&circuit, &train, block_physics_config(), Vec::new())
        .expect("powered lift layout validates");
    assert!(
        layout
            .boundaries()
            .iter()
            .enumerate()
            .any(|(index, _)| { !layout.powered_hold_pieces(index).is_empty() })
    );

    let mut legacy_fingerprint = vec![layout.capacity() as u64];
    for (index, boundary) in layout.boundaries().iter().enumerate() {
        let brake_pieces = layout.brake_pieces(index);
        legacy_fingerprint.push(boundary.to_bits());
        legacy_fingerprint.push(brake_pieces.len() as u64);
        legacy_fingerprint.extend(brake_pieces.iter().map(|piece| *piece as u64));
    }
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    simulation
        .block_throughput_cache
        .insert(legacy_fingerprint, (1_234.0, String::new()));

    simulation.install_block_layout(layout, false);

    assert!(
        simulation.block_throughput_estimator.is_some(),
        "native hold identity must prevent adopting the old cache entry"
    );
}

#[test]
fn block_boundary_fingerprint_includes_native_hold_piece_identity() {
    assert_ne!(
        super::block_boundary_fingerprint(80.0, &[], &[3]),
        super::block_boundary_fingerprint(80.0, &[], &[4])
    );
}

#[test]
fn throughput_simulation_matches_recording_train_motion() {
    let circuit = block_test_circuit();
    let train = hybrid_model().train;
    let layout = BlockLayout::new(
        &circuit,
        &train,
        block_physics_config(),
        block_test_boundaries(&circuit),
    )
    .unwrap();
    assert!(layout.capacity() >= 3);
    let mut recorded = super::BlockOperation::new(layout.clone());
    let mut unrecorded = super::BlockOperation::new_without_reports(layout);
    let mut completed = 0;
    for _ in 0..30_000 {
        assert_eq!(recorded.can_dispatch(), unrecorded.can_dispatch());
        if recorded.can_dispatch() {
            assert_eq!(recorded.dispatch(), unrecorded.dispatch());
        }
        let recorded_outcomes = recorded.step(&[]);
        let unrecorded_outcomes = unrecorded.step(&[]);
        assert_eq!(recorded_outcomes.len(), unrecorded_outcomes.len());
        for ((id, outcome), (other_id, other_outcome)) in
            recorded_outcomes.iter().zip(&unrecorded_outcomes)
        {
            assert_eq!(id, other_id);
            assert_eq!(outcome.recovered, other_outcome.recovered);
            completed += 1;
        }
        let recorded_positions: Vec<_> = recorded
            .trains()
            .map(|(id, train)| (id, train.distance(), train.speed()))
            .collect();
        let unrecorded_positions: Vec<_> = unrecorded
            .trains()
            .map(|(id, train)| (id, train.distance(), train.speed()))
            .collect();
        assert_eq!(recorded_positions, unrecorded_positions);
    }
    assert!(completed > 0);
}

#[test]
fn automatic_search_publishes_baseline_verification_without_recovering_train() {
    let circuit = block_test_circuit();
    let train = hybrid_model().train;
    let config = block_physics_config();
    let search = super::BlockPlacementSearch::new(circuit, train, config).unwrap();
    let baseline = search.best().clone();
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    simulation.install_block_layout(baseline.clone(), true);
    simulation.coaster_operation = Some(super::BlockOperation::new(baseline));
    simulation.coaster_open = true;
    let train_id = simulation.coaster_operation.as_mut().unwrap().dispatch();
    simulation.block_placement_search = Some(search);
    for _ in 0..1_000 {
        simulation.construction_advance_block_analysis(1_000);
        if simulation
            .block_layout
            .as_ref()
            .unwrap()
            .capacity_search_complete()
        {
            break;
        }
    }
    assert!(
        simulation
            .block_layout
            .as_ref()
            .unwrap()
            .capacity_search_complete()
    );
    assert!(simulation.block_placement_search.is_some());
    let ids: Vec<_> = simulation
        .coaster_operation
        .as_ref()
        .unwrap()
        .trains()
        .map(|(id, _)| id)
        .collect();
    assert_eq!(ids, vec![train_id]);
}

#[test]
fn stalled_baseline_waits_for_active_train_before_reducing_capacity() {
    let circuit = super::demo_circuit();
    let train = hybrid_model().train;
    let config = super::PhysicsConfig {
        powered_sections: false,
        ..Default::default()
    };
    let baseline = super::block_layout_for_open(&circuit, &train, config).unwrap();
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    build_block_test_construction(&mut simulation);
    simulation.install_block_layout(baseline.clone(), false);
    simulation.coaster_operation = Some(super::BlockOperation::new(baseline));
    simulation.coaster_open = true;
    let train_id = simulation.coaster_operation.as_mut().unwrap().dispatch();
    simulation.block_manual_search =
        Some(super::BlockManualSearch::new(circuit, train, config, Vec::new()).unwrap());
    simulation.block_baseline_validation = true;
    for _ in 0..10 {
        simulation.construction_advance_block_analysis(1_000);
        if simulation.block_layout_change_pending() {
            break;
        }
    }
    assert!(simulation.block_layout_change_pending());
    assert_eq!(simulation.block_layout.as_ref().unwrap().capacity(), 1);
    assert!(
        simulation
            .construction_block_report_json()
            .contains("New dispatches are paused")
    );
    assert_eq!(
        simulation
            .coaster_operation
            .as_ref()
            .unwrap()
            .trains()
            .next()
            .unwrap()
            .0,
        train_id
    );
    for _ in 0..1_000 {
        if !simulation
            .coaster_operation
            .as_mut()
            .unwrap()
            .step(&[])
            .is_empty()
        {
            break;
        }
    }
    assert_eq!(
        simulation
            .coaster_operation
            .as_ref()
            .unwrap()
            .trains()
            .count(),
        0
    );
    simulation.construction_advance_block_analysis(1);
    assert_eq!(simulation.block_layout.as_ref().unwrap().capacity(), 0);
    assert!(
        simulation
            .construction_block_report_json()
            .contains("A train stalled")
    );
}

#[test]
fn baseline_validation_waits_to_adopt_powered_holds_until_the_train_completes() {
    let circuit = block_test_circuit();
    let train = hybrid_model().train;
    let config = block_physics_config();
    let baseline = super::block_layout_for_open(&circuit, &train, config).unwrap();
    assert!(!baseline.capacity_search_complete());
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    build_block_test_construction(&mut simulation);
    simulation.install_block_layout(baseline.clone(), false);
    simulation.coaster_operation = Some(super::BlockOperation::new(baseline));
    simulation.coaster_open = true;
    let train_id = simulation.coaster_operation.as_mut().unwrap().dispatch();
    simulation.block_manual_search =
        Some(super::BlockManualSearch::new(circuit, train, config, Vec::new()).unwrap());
    for _ in 0..1_000 {
        simulation.construction_advance_block_analysis(1_000);
        if simulation
            .block_manual_search
            .as_ref()
            .and_then(super::BlockManualSearch::result)
            .is_some()
        {
            break;
        }
    }
    assert!(simulation.block_layout_change_pending());
    assert!(
        !simulation
            .block_layout
            .as_ref()
            .unwrap()
            .capacity_search_complete()
    );
    let validated_layout = simulation
        .block_manual_search
        .as_ref()
        .and_then(super::BlockManualSearch::result)
        .and_then(|result| result.as_ref().ok())
        .expect("baseline validation found native powered holds");
    assert!(validated_layout.capacity_search_complete());
    assert_eq!(
        simulation
            .coaster_operation
            .as_ref()
            .unwrap()
            .trains()
            .map(|(id, _)| id)
            .collect::<Vec<_>>(),
        vec![train_id]
    );
    let outcome = (0..120_000)
        .find_map(|_| {
            simulation
                .coaster_operation
                .as_mut()
                .unwrap()
                .step(&[])
                .into_iter()
                .next()
        })
        .expect("running baseline train completes before adoption")
        .1;
    assert!(!outcome.recovered);
    simulation.construction_advance_block_analysis(0);
    assert!(simulation.block_manual_search.is_none());
    assert!(
        simulation
            .block_layout
            .as_ref()
            .unwrap()
            .capacity_search_complete()
    );
    let trains: Vec<_> = simulation
        .coaster_operation
        .as_ref()
        .unwrap()
        .trains()
        .map(|(id, _)| id)
        .collect();
    assert!(trains.is_empty());
}

#[test]
fn opening_starts_with_one_train_without_block_placement() {
    let circuit = block_test_circuit();
    let train = hybrid_model().train;
    let layout = super::block_layout_for_open(&circuit, &train, block_physics_config()).unwrap();
    assert_eq!(layout.capacity(), 1);
    assert!(editable_block_boundaries(&layout).is_empty());
    assert!(
        !layout.capacity_search_complete(),
        "opening should not synchronously simulate a lap"
    );
}

#[test]
fn throughput_uses_sustained_dispatches_after_the_initial_fill() {
    let circuit = unpowered_block_test_circuit();
    let train = hybrid_model().train;
    let physics = unpowered_block_physics_config();
    let layout =
        BlockLayout::new(&circuit, &train, physics, block_test_boundaries(&circuit)).unwrap();
    assert_eq!(layout.capacity(), 3);

    let mut operation = super::BlockOperation::new(layout.clone());
    while operation.can_dispatch() {
        operation.dispatch();
    }
    let mut dispatch_steps = Vec::new();
    for step in 1..=200_000 {
        assert!(
            operation
                .step(&[])
                .iter()
                .all(|(_, outcome)| !outcome.recovered)
        );
        while operation.can_dispatch() {
            operation.dispatch();
            dispatch_steps.push(step);
        }
        if dispatch_steps.len() >= 4 {
            break;
        }
    }
    assert!(
        dispatch_steps.len() >= 4,
        "not enough dispatches for a sustained estimate"
    );
    let seconds = (dispatch_steps[3] - dispatch_steps[0]) as f64 * TIME_STEP;
    let seats = usize::from(train.cars)
        * usize::from(train.rows_per_car)
        * usize::from(train.seats_per_row);
    let expected = seats as f64 * 3.0 * 3600.0 / seconds;
    let actual = theoretical_hourly_riders(&layout).unwrap();
    assert!(
        (actual - expected).abs() < 1e-6,
        "expected {expected}, got {actual}"
    );
}

#[test]
fn block_report_uses_simulated_dispatch_spacing_for_one_two_and_three_trains() {
    let circuit = unpowered_block_test_circuit();
    let train = hybrid_model().train;
    let physics = unpowered_block_physics_config();
    let brake_boundaries = block_test_boundaries(&circuit);

    let mut hourly_throughputs = Vec::new();
    for train_capacity in 1..=3 {
        let boundaries = brake_boundaries[..train_capacity + 1].to_vec();
        let layout = BlockLayout::new(&circuit, &train, physics, boundaries)
            .expect("the selected block layout is physically valid");
        assert_eq!(layout.capacity(), train_capacity);

        let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
        simulation.block_layout = Some(layout.clone());
        match theoretical_hourly_riders(&layout) {
            Ok(throughput) => simulation.block_hourly_throughput = throughput,
            Err(reason) => simulation.block_throughput_reason = reason.to_owned(),
        }
        let report = simulation.construction_block_report_json();
        assert_eq!(
            report_number(&report, "trainCapacity"),
            train_capacity as f64
        );
        assert!(
            report_number(&report, "theoreticalHourlyRiders") > 0.0,
            "capacity {train_capacity}: {report}"
        );
        assert!(report.contains("\"editable\":true"));
        assert!(report.contains("\"reasons\":[]"));
        assert!(report.contains("\"placementSearchComplete\":null"));
        assert!(report.contains("\"notes\":["));
        assert!(report.contains("\"suggestions\":[\""));
        hourly_throughputs.push(report_number(&report, "theoreticalHourlyRiders"));
    }
    assert!(
        hourly_throughputs
            .iter()
            .all(|throughput| throughput.is_finite())
    );
    assert!(
        hourly_throughputs[1] > hourly_throughputs[0],
        "two-train throughput {}/h did not exceed one-train throughput {}/h",
        hourly_throughputs[1],
        hourly_throughputs[0]
    );
    assert!(
        hourly_throughputs[2] > hourly_throughputs[0],
        "three-train throughput {}/h did not exceed one-train throughput {}/h",
        hourly_throughputs[2],
        hourly_throughputs[0]
    );
}

#[test]
fn train_count_rejects_values_outside_capacity_and_limits_dispatch() {
    let mut simulation = simulation_with_two_occupied_coaster_trains();
    assert_eq!(simulation.construction_train_count(), 2);
    assert!(!simulation.construction_set_train_count(0));
    assert!(!simulation.construction_set_train_count(3));
    assert_eq!(simulation.construction_train_count(), 2);
    assert!(simulation.construction_set_train_count(1));
    for _ in 0..5000 {
        simulation.tick();
        if simulation
            .coaster_operation
            .as_ref()
            .unwrap()
            .trains()
            .count()
            <= 1
        {
            break;
        }
    }
    assert!(
        simulation
            .coaster_operation
            .as_ref()
            .unwrap()
            .trains()
            .count()
            <= 1
    );
    for _ in 0..1000 {
        simulation.tick();
        assert!(
            simulation
                .coaster_operation
                .as_ref()
                .unwrap()
                .trains()
                .count()
                <= 1
        );
    }
}

fn simulation_with_two_occupied_coaster_trains() -> WasmSimulation {
    let mut simulation = WasmSimulation::new(42, 64, String::from("demo"));
    simulation.world = World::new(
        64,
        64,
        Surface {
            corners: [0; 4],
            water: None,
        },
    );
    simulation.facilities = ParkFacilities::new();
    simulation.construction_start(60, 60, 8, 0);
    for (piece, chain) in [
        ("Straight", false),
        ("Straight", false),
        ("Straight", true),
        ("Straight", false),
        ("Straight", false),
        ("TurnLargeEntry", false),
        ("TurnLargeExit", false),
        ("Straight", false),
        ("Straight", false),
        ("Straight", true),
        ("Straight", false),
        ("Straight", false),
        ("Straight", false),
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
    assert!(simulation.construction_can_close());
    simulation.construction_close();
    apply_automatic_blocks_after_finding(&mut simulation, 3);
    assert!(editable_block_boundaries(simulation.block_layout.as_ref().unwrap()).len() >= 3);
    assert!(simulation.construction_set_block_sections(Vec::new()));
    finish_manual_block_validation(&mut simulation);
    assert_eq!(simulation.block_layout.as_ref().unwrap().capacity(), 2);
    assert_eq!(simulation.construction_train_count(), 1);
    assert!(simulation.construction_set_train_count(2));
    connect_station_to_park_paths(&mut simulation);
    simulation.refresh_construction_support_rendering();
    assert!(!simulation.construction_placed_supports().is_empty());
    assert!(
        simulation.construction_message().is_empty(),
        "support route message: {}",
        simulation.construction_message()
    );
    let construction = simulation.construction.as_ref().unwrap();
    for piece in construction.pieces() {
        assert_eq!(
            theme_park_sim::track::clearance::check_clearance(
                &simulation.world,
                piece,
                &construction.model().train,
                &[],
                &[]
            ),
            Ok(()),
            "{:?}",
            piece.id
        );
    }

    simulation.tick();
    assert_eq!(
        simulation
            .coaster_operation
            .as_ref()
            .unwrap()
            .trains()
            .count(),
        0,
        "empty trains should not leave the station"
    );

    let mut occupied_train_ids = Vec::new();
    for _ in 0..10_000 {
        simulation.tick();
        occupied_train_ids = simulation
            .coaster_riders
            .iter()
            .filter_map(|(&train_id, guests)| (!guests.is_empty()).then_some(train_id))
            .collect();
        if occupied_train_ids.len() >= 2 {
            break;
        }
    }
    assert!(
        occupied_train_ids.len() >= 2,
        "expected passengers on two simultaneous trains, got {occupied_train_ids:?}; active={}, aware={}, revenue={}, trains={:?}",
        simulation
            .simulation
            .walking()
            .traits()
            .ids()
            .filter(|&id| simulation.simulation.walking().traits().is_active(id))
            .count(),
        simulation
            .simulation
            .behaviors()
            .guests
            .iter()
            .filter(|guest| guest
                .known_places()
                .iter()
                .any(|place| place.id == super::COASTER_PLACE_ID))
            .count(),
        simulation.simulation.revenue(),
        simulation
            .coaster_operation
            .as_ref()
            .unwrap()
            .trains()
            .map(|(id, train)| (id, train.distance(), train.speed()))
            .collect::<Vec<_>>()
    );
    simulation
}

#[test]
fn deleting_coaster_track_recovers_passengers_from_two_live_trains() {
    let mut simulation = simulation_with_two_occupied_coaster_trains();
    let occupied_train_ids: Vec<_> = simulation.coaster_riders.keys().copied().collect();
    let riders_before_deletion: Vec<GuestId> = occupied_train_ids
        .iter()
        .flat_map(|train_id| simulation.coaster_riders[train_id].iter().copied())
        .collect();
    assert!(!riders_before_deletion.is_empty());

    let train_ids: Vec<_> = simulation
        .coaster_operation
        .as_ref()
        .unwrap()
        .trains()
        .map(|(train_id, _)| train_id as f32)
        .collect();
    let train_states = simulation.construction_train_states();
    assert_eq!(train_states.len(), train_ids.len() * 6);
    assert_eq!(
        train_states
            .chunks_exact(6)
            .map(|state| state[0])
            .collect::<Vec<_>>(),
        train_ids
    );

    simulation.construction_delete(1);

    assert!(!simulation.coaster_open);
    assert!(simulation.coaster_riders.is_empty());
    assert!(simulation.construction_riders().is_empty());
    for rider in riders_before_deletion {
        assert!(simulation.simulation.walking().traits().is_active(rider));
        assert!(!simulation.simulation.behaviors().is_coaster_riding(rider));
    }
}

#[test]
fn automatic_improvement_waits_for_passengers_before_installing() {
    let mut simulation = simulation_with_two_occupied_coaster_trains();
    let riders: Vec<_> = simulation
        .coaster_riders
        .values()
        .flatten()
        .copied()
        .collect();
    let rider_train_ids: Vec<_> = simulation.coaster_riders.keys().copied().collect();
    let completed_before: Vec<_> = riders
        .iter()
        .map(|rider| simulation.simulation.behaviors().guests[rider.0 as usize].rides_completed)
        .collect();
    let circuit = simulation.block_layout.as_ref().unwrap().circuit();
    let station_index = circuit
        .pieces()
        .iter()
        .position(|piece| piece.kind == super::PieceKind::Station)
        .unwrap();
    let station_distance: f64 = circuit.pieces()[..station_index]
        .iter()
        .map(|piece| piece.length())
        .sum::<f64>()
        + circuit.pieces()[station_index].length() / 2.0;
    let circuit_length = circuit.length();
    let mut train_history = vec![(
        0,
        simulation
            .coaster_operation
            .as_ref()
            .unwrap()
            .trains()
            .map(|(id, train)| {
                (
                    id,
                    train.distance(),
                    train.speed(),
                    simulation.coaster_riders.get(&id).map_or(0, Vec::len),
                )
            })
            .collect::<Vec<_>>(),
    )];
    assert_eq!(simulation.block_layout.as_ref().unwrap().capacity(), 2);
    assert!(simulation.construction_auto_blocks());
    for _ in 0..10_000 {
        simulation.construction_advance_block_analysis(1_000);
        if simulation
            .block_placement_search
            .as_ref()
            .is_some_and(|search| search.best().capacity() > 2)
        {
            break;
        }
    }
    assert!(
        simulation
            .block_placement_search
            .as_ref()
            .is_some_and(|search| search.best().capacity() > 2)
    );
    assert_eq!(simulation.block_layout.as_ref().unwrap().capacity(), 2);
    assert!(
        riders
            .iter()
            .all(|&rider| simulation.simulation.behaviors().is_coaster_riding(rider))
    );
    let mut saw_empty_dispatch_pause = false;
    for tick in 1..=10_000 {
        simulation.construction_advance_block_analysis(0);
        simulation.tick();
        let active_train_ids: Vec<_> = simulation
            .coaster_operation
            .as_ref()
            .unwrap()
            .trains()
            .map(|(id, _)| id)
            .collect();
        assert!(
            simulation.block_layout.as_ref().unwrap().capacity() > 2
                || active_train_ids
                    .iter()
                    .all(|train_id| rider_train_ids.contains(train_id)),
            "a replacement train dispatched before the pending block layout was installed: active={active_train_ids:?}, original={rider_train_ids:?}"
        );
        if active_train_ids.is_empty()
            && simulation.block_layout.as_ref().unwrap().capacity() == 2
            && simulation
                .block_placement_search
                .as_ref()
                .is_some_and(|search| search.best().capacity() > 2)
        {
            assert!(simulation.block_layout_change_pending());
            saw_empty_dispatch_pause = true;
        }
        if tick % 1_000 == 0 {
            train_history.push((
                tick,
                simulation
                    .coaster_operation
                    .as_ref()
                    .unwrap()
                    .trains()
                    .map(|(id, train)| {
                        (
                            id,
                            train.distance(),
                            train.speed(),
                            simulation.coaster_riders.get(&id).map_or(0, Vec::len),
                        )
                    })
                    .collect::<Vec<_>>(),
            ));
        }
        if simulation.block_layout.as_ref().unwrap().capacity() > 2 {
            break;
        }
    }
    assert!(
        simulation.block_layout.as_ref().unwrap().capacity() > 2,
        "old capacity={}, open={}, waiting={}, circuit length={}, station distance={}, history={:?}, active={:?}, riders={:?}, search={:?}",
        simulation.block_layout.as_ref().unwrap().capacity(),
        simulation.coaster_open,
        simulation.block_layout_change_pending(),
        circuit_length,
        station_distance,
        train_history,
        simulation
            .coaster_operation
            .as_ref()
            .unwrap()
            .trains()
            .map(|(id, train)| (id, train.distance(), train.speed()))
            .collect::<Vec<_>>(),
        simulation
            .coaster_riders
            .iter()
            .map(|(id, guests)| (*id, guests.len()))
            .collect::<Vec<_>>(),
        simulation.block_placement_search.as_ref().map(|search| (
            search.best().capacity(),
            search.work_completed(),
            search.is_complete()
        ))
    );
    assert!(
        saw_empty_dispatch_pause,
        "fixture did not exercise the empty substep window before layout installation"
    );
    assert!(
        simulation.block_placement_search.is_some(),
        "search should continue toward the maximum"
    );
    for (rider, before) in riders.into_iter().zip(completed_before) {
        assert!(
            simulation.simulation.behaviors().guests[rider.0 as usize].rides_completed > before,
            "rider must complete, not be recovered"
        );
    }
}

#[test]
fn block_commands_keep_valid_layout_after_rejected_edit() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    build_block_test_construction(&mut simulation);
    apply_automatic_blocks_after_finding(&mut simulation, 1);
    let layout = simulation.block_layout.as_ref().unwrap();
    let original_boundaries = editable_block_boundaries(layout);
    assert!(!original_boundaries.is_empty());

    assert!(!simulation.construction_move_block_section(0, f64::NAN));
    assert!(
        simulation
            .construction_message()
            .contains("outside the circuit")
    );
    assert_eq!(
        editable_block_boundaries(simulation.block_layout.as_ref().unwrap()),
        original_boundaries
    );

    let first_boundary = original_boundaries[0];
    assert!(simulation.construction_move_block_section(0, first_boundary));
    finish_manual_block_validation(&mut simulation);
    assert!(simulation.construction_remove_block_section(0));
    finish_manual_block_validation(&mut simulation);
    assert!(simulation.construction_add_block_section(first_boundary));
    finish_manual_block_validation(&mut simulation);
    assert_eq!(
        editable_block_boundaries(simulation.block_layout.as_ref().unwrap()),
        original_boundaries
    );
    let report = simulation.construction_block_report_json();
    assert!(report.contains("\"reasons\":[]"));
    assert!(report.contains("\"editable\":false"));
    assert!(report.contains("\"trackKind\":\"station\""));

    simulation.construction_delete(3);
    assert!(simulation.block_layout.is_none());
    assert!(
        simulation
            .construction_block_report_json()
            .contains("\"trainCapacity\":0")
    );
}

#[test]
fn demo_exports_wooden_lattice_diagonals() {
    let simulation = WasmSimulation::new(42, 0, String::from("demo"));
    assert!(
        simulation
            .coaster_supports()
            .chunks_exact(7)
            .any(|segment| {
                (segment[1] != segment[4] || segment[2] != segment[5]) && segment[3] != segment[6]
            })
    );
}

#[test]
fn coaster_exports_rails_and_evenly_spaced_ties_from_circuit_curves() {
    let simulation = WasmSimulation::new(42, 0, String::from("demo"));
    let rails = simulation.coaster_rails();
    let ties = simulation.coaster_ties();
    assert!(!rails.is_empty() && rails.len() % 19 == 0);
    assert_eq!(
        ties.len() / 13,
        (simulation
            .demo_coaster()
            .expect("demo park")
            .circuit
            .length()
            / 2.0)
            .ceil() as usize
    );
    assert_eq!(ties.len() % 13, 0);
    for (index, tie) in ties.chunks_exact(13).enumerate() {
        let sample = simulation
            .demo_coaster()
            .expect("demo park")
            .circuit
            .sample(index as f64 * 2.0);
        for axis in 0..3 {
            let expected = [sample.position.x, sample.position.y, sample.position.z][axis] / 4.0;
            let midpoint = (f64::from(tie[1 + axis]) + f64::from(tie[4 + axis])) / 2.0;
            assert!((midpoint - expected).abs() < 0.0001);
        }
        let dx = tie[1] - tie[4];
        let dy = tie[2] - tie[5];
        let dz = tie[3] - tie[6];
        assert!(((dx * dx + dy * dy + dz * dz).sqrt() - 0.44).abs() < 0.0001);
    }
    let first = simulation
        .demo_coaster()
        .expect("demo park")
        .circuit
        .pieces()[0]
        .sample(0.0);
    let lateral = first.up.cross(first.tangent);
    for (rail, sign) in rails.chunks_exact(19).take(2).zip([-1.0, 1.0]) {
        let expected = (first.position + lateral * (sign * 0.72)) / 4.0;
        for (actual, expected) in rail[1..4].iter().zip([expected.x, expected.y, expected.z]) {
            assert!((f64::from(*actual) - expected).abs() < 0.0001);
        }
    }
    let segments: Vec<_> = rails.chunks_exact(38).collect();
    for index in 0..segments.len() {
        let current = segments[index];
        let next = segments[(index + 1) % segments.len()];
        for rail_offset in [0, 19] {
            for axis in 0..3 {
                assert!(
                    (current[rail_offset + 10 + axis] - next[rail_offset + 1 + axis]).abs()
                        < 0.0001
                );
            }
        }
    }
    assert!(rails.iter().chain(&ties).all(|value| value.is_finite()));
}

#[test]
fn supports_reroute_around_another_sampled_track() {
    let straight = catalog()
        .iter()
        .find(|piece| piece.id == PieceId::Straight)
        .unwrap();
    let state = TrackState {
        x: 4,
        y: 4,
        z: 5,
        ..TrackState::origin()
    };
    let upper = straight.at(state).unwrap();
    let lower = straight.at(TrackState { z: 2, ..state }).unwrap();
    let world = World::new(
        16,
        16,
        Surface {
            corners: [0; 4],
            water: None,
        },
    );
    let direct = generate_supports(&upper, &world, &[]).unwrap();
    let rerouted = generate_supports(
        &upper,
        &world,
        &track_support_obstacles(&[lower], &hybrid_model().train),
    )
    .unwrap();
    assert!(
        direct
            .iter()
            .all(|support| support.footing.y == support.anchor.y)
    );
    assert!(
        rerouted
            .iter()
            .all(|support| support.footing.y != support.anchor.y)
    );
}

#[test]
fn demo_train_state_reports_lift_and_valley_forces() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    let lift_speed = 5.0;
    let mut saw_lift = false;
    let mut saw_valley = false;
    for _ in 0..20_000 {
        simulation.tick();
        let state = simulation.coaster_train_states();
        assert_eq!(state.len(), 6);
        assert_eq!(state[0], 0.0);
        assert_eq!(state[5], 1.0);
        saw_lift |= state[2] == 1.0 && (state[1] - lift_speed).abs() < 1e-4;
        saw_valley |= saw_lift && state[2] == 0.0 && state[3] > 1.1;
        if saw_valley {
            break;
        }
    }
    assert!(saw_lift, "train never reached lift speed on the lift");
    assert!(saw_valley, "train never reached a valley after the lift");
}

#[test]
fn construction_train_states_follow_block_operation_trains() {
    let circuit = unpowered_block_test_circuit();
    let train = hybrid_model().train;
    let layout = BlockLayout::new(
        &circuit,
        &train,
        unpowered_block_physics_config(),
        block_test_boundaries(&circuit),
    )
    .unwrap();
    let mut operation = super::BlockOperation::new(layout.clone());
    operation.dispatch();
    for _ in 0..200_000 {
        operation.step(&[]);
        if operation.can_dispatch() {
            operation.dispatch();
            break;
        }
    }
    assert_eq!(operation.trains().count(), 2);
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    simulation.install_block_layout(layout, true);
    simulation.coaster_operation = Some(operation);
    let states = simulation.construction_train_states();
    assert_eq!(states.len(), 12);
    assert!(states[0] < states[6]);
    assert_eq!(states[5], 0.0);
    assert_eq!(states[11], 0.0);
}

#[test]
fn coaster_snapshots_contain_stable_geometry_and_moving_train_poses() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    let geometry = simulation.coaster_geometry();
    let supports = simulation.coaster_supports();
    let initial_poses = simulation.coaster_train_poses();
    assert!(!geometry.is_empty() && geometry.len() % 11 == 0);
    assert!(!supports.is_empty() && supports.len() % 7 == 0);
    assert!(!initial_poses.is_empty() && initial_poses.len() % 11 == 0);
    let coaster = simulation.demo_coaster().expect("demo park");
    let tangent = coaster.circuit.sample(coaster.train.distance()).tangent;
    assert_eq!(
        &initial_poses[4..7],
        &[tangent.x as f32, tangent.y as f32, tangent.z as f32]
    );
    assert!(
        geometry
            .iter()
            .chain(&supports)
            .chain(&initial_poses)
            .all(|value| value.is_finite())
    );
    for _ in 0..30 {
        simulation.tick();
    }
    assert_ne!(simulation.coaster_train_poses(), initial_poses);
    assert_eq!(simulation.coaster_geometry(), geometry);
    assert_eq!(simulation.coaster_supports(), supports);
}

#[test]
fn demo_train_completes_a_circuit_on_simulation_ticks() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    let circuit_length = simulation
        .demo_coaster()
        .expect("demo park")
        .circuit
        .length();
    let mut maximum_distance: f64 = 0.0;
    for _ in 0..9000 {
        simulation.tick();
        maximum_distance = maximum_distance.max(
            simulation
                .demo_coaster()
                .expect("demo park")
                .train
                .distance(),
        );
        if simulation
            .demo_coaster()
            .expect("demo park")
            .train
            .distance()
            >= circuit_length
        {
            return;
        }
    }
    panic!(
        "demo train failed to complete circuit: distance={}, maximum={}, speed={}, length={}, notes={:?}",
        simulation
            .demo_coaster()
            .expect("demo park")
            .train
            .distance(),
        maximum_distance,
        simulation.demo_coaster().expect("demo park").train.speed(),
        circuit_length,
        simulation.demo_coaster().expect("demo park").train.notes()
    );
}

#[test]
fn guest_snapshot_contains_stable_ids_appearance_and_walking_state() {
    let mut simulation = WasmSimulation::new(42, 12, String::from("demo"));
    let ids = simulation.guest_ids();
    let appearances = simulation.guest_appearances();
    let initial = simulation.snapshot();
    assert_eq!(ids, (0..12).collect::<Vec<_>>());
    assert_eq!(appearances.len(), 12 * 5);
    assert_eq!(initial.len(), 12 * 7);
    for _ in 0..30 {
        simulation.tick();
    }
    assert_eq!(simulation.guest_ids(), ids);
    assert_eq!(simulation.guest_appearances(), appearances);
    assert_ne!(simulation.snapshot(), initial);
    assert_eq!(simulation.tick_count(), 30);
}

#[test]
fn surface_edits_report_changes_and_update_the_snapshot() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    let surface = simulation.world.surface(10, 20);
    assert!(!simulation.set_world_surface(10, 20, surface.corners.to_vec(), surface.water));
    assert!(simulation.set_world_surface(10, 20, vec![1, 2, 3, 4], Some(-1)));
    assert_eq!(
        simulation.world.surface(10, 20),
        Surface {
            corners: [1, 2, 3, 4],
            water: Some(-1)
        }
    );
    let offset = (20 * 512 + 10) * 6;
    assert_eq!(
        &simulation.world_surfaces()[offset..offset + 6],
        &[1, 2, 3, 4, 1, -1]
    );
    let path = Position {
        x: 218,
        y: 256,
        height: 8,
    };
    assert!(simulation.world.path(path).is_some());
    assert!(simulation.set_world_surface(218, 256, vec![9; 4], None));
    assert!(simulation.world.path(path).is_none());
    assert!(
        !simulation
            .world_paths()
            .chunks_exact(4)
            .any(|entry| entry == [218, 256, 8, 0])
    );
}

#[test]
fn demo_has_two_localized_hills_with_matching_neighbor_edges() {
    let simulation = WasmSimulation::new(42, 0, String::from("demo"));
    for (center_x, center_y, base) in [(90, 90, 8), (400, 400, 23)] {
        assert!(simulation.world.surface(center_x, center_y).corners[0] > base);
        assert_eq!(
            simulation.world.surface(center_x - 13, center_y).corners,
            [base; 4]
        );
        for y in center_y - 12..center_y + 12 {
            for x in center_x - 12..center_x + 12 {
                let tile = simulation.world.surface(x, y).corners;
                let east = simulation.world.surface(x + 1, y).corners;
                let south = simulation.world.surface(x, y + 1).corners;
                assert_eq!([tile[1], tile[2]], [east[0], east[3]]);
                assert_eq!([tile[3], tile[2]], [south[0], south[1]]);
            }
        }
    }
}

#[test]
fn demo_snapshot_contains_real_terrain_water_and_paths() {
    let simulation = WasmSimulation::new(42, 0, String::from("demo"));
    assert_eq!(simulation.world_dimensions(), vec![512, 512]);
    let surfaces = simulation.world_surfaces();
    assert_eq!(surfaces.len(), 512 * 512 * 6);
    assert!(surfaces.chunks_exact(6).any(|tile| tile[0..4] != [8; 4]));
    assert!(surfaces.chunks_exact(6).any(|tile| tile[4] == 1));
    let carved = simulation.world_carved();
    assert!(carved.chunks_exact(4).any(|span| span == [224, 250, 3, 5]));
    let hash = simulation.world_state_hash();
    assert_eq!(
        hash,
        WasmSimulation::new(42, 0, String::from("demo")).world_state_hash()
    );
    let paths = simulation.world_paths();
    assert!(!paths.is_empty());
    assert_eq!(paths.len() % 4, 0);
    assert!(paths.chunks_exact(4).any(|path| path[3] != 0));
    for path in paths.chunks_exact(4) {
        let tile = (path[1] as usize * 512 + path[0] as usize) * 6;
        let corners = &surfaces[tile..tile + 4];
        let height = path[2];
        let expected = match path[3] {
            0 => [height; 4],
            1 => [height + 1, height + 1, height, height],
            2 => [height, height + 1, height + 1, height],
            3 => [height, height, height + 1, height + 1],
            4 => [height + 1, height, height, height + 1],
            _ => panic!("invalid slope"),
        };
        let surface_path = corners == expected;
        let underground_path = carved.chunks_exact(4).any(|span| {
            span[0] == path[0]
                && span[1] == path[1]
                && span[2] <= height
                && span[3] >= height + if path[3] == 0 { 1 } else { 2 }
        });
        assert!(surface_path || underground_path);
    }
}

#[test]
fn demo_paths_form_a_surface_network_connected_to_an_underground_loop() {
    let simulation = WasmSimulation::new(42, 0, String::from("demo"));
    let world = &simulation.world;
    let position = |x, y, height| Position { x, y, height };
    let junction = position(218, 256, 8);
    assert!(world.neighbours(junction).len() >= 2);
    let entrance = [
        position(218, 250, 8),
        position(219, 250, 7),
        position(220, 250, 6),
        position(221, 250, 5),
        position(222, 250, 4),
        position(223, 250, 3),
        position(224, 250, 3),
    ];
    for pair in entrance.windows(2) {
        assert!(world.neighbours(pair[0]).contains(&pair[1]));
    }
    let loop_positions = [
        position(224, 250, 3),
        position(225, 250, 3),
        position(226, 250, 3),
        position(227, 250, 3),
        position(228, 250, 3),
        position(228, 251, 3),
        position(228, 252, 3),
        position(228, 253, 3),
        position(228, 254, 3),
        position(227, 254, 3),
        position(226, 254, 3),
        position(225, 254, 3),
        position(224, 254, 3),
        position(224, 253, 3),
        position(224, 252, 3),
        position(224, 251, 3),
        position(224, 250, 3),
    ];
    for pair in loop_positions.windows(2) {
        assert!(world.neighbours(pair[0]).contains(&pair[1]));
    }
    assert_eq!(world.neighbours(position(224, 250, 3)).len(), 3);
    for pair in (250..256).map(|y| (position(218, y, 8), position(218, y + 1, 8))) {
        assert!(world.neighbours(pair.0).contains(&pair.1));
    }
}
