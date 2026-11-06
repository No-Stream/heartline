use super::super::Park;
use super::{BlockCommandError, block_physics_config};
use crate::facilities::{ParkFacilities, coaster_access_openings, coaster_facility};
use crate::guests::park::ParkGuests;
use crate::physics::{
    CarReading, Circuit, ConstructionTestRunRecorder, GForces, PhysicsConfig, Train,
};
use crate::track::blocks::{BlockLayout, BlockManualSearch, BlockPlacementSearch, brake_runs};
use crate::track::construction::Construction;
use crate::track::{PieceId, TrackState, TrainConfiguration, hybrid_model};
use crate::world::{Path, PathKind, Position, Surface, World};

fn advance_successfully(park: &mut Park, work_steps: u32) -> bool {
    let (complete, error) = park.construction_advance_block_analysis(work_steps);
    assert!(
        error.is_none(),
        "unexpected block analysis error: {error:?}"
    );
    complete
}

fn expect_command_success(result: Result<(), BlockCommandError>) {
    assert!(result.is_ok(), "unexpected block command error: {result:?}");
}

pub(super) fn block_test_park() -> Park {
    let mut world = World::new(
        512,
        512,
        Surface {
            corners: [0; 4],
            water: None,
        },
    );
    let entrance = Position {
        x: 0,
        y: 0,
        height: 0,
    };
    world.place_path(Path {
        position: entrance,
        slope: None,
    });
    let train_configuration = hybrid_model().train;
    let mut construction = Construction::new(
        TrackState {
            x: 100,
            y: 100,
            z: 40,
            ..TrackState::origin()
        },
        hybrid_model(),
    );
    for (index, piece_id) in [
        PieceId::Station,
        PieceId::Straight,
        PieceId::Straight,
        PieceId::Straight,
        PieceId::Straight,
        PieceId::Straight,
        PieceId::TurnLargeEntry,
        PieceId::TurnLargeExit,
        PieceId::Straight,
        PieceId::Straight,
        PieceId::Straight,
        PieceId::Straight,
        PieceId::Straight,
        PieceId::Straight,
        PieceId::TurnLargeEntry,
        PieceId::TurnLargeExit,
    ]
    .into_iter()
    .enumerate()
    {
        construction.set_chain(matches!(index, 3 | 10));
        construction
            .append(piece_id, &world, &[])
            .unwrap_or_else(|error| panic!("{piece_id:?}: {error:?}"));
    }
    construction.close().expect("test circuit closes");
    let circuit = construction
        .circuit()
        .expect("closed construction has a circuit");
    let train = Train::new(
        &circuit,
        &train_configuration,
        PhysicsConfig::default(),
        0.0,
        0.0,
    );
    let simulation = ParkGuests::new(42, 0, &world, entrance, 1);
    let mut park = Park::new(
        world,
        ParkFacilities::new(),
        simulation,
        circuit,
        train,
        train_configuration,
    );
    park.construction = Some(construction);
    park
}

pub(super) fn construction_inputs(park: &Park) -> (Circuit, TrainConfiguration) {
    let construction = park.construction.as_ref().expect("test coaster exists");
    (
        construction.circuit().expect("test coaster is closed"),
        construction.model().train.clone(),
    )
}

pub(super) fn unverified_layout(park: &Park) -> BlockLayout {
    let (circuit, train) = construction_inputs(park);
    BlockLayout::unverified_baseline(&circuit, &train, block_physics_config())
        .expect("test circuit supports a baseline layout")
}

#[test]
fn test_placement_advance_uses_exact_work_independent_of_throughput() {
    let mut park = block_test_park();
    park.install_block_layout(unverified_layout(&park), false);
    expect_command_success(park.construction_auto_blocks());
    let throughput_before = park
        .block_throughput_estimator
        .as_ref()
        .unwrap()
        .work_steps_completed();

    park.construction_advance_block_placement_for_test(200);

    assert_eq!(
        park.block_placement_search
            .as_ref()
            .unwrap()
            .work_completed(),
        200
    );
    assert_eq!(
        park.block_throughput_estimator
            .as_ref()
            .unwrap()
            .work_steps_completed(),
        throughput_before
    );
}

#[test]
fn manual_validation_and_throughput_share_the_requested_work_budget() {
    let mut park = block_test_park();
    let (circuit, train) = construction_inputs(&park);
    let config = block_physics_config();
    let boundary = brake_runs(&circuit, train.length_metres)[0].boundary_metres;
    park.install_block_layout(unverified_layout(&park), false);
    park.block_manual_search = Some(
        BlockManualSearch::new(circuit, train, config, vec![boundary])
            .expect("candidate brake boundary is accepted"),
    );

    let hash_before_analysis = park.state_hash();
    assert!(!advance_successfully(&mut park, 200));
    assert_ne!(park.state_hash(), hash_before_analysis);

    let validation_work = park
        .block_manual_search
        .as_ref()
        .expect("manual validation remains pending")
        .work_completed();
    let throughput_work = park
        .block_throughput_estimator
        .as_ref()
        .expect("throughput estimate remains pending")
        .work_steps_completed();
    assert!(validation_work > 0);
    assert!(throughput_work > 0);
    assert!(validation_work + throughput_work <= 200);
}

#[test]
fn automatic_placement_and_throughput_share_bounded_work_without_starvation() {
    let mut park = block_test_park();
    park.install_block_layout(unverified_layout(&park), false);
    expect_command_success(park.construction_auto_blocks());

    let progress = |park: &Park| {
        (
            park.block_placement_search
                .as_ref()
                .expect("placement search remains pending")
                .work_completed(),
            park.block_throughput_estimator
                .as_ref()
                .expect("throughput estimate remains pending")
                .work_steps_completed(),
        )
    };
    assert_eq!(progress(&park), (0, 0));
    assert!(!advance_successfully(&mut park, 0));
    assert_eq!(progress(&park), (0, 0));

    assert!(!advance_successfully(&mut park, 200));
    let (placement_work, throughput_work) = progress(&park);
    assert!(placement_work > 0 && throughput_work > 0);
    assert!(placement_work + throughput_work <= 200);

    assert!(!advance_successfully(&mut park, 1));
    assert!(!advance_successfully(&mut park, 1));
    let (next_placement_work, next_throughput_work) = progress(&park);
    assert!(next_placement_work > placement_work);
    assert!(next_throughput_work > throughput_work);
    assert!(next_placement_work - placement_work + next_throughput_work - throughput_work <= 2);

    assert!(!advance_successfully(&mut park, 5_000));
    let (capped_placement_work, capped_throughput_work) = progress(&park);
    assert!(
        capped_placement_work - next_placement_work + capped_throughput_work - next_throughput_work
            <= 1_000
    );
}

#[test]
fn manual_layout_installation_waits_until_active_train_is_gone() {
    let mut park = block_test_park();
    let (circuit, train) = construction_inputs(&park);
    let config = block_physics_config();
    let baseline = unverified_layout(&park);
    park.coaster_open = true;
    park.install_block_layout(baseline, false);
    let train_id = park
        .coaster_operation
        .as_mut()
        .expect("open coaster has an operation")
        .dispatch();
    park.block_manual_search = Some(
        BlockManualSearch::new(circuit, train, config, Vec::new())
            .expect("baseline validation inputs are valid"),
    );

    for _ in 0..1_000 {
        advance_successfully(&mut park, 1_000);
        if park
            .block_manual_search
            .as_ref()
            .and_then(BlockManualSearch::result)
            .is_some()
        {
            break;
        }
    }

    assert!(park.block_layout_change_pending());
    assert!(
        !park
            .block_layout
            .as_ref()
            .expect("baseline remains installed")
            .capacity_search_complete()
    );
    assert_eq!(
        park.coaster_operation
            .as_ref()
            .expect("open coaster keeps its operation")
            .trains()
            .map(|(id, _)| id)
            .collect::<Vec<_>>(),
        vec![train_id]
    );

    park.coaster_operation
        .as_mut()
        .expect("open coaster keeps its operation")
        .recover_all();
    advance_successfully(&mut park, 0);

    assert!(park.block_manual_search.is_none());
    assert!(
        park.block_layout
            .as_ref()
            .expect("verified layout is installed")
            .capacity_search_complete()
    );
    assert!(
        park.coaster_operation
            .as_ref()
            .expect("installing a layout resets the live operation")
            .trains()
            .next()
            .is_none()
    );
}

#[test]
fn installing_a_live_layout_preserves_the_coaster_model_force_limits() {
    let mut park = block_test_park();
    let layout = unverified_layout(&park);
    let limits = park
        .construction
        .as_ref()
        .unwrap()
        .model()
        .comfortable_force_limits;
    park.coaster_open = true;
    park.install_block_layout(layout.clone(), false);

    let expected = crate::physics::BlockOperation::new_with_limits(layout, limits);
    assert_eq!(
        park.coaster_operation.as_ref().unwrap().state_hash(),
        expected.state_hash()
    );
}

#[test]
fn coaster_facility_uses_and_preserves_the_latest_front_car_ratings() {
    let mut park = block_test_park();
    let station = park
        .construction
        .as_ref()
        .unwrap()
        .pieces()
        .iter()
        .find(|piece| piece.kind == crate::track::PieceKind::Station)
        .unwrap();
    let (queue, exit) = crate::facilities::station_positions(station).unwrap();
    for (position, kind) in [(queue, PathKind::Queue), (exit, PathKind::Normal)] {
        park.world.set_surface(
            position.x,
            position.y,
            Surface {
                corners: [position.height; 4],
                water: None,
            },
        );
        park.world
            .build_path_route(position, position, kind, 0)
            .unwrap();
    }

    expect_command_success(park.construction_set_open(true));
    let default_facility = park.coaster_facility.unwrap();
    assert_eq!(default_facility.place.intensity, 50);
    assert_eq!(default_facility.place.nausea, 30);
    expect_command_success(park.construction_set_open(false));

    let (circuit, _) = construction_inputs(&park);
    let mut model = crate::track::hybrid_model();
    model.train.cars = 2;
    let mut recorder = ConstructionTestRunRecorder::for_circuit_with_train_and_limits(
        &circuit,
        &model.train,
        model.comfortable_force_limits,
    );
    let mut distance = 0.0;
    while distance < circuit.length() {
        let sample = circuit.sample(distance);
        let reading = |vertical| CarReading {
            distance,
            speed: 20.0,
            g: GForces {
                vertical,
                lateral: 0.0,
                longitudinal: 0.0,
            },
            airtime: vertical < 0.0,
            height_m: sample.position.z,
            inverted: sample.up.z < 0.0,
            roll: sample.roll,
            piece_index: circuit.piece_index(distance),
        };
        recorder.record(&[vec![reading(1.0), reading(5.0)]]);
        distance += 1.0;
    }
    let (averaged_report, front_car_report) = recorder.finish_with_front_car(true, false);
    assert_ne!(
        crate::guests::knowledge::guest_rating(averaged_report.intensity),
        crate::guests::knowledge::guest_rating(front_car_report.intensity)
    );
    assert_ne!(
        crate::guests::knowledge::guest_rating(averaged_report.nausea),
        crate::guests::knowledge::guest_rating(front_car_report.nausea)
    );
    park.construction_report = Some(averaged_report);
    park.remember_front_car_ratings(&front_car_report);
    let expected_intensity = crate::guests::knowledge::guest_rating(front_car_report.intensity);
    let expected_nausea = crate::guests::knowledge::guest_rating(front_car_report.nausea);

    expect_command_success(park.construction_set_open(true));
    assert!(park.construction_report.is_none());
    let facility = park.coaster_facility.unwrap();
    assert_eq!(facility.place.intensity, expected_intensity);
    assert_eq!(facility.place.nausea, expected_nausea);

    expect_command_success(park.construction_set_open(false));
    expect_command_success(park.construction_set_open(true));
    let rebuilt_facility = park.coaster_facility.unwrap();
    assert_eq!(rebuilt_facility.place.intensity, expected_intensity);
    assert_eq!(rebuilt_facility.place.nausea, expected_nausea);
}

#[test]
fn throughput_is_cached_and_clear_resets_analysis_state() {
    let mut park = block_test_park();
    let layout = unverified_layout(&park);
    park.install_block_layout(layout.clone(), false);
    assert!(park.block_throughput_estimator.is_some());
    assert_eq!(park.block_hourly_throughput, 0.0);

    for _ in 0..1_000 {
        if advance_successfully(&mut park, 1_000) {
            break;
        }
    }

    assert!(park.block_throughput_estimator.is_none());
    assert!(park.block_hourly_throughput.is_finite());
    assert!(park.block_hourly_throughput > 0.0);
    assert!(park.block_throughput_reason.is_empty());
    assert_eq!(park.block_throughput_cache.len(), 1);
    let cached_throughput = park.block_hourly_throughput;

    park.install_block_layout(layout, false);
    assert!(park.block_throughput_estimator.is_none());
    assert_eq!(park.block_hourly_throughput, cached_throughput);

    let (circuit, train) = construction_inputs(&park);
    park.block_manual_search = Some(
        BlockManualSearch::new(
            circuit.clone(),
            train.clone(),
            block_physics_config(),
            Vec::new(),
        )
        .expect("baseline validation inputs are valid"),
    );
    park.block_placement_search = Some(
        BlockPlacementSearch::new(circuit.clone(), train.clone(), block_physics_config())
            .expect("placement search inputs are valid"),
    );
    park.block_placement_cache = Some(
        BlockPlacementSearch::new(circuit, train, block_physics_config())
            .expect("cached placement search inputs are valid"),
    );
    park.block_apply_best_requested = true;
    park.block_baseline_validation = true;
    park.block_layout_is_automatic = true;
    park.block_analysis_extra_to_throughput = true;
    park.desired_train_count = Some(1);
    park.block_message = "stale block message".into();

    park.clear_block_layout();

    assert!(park.block_layout.is_none());
    assert!(park.block_manual_search.is_none());
    assert!(park.block_placement_search.is_none());
    assert!(park.block_placement_cache.is_none());
    assert!(!park.block_apply_best_requested);
    assert!(!park.block_baseline_validation);
    assert!(!park.block_layout_is_automatic);
    assert!(!park.block_analysis_extra_to_throughput);
    assert!(park.desired_train_count.is_none());
    assert!(park.block_message.is_empty());
    assert_eq!(park.block_hourly_throughput, 0.0);
    assert!(park.block_throughput_reason.is_empty());
    assert!(park.block_throughput_estimator.is_none());
    assert!(park.block_throughput_cache.is_empty());
}

#[test]
fn cars_per_train_changes_configuration_and_invalidates_analysis() {
    let mut park = block_test_park();
    park.install_block_layout(unverified_layout(&park), false);
    let old_train = construction_inputs(&park).1;
    assert_eq!(old_train.cars, 6);
    assert_eq!(
        park.construction_set_cars_per_train(3),
        Err(BlockCommandError::InvalidCarsPerTrain)
    );
    assert!(park.block_layout.is_some());
    park.construction_set_cars_per_train(4).unwrap();
    let train = construction_inputs(&park).1;
    assert_eq!(train.cars, 4);
    assert_eq!(train.length_metres, 12.0);
    assert!(park.block_placement_search.is_some());
    assert_eq!(park.block_layout.as_ref().unwrap().train().cars, 4);
    park.construction_set_cars_per_train(6).unwrap();
    assert_eq!(
        construction_inputs(&park).1.length_metres,
        old_train.length_metres
    );
}

#[test]
fn cars_per_train_ranges_preserve_existing_defaults() {
    use crate::track::{multi_launch_model, wooden_twister_model};
    for (model, minimum, maximum, default) in [
        (hybrid_model(), 4, 8, 6),
        (wooden_twister_model(), 4, 12, 12),
        (multi_launch_model(), 3, 6, 6),
    ] {
        assert_eq!(model.cars_per_train, minimum..=maximum);
        assert_eq!(model.train.cars, default);
    }
}

#[test]
fn cars_per_train_rejects_changes_while_open_and_preserves_noop_analysis() {
    let mut park = block_test_park();
    park.install_block_layout(unverified_layout(&park), false);
    park.construction_set_cars_per_train(6).unwrap();
    assert!(park.block_layout.is_some());
    park.coaster_open = true;
    assert_eq!(
        park.construction_set_cars_per_train(4),
        Err(BlockCommandError::CoasterOpen)
    );
    assert_eq!(construction_inputs(&park).1.cars, 6);
    assert!(park.block_layout.is_some());
}

#[test]
fn cars_per_train_controls_physics_and_station_capacity() {
    let mut park = block_test_park();
    let [queue, exit] = coaster_access_openings(park.construction.as_ref().unwrap()).unwrap();
    let queue_entry = Position {
        x: queue.position.x - 1,
        ..queue.position
    };
    for position in [queue.position, exit.position, queue_entry] {
        assert!(
            park.world
                .set_surface(
                    position.x,
                    position.y,
                    Surface {
                        corners: [position.height; 4],
                        water: None,
                    }
                )
                .is_empty()
        );
    }
    park.world
        .build_path_route(queue_entry, queue_entry, PathKind::Normal, 0)
        .unwrap();
    park.world
        .build_path_route(exit.position, exit.position, PathKind::Normal, 0)
        .unwrap();
    park.world
        .build_path_route(queue_entry, queue.position, PathKind::Queue, 0)
        .unwrap();

    for (cars, capacity) in [(4, 16), (8, 32)] {
        park.construction_set_cars_per_train(cars).unwrap();
        let construction = park.construction.as_ref().unwrap();
        let facility =
            coaster_facility(&park.world, construction, 1, 0, park.coaster_ratings).unwrap();
        assert_eq!(facility.capacity, capacity);

        let circuit = construction.circuit().unwrap();
        let mut train = Train::new(
            &circuit,
            &construction.model().train,
            PhysicsConfig::default(),
            100.0,
            10.0,
        );
        train.step(&circuit);
        let frame = train.readings().last().unwrap();
        assert_eq!(frame.len(), usize::from(cars));
        assert!(
            (frame[0].distance - frame.last().unwrap().distance - 3.0 * f64::from(cars - 1)).abs()
                < 1e-9
        );
    }
}
