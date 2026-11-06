use theme_park_sim::physics::rated_piece::{rate_piece, unpowered};
use theme_park_sim::track::{
    PieceId, catalog, hybrid_model, multi_launch_model, wooden_twister_model,
};

fn main() {
    let mut arguments = std::env::args().skip(1);
    let selected_piece = arguments.next();
    let step: f64 = arguments
        .next()
        .map_or(1.0, |value| value.parse().expect("step in m/s"));
    assert!(
        arguments.next().is_none() && step > 0.0 && step.is_finite(),
        "usage: measure_force_windows [piece-id [step-m/s]]"
    );
    let ids = [
        PieceId::SixtyToSteepDown,
        PieceId::HalfCorkscrewLeftDown,
        PieceId::HalfCorkscrewRightDown,
        PieceId::Loop,
        PieceId::FloaterHill6x64,
    ];
    let mut matched = false;
    for id in ids {
        if selected_piece
            .as_ref()
            .is_some_and(|name| *name != format!("{id:?}"))
        {
            continue;
        }
        matched = true;
        let piece = catalog().iter().find(|piece| piece.id == id).unwrap();
        let rated_speeds = piece.rated_speeds.unwrap();
        let current_maximum = rated_speeds.maximum_entry;
        println!(
            "{id:?}: existing catalog minimum {} m/s",
            rated_speeds.minimum_entry
        );
        let ceiling = 100.0_f64.max(3.0 * current_maximum);
        let mut passing = Vec::new();
        let mut unreachable = Vec::new();
        let mut evaluated = 0;
        let mut unavailable = 0;
        let mut first_failure = None;
        let mut near_current = None;
        for index in 1..=(ceiling / step).ceil() as usize {
            let speed = index as f64 * step;
            let mut passed = true;
            let mut target_unavailable = false;
            let mut diagnostic = None;
            for model in [hybrid_model(), multi_launch_model(), wooden_twister_model()] {
                if !model.allowed_pieces.contains(&id) {
                    continue;
                }
                for cars in model.cars_per_train_range() {
                    match rate_piece(&model, cars, piece, speed, unpowered()) {
                        Ok(rating) => {
                            evaluated += 1;
                            if !rating.passed {
                                passed = false;
                                if diagnostic.is_none() {
                                    diagnostic =
                                        Some(format!("{} {cars} cars: {rating:?}", model.name));
                                }
                            }
                        }
                        Err(error) if error.contains("unreachable") => {
                            unavailable += 1;
                            passed = false;
                            target_unavailable = true;
                            if diagnostic.is_none() {
                                diagnostic = Some(error);
                            }
                        }
                        Err(error) => panic!("{error}"),
                    }
                }
            }
            if passed {
                passing.push(speed);
            }
            if target_unavailable {
                unreachable.push(speed);
            }
            if !passed && index > 1 && first_failure.is_none() {
                first_failure = Some((speed, diagnostic.clone()));
            }
            if near_current
                .as_ref()
                .is_none_or(|(previous, _): &(f64, Option<String>)| {
                    (speed - current_maximum).abs() < (*previous - current_maximum).abs()
                })
            {
                near_current = Some((speed, diagnostic));
            }
        }
        let gaps = passing
            .windows(2)
            .filter(|pair| pair[1] - pair[0] > step * 1.000001)
            .collect::<Vec<_>>();
        println!(
            "{id:?}: scanned {step}..{:.2} m/s, passing {passing:?}; unreachable {unreachable:?}; evaluated {evaluated}, unreachable configurations {unavailable}; gaps {gaps:?}",
            (ceiling / step).ceil() * step
        );
        println!(
            "  first failure above lowest: {first_failure:?}; nearest old maximum {current_maximum:.2}: {near_current:?}"
        );
        assert!(
            !passing.is_empty() && gaps.is_empty(),
            "{id:?}: no contiguous passing window"
        );
    }
    assert!(matched, "unknown piece");
}
