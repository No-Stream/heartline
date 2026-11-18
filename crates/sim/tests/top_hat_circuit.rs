use theme_park_sim::physics::{CoasterRun, PhysicsConfig};
use theme_park_sim::track::construction::Construction;
use theme_park_sim::track::{PieceId, TrackState, wooden_twister_model};
use theme_park_sim::world::{Surface, World};

const RETURN_PIECES: [PieceId; 9] = [
    PieceId::Straight,
    PieceId::HalfTileStraight,
    PieceId::OneTileStraight,
    PieceId::TurnSmallStandalone,
    PieceId::TurnSmallRightStandalone,
    PieceId::TurnLargeStandalone,
    PieceId::TurnLargeRightStandalone,
    PieceId::OffsetLeft,
    PieceId::OffsetRight,
];

fn state_key(state: TrackState) -> (i32, i32, i32, u8, u8, u8, u8, u8) {
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

fn return_path(start: TrackState, model: &theme_park_sim::track::CoasterModel) -> Vec<PieceId> {
    let templates = theme_park_sim::track::catalog();
    let mut queue = VecDeque::from([(start, Vec::new())]);
    let mut visited = HashSet::from([state_key(start)]);
    while let Some((state, path)) = queue.pop_front() {
        if state == TrackState::origin() {
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
    panic!("no bounded return path from {start:?}");
}

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
    for (index, id) in ids.into_iter().enumerate() {
        construction.set_chain((1..=12).contains(&index));
        construction
            .append(id, &world, &[])
            .unwrap_or_else(|error| panic!("{id:?}: {error:?}"));
    }
    let end = construction.end();
    let return_start = TrackState {
        x: end.x - start.x,
        y: end.y - start.y,
        z: end.z - start.z,
        ..end
    };
    for id in return_path(return_start, &model) {
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
        .find_map(|_| run.step(&circuit, model.name))
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
use std::collections::{HashSet, VecDeque};
