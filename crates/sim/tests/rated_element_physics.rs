use std::collections::{HashSet, VecDeque};

use theme_park_sim::physics::rated_piece::{
    connected_measurement_track, measurement_contexts, rate_piece, rate_sequence,
    solve_approach_speed, unpowered as rating_physics,
};
use theme_park_sim::physics::{Circuit, ForceEvaluator, PhysicsConfig, TIME_STEP, Train};
use theme_park_sim::track::rated_windows::measured_rated_window;
use theme_park_sim::track::{
    CoasterModel, ForceRating, ForceRatingAxis, HillFeel, Piece, PieceFamily, PieceId, RatedSpeeds,
    TrackState, catalog, coaster_models, hybrid_model, multi_launch_model, rated_speeds,
    wooden_twister_model,
};

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

fn connector_key(state: TrackState) -> (i32, i32, i32, u8, u8, u8, u8, u8) {
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

fn next_connector(piece: &Piece, state: TrackState) -> TrackState {
    assert_eq!(state.slope, piece.entry.slope);
    assert_eq!(state.bank, piece.entry.bank);
    assert_eq!(state.horizontal_curvature, piece.entry.horizontal_curvature);
    assert_eq!(state.vertical_curvature, piece.entry.vertical_curvature);

    let rotation = (state.heading as i32 - piece.entry.heading as i32).rem_euclid(8);
    let delta_x = piece.exit.x - piece.entry.x;
    let delta_y = piece.exit.y - piece.entry.y;
    let (rotated_x, rotated_y) = match rotation {
        0 => (delta_x, delta_y),
        1 => (delta_x - delta_y, delta_x + delta_y),
        2 => (-delta_y, delta_x),
        3 => (-delta_x - delta_y, delta_x - delta_y),
        4 => (-delta_x, -delta_y),
        5 => (delta_y - delta_x, -delta_x - delta_y),
        6 => (delta_y, -delta_x),
        7 => (delta_x + delta_y, delta_y - delta_x),
        _ => unreachable!(),
    };
    TrackState {
        x: state.x + rotated_x,
        y: state.y + rotated_y,
        z: state.z
            + if rotation % 2 == 0 {
                piece.exit.z - piece.entry.z
            } else {
                (f64::from(piece.exit.z - piece.entry.z) * std::f64::consts::SQRT_2).round() as i32
            },
        heading: match (piece.exit.heading as i32 + rotation).rem_euclid(8) {
            0 => theme_park_sim::track::Heading::East,
            1 => theme_park_sim::track::Heading::NorthEast,
            2 => theme_park_sim::track::Heading::North,
            3 => theme_park_sim::track::Heading::NorthWest,
            4 => theme_park_sim::track::Heading::West,
            5 => theme_park_sim::track::Heading::SouthWest,
            6 => theme_park_sim::track::Heading::South,
            7 => theme_park_sim::track::Heading::SouthEast,
            _ => unreachable!(),
        },
        slope: piece.exit.slope,
        bank: piece.exit.bank,
        horizontal_curvature: piece.exit.horizontal_curvature,
        vertical_curvature: piece.exit.vertical_curvature,
    }
}

fn level_return_path(start: TrackState, templates: &[Piece]) -> Vec<PieceId> {
    let margin = 64;
    let x_bounds = start.x.min(0) - margin..=start.x.max(0) + margin;
    let y_bounds = start.y.min(0) - margin..=start.y.max(0) + margin;
    let mut queue = VecDeque::from([(start, Vec::new())]);
    let mut visited = HashSet::from([connector_key(start)]);
    while let Some((state, path)) = queue.pop_front() {
        if state == TrackState::origin() {
            return path;
        }
        if path.len() == 48 {
            continue;
        }
        for id in RETURN_PIECES {
            let template = templates.iter().find(|piece| piece.id == id).unwrap();
            if state.slope != template.entry.slope
                || state.bank != template.entry.bank
                || state.horizontal_curvature != template.entry.horizontal_curvature
                || state.vertical_curvature != template.entry.vertical_curvature
            {
                continue;
            }
            let next = next_connector(template, state);
            if !x_bounds.contains(&next.x) || !y_bounds.contains(&next.y) || next.z != 0 {
                continue;
            }
            if !visited.insert(connector_key(next)) {
                continue;
            }
            let mut next_path = path.clone();
            next_path.push(id);
            queue.push_back((next, next_path));
        }
    }
    panic!("no bounded level return path from {start:?}");
}

fn circuit_for_level_piece(piece_id: PieceId) -> (Circuit, f64, f64) {
    let templates = catalog();
    let template = |id| templates.iter().find(|piece| piece.id == id).unwrap();
    let element_template = template(piece_id);
    assert_eq!(
        element_template.entry,
        TrackState::origin(),
        "{piece_id:?} entry"
    );
    assert_eq!(
        element_template.exit.slope,
        theme_park_sim::track::Slope::Level
    );
    assert_eq!(
        element_template.exit.bank,
        theme_park_sim::track::Bank::Level
    );

    let mut state = TrackState::origin();
    let mut pieces = Vec::new();
    for id in [PieceId::Station, PieceId::Straight, PieceId::Straight] {
        let piece = template(id).at(state).unwrap();
        state = piece.exit;
        pieces.push(piece);
    }
    let element_start = pieces.iter().map(Piece::length).sum();
    let element = element_template.at(state).unwrap();
    state = element.exit;
    pieces.push(element);
    let element_end = pieces.iter().map(Piece::length).sum();

    let mut return_path = level_return_path(state, templates);
    for id in return_path.drain(..) {
        let piece = template(id).at(state).unwrap();
        state = piece.exit;
        pieces.push(piece);
    }
    assert_eq!(state, TrackState::origin());
    (Circuit::new(pieces).unwrap(), element_start, element_end)
}

#[test]
fn rolled_inversion_minimum_speeds_clear_with_energy_margin() {
    use PieceId::*;
    for id in [
        ZeroGRollFastLeft,
        ZeroGRollFastRight,
        ZeroGRollCompactLeft,
        ZeroGRollCompactRight,
        StallLeft,
        StallRight,
    ] {
        let piece = catalog().iter().find(|piece| piece.id == id).unwrap();
        let (circuit, start, end) = circuit_for_level_piece(id);
        for model in [hybrid_model(), multi_launch_model()] {
            let cars = model.train.cars;
            let rated = rated_window(&model, piece);
            assert_train_crosses_element(
                &model,
                &circuit,
                id,
                start..end,
                cars,
                rated.minimum_entry,
                true,
            );
        }
    }
}

#[test]
fn level_elements_clear_from_their_minimum_after_a_steep_crest() {
    use PieceId::*;
    for piece in catalog().iter().filter(|piece| {
        piece.design_speeds.is_some()
            && piece.entry == TrackState::origin()
            && piece.exit.slope == theme_park_sim::track::Slope::Level
            && piece.exit.bank == theme_park_sim::track::Bank::Level
    }) {
        let (circuit, ranges) = circuit_for_sequence(&[LevelToSixtyUp, SixtyUpToLevel, piece.id]);
        let (_, start, end) = ranges[2];
        for model in models()
            .into_iter()
            .filter(|model| model.allows_piece(piece))
        {
            let cars = model.train.cars;
            let rated = rated_window(&model, piece);
            assert_train_crosses_element(
                &model,
                &circuit,
                piece.id,
                start..end,
                cars,
                rated.minimum_entry,
                false,
            );
        }
    }
}

#[test]
fn half_inversion_windows_hold_after_preceding_inversion() {
    use PieceId::*;
    for id in [
        HalfCorkscrewLeftDown,
        HalfCorkscrewRightDown,
        RavenRollLeftDown,
        RavenRollRightDown,
        HalfLoopDown,
    ] {
        let piece = catalog().iter().find(|piece| piece.id == id).unwrap();
        for model in [hybrid_model(), multi_launch_model()] {
            if !model.allows_piece(piece) {
                continue;
            }
            let cars = model.train.cars;
            let rated = rated_window(&model, piece);
            for speed in [rated.minimum_entry, rated.maximum_entry] {
                assert_shared_rating_passes_every_applicable_context(&model, piece, cars, speed);
            }
        }
    }
}

#[test]
fn sixty_degree_windows_hold_with_connected_approaches() {
    use PieceId::*;
    let transition_ids = [
        LevelToSixtyUp,
        SixtyUpToLevel,
        LevelToSixtyDown,
        SixtyDownToLevel,
        SteepToSixtyUp,
        SixtyToSteepUp,
        SteepToSixtyDown,
        SixtyToSteepDown,
    ];
    for id in transition_ids {
        let piece = catalog()
            .iter()
            .find(|piece| piece.id == id && piece.design_speeds.is_some())
            .unwrap();
        for model in models()
            .into_iter()
            .filter(|model| model.allows_piece(piece))
        {
            let rated = rated_window(&model, piece);
            let measured_from = measured_rated_window(&model, piece)
                .expect("piece must have a measured window")
                .measured_from;
            let cars = model.train.cars;
            for speed in [rated.minimum_entry.max(measured_from), rated.maximum_entry] {
                assert_shared_rating_passes_every_applicable_context(&model, piece, cars, speed);
            }
        }
    }
}

fn circuit_for_sequence(elements: &[PieceId]) -> (Circuit, Vec<(PieceId, f64, f64)>) {
    circuit_for_sequence_with_initial_runway(elements, 0)
}

fn circuit_for_sequence_with_initial_runway(
    elements: &[PieceId],
    additional_straights: usize,
) -> (Circuit, Vec<(PieceId, f64, f64)>) {
    let templates = catalog();
    let mut state = TrackState::origin();
    let mut pieces = Vec::new();
    for id in [PieceId::Station, PieceId::Straight, PieceId::Straight] {
        let piece = templates
            .iter()
            .find(|piece| piece.id == id)
            .unwrap()
            .at(state)
            .unwrap();
        state = piece.exit;
        pieces.push(piece);
    }
    for _ in 0..additional_straights {
        let straight = templates
            .iter()
            .find(|piece| piece.id == PieceId::Straight)
            .unwrap()
            .at(state)
            .unwrap();
        state = straight.exit;
        pieces.push(straight);
    }

    let mut element_ranges = Vec::with_capacity(elements.len());
    for &id in elements {
        let element_start = pieces.iter().map(Piece::length).sum();
        let template = templates
            .iter()
            .find(|piece| {
                piece.id == id
                    && piece.entry.slope == state.slope
                    && piece.entry.bank == state.bank
                    && piece.entry.horizontal_curvature == state.horizontal_curvature
                    && piece.entry.vertical_curvature == state.vertical_curvature
            })
            .unwrap_or_else(|| panic!("no {id:?} template joins from {state:?}"));
        let element = template.at(state).unwrap();
        state = element.exit;
        pieces.push(element);
        element_ranges.push((id, element_start, pieces.iter().map(Piece::length).sum()));
    }
    while state.z != 0 {
        let id = if state.z > 0 {
            PieceId::ShortFall
        } else {
            PieceId::ShortRise
        };
        let piece = templates
            .iter()
            .find(|piece| piece.id == id)
            .unwrap()
            .at(state)
            .unwrap();
        state = piece.exit;
        pieces.push(piece);
    }
    assert_eq!(
        (state.z, state.slope, state.bank),
        (
            0,
            theme_park_sim::track::Slope::Level,
            theme_park_sim::track::Bank::Level
        ),
        "sequence does not return to level height and bank: {elements:?}"
    );

    let return_path = level_return_path(state, templates);
    for id in return_path {
        let template = templates.iter().find(|piece| piece.id == id).unwrap();
        let piece = template.at(state).unwrap();
        state = piece.exit;
        pieces.push(piece);
    }
    assert_eq!(state, TrackState::origin());
    (Circuit::new(pieces).unwrap(), element_ranges)
}

fn unpowered() -> PhysicsConfig {
    PhysicsConfig {
        rolling_resistance: 0.0,
        drag_coefficient: 0.0,
        powered_sections: false,
        ..PhysicsConfig::default()
    }
}

#[derive(Clone, Copy, Debug)]
struct ElementForces {
    minimum_vertical: f64,
    maximum_vertical: f64,
    maximum_lateral: f64,
    maximum_longitudinal: f64,
    peak_lateral_distance: f64,
    peak_lateral_speed: f64,
}

#[derive(Clone, Debug)]
struct ElementScan {
    forces: ElementForces,
    all_cars_seen: bool,
    train_cleared: bool,
    stalled: bool,
    rolled_back: bool,
    minimum_speed: f64,
    /// The first evaluator violation inside the element whose excursion began there.
    violation: Option<String>,
}

fn scan_train_element(
    model: &CoasterModel,
    circuit: &Circuit,
    element_range: std::ops::Range<f64>,
    cars: u8,
    entry_speed: f64,
) -> ElementScan {
    scan_train_element_from_start(
        model,
        circuit,
        element_range.clone(),
        cars,
        element_range.start,
        entry_speed,
    )
}

fn scan_train_element_after_preroll(
    model: &CoasterModel,
    circuit: &Circuit,
    element_range: std::ops::Range<f64>,
    cars: u8,
    entry_speed: f64,
) -> Result<ElementScan, String> {
    let mut train_config = model.train.clone();
    train_config.length_metres *= f64::from(cars) / f64::from(train_config.cars);
    train_config.cars = cars;
    let approach_start = element_range.start - 10.0_f64.max(0.5 * entry_speed);
    assert!(
        approach_start >= train_config.length_metres,
        "pre-roll does not leave the full train on track before the element"
    );
    let initial_speed = solve_approach_speed(
        circuit,
        &train_config,
        unpowered(),
        approach_start,
        element_range.start,
        entry_speed,
    )?;
    Ok(scan_train_element_from_start(
        model,
        circuit,
        element_range,
        cars,
        approach_start,
        initial_speed,
    ))
}

fn scan_train_element_from_start(
    model: &CoasterModel,
    circuit: &Circuit,
    element_range: std::ops::Range<f64>,
    cars: u8,
    start_distance: f64,
    start_speed: f64,
) -> ElementScan {
    let mut train_config = model.train.clone();
    train_config.length_metres *= f64::from(cars) / f64::from(train_config.cars);
    train_config.cars = cars;
    let mut train = Train::new(
        circuit,
        &train_config,
        unpowered(),
        start_distance,
        start_speed,
    );
    let mut evaluators =
        vec![ForceEvaluator::new(model.comfortable_force_limits, TIME_STEP); usize::from(cars)];
    // An excursion that began on an earlier piece belongs to that piece, as in the isolated rating.
    let mut excursion_began_before_element = vec![[false; 3]; usize::from(cars)];
    let mut cars_seen = vec![false; usize::from(cars)];
    let mut minimum_vertical = f64::INFINITY;
    let mut maximum_vertical = f64::NEG_INFINITY;
    let mut maximum_lateral: f64 = 0.0;
    let mut maximum_longitudinal: f64 = 0.0;
    let mut peak_lateral_distance = element_range.start;
    let mut peak_lateral_speed = start_speed;
    let mut minimum_speed = f64::INFINITY;
    let mut stalled = false;
    let mut violation = None;
    for _ in 0..100_000 {
        train.step(circuit);
        for frame in train.take_readings() {
            for (index, reading) in frame.into_iter().enumerate() {
                let evaluated = evaluators[index].update(reading.g);
                for (began_before, exceeding) in excursion_began_before_element[index]
                    .iter_mut()
                    .zip(evaluated.axis_exceeding_sustained)
                {
                    if !exceeding {
                        *began_before = false;
                    } else if reading.distance < element_range.start {
                        *began_before = true;
                    }
                }
                if element_range.contains(&reading.distance) {
                    cars_seen[index] = true;
                    minimum_vertical = minimum_vertical.min(reading.g.vertical);
                    maximum_vertical = maximum_vertical.max(reading.g.vertical);
                    if reading.g.lateral.abs() > maximum_lateral {
                        maximum_lateral = reading.g.lateral.abs();
                        peak_lateral_distance = reading.distance;
                        peak_lateral_speed = reading.speed;
                    }
                    maximum_longitudinal = maximum_longitudinal.max(reading.g.longitudinal.abs());
                    minimum_speed = minimum_speed.min(reading.speed);
                    let element_violation = (0..3).any(|axis| {
                        !evaluated.axis_within_limits[axis]
                            && !excursion_began_before_element[index][axis]
                    });
                    if element_violation && violation.is_none() {
                        violation = Some(format!(
                            "car {index} at {:.2} m, filtered {:?}",
                            reading.distance, evaluated.filtered
                        ));
                    }
                }
            }
        }
        if train.distance() >= element_range.end + train_config.length_metres {
            break;
        }
        if train.speed() <= 0.0 {
            stalled = true;
            break;
        }
        if !train.notes().is_empty() {
            break;
        }
    }

    ElementScan {
        forces: ElementForces {
            minimum_vertical,
            maximum_vertical,
            maximum_lateral,
            maximum_longitudinal,
            peak_lateral_distance,
            peak_lateral_speed,
        },
        all_cars_seen: cars_seen.into_iter().all(|seen| seen),
        train_cleared: train.distance() >= element_range.end + train_config.length_metres,
        stalled,
        rolled_back: !train.notes().is_empty(),
        minimum_speed,
        violation,
    }
}

fn assert_train_crosses_element(
    model: &CoasterModel,
    circuit: &Circuit,
    piece_id: PieceId,
    element_range: std::ops::Range<f64>,
    cars: u8,
    entry_speed: f64,
    minimum_speed_required: bool,
) -> ElementForces {
    let scan = scan_train_element(model, circuit, element_range, cars, entry_speed);
    assert_train_scan_result(
        model,
        piece_id,
        "supplied test circuit",
        cars,
        entry_speed,
        minimum_speed_required,
        scan,
    )
}

fn assert_train_crosses_element_after_preroll(
    model: &CoasterModel,
    circuit: &Circuit,
    piece_id: PieceId,
    element_range: std::ops::Range<f64>,
    cars: u8,
    entry_speed: f64,
    context: &str,
) -> ElementForces {
    let scan = scan_train_element_after_preroll(model, circuit, element_range, cars, entry_speed)
        .unwrap_or_else(|error| {
            panic!(
                "{piece_id:?} {} {cars} cars at {entry_speed}: cannot solve pre-roll: {error}",
                model.name
            )
        });
    assert_train_scan_result(model, piece_id, context, cars, entry_speed, false, scan)
}

fn assert_train_scan_result(
    model: &CoasterModel,
    piece_id: PieceId,
    context: &str,
    cars: u8,
    entry_speed: f64,
    minimum_speed_required: bool,
    scan: ElementScan,
) -> ElementForces {
    let forces = scan.forces;
    assert!(
        scan.all_cars_seen,
        "{context}: {piece_id:?} {} {cars} cars at {entry_speed}: not every car entered",
        model.name
    );
    assert!(
        scan.train_cleared,
        "{context}: {piece_id:?} {} {cars} cars at {entry_speed}: train did not clear",
        model.name
    );
    assert!(
        !scan.rolled_back,
        "{context}: {piece_id:?} {} {cars} cars at {entry_speed}: rollback",
        model.name
    );
    assert!(
        !scan.stalled,
        "{context}: {piece_id:?} {} {cars} cars at {entry_speed}: stalled",
        model.name
    );
    assert!(
        scan.violation.is_none(),
        "{context}: {piece_id:?} {} {cars} cars at {entry_speed}: {}",
        model.name,
        scan.violation.as_deref().unwrap_or_default()
    );
    if minimum_speed_required {
        assert!(
            scan.minimum_speed >= 5.0,
            "{context}: {piece_id:?} minimum speed {}",
            scan.minimum_speed
        );
    }
    eprintln!(
        "{context}: {piece_id:?} {} {cars} cars at {entry_speed:.3}: vertical {:.3}..{:.3}, lateral {:.3}, longitudinal {:.3}",
        model.name,
        forces.minimum_vertical,
        forces.maximum_vertical,
        forces.maximum_lateral,
        forces.maximum_longitudinal
    );
    forces
}

fn peak_roll_rate_degrees_per_second(piece: &Piece, speed: f64) -> f64 {
    let sample_count = (piece.length() / 0.05).ceil() as usize;
    let mut previous_distance = 0.0;
    let mut previous_roll = piece.sample(0.0).roll;
    let mut peak_rate: f64 = 0.0;
    for index in 1..=sample_count {
        let distance = if index == sample_count {
            piece.length()
        } else {
            piece.length() * index as f64 / sample_count as f64
        };
        let roll = piece.sample(distance).roll;
        let roll_rate = (roll - previous_roll).abs() / (distance - previous_distance) * speed;
        peak_rate = peak_rate.max(roll_rate.to_degrees());
        previous_distance = distance;
        previous_roll = roll;
    }
    peak_rate
}

fn models() -> Vec<CoasterModel> {
    coaster_models()
}

fn rated_window(model: &CoasterModel, piece: &Piece) -> RatedSpeeds {
    rated_speeds(model, piece).unwrap_or_else(|| {
        panic!(
            "{} has no rated speed window for {:?}",
            model.name, piece.id
        )
    })
}

fn is_known_unavailable_banked_straight(model: &CoasterModel, piece: &Piece) -> bool {
    model.key == "family_coaster"
        && piece.entry.slope == theme_park_sim::track::Slope::UpGentle
        && matches!(
            piece.id,
            PieceId::BankedStraightLeft | PieceId::BankedStraightRight
        )
}

fn assert_shared_rating_passes_every_applicable_context(
    model: &CoasterModel,
    piece: &Piece,
    cars: u8,
    entry_speed: f64,
) {
    let rating = rate_piece(model, cars, piece, entry_speed, rating_physics())
        .unwrap_or_else(|error| panic!("{:?} {} {entry_speed}: {error}", piece.id, model.name));
    assert!(
        rating
            .context_results
            .iter()
            .any(|context| context.applicable),
        "{:?} {} {entry_speed}: no approach context applies: {:?}",
        piece.id,
        model.name,
        rating.context_results
    );
    assert!(
        rating
            .context_results
            .iter()
            .all(|context| context.applicable || context.inapplicable_reason.is_some()),
        "{:?} {} {entry_speed}: context applicability is not explained: {:?}",
        piece.id,
        model.name,
        rating.context_results
    );
    assert!(
        rating.passed,
        "{:?} {} {cars} cars at {entry_speed}: {:?}",
        piece.id, model.name, rating.context_results
    );
}

#[test]
fn every_buildable_inversion_drop_and_sixty_transition_has_a_rated_window() {
    use PieceId::*;

    let sixty_transitions = [
        LevelToSixtyUp,
        SixtyUpToLevel,
        LevelToSixtyDown,
        SixtyDownToLevel,
        SteepToSixtyUp,
        SixtyToSteepUp,
        SteepToSixtyDown,
        SixtyToSteepDown,
        WideLevelToSixtyDown,
        WideSixtyDownToLevel,
    ];
    let drops = [
        BankedDropLeft,
        BankedDropRight,
        TwistedDropLeft,
        TwistedDropRight,
        BarrelDropLeft,
        BarrelDropRight,
    ];
    let allowed_models = models();
    for piece in catalog().iter().filter(|piece| {
        matches!(piece.id, TopHat | LaunchedTopHat)
            || piece.inversion.is_some()
            || sixty_transitions.contains(&piece.id)
            || drops.contains(&piece.id)
    }) {
        if allowed_models.iter().any(|model| model.allows_piece(piece)) {
            let minimum_width =
                if piece.inversion == Some(theme_park_sim::track::Inversion::VerticalLoop) {
                    1.5
                } else {
                    2.0
                };
            for model in allowed_models
                .iter()
                .filter(|model| model.allows_piece(piece))
            {
                let rated = rated_window(model, piece);
                assert!(
                    rated.minimum_entry >= 0.0
                        && rated.maximum_entry - rated.minimum_entry >= minimum_width,
                    "{:?} {}: invalid rated window {rated:?}",
                    piece.id,
                    model.name
                );
            }
        }
    }
}

#[test]
fn every_level_to_level_rated_maximum_passes_for_default_trains_in_isolation() {
    for piece in catalog().iter().filter(|piece| {
        piece.design_speeds.is_some()
            && piece.entry == TrackState::origin()
            && piece.exit.slope == theme_park_sim::track::Slope::Level
            && piece.exit.bank == theme_park_sim::track::Bank::Level
    }) {
        for model in models()
            .into_iter()
            .filter(|model| model.allows_piece(piece))
        {
            let maximum_entry = rated_window(&model, piece).maximum_entry;
            let cars = model.train.cars;
            let rating = rate_piece(&model, cars, piece, maximum_entry, rating_physics())
                .unwrap_or_else(|error| panic!("{:?} {}: {error}", piece.id, model.name));
            assert!(
                rating.passed,
                "{:?} {} {cars} cars at {maximum_entry}: {rating:?}",
                piece.id, model.name
            );
        }
    }
}

#[test]
fn level_to_level_inversion_rated_maxima_pass_for_allowed_default_trains_in_isolation() {
    let mut checked_configurations = 0;
    for piece in catalog().iter().filter(|piece| {
        piece.inversion.is_some()
            && piece.design_speeds.is_some()
            && piece.entry == TrackState::origin()
            && piece.exit.slope == theme_park_sim::track::Slope::Level
            && piece.exit.bank == theme_park_sim::track::Bank::Level
    }) {
        for model in models()
            .into_iter()
            .filter(|model| model.allows_piece(piece))
        {
            let maximum_entry = rated_window(&model, piece).maximum_entry;
            let cars = model.train.cars;
            let rating = rate_piece(&model, cars, piece, maximum_entry, rating_physics())
                .unwrap_or_else(|error| panic!("{:?} {}: {error}", piece.id, model.name));
            assert!(
                rating.passed,
                "{:?} {} {cars} cars at {maximum_entry}: {rating:?}",
                piece.id, model.name
            );
            checked_configurations += 1;
        }
    }
    assert!(
        checked_configurations > 0,
        "no rated inversions were tested"
    );
}

#[test]
fn every_level_to_level_rated_element_passes_default_trains_at_both_window_ends() {
    let mut failures = Vec::new();
    for template in catalog().iter().filter(|piece| {
        piece.design_speeds.is_some()
            && piece.entry == TrackState::origin()
            && piece.exit.slope == theme_park_sim::track::Slope::Level
            && piece.exit.bank == theme_park_sim::track::Bank::Level
    }) {
        let (circuit, element_start, element_end) = circuit_for_level_piece(template.id);
        eprintln!(
            "{:?}: catalog length {:.3} m, midpoint {:?}",
            template.id,
            template.length(),
            template.sample(template.length() / 2.0).position
        );
        for model in models()
            .into_iter()
            .filter(|model| model.allows_piece(template))
        {
            let rated = rated_window(&model, template);
            let cars = model.train.cars;
            for entry_speed in [rated.minimum_entry, rated.maximum_entry] {
                let is_ascending = template.exit.position().z > template.entry.position().z
                    || matches!(
                        template.id,
                        PieceId::TopHat
                            | PieceId::LaunchedTopHat
                            | PieceId::HalfLoopUp
                            | PieceId::Loop
                            | PieceId::LoopRight
                            | PieceId::LargeLoop
                            | PieceId::LargeLoopRight
                            | PieceId::SteelLoop
                            | PieceId::SteelLoopRight
                    );
                if std::panic::catch_unwind(|| {
                    assert_train_crosses_element(
                        &model,
                        &circuit,
                        template.id,
                        element_start..element_end,
                        cars,
                        entry_speed,
                        entry_speed == rated.minimum_entry && is_ascending,
                    );
                })
                .is_err()
                {
                    failures.push(format!(
                        "{:?} {} {cars} cars at {entry_speed}",
                        template.id, model.name
                    ));
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "rated physics failures: {}",
        failures.join("; ")
    );
}

#[test]
fn wooden_strong_floater_is_rated_and_available_on_briar_blitz() {
    let piece = catalog()
        .iter()
        .find(|piece| piece.id == PieceId::WoodenEjectorHill8x64)
        .expect("wooden strong floater is in the catalog");
    assert!(piece.name.contains("floater"));
    assert!(piece.design_speeds.is_some());
    assert!(wooden_twister_model().allows_piece(piece));
    assert!(!hybrid_model().allows_piece(piece));
    assert!(!multi_launch_model().allows_piece(piece));
    let model = wooden_twister_model();
    let rated = rated_window(&model, piece);
    let (circuit, element_start, element_end) = circuit_for_level_piece(piece.id);
    let cars = model.train.cars;
    let mut speeds = vec![rated.minimum_entry];
    let mut speed = rated.maximum_entry;
    while speed > rated.minimum_entry {
        speeds.push(speed);
        speed -= 1.0;
    }
    for entry_speed in speeds {
        assert_train_crosses_element(
            &model,
            &circuit,
            piece.id,
            element_start..element_end,
            cars,
            entry_speed,
            false,
        );
    }
    let rating = rate_piece(&model, cars, piece, rated.maximum_entry, rating_physics())
        .expect("rating must complete");
    assert_eq!(
        model.comfortable_force_limits.classify_force(
            ForceRatingAxis::Airtime,
            rating.minimum_filtered_vertical,
            model.name,
        ),
        ForceRating::Good,
        "{cars} cars: {:.3} g",
        rating.minimum_filtered_vertical
    );
}

#[test]
fn every_ejector_hill_reaches_great_airtime_for_each_allowed_default_train() {
    let allowed_models = models();
    let ejector_hills: Vec<_> = catalog()
        .iter()
        .filter(|piece| {
            matches!(piece.family, Some(PieceFamily::Hill(family))
                if family.feel == HillFeel::Ejector && family.twist.is_none())
                && allowed_models
                    .iter()
                    .any(|model| model.allowed_pieces.contains(&piece.id))
        })
        .collect();
    assert!(!ejector_hills.is_empty(), "no buildable ejector hills");

    let mut failures = Vec::new();
    for piece in ejector_hills {
        for model in allowed_models
            .iter()
            .filter(|model| model.allows_piece(piece))
        {
            let rated = rated_window(model, piece);
            let sample_count = ((rated.maximum_entry - rated.minimum_entry) / 0.25).ceil() as usize;
            let found_great = (0..=sample_count).any(|index| {
                let speed = (rated.minimum_entry + index as f64 * 0.25).min(rated.maximum_entry);
                let rating = rate_piece(model, model.train.cars, piece, speed, rating_physics())
                    .expect("rating must complete");
                rating.passed
                    && model.comfortable_force_limits.classify_force(
                        ForceRatingAxis::Airtime,
                        rating.minimum_filtered_vertical,
                        model.name,
                    ) == ForceRating::Great
            });
            if !found_great {
                failures.push(format!(
                    "{} {} {} cars: no Great airtime at sampled speeds in {rated:?}",
                    piece.name, model.name, model.train.cars
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn every_floater_hill_gives_default_trains_at_most_good_airtime_at_its_maximum() {
    let mut failures = Vec::new();
    for piece in catalog().iter().filter(|piece| {
        matches!(piece.family, Some(PieceFamily::Hill(family))
            if matches!(family.feel, HillFeel::Floater | HillFeel::StrongFloater)
                && family.twist.is_none())
    }) {
        for model in models()
            .into_iter()
            .filter(|model| model.allows_piece(piece))
        {
            let maximum = rated_window(&model, piece).maximum_entry;
            let rating = rate_piece(&model, model.train.cars, piece, maximum, rating_physics())
                .expect("rating must complete");
            let airtime = model.comfortable_force_limits.classify_force(
                ForceRatingAxis::Airtime,
                rating.minimum_filtered_vertical,
                model.name,
            );
            if !matches!(airtime, ForceRating::Weak | ForceRating::Good) {
                failures.push(format!(
                    "{} {} at {maximum}: {airtime:?} ({:.3} g)",
                    piece.name, model.name, rating.minimum_filtered_vertical
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn every_model_rated_window_endpoint_passes_with_its_default_train_in_each_applicable_context() {
    for piece in catalog()
        .iter()
        .filter(|piece| piece.design_speeds.is_some())
    {
        for model in models()
            .into_iter()
            .filter(|model| model.allows_piece(piece))
        {
            let Some(rated) = rated_speeds(&model, piece) else {
                assert!(
                    is_known_unavailable_banked_straight(&model, piece),
                    "{} has no rated window for {:?} at {:?}",
                    model.name,
                    piece.id,
                    piece.entry.slope
                );
                continue;
            };
            let cars = model.train.cars;
            let measured_from = measured_rated_window(&model, piece)
                .expect("piece must have a measured window")
                .measured_from;
            for speed in [rated.minimum_entry.max(measured_from), rated.maximum_entry] {
                let rating = rate_piece(&model, cars, piece, speed, rating_physics())
                    .expect("rating must complete");
                assert!(
                    rating.passed,
                    "{:?} {} {cars} cars at {speed}: {rating:?}",
                    piece.id, model.name
                );
                let contexts = measurement_contexts(&model, cars, piece, speed);
                assert_eq!(contexts.len(), rating.context_results.len());
                let mut applicable_contexts = 0;
                for (context, context_rating) in contexts.iter().zip(&rating.context_results) {
                    assert_eq!(context.name, context_rating.context);
                    if piece.id == PieceId::SteepToSixtyUp
                        && context.name == "connected model-allowed approach"
                    {
                        let element_index =
                            context.circuit.piece_index(context.element_range.start);
                        let preceding_piece = &context.circuit.pieces()[element_index - 1];
                        assert_eq!(preceding_piece.exit.slope, piece.entry.slope);
                        assert_eq!(preceding_piece.exit.bank, piece.entry.bank);
                        assert_eq!(
                            preceding_piece.exit.horizontal_curvature,
                            piece.entry.horizontal_curvature
                        );
                        assert_eq!(
                            preceding_piece.exit.vertical_curvature,
                            piece.entry.vertical_curvature
                        );
                        assert_eq!(piece.entry.slope, theme_park_sim::track::Slope::UpSteep);
                        assert!(model.allows_piece(preceding_piece));
                    }
                    if context_rating.applicable {
                        applicable_contexts += 1;
                        assert!(
                            context_rating.passed,
                            "{:?} {} {cars} cars at {speed}, context {}: {context_rating:?}",
                            piece.id, model.name, context.name
                        );
                        assert_train_crosses_element_after_preroll(
                            &model,
                            &context.circuit,
                            piece.id,
                            context.element_range.clone(),
                            cars,
                            speed,
                            context.name,
                        );
                    } else {
                        assert!(
                            context_rating.inapplicable_reason.is_some(),
                            "{:?} {} {cars} cars at {speed}, context {} has no applicability reason: {context_rating:?}",
                            piece.id,
                            model.name,
                            context.name
                        );
                    }
                }
                assert!(
                    applicable_contexts > 0,
                    "{:?} {} {cars} cars at {speed}: no approach context applies",
                    piece.id,
                    model.name
                );
            }
        }
    }
}

#[test]
fn every_tilted_roll_and_drop_variant_passes_full_train_at_both_window_ends() {
    use PieceId::*;
    let rated_ids = [
        HalfCorkscrewLeftUp,
        HalfCorkscrewLeftDown,
        HalfCorkscrewRightUp,
        HalfCorkscrewRightDown,
        RavenRollLeftUp,
        RavenRollLeftDown,
        RavenRollRightUp,
        RavenRollRightDown,
        BankedDropLeft,
        BankedDropRight,
        TwistedDropLeft,
        TwistedDropRight,
        BarrelDropLeft,
        BarrelDropRight,
    ];

    for piece_id in rated_ids {
        let piece = catalog().iter().find(|piece| piece.id == piece_id).unwrap();
        for model in models()
            .into_iter()
            .filter(|model| model.allows_piece(piece))
        {
            let rated = rated_window(&model, piece);
            let cars = model.train.cars;
            for entry_speed in [rated.minimum_entry, rated.maximum_entry] {
                assert_shared_rating_passes_every_applicable_context(
                    &model,
                    piece,
                    cars,
                    entry_speed,
                );
            }
        }
    }
}

#[test]
fn barrel_roll_drop_rail_descends_monotonically_for_both_mirrors() {
    use PieceId::{BarrelDropLeft, BarrelDropRight};

    for piece_id in [BarrelDropLeft, BarrelDropRight] {
        let piece = catalog().iter().find(|piece| piece.id == piece_id).unwrap();
        let maximum_upward_tangent = (0..=2048)
            .map(|step| {
                piece
                    .sample(piece.length() * f64::from(step) / 2048.0)
                    .tangent
                    .z
            })
            .fold(f64::NEG_INFINITY, f64::max);
        assert!(
            maximum_upward_tangent <= 1e-8,
            "{piece_id:?} has upward rail tangent {maximum_upward_tangent}"
        );
    }
}

#[test]
fn banked_straights_pass_isolated_ratings_at_both_window_ends() {
    for piece in catalog().iter().filter(|piece| {
        matches!(
            piece.id,
            PieceId::BankedStraightLeft | PieceId::BankedStraightRight
        )
    }) {
        for model in models()
            .into_iter()
            .filter(|model| model.allows_piece(piece))
        {
            let Some(rated) = rated_speeds(&model, piece) else {
                assert!(
                    is_known_unavailable_banked_straight(&model, piece),
                    "{} has no rated window for {:?} at {:?}",
                    model.name,
                    piece.id,
                    piece.entry.slope
                );
                continue;
            };
            let cars = model.train.cars;
            let measured_from = measured_rated_window(&model, piece)
                .expect("piece must have a measured window")
                .measured_from;
            for speed in [rated.minimum_entry.max(measured_from), rated.maximum_entry] {
                let rating = rate_piece(&model, cars, piece, speed, rating_physics())
                    .unwrap_or_else(|error| {
                        panic!(
                            "{:?} {} {cars} cars at {speed}: {error}",
                            piece.id, model.name
                        )
                    });
                assert!(
                    rating.passed,
                    "{:?} {} {cars} cars at {speed}: {rating:?}",
                    piece.id, model.name
                );
            }
        }
    }
}

#[test]
fn daisy_dash_uphill_banked_straights_are_unavailable_after_a_gentle_up_bank() {
    use PieceId::{BankedStraightLeft, BankedStraightRight};
    let model = models()
        .into_iter()
        .find(|model| model.key == "family_coaster")
        .unwrap();
    for (piece_id, context_name) in [
        (
            BankedStraightLeft,
            "Gentle up, bank left, banked straight, then left rollout",
        ),
        (
            BankedStraightRight,
            "Gentle up, bank right, banked straight, then right rollout",
        ),
    ] {
        let piece = catalog()
            .iter()
            .find(|piece| {
                piece.id == piece_id && piece.entry.slope == theme_park_sim::track::Slope::UpGentle
            })
            .unwrap();
        assert!(model.allows_piece(piece));
        assert!(piece.design_speeds.is_some());
        assert!(is_known_unavailable_banked_straight(&model, piece));
        assert!(rated_speeds(&model, piece).is_none());

        let minimum_design_speed = piece.design_speeds.unwrap().minimum_entry;
        let rating = rate_piece(
            &model,
            model.train.cars,
            piece,
            minimum_design_speed,
            rating_physics(),
        )
        .expect("rating must complete");
        assert!(!rating.passed, "{piece_id:?}: {rating:?}");
        let context = rating
            .context_results
            .iter()
            .find(|context| context.context == context_name)
            .unwrap_or_else(|| panic!("{piece_id:?} lacks {context_name}: {rating:?}"));
        assert!(context.applicable, "{piece_id:?}: {context:?}");
        assert!(!context.passed, "{piece_id:?}: {context:?}");
        assert_eq!(
            context.violation.as_ref().unwrap().axis,
            1,
            "{piece_id:?}: {context:?}"
        );
    }
}

#[test]
fn banked_straight_compositions_pass_with_continuous_force_history() {
    for (bank, straight, rollout) in [
        (
            PieceId::BankLeft,
            PieceId::BankedStraightLeft,
            PieceId::RolloutLeft,
        ),
        (
            PieceId::BankRight,
            PieceId::BankedStraightRight,
            PieceId::RolloutRight,
        ),
    ] {
        let (circuit, ranges) = circuit_for_sequence(&[
            bank,
            straight,
            rollout,
            PieceId::Straight,
            PieceId::Straight,
            PieceId::Straight,
            PieceId::Straight,
        ]);
        let tested_ranges: Vec<_> = ranges[..3]
            .iter()
            .map(|(_, start, end)| *start..*end)
            .collect();
        let piece = catalog().iter().find(|piece| piece.id == straight).unwrap();
        for model in models() {
            if !model.allows_piece(piece) {
                continue;
            }
            let rated = rated_window(&model, piece);
            let rating = rate_sequence(
                &circuit,
                &tested_ranges,
                &model.train,
                rating_physics(),
                model.comfortable_force_limits,
                tested_ranges[0].start - 10.0,
                rated.minimum_entry,
            );
            assert!(
                rating.failure.is_none() && rating.violation.is_none(),
                "{straight:?} {}: {rating:?}",
                model.name
            );
        }
    }
}

#[test]
fn banked_straights_and_rollouts_pass_full_train_at_rated_window_ends() {
    use PieceId::*;
    for (bank, straight, rollout) in [
        (BankLeft, BankedStraightLeft, RolloutLeft),
        (BankRight, BankedStraightRight, RolloutRight),
    ] {
        for (entry, exit) in [
            (None, None),
            (Some(LevelToGentleUp), Some(GentleUpToLevel)),
            (Some(LevelToGentleDown), Some(GentleDownToLevel)),
        ] {
            let mut sequence = Vec::new();
            sequence.extend(entry);
            sequence.extend([bank, straight, rollout]);
            sequence.extend(exit);
            let (circuit, ranges) = circuit_for_sequence_with_initial_runway(&sequence, 8);
            for &(id, start, end) in &ranges {
                if id != straight && id != rollout {
                    continue;
                }
                let slope = if entry == Some(LevelToGentleUp) {
                    theme_park_sim::track::Slope::UpGentle
                } else if entry == Some(LevelToGentleDown) {
                    theme_park_sim::track::Slope::DownGentle
                } else {
                    theme_park_sim::track::Slope::Level
                };
                let piece = catalog()
                    .iter()
                    .find(|piece| piece.id == id && piece.entry.slope == slope)
                    .unwrap();
                for model in models()
                    .into_iter()
                    .filter(|model| model.allows_piece(piece))
                {
                    let Some(rated) = rated_speeds(&model, piece) else {
                        assert!(
                            is_known_unavailable_banked_straight(&model, piece),
                            "{} has no rated window for {:?} at {:?}",
                            model.name,
                            piece.id,
                            piece.entry.slope
                        );
                        continue;
                    };
                    let cars = model.train.cars;
                    let measured_from = measured_rated_window(&model, piece)
                        .expect("piece must have a measured window")
                        .measured_from;
                    for speed in [rated.minimum_entry.max(measured_from), rated.maximum_entry] {
                        assert_train_crosses_element_after_preroll(
                            &model,
                            &circuit,
                            id,
                            start..end,
                            cars,
                            speed,
                            "banked straight and rollout composition",
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn wave_and_overbank_turns_have_rated_windows_for_both_steel_models() {
    for id in [
        PieceId::WaveTurnLeft,
        PieceId::WaveTurnRight,
        PieceId::HybridOverbankLeft,
        PieceId::HybridOverbankRight,
    ] {
        let piece = catalog().iter().find(|piece| piece.id == id).unwrap();
        for model in [hybrid_model(), multi_launch_model()] {
            assert!(
                model.allows_piece(piece),
                "{} does not allow {id:?}",
                model.name
            );
            let rated = rated_window(&model, piece);
            assert!(rated.maximum_entry - rated.minimum_entry >= 2.0);
        }
    }
}

#[test]
fn wave_and_overbank_turns_have_contiguous_rated_windows() {
    for id in [
        PieceId::WaveTurnLeft,
        PieceId::WaveTurnRight,
        PieceId::HybridOverbankLeft,
        PieceId::HybridOverbankRight,
    ] {
        let piece = catalog().iter().find(|piece| piece.id == id).unwrap();
        let (circuit, start, end) = circuit_for_level_piece(id);
        for model in [hybrid_model(), multi_launch_model()] {
            let rated = rated_window(&model, piece);
            let mut speed = rated.minimum_entry;
            while speed <= rated.maximum_entry {
                let cars = model.train.cars;
                assert_train_crosses_element(&model, &circuit, id, start..end, cars, speed, false);
                speed += 1.0;
            }
        }
    }
}

#[test]
fn speed_matched_turns_stay_inside_rated_force_limits_and_roll_rate() {
    const MAXIMUM_DESIGN_SPEED_ROLL_RATE_DEGREES_PER_SECOND: f64 = 95.0;
    const MAXIMUM_RATED_WINDOW_ROLL_RATE_DEGREES_PER_SECOND: f64 = 200.0;

    let speed_turns: Vec<_> = catalog()
        .iter()
        .filter(|piece| {
            matches!(
                piece.family,
                Some(theme_park_sim::track::PieceFamily::SpeedTurn { .. })
            )
        })
        .collect();
    assert_eq!(speed_turns.len(), 10);
    let mut failures = Vec::new();
    let all_models = models();
    let mut scanned_candidates = 0;

    for piece in speed_turns {
        let Some(theme_park_sim::track::PieceFamily::SpeedTurn {
            design_speed_metres_per_second: design_speed,
            ..
        }) = piece.family
        else {
            unreachable!("filtered speed-turn piece lost its family metadata");
        };
        let (circuit, start, end) = circuit_for_level_piece(piece.id);
        let mut allowed_model_count = 0;
        for model in all_models.iter().filter(|model| model.allows_piece(piece)) {
            allowed_model_count += 1;
            let rated = rated_window(model, piece);
            let mut speeds = Vec::new();
            let mut speed = rated.minimum_entry;
            while speed < rated.maximum_entry {
                speeds.push(speed);
                speed += 1.0;
            }
            speeds.push(rated.maximum_entry);
            if (rated.minimum_entry..=rated.maximum_entry).contains(&design_speed) {
                speeds.push(design_speed);
            }
            speeds.sort_by(f64::total_cmp);
            speeds.dedup_by(|left, right| (*left - *right).abs() < 1e-8);

            let mut maximum_design_speed_lateral: f64 = 0.0;
            let mut design_lateral_context = String::from("design speed outside rated window");
            let mut window_forces = ElementForces {
                minimum_vertical: f64::INFINITY,
                maximum_vertical: f64::NEG_INFINITY,
                maximum_lateral: 0.0,
                maximum_longitudinal: 0.0,
                peak_lateral_distance: start,
                peak_lateral_speed: rated.minimum_entry,
            };
            let mut window_lateral_context = String::from("no model scan");
            for speed in speeds {
                let cars = model.train.cars;
                let scan = scan_train_element(model, &circuit, start..end, cars, speed);
                let forces = scan.forces;
                if !scan.all_cars_seen {
                    failures.push(format!(
                        "{:?} {} {cars} cars at {speed:.1} m/s: not every car entered in the candidate scan",
                        piece.id, model.name
                    ));
                }
                if !scan.train_cleared {
                    failures.push(format!(
                        "{:?} {} {cars} cars at {speed:.1} m/s: train did not clear in the candidate scan",
                        piece.id, model.name
                    ));
                }
                if scan.stalled {
                    failures.push(format!(
                        "{:?} {} {cars} cars at {speed:.1} m/s: train stalled in the candidate scan",
                        piece.id, model.name
                    ));
                }
                if scan.rolled_back {
                    failures.push(format!(
                        "{:?} {} {cars} cars at {speed:.1} m/s: train rolled back in the candidate scan",
                        piece.id, model.name
                    ));
                }
                if let Some(violation) = &scan.violation {
                    failures.push(format!(
                        "{:?} {} {cars} cars at {speed:.1} m/s: {violation}",
                        piece.id, model.name
                    ));
                }
                window_forces.minimum_vertical =
                    window_forces.minimum_vertical.min(forces.minimum_vertical);
                window_forces.maximum_vertical =
                    window_forces.maximum_vertical.max(forces.maximum_vertical);
                if forces.maximum_lateral > window_forces.maximum_lateral {
                    window_forces.maximum_lateral = forces.maximum_lateral;
                    window_forces.peak_lateral_distance = forces.peak_lateral_distance;
                    window_forces.peak_lateral_speed = forces.peak_lateral_speed;
                    window_lateral_context = format!("{} cars at {speed:.1} m/s", cars);
                }
                window_forces.maximum_longitudinal = window_forces
                    .maximum_longitudinal
                    .max(forces.maximum_longitudinal);
                if (speed - design_speed).abs() < 1e-8 {
                    maximum_design_speed_lateral = forces.maximum_lateral;
                    design_lateral_context = format!(
                        "{cars} cars at {:.2} m, car speed {:.2} m/s",
                        forces.peak_lateral_distance, forces.peak_lateral_speed
                    );
                }
            }

            let window_peak_roll_rate_degrees_per_second =
                peak_roll_rate_degrees_per_second(piece, rated.maximum_entry);
            eprintln!(
                "{:?} {}: full-train vertical {:.3}..{:.3} g, lateral {:.3} g across window at {window_lateral_context}, {:.3} g at design speed at {design_lateral_context}, longitudinal {:.3} g, peak rated-window roll rate {window_peak_roll_rate_degrees_per_second:.1} degrees/s",
                piece.id,
                model.name,
                window_forces.minimum_vertical,
                window_forces.maximum_vertical,
                window_forces.maximum_lateral,
                maximum_design_speed_lateral,
                window_forces.maximum_longitudinal
            );
            if maximum_design_speed_lateral > 0.35 {
                failures.push(format!(
                    "{} {} design-speed worst-car lateral {maximum_design_speed_lateral:.3} g at {design_lateral_context} exceeds 0.35 g",
                    piece.name, model.name
                ));
            }
            if window_peak_roll_rate_degrees_per_second
                > MAXIMUM_RATED_WINDOW_ROLL_RATE_DEGREES_PER_SECOND
            {
                failures.push(format!(
                    "{} {} peak rated-window roll rate {window_peak_roll_rate_degrees_per_second:.1} degrees/s exceeds {MAXIMUM_RATED_WINDOW_ROLL_RATE_DEGREES_PER_SECOND:.1} degrees/s",
                    piece.name, model.name
                ));
            }
        }
        assert!(allowed_model_count > 0, "no model allows {:?}", piece.id);
        scanned_candidates += 1;
        let design_speed_peak_roll_rate_degrees_per_second =
            peak_roll_rate_degrees_per_second(piece, design_speed);
        if design_speed_peak_roll_rate_degrees_per_second
            > MAXIMUM_DESIGN_SPEED_ROLL_RATE_DEGREES_PER_SECOND
        {
            failures.push(format!(
                "{} design-speed roll rate {design_speed_peak_roll_rate_degrees_per_second:.1} degrees/s exceeds {MAXIMUM_DESIGN_SPEED_ROLL_RATE_DEGREES_PER_SECOND:.1} degrees/s",
                piece.name
            ));
        }
    }
    assert_eq!(
        scanned_candidates, 10,
        "full-train scan covers every speed-turn candidate"
    );
    assert!(
        failures.is_empty(),
        "speed-turn force scan failures:\n{}",
        failures.join("\n")
    );
}

#[test]
fn floater_point_mass_forecast_compared_with_default_train() {
    use theme_park_sim::track::predicted_hill_forces;

    let piece = catalog()
        .iter()
        .find(|piece| piece.id == PieceId::FloaterHill8x64)
        .unwrap();
    let model = hybrid_model();
    let entry_speed = rated_window(&model, piece).maximum_entry;
    let forecast = predicted_hill_forces(piece, entry_speed).unwrap();
    let (circuit, start, end) = circuit_for_level_piece(piece.id);
    let config = model.train;
    let cars = config.cars;
    let mut train = Train::new(&circuit, &config, unpowered(), start, entry_speed);
    let mut minimum = f64::INFINITY;
    for _ in 0..100_000 {
        train.step(&circuit);
        for frame in train.take_readings() {
            for reading in frame {
                if (start..end).contains(&reading.distance) {
                    minimum = minimum.min(reading.g.vertical);
                }
            }
        }
        if train.distance() >= end + config.length_metres {
            break;
        }
        assert!(train.speed() > 0.0 && train.notes().is_empty());
    }
    assert!(minimum.is_finite());
    assert!(train.distance() >= end + config.length_metres);
    eprintln!(
        "FloaterHill8x64 {cars} cars at {entry_speed:.3} m/s: forecast {:.3} g, full train {minimum:.3} g, difference {:+.3} g",
        forecast.minimum_vertical_g,
        minimum - forecast.minimum_vertical_g
    );
}

#[test]
fn connected_measurement_approaches_use_pieces_allowed_for_the_model() {
    let mut failures = Vec::new();
    for piece in catalog()
        .iter()
        .filter(|piece| piece.design_speeds.is_some())
    {
        for model in coaster_models()
            .iter()
            .filter(|model| model.allows_piece(piece))
        {
            let (circuit, element_range) =
                connected_measurement_track(model, model.train.cars, piece, 20.0);
            let element_index = circuit.piece_index(element_range.start);
            let unsupported = circuit.pieces()[..element_index]
                .iter()
                .filter(|approach_piece| !model.allows_piece(approach_piece))
                .map(|approach_piece| approach_piece.id)
                .collect::<Vec<_>>();
            if !unsupported.is_empty() {
                failures.push(format!(
                    "{} approach to {:?} uses unsupported pieces {unsupported:?}",
                    model.name, piece.id
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
