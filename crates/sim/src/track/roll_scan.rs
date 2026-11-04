use crate::physics::{Circuit, PhysicsConfig, Train};
use crate::track::roll::RollGeometry;
use crate::track::{
    CoasterModel, Piece, PieceId, Shape, TrackState, hybrid_model, multi_launch_model,
    wooden_twister_model,
};
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashSet};
use std::f64::consts::{PI, TAU};
use std::sync::{Arc, OnceLock};

type ConnectorKey = (i32, i32, i32, u8, u8, u8, u8, u8);

fn connector_key(state: TrackState) -> ConnectorKey {
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

fn circuit_for_scan(templates: &[Piece], elements: &[PieceId]) -> Circuit {
    let template = |id| templates.iter().find(|piece| piece.id == id).unwrap();
    let mut pieces = Vec::new();
    let mut state = TrackState::origin();
    for id in [PieceId::Station, PieceId::Straight, PieceId::Straight]
        .into_iter()
        .chain(elements.iter().copied())
        .chain([PieceId::Straight, PieceId::Straight])
    {
        let piece = templates
            .iter()
            .filter(|piece| piece.id == id)
            .find_map(|piece| piece.at(state))
            .unwrap();
        state = piece.exit;
        pieces.push(piece);
    }
    let mut nodes = vec![(state, Vec::<PieceId>::new())];
    let mut queue = BinaryHeap::from([Reverse((0_usize, 0_usize))]);
    let mut visited = HashSet::from([connector_key(state)]);
    let path = loop {
        let Reverse((_, index)) = queue.pop().expect("no return path");
        let (state, path) = nodes[index].clone();
        if state == TrackState::origin() {
            break path;
        }
        if path.len() >= 60 {
            continue;
        }
        for id in [
            PieceId::Straight,
            PieceId::HalfTileStraight,
            PieceId::OneTileStraight,
            PieceId::TurnSmallStandalone,
            PieceId::TurnSmallRightStandalone,
            PieceId::TurnLargeStandalone,
            PieceId::TurnLargeRightStandalone,
            PieceId::OffsetLeft,
            PieceId::OffsetRight,
        ] {
            let next = template(id).exit_at(state).unwrap();
            if next.x.abs() > 300 || next.y.abs() > 300 || !visited.insert(connector_key(next)) {
                continue;
            }
            let mut next_path = path.clone();
            next_path.push(id);
            let priority =
                (next.x.unsigned_abs() + next.y.unsigned_abs()) as usize * 100 + next_path.len();
            nodes.push((next, next_path));
            queue.push(Reverse((priority, nodes.len() - 1)));
        }
    };
    for id in path {
        let piece = template(id).at(state).unwrap();
        state = piece.exit;
        pieces.push(piece);
    }
    Circuit::new(pieces).unwrap()
}

pub(super) fn corkscrew_spans(templates: &[Piece]) -> (i32, i32) {
    static SPANS: OnceLock<(i32, i32)> = OnceLock::new();
    *SPANS.get_or_init(|| (solve(templates, false), solve(templates, true)))
}

fn candidate_templates(templates: &[Piece], span: i32, half: bool) -> Vec<Piece> {
    let mut candidates = templates.to_vec();
    let geometry = Arc::new(RollGeometry::helix(
        f64::from(span),
        if half { PI } else { TAU },
        if half { 4.8 } else { 5.0 },
    ));
    for piece in &mut candidates {
        let matches = if half {
            matches!(
                piece.id,
                PieceId::HalfCorkscrewLeftUp
                    | PieceId::HalfCorkscrewRightUp
                    | PieceId::HalfCorkscrewLeftDown
                    | PieceId::HalfCorkscrewRightDown
            )
        } else {
            matches!(piece.id, PieceId::Corkscrew | PieceId::CorkscrewRight)
        };
        if matches {
            piece.exit.x = span / 2;
            piece.shape = Shape::Roll(Arc::clone(&geometry));
            piece.tangent_scale = (piece.exit.position() - piece.entry.position()).length();
            piece.rebuild_arc();
        }
    }
    candidates
}

fn solve(templates: &[Piece], half: bool) -> i32 {
    for span in (20..=200).step_by(2) {
        let candidates = candidate_templates(templates, span, half);
        let critical = if half {
            PieceId::HalfCorkscrewLeftDown
        } else {
            PieceId::Corkscrew
        };
        let elements = if half {
            vec![
                PieceId::HalfLoopUp,
                critical,
                PieceId::HalfCorkscrewLeftUp,
                PieceId::HalfLoopDown,
            ]
        } else {
            vec![critical]
        };
        let circuit = circuit_for_scan(&candidates, &elements);
        let model = hybrid_model();
        let critical_speed = candidates
            .iter()
            .find(|piece| piece.id == critical)
            .unwrap()
            .rated_speeds
            .unwrap()
            .maximum_entry;
        if !passes(&circuit, critical, &model, 6, critical_speed) {
            continue;
        }
        let ids: &[PieceId] = if half {
            &[
                PieceId::HalfCorkscrewLeftUp,
                PieceId::HalfCorkscrewRightUp,
                PieceId::HalfCorkscrewLeftDown,
                PieceId::HalfCorkscrewRightDown,
            ]
        } else {
            &[PieceId::Corkscrew, PieceId::CorkscrewRight]
        };
        let mut all_pass = true;
        for &id in ids {
            let elements = if half {
                let (down, up) = if matches!(
                    id,
                    PieceId::HalfCorkscrewRightUp | PieceId::HalfCorkscrewRightDown
                ) {
                    (
                        PieceId::HalfCorkscrewRightDown,
                        PieceId::HalfCorkscrewRightUp,
                    )
                } else {
                    (PieceId::HalfCorkscrewLeftDown, PieceId::HalfCorkscrewLeftUp)
                };
                vec![PieceId::HalfLoopUp, down, up, PieceId::HalfLoopDown]
            } else {
                vec![id]
            };
            let circuit = circuit_for_scan(&candidates, &elements);
            let piece = candidates.iter().find(|piece| piece.id == id).unwrap();
            let rated = piece.rated_speeds.unwrap();
            for model in [wooden_twister_model(), hybrid_model(), multi_launch_model()] {
                if model.allows_piece(piece) {
                    for cars in [6, 12] {
                        for speed in [rated.minimum_entry, rated.maximum_entry] {
                            all_pass &= passes(&circuit, id, &model, cars, speed);
                        }
                    }
                }
            }
        }
        if all_pass {
            return span;
        }
    }
    panic!("no passing corkscrew span");
}

fn passes(circuit: &Circuit, target: PieceId, model: &CoasterModel, cars: u8, speed: f64) -> bool {
    let mut start = 0.0;
    let piece = circuit
        .pieces()
        .iter()
        .find(|piece| {
            if piece.id == target {
                true
            } else {
                start += piece.length();
                false
            }
        })
        .unwrap();
    let end = start + piece.length();
    let mut config = model.train.clone();
    config.cars = cars;
    let physics = PhysicsConfig {
        rolling_resistance: 0.0,
        drag_coefficient: 0.0,
        powered_sections: false,
        ..PhysicsConfig::default()
    };
    let mut train = Train::new(circuit, &config, physics, start, speed);
    let mut seen = vec![false; usize::from(cars)];
    let limits = model.comfortable_force_limits;
    for _ in 0..100_000 {
        train.step(circuit);
        for frame in train.take_readings() {
            for (index, reading) in frame.into_iter().enumerate() {
                if (start..end).contains(&reading.distance) {
                    seen[index] = true;
                    if !reading.speed.is_finite()
                        || (matches!(
                            target,
                            PieceId::HalfCorkscrewLeftUp | PieceId::HalfCorkscrewRightUp
                        ) && speed == piece.rated_speeds.unwrap().minimum_entry
                            && reading.speed < 5.0)
                        || !reading.g.vertical.is_finite()
                        || !reading.g.lateral.is_finite()
                        || !reading.g.longitudinal.is_finite()
                        || reading.g.vertical < limits.minimum_vertical_g
                        || reading.g.vertical > limits.maximum_vertical_g
                        || reading.g.lateral.abs() > limits.maximum_lateral_g
                        || reading.g.longitudinal.abs() > limits.maximum_longitudinal_g
                    {
                        return false;
                    }
                }
            }
        }
        if !train.notes().is_empty() {
            return false;
        }
        if train.distance() >= end + config.length_metres {
            return seen.iter().all(|seen| *seen);
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use crate::track::{PieceId, catalog};

    #[test]
    fn solved_corkscrew_spans_match_full_train_scan() {
        let pieces = catalog();
        let (full_span, half_span) = super::corkscrew_spans(&pieces);
        assert_eq!(full_span, 86);
        assert_eq!(half_span, 64);

        for (id, expected_span) in [
            (PieceId::Corkscrew, 86),
            (PieceId::HalfCorkscrewLeftDown, 64),
        ] {
            let piece = pieces.iter().find(|piece| piece.id == id).unwrap();
            assert_eq!(piece.exit.x * 2, expected_span);
        }
    }
}
