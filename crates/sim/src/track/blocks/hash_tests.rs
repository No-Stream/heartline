use super::{BlockManualSearch, BlockPlacementSearch};
use crate::physics::{PhysicsConfig, demo_circuit};
use crate::state_hash::StateHasher;
use crate::track::hybrid_model;

fn placement_hash(search: &BlockPlacementSearch) -> u64 {
    let mut hash = StateHasher::new();
    search.hash_into(&mut hash);
    hash.finish()
}

fn manual_hash(search: &BlockManualSearch) -> u64 {
    let mut hash = StateHasher::new();
    search.hash_into(&mut hash);
    hash.finish()
}

#[test]
fn placement_search_hash_tracks_progress_and_is_chunk_independent() {
    let circuit = demo_circuit();
    let train = hybrid_model().train;
    let mut first =
        BlockPlacementSearch::new(circuit.clone(), train.clone(), PhysicsConfig::default())
            .unwrap();
    let mut second = BlockPlacementSearch::new(circuit, train, PhysicsConfig::default()).unwrap();
    first.advance(2);
    second.advance(1);
    second.advance(1);
    assert_eq!(placement_hash(&first), placement_hash(&second));
    second.advance(1);
    assert_ne!(placement_hash(&first), placement_hash(&second));
}

#[test]
fn manual_search_hash_tracks_progress_and_is_chunk_independent() {
    let circuit = demo_circuit();
    let train = hybrid_model().train;
    let mut first = BlockManualSearch::new(
        circuit.clone(),
        train.clone(),
        PhysicsConfig::default(),
        Vec::new(),
    )
    .unwrap();
    let mut second =
        BlockManualSearch::new(circuit, train, PhysicsConfig::default(), Vec::new()).unwrap();
    first.advance(2);
    second.advance(1);
    second.advance(1);
    assert_eq!(manual_hash(&first), manual_hash(&second));
    second.advance(1);
    assert_ne!(manual_hash(&first), manual_hash(&second));
}

#[test]
fn placement_search_round_trip_preserves_continuation() {
    let mut original = BlockPlacementSearch::new(
        demo_circuit(),
        hybrid_model().train,
        PhysicsConfig::default(),
    )
    .unwrap();
    for steps in [0, 1, 50, 500] {
        original.advance(steps);
        let encoded = bincode::serialize(&original).unwrap();
        let mut restored: BlockPlacementSearch = bincode::deserialize(&encoded).unwrap();
        assert_eq!(placement_hash(&original), placement_hash(&restored));
        for _ in 0..5 {
            original.advance(1_000);
            restored.advance(1_000);
            assert_eq!(placement_hash(&original), placement_hash(&restored));
        }
    }
}

#[test]
fn manual_search_round_trip_preserves_continuation() {
    let circuit = demo_circuit();
    let train = hybrid_model().train;
    let boundary = super::brake_runs(&circuit, train.length_metres)[0].boundary_metres;
    let mut original =
        BlockManualSearch::new(circuit, train, PhysicsConfig::default(), vec![boundary]).unwrap();
    for steps in [0, 1, 50, 500] {
        original.advance(steps);
        let encoded = bincode::serialize(&original).unwrap();
        let mut restored: BlockManualSearch = bincode::deserialize(&encoded).unwrap();
        assert_eq!(manual_hash(&original), manual_hash(&restored));
        for _ in 0..5 {
            original.advance(1_000);
            restored.advance(1_000);
            assert_eq!(manual_hash(&original), manual_hash(&restored));
        }
    }
}

#[test]
fn binary_search_snapshots_round_trip() {
    let circuit = demo_circuit();
    let restored_circuit: crate::physics::Circuit =
        bincode::deserialize(&bincode::serialize(&circuit).unwrap()).unwrap();
    for (index, (first, second)) in circuit
        .pieces()
        .iter()
        .zip(restored_circuit.pieces())
        .enumerate()
    {
        let mut first_hash = StateHasher::new();
        let mut second_hash = StateHasher::new();
        first.hash_into(&mut first_hash);
        second.hash_into(&mut second_hash);
        assert_eq!(
            first_hash.finish(),
            second_hash.finish(),
            "piece {index} {:?}",
            first.id
        );
    }
    let mut placement = BlockPlacementSearch::new(
        demo_circuit(),
        hybrid_model().train,
        PhysicsConfig::default(),
    )
    .unwrap();
    placement.advance(100);
    let restored: BlockPlacementSearch =
        bincode::deserialize(&bincode::serialize(&placement).unwrap()).unwrap();
    assert_eq!(placement_hash(&placement), placement_hash(&restored));

    let mut manual = BlockManualSearch::new(
        demo_circuit(),
        hybrid_model().train,
        PhysicsConfig::default(),
        Vec::new(),
    )
    .unwrap();
    manual.advance(100);
    let restored: BlockManualSearch =
        bincode::deserialize(&bincode::serialize(&manual).unwrap()).unwrap();
    assert_eq!(manual_hash(&manual), manual_hash(&restored));
}

#[test]
fn search_snapshot_rejects_work_after_completion() {
    let circuit = demo_circuit();
    let train = hybrid_model().train;
    let config = PhysicsConfig::default();
    let mut search =
        BlockManualSearch::new(circuit.clone(), train.clone(), config, vec![]).unwrap();
    while search.result().is_none() {
        search.advance(1_000);
    }
    let invalid = super::ManualSearchSnapshot {
        circuit,
        train,
        config,
        boundaries: vec![],
        work_completed: search.work_completed() + 1,
    };
    assert!(
        bincode::deserialize::<BlockManualSearch>(&bincode::serialize(&invalid).unwrap()).is_err()
    );
}
