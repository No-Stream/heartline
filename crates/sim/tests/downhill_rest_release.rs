use theme_park_sim::physics::{Circuit, ForceEvaluator, PhysicsConfig, Train, TIME_STEP};
use theme_park_sim::track::catalog::unmeasured_catalog;
use theme_park_sim::track::{hybrid_model, Piece, PieceId};

fn piece_template(id: PieceId) -> Piece {
    unmeasured_catalog()
        .iter()
        .find(|piece| piece.id == id)
        .unwrap_or_else(|| panic!("missing {id:?} template"))
        .clone()
}

#[test]
fn downhill_rest_release_halfway_along_sixty_down_enters_below_twelve_mps() {
    let model = hybrid_model();
    let station = piece_template(PieceId::Station);
    assert!(model.allows_piece(&station));
    let mut state = station.exit;
    let mut pieces = vec![station];

    let transition = piece_template(PieceId::LevelToSixtyDown)
        .at(state)
        .expect("LevelToSixtyDown connects after Station");
    assert!(model.allows_piece(&transition));
    state = transition.exit;
    pieces.push(transition);

    let downhill_straight = piece_template(PieceId::SixtyDown);
    let downhill_straight_length = downhill_straight.length();
    let required_downhill_length = model.train.length_metres + downhill_straight_length / 2.0;
    let mut approach_length = 0.0;
    while approach_length < required_downhill_length {
        let next = downhill_straight
            .at(state)
            .expect("SixtyDown straight connects after the preceding track");
        assert!(model.allows_piece(&next));
        approach_length += next.length();
        state = next.exit;
        pieces.push(next);
    }

    let target = piece_template(PieceId::SixtyDownToLevel)
        .at(state)
        .expect("SixtyDownToLevel connects after SixtyDown");
    assert!(model.allows_piece(&target));
    let target_start = pieces.iter().map(Piece::length).sum::<f64>();
    let target_end = target_start + target.length();
    state = target.exit;
    pieces.push(target);
    let exit_straight = piece_template(PieceId::Straight);
    while pieces.iter().map(Piece::length).sum::<f64>()
        < target_end + model.train.length_metres + 10.0
    {
        let exit = exit_straight
            .at(state)
            .expect("level exit straight connects");
        assert!(model.allows_piece(&exit));
        state = exit.exit;
        pieces.push(exit);
    }

    let circuit = Circuit::open(pieces).expect("downhill release circuit is connected");
    let start_distance = target_start - downhill_straight_length / 2.0;
    let car_spacing = model.train.length_metres / f64::from(model.train.cars);
    for car in 0..usize::from(model.train.cars) {
        let car_distance = start_distance - car as f64 * car_spacing;
        let piece_index = circuit.piece_index(car_distance);
        assert_eq!(
            circuit.pieces()[piece_index].id,
            PieceId::SixtyDown,
            "car {car} must start supported on a SixtyDown straight"
        );
    }

    // This is a rest release halfway down the final SixtyDown straight, not a slow-crest route.
    let unpowered_physics = PhysicsConfig {
        rolling_resistance: 0.0,
        drag_coefficient: 0.0,
        powered_sections: false,
        ..PhysicsConfig::default()
    };
    let mut train = Train::new(
        &circuit,
        &model.train,
        unpowered_physics,
        start_distance,
        0.0,
    );
    let cars = usize::from(model.train.cars);
    let mut evaluators: Vec<_> = (0..cars)
        .map(|_| ForceEvaluator::new(model.comfortable_force_limits, TIME_STEP))
        .collect();
    let mut force_passes = vec![true; cars];
    let mut samples_in_element = vec![0; cars];
    let mut cleared_element = vec![false; cars];
    let mut previous_lead = (train.distance(), train.speed());
    let mut entry_speed = None;
    for _ in 0..20_000 {
        train.step(&circuit);
        if entry_speed.is_none() && train.distance() >= target_start {
            let fraction = (target_start - previous_lead.0) / (train.distance() - previous_lead.0);
            entry_speed = Some(previous_lead.1 + fraction * (train.speed() - previous_lead.1));
        }
        assert!(
            train.speed() > 0.0,
            "resting hybrid train must roll downhill"
        );
        let frames = train.take_readings();
        assert_eq!(frames.len(), 1);
        for (car, reading) in frames[0].iter().enumerate() {
            let evaluated = evaluators[car].update(reading.g);
            if (target_start..target_end).contains(&reading.distance) {
                samples_in_element[car] += 1;
                force_passes[car] &= evaluated.within_limits;
            }
            if reading.distance >= target_end {
                cleared_element[car] = true;
            }
        }
        previous_lead = (train.distance(), train.speed());
        if cleared_element.iter().all(|cleared| *cleared) {
            break;
        }
    }

    let entry_speed = entry_speed.expect("lead train reaches the target element");
    assert!(entry_speed > 0.0);
    assert!(
        entry_speed < 12.0,
        "halfway downhill rest release entered at {entry_speed:.6} m/s"
    );
    assert!(
        samples_in_element.iter().all(|samples| *samples > 0),
        "every car must be evaluated inside the target element: {samples_in_element:?}"
    );
    assert!(
        force_passes.iter().all(|passed| *passed),
        "every car must pass the duration-aware force evaluator inside the target element: {force_passes:?}"
    );
    assert!(
        cleared_element.iter().all(|cleared| *cleared),
        "every car must clear the target element: {cleared_element:?}"
    );
}
