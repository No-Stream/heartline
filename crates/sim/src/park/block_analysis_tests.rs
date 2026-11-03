use super::super::Park;
use super::{BlockCommandError, block_physics_config};
use crate::facilities::ParkFacilities;
use crate::guests::park::ParkGuests;
use crate::physics::{Circuit, PhysicsConfig, Train};
use crate::track::blocks::{BlockLayout, BlockManualSearch, BlockPlacementSearch, brake_runs};
use crate::track::construction::Construction;
use crate::track::{PieceId, TrackState, TrainConfiguration, hybrid_model};
use crate::world::{Path, Position, Surface, World};

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
