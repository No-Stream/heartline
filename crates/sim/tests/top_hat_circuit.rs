use theme_park_sim::physics::{CoasterRun, PhysicsConfig};
use theme_park_sim::track::construction::Construction;
use theme_park_sim::track::{PieceId, TrackState, wooden_twister_model};
use theme_park_sim::world::{Surface, World};

#[test]
fn wooden_twister_completes_a_constructed_top_hat_circuit_with_resistance() {
    use PieceId::*;
    let world = World::new(
        512,
        512,
        Surface {
            corners: [0; 4],
            water: None,
        },
    );
    let model = wooden_twister_model();
    let start = TrackState {
        x: 256,
        y: 256,
        z: 5,
        ..TrackState::origin()
    };
    let mut construction = Construction::new(start, model.clone());
    let mut ids = vec![Station];
    ids.push(LevelToGentleUp);
    ids.extend([GentleUp; 8]);
    ids.extend([GentleUpToLevel, Straight, Straight, LevelToGentleDown]);
    ids.extend([GentleDown; 8]);
    ids.extend([GentleDownToLevel, TopHat]);
    ids.extend([Straight; 4]);
    ids.extend([TurnSmallStandalone; 2]);
    ids.extend([Straight; 35]);
    ids.extend([TurnSmallStandalone; 2]);
    for (index, id) in ids.into_iter().enumerate() {
        construction.set_chain((1..=12).contains(&index));
        construction
            .append(id, &world, &[])
            .unwrap_or_else(|error| panic!("{id:?}: {error:?}"));
    }
    assert_eq!(construction.end(), start);
    construction.close().unwrap();
    let circuit = construction.circuit().unwrap();
    let top_hat_start: f64 = circuit
        .pieces()
        .iter()
        .take_while(|piece| piece.id != TopHat)
        .map(|piece| piece.length())
        .sum();
    let top_hat_end = top_hat_start
        + circuit
            .pieces()
            .iter()
            .find(|piece| piece.id == TopHat)
            .unwrap()
            .length();
    let mut run = CoasterRun::new(&circuit, &model.train, PhysicsConfig::default());
    let outcome = (0..200_000)
        .find_map(|_| run.step(&circuit))
        .expect("ride test did not finish");
    let report = &outcome.reports[0];
    assert!(
        report.completed,
        "distance {}, top hat {top_hat_start}..{top_hat_end}, notes {:?}",
        report.furthest_distance, report.notes
    );
    assert!(!outcome.recovered);
    assert!(
        report
            .samples
            .iter()
            .any(|sample| sample.distance >= top_hat_start && sample.distance < top_hat_end)
    );
    assert!(
        report
            .samples
            .iter()
            .any(|sample| sample.distance >= top_hat_end)
    );
}
