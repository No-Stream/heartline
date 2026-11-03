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
