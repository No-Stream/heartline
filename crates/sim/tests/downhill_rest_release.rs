use theme_park_sim::physics::{Circuit, PhysicsConfig, Train};
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
    state = target.exit;
    pieces.push(target);
    let exit = piece_template(PieceId::Straight)
        .at(state)
        .expect("level exit straight connects");
    assert!(model.allows_piece(&exit));
    pieces.push(exit);

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
    let entry_speed = loop {
        let previous_distance = train.distance();
        let previous_speed = train.speed();
        train.step(&circuit);
        if train.distance() >= target_start {
            let fraction =
                (target_start - previous_distance) / (train.distance() - previous_distance);
            break previous_speed + fraction * (train.speed() - previous_speed);
        }
        assert!(
            train.speed() > 0.0,
            "resting hybrid train must roll downhill"
        );
    };

    assert!(entry_speed > 0.0);
    assert!(
        entry_speed < 12.0,
        "halfway downhill rest release entered at {entry_speed:.6} m/s"
    );
}
