use std::collections::{HashMap, HashSet, VecDeque};

use theme_park_sim::physics::{Circuit, CoasterRun, PhysicsConfig};
use theme_park_sim::track::{
    CoasterModel, Heading, LaunchType, Piece, PieceId, PieceKind, TrackState, catalog,
    hybrid_model, multi_launch_model, wooden_twister_model,
};

struct Composition {
    name: &'static str,
    model: CoasterModel,
    lift_gentle_pieces: usize,
    lift_one_tile_pieces: usize,
    elements: Vec<PieceId>,
    launch: bool,
    restoration: HeightRestoration,
    requires_inversion: bool,
    expected_inversion_passages: usize,
}

#[derive(Clone, Copy)]
enum HeightRestoration {
    None,
    RestoreToStartingHeight,
    LiftToStartingHeight,
}

fn append_piece(
    pieces: &mut Vec<Piece>,
    state: &mut TrackState,
    templates: &[Piece],
    id: PieceId,
    kind: Option<PieceKind>,
) {
    let template = templates
        .iter()
        .find(|piece| piece.id == id)
        .unwrap_or_else(|| panic!("catalog does not contain {id:?}"));
    let mut piece = template
        .at(*state)
        .unwrap_or_else(|| panic!("{id:?} does not join track state {state:?}"));
    if let Some(kind) = kind {
        piece.kind = kind;
    }
    *state = piece.exit;
    pieces.push(piece);
}

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

fn heading_index(heading: Heading) -> i32 {
    match heading {
        Heading::East => 0,
        Heading::NorthEast => 1,
        Heading::North => 2,
        Heading::NorthWest => 3,
        Heading::West => 4,
        Heading::SouthWest => 5,
        Heading::South => 6,
        Heading::SouthEast => 7,
    }
}

fn heading_from_index(index: i32) -> Heading {
    [
        Heading::East,
        Heading::NorthEast,
        Heading::North,
        Heading::NorthWest,
        Heading::West,
        Heading::SouthWest,
        Heading::South,
        Heading::SouthEast,
    ][index.rem_euclid(8) as usize]
}

fn connector_exit_at(piece: &Piece, state: TrackState) -> Option<TrackState> {
    if state.slope != piece.entry.slope
        || state.bank != piece.entry.bank
        || state.horizontal_curvature != piece.entry.horizontal_curvature
        || state.vertical_curvature != piece.entry.vertical_curvature
    {
        return None;
    }
    let rotation =
        (heading_index(state.heading) - heading_index(piece.entry.heading)).rem_euclid(8);
    let dx = piece.exit.x - piece.entry.x;
    let dy = piece.exit.y - piece.entry.y;
    let (x, y) = match rotation {
        0 => (dx, dy),
        1 => (dx - dy, dx + dy),
        2 => (-dy, dx),
        3 => (-dx - dy, dx - dy),
        4 => (-dx, -dy),
        5 => (dy - dx, -dx - dy),
        6 => (dy, -dx),
        7 => (dx + dy, dy - dx),
        _ => unreachable!(),
    };
    Some(TrackState {
        x: state.x + x,
        y: state.y + y,
        z: state.z
            + if rotation % 2 == 0 {
                piece.exit.z - piece.entry.z
            } else {
                (f64::from(piece.exit.z - piece.entry.z) * std::f64::consts::SQRT_2).round() as i32
            },
        heading: heading_from_index(heading_index(piece.exit.heading) + rotation),
        slope: piece.exit.slope,
        bank: piece.exit.bank,
        horizontal_curvature: piece.exit.horizontal_curvature,
        vertical_curvature: piece.exit.vertical_curvature,
    })
}

fn level_return_path(start: TrackState, model: &CoasterModel, templates: &[Piece]) -> Vec<PieceId> {
    let candidates: Vec<_> = [
        PieceId::Straight,
        PieceId::TurnSmallStandalone,
        PieceId::TurnSmallRightStandalone,
        PieceId::TurnLargeStandalone,
        PieceId::TurnLargeRightStandalone,
        PieceId::OffsetLeft,
        PieceId::OffsetRight,
    ]
    .into_iter()
    .map(|id| templates.iter().find(|piece| piece.id == id).unwrap())
    .filter(|piece| model.allows_piece(piece))
    .collect();

    let start_key = connector_key(start);
    let target_key = connector_key(TrackState::origin());
    let mut queue = VecDeque::from([(start, start_key, 0_usize)]);
    let mut visited = HashSet::from([start_key]);
    let mut predecessors: HashMap<ConnectorKey, (ConnectorKey, PieceId)> = HashMap::new();
    loop {
        let (state, current_key, path_length) = queue.pop_front().expect("no level return path");
        if current_key == target_key {
            let mut path = Vec::new();
            let mut key = current_key;
            while key != start_key {
                let (previous_key, piece_id) = predecessors[&key];
                path.push(piece_id);
                key = previous_key;
            }
            path.reverse();
            return path;
        }
        if path_length >= 40 {
            continue;
        }
        for template in &candidates {
            let next_state = connector_exit_at(template, state)
                .expect("level return connector rejected a compatible state");
            if next_state.x.abs() > 300 || next_state.y.abs() > 300 {
                continue;
            }
            let next_key = connector_key(next_state);
            if !visited.insert(next_key) {
                continue;
            }
            predecessors.insert(next_key, (current_key, template.id));
            queue.push_back((next_state, next_key, path_length + 1));
        }
    }
}

fn append_height_restoration(
    pieces: &mut Vec<Piece>,
    state: &mut TrackState,
    templates: &[Piece],
    lift_kind: Option<PieceKind>,
) {
    if state.z > 0 {
        let units_to_drop = usize::try_from(state.z).unwrap();
        let (short_falls, gentle_downs) = if units_to_drop < 4 {
            (units_to_drop, 0)
        } else {
            let remainder = (units_to_drop - 4) % 4;
            (remainder, (units_to_drop - 4 - remainder) / 4)
        };
        for _ in 0..short_falls {
            append_piece(pieces, state, templates, PieceId::ShortFall, None);
        }
        if units_to_drop >= 4 {
            append_piece(pieces, state, templates, PieceId::LevelToGentleDown, None);
            for _ in 0..gentle_downs {
                append_piece(pieces, state, templates, PieceId::GentleDown, None);
            }
            append_piece(pieces, state, templates, PieceId::GentleDownToLevel, None);
        }
    } else if state.z < 0 {
        let units_to_climb = state.z.unsigned_abs() as usize;
        if lift_kind.is_some() {
            assert!(
                units_to_climb >= 4,
                "drop is shorter than the lift transitions"
            );
            let gentle_ups = (units_to_climb - 4) / 4;
            let one_tile_ups = (units_to_climb - 4) % 4;
            append_piece(
                pieces,
                state,
                templates,
                PieceId::LevelToGentleUp,
                lift_kind,
            );
            for _ in 0..gentle_ups {
                append_piece(pieces, state, templates, PieceId::GentleUp, lift_kind);
            }
            for _ in 0..one_tile_ups {
                append_piece(
                    pieces,
                    state,
                    templates,
                    PieceId::OneTileGentleUp,
                    lift_kind,
                );
            }
            append_piece(
                pieces,
                state,
                templates,
                PieceId::GentleUpToLevel,
                lift_kind,
            );
        } else {
            let (gentle_up_count, short_rises) = if units_to_climb < 4 {
                (0, units_to_climb)
            } else {
                let remainder = (units_to_climb - 4) % 4;
                ((units_to_climb - 4 - remainder) / 4, remainder)
            };
            if units_to_climb >= 4 {
                append_piece(pieces, state, templates, PieceId::LevelToGentleUp, None);
                for _ in 0..gentle_up_count {
                    append_piece(pieces, state, templates, PieceId::GentleUp, None);
                }
                append_piece(pieces, state, templates, PieceId::GentleUpToLevel, None);
            }
            for _ in 0..short_rises {
                append_piece(pieces, state, templates, PieceId::ShortRise, None);
            }
        }
    }
}

fn build_composition(composition: &Composition) -> (Circuit, Vec<(PieceId, std::ops::Range<f64>)>) {
    let templates = catalog();
    let mut pieces = Vec::new();
    let mut state = TrackState::origin();
    let sixty_degree_drop = composition.elements.first() == Some(&PieceId::LevelToSixtyDown);
    append_piece(&mut pieces, &mut state, templates, PieceId::Station, None);
    if !sixty_degree_drop {
        append_piece(&mut pieces, &mut state, templates, PieceId::Straight, None);
        append_piece(&mut pieces, &mut state, templates, PieceId::Straight, None);
    }

    if composition.launch {
        for _ in 0..4 {
            append_piece(
                &mut pieces,
                &mut state,
                templates,
                PieceId::LaunchLong,
                None,
            );
        }
    } else if sixty_degree_drop {
        append_piece(&mut pieces, &mut state, templates, PieceId::ShortFall, None);
        append_piece(&mut pieces, &mut state, templates, PieceId::ShortFall, None);
    } else {
        append_piece(
            &mut pieces,
            &mut state,
            templates,
            PieceId::LevelToGentleUp,
            Some(PieceKind::Lift),
        );
        for _ in 0..composition.lift_gentle_pieces {
            append_piece(
                &mut pieces,
                &mut state,
                templates,
                PieceId::GentleUp,
                Some(PieceKind::Lift),
            );
        }
        for _ in 0..composition.lift_one_tile_pieces {
            append_piece(
                &mut pieces,
                &mut state,
                templates,
                PieceId::OneTileGentleUp,
                Some(PieceKind::Lift),
            );
        }
        append_piece(
            &mut pieces,
            &mut state,
            templates,
            PieceId::GentleUpToLevel,
            Some(PieceKind::Lift),
        );
        append_piece(
            &mut pieces,
            &mut state,
            templates,
            PieceId::Straight,
            Some(PieceKind::Lift),
        );
        append_piece(
            &mut pieces,
            &mut state,
            templates,
            PieceId::Straight,
            Some(PieceKind::Lift),
        );
        append_piece(
            &mut pieces,
            &mut state,
            templates,
            PieceId::LevelToGentleDown,
            None,
        );
        for _ in 0..composition.lift_gentle_pieces {
            append_piece(
                &mut pieces,
                &mut state,
                templates,
                PieceId::GentleDown,
                None,
            );
        }
        for _ in 0..composition.lift_one_tile_pieces {
            append_piece(
                &mut pieces,
                &mut state,
                templates,
                PieceId::OneTileGentleDown,
                None,
            );
        }
        append_piece(
            &mut pieces,
            &mut state,
            templates,
            PieceId::GentleDownToLevel,
            None,
        );
    }

    let mut element_ranges = Vec::new();
    for &id in &composition.elements {
        assert!(
            composition
                .model
                .allows_piece(templates.iter().find(|piece| piece.id == id).unwrap()),
            "{} does not allow {id:?}",
            composition.model.name
        );
        let element_start = pieces.iter().map(Piece::length).sum();
        append_piece(&mut pieces, &mut state, templates, id, None);
        let element_end = pieces.iter().map(Piece::length).sum();
        element_ranges.push((id, element_start..element_end));
    }
    match composition.restoration {
        HeightRestoration::None => {}
        HeightRestoration::RestoreToStartingHeight => {
            append_height_restoration(&mut pieces, &mut state, templates, None);
        }
        HeightRestoration::LiftToStartingHeight => {
            assert!(
                state.z < 0,
                "lift return requires a drop below station height"
            );
            append_height_restoration(&mut pieces, &mut state, templates, Some(PieceKind::Lift));
        }
    }
    assert_eq!(
        state.z, 0,
        "{} did not restore starting height",
        composition.name
    );

    if sixty_degree_drop {
        let launch_long = templates
            .iter()
            .find(|piece| piece.id == PieceId::LaunchLong)
            .expect("catalog must contain a long launch");
        assert!(
            composition
                .model
                .launch_types
                .contains(&LaunchType::LinearMotor),
            "{} does not support launch sections",
            composition.model.name
        );
        for _ in 0..2 {
            assert!(
                composition
                    .model
                    .available_pieces(state)
                    .iter()
                    .any(|piece| piece.id == PieceId::LaunchLong),
                "{} does not offer a launch at the powered return connector",
                composition.model.name
            );
            assert!(
                composition.model.allows_piece(launch_long),
                "{} does not allow a linear motor launch",
                composition.model.name,
            );
            append_piece(
                &mut pieces,
                &mut state,
                templates,
                PieceId::LaunchLong,
                None,
            );
        }
    }

    let return_path = level_return_path(state, &composition.model, templates);
    for id in return_path {
        append_piece(&mut pieces, &mut state, templates, id, None);
    }
    assert_eq!(state, TrackState::origin());
    (Circuit::new(pieces).unwrap(), element_ranges)
}

fn run_and_check(composition: Composition) -> Vec<String> {
    let (circuit, element_ranges) = build_composition(&composition);
    let mut run = CoasterRun::new(&circuit, &composition.model.train, PhysicsConfig::default());
    let outcome = (0..200_000)
        .find_map(|_| run.step(&circuit))
        .unwrap_or_else(|| panic!("{} test run did not finish", composition.name));
    let report = &outcome.reports[0];
    let mut failures = Vec::new();
    if !report.completed {
        let station_index = circuit
            .pieces()
            .iter()
            .position(|piece| piece.id == PieceId::Station)
            .unwrap();
        let station_start: f64 = circuit.pieces()[..station_index]
            .iter()
            .map(Piece::length)
            .sum();
        let station_distance = station_start + circuit.pieces()[station_index].length() / 2.0;
        let furthest_position =
            (station_distance + report.furthest_distance).rem_euclid(circuit.length());
        let mut piece_start = 0.0;
        let stopped_piece = circuit.pieces().iter().find_map(|piece| {
            let piece_end = piece_start + piece.length();
            let contains = (piece_start..piece_end).contains(&furthest_position);
            piece_start = piece_end;
            contains.then_some(piece.id)
        });
        failures.push(format!(
            "{} did not complete at {:.1} m in {stopped_piece:?}: {:?}",
            composition.name, report.furthest_distance, report.notes
        ));
    }

    let mut element_samples: Vec<_> = element_ranges
        .iter()
        .flat_map(|(_, range)| {
            report
                .samples
                .iter()
                .filter(move |sample| range.contains(&sample.distance))
        })
        .collect();
    element_samples.sort_by(|first, second| first.distance.total_cmp(&second.distance));
    if element_samples.is_empty() {
        failures.push(format!("{} had no element samples", composition.name));
        return failures;
    }
    let limits = composition.model.comfortable_force_limits;
    let minimum_vertical = element_samples
        .iter()
        .map(|sample| sample.vertical_g)
        .fold(f64::INFINITY, f64::min);
    let maximum_vertical = element_samples
        .iter()
        .map(|sample| sample.vertical_g)
        .fold(f64::NEG_INFINITY, f64::max);
    let maximum_lateral = element_samples
        .iter()
        .map(|sample| sample.lateral_g.abs())
        .fold(0.0, f64::max);
    let maximum_longitudinal = element_samples
        .iter()
        .map(|sample| sample.longitudinal_g.abs())
        .fold(0.0, f64::max);
    let force_violation = !(minimum_vertical >= limits.minimum_vertical_g
        && maximum_vertical <= limits.maximum_vertical_g
        && maximum_lateral <= limits.maximum_lateral_g
        && maximum_longitudinal <= limits.maximum_longitudinal_g);
    let inversion_matches =
        element_samples.iter().any(|sample| sample.inverted) == composition.requires_inversion;
    let starts_upright = !element_samples[0].inverted;
    let mut previous_inverted = false;
    let mut inversion_passages = 0;
    for sample in &element_samples {
        if sample.inverted && !previous_inverted {
            inversion_passages += 1;
        }
        previous_inverted = sample.inverted;
    }
    let inversion_count_matches = inversion_passages == composition.expected_inversion_passages;
    let catalog = catalog();
    for (id, range) in &element_ranges {
        let piece = catalog.iter().find(|piece| piece.id == *id).unwrap();
        let Some(rated) = piece.rated_speeds else {
            failures.push(format!(
                "{} {id:?} has no rated entry band",
                composition.name
            ));
            continue;
        };
        let sample = report
            .samples
            .iter()
            .filter(|sample| range.contains(&sample.distance))
            .min_by(|first, second| first.distance.total_cmp(&second.distance))
            .or_else(|| {
                failures.push(format!("{} had no {id:?} sample", composition.name));
                None
            });
        let Some(sample) = sample else {
            continue;
        };
        eprintln!(
            "{} {id:?} entry speed {:.2} m/s; rated {:.2}..{:.2} m/s",
            composition.name, sample.speed, rated.minimum_entry, rated.maximum_entry
        );
        if !(rated.minimum_entry..=rated.maximum_entry).contains(&sample.speed) {
            failures.push(format!(
                "{} {id:?} entered at {:.2} m/s, outside {:.2}..{:.2}",
                composition.name, sample.speed, rated.minimum_entry, rated.maximum_entry
            ));
        }
    }
    if force_violation {
        failures.push(format!(
            "{} exceeds force limits: vertical {minimum_vertical}..{maximum_vertical}, lateral {maximum_lateral}, longitudinal {maximum_longitudinal}; limits {limits:?}",
            composition.name
        ));
    }
    if !inversion_matches {
        failures.push(format!(
            "{} inverted-rider observation {}, expected {}",
            composition.name,
            element_samples.iter().any(|sample| sample.inverted),
            composition.requires_inversion
        ));
    }
    if !starts_upright {
        failures.push(format!(
            "{} entered its first inversion element already inverted",
            composition.name
        ));
    }
    if !inversion_count_matches {
        failures.push(format!(
            "{} had {inversion_passages} inversion passage(s), expected {}",
            composition.name, composition.expected_inversion_passages
        ));
    }
    failures
}

#[test]
fn lifted_and_launched_inversion_compositions_complete_within_force_limits() {
    use PieceId::*;
    let half_loop_compositions = [
        ("Immelmann", vec![HalfLoopUp, HalfCorkscrewLeftDown]),
        ("Dive loop", vec![HalfCorkscrewLeftUp, HalfLoopDown]),
        ("Raven turn", vec![HalfLoopUp, RavenRollLeftDown]),
        (
            "Sea serpent",
            vec![
                HalfLoopUp,
                RavenRollLeftDown,
                RavenRollRightUp,
                HalfLoopDown,
            ],
        ),
    ];
    let mut failures = Vec::new();
    for (name, elements) in half_loop_compositions {
        let (lift_gentle_pieces, lift_one_tile_pieces) = match name {
            "Dive loop" => (1, 1),
            _ => (4, 3),
        };
        let restoration = match name {
            "Immelmann" | "Dive loop" | "Raven turn" => HeightRestoration::RestoreToStartingHeight,
            "Sea serpent" => HeightRestoration::None,
            _ => unreachable!(),
        };
        let expected_inversion_passages = match name {
            "Sea serpent" => 2,
            "Immelmann" | "Dive loop" | "Raven turn" => 1,
            _ => unreachable!(),
        };
        failures.extend(run_and_check(Composition {
            name,
            model: hybrid_model(),
            lift_gentle_pieces,
            lift_one_tile_pieces,
            elements,
            launch: false,
            restoration,
            requires_inversion: true,
            expected_inversion_passages,
        }));
    }

    failures.extend(run_and_check(Composition {
        name: "lift into corkscrew",
        model: hybrid_model(),
        lift_gentle_pieces: 1,
        lift_one_tile_pieces: 1,
        elements: vec![Corkscrew],
        launch: false,
        restoration: HeightRestoration::None,
        requires_inversion: true,
        expected_inversion_passages: 1,
    }));

    for (name, model, lift_gentle_pieces, lift_one_tile_pieces, element) in [
        ("Timber Tumbler standard loop", hybrid_model(), 3, 2, Loop),
        ("Timber Tumbler large loop", hybrid_model(), 4, 3, LargeLoop),
        (
            "Briar Blitz steel loop",
            wooden_twister_model(),
            3,
            2,
            SteelLoop,
        ),
    ] {
        failures.extend(run_and_check(Composition {
            name,
            model,
            lift_gentle_pieces,
            lift_one_tile_pieces,
            elements: vec![element],
            launch: false,
            restoration: HeightRestoration::None,
            requires_inversion: true,
            expected_inversion_passages: 1,
        }));
    }
    failures.extend(run_and_check(Composition {
        name: "Velocity Viper launched large loop",
        model: multi_launch_model(),
        lift_gentle_pieces: 0,
        lift_one_tile_pieces: 0,
        elements: vec![LargeLoop],
        launch: true,
        restoration: HeightRestoration::None,
        requires_inversion: true,
        expected_inversion_passages: 1,
    }));
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn first_sixty_degree_drop_runs_into_its_pull_out_and_returns_to_station() {
    let failures = run_and_check(Composition {
        name: "60-degree first drop",
        model: hybrid_model(),
        lift_gentle_pieces: 4,
        lift_one_tile_pieces: 3,
        elements: vec![PieceId::LevelToSixtyDown, PieceId::SixtyDownToLevel],
        launch: false,
        restoration: HeightRestoration::LiftToStartingHeight,
        requires_inversion: false,
        expected_inversion_passages: 0,
    });
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn lifted_composition_enters_its_rated_loop_window() {
    for (name, model, lift_gentle_pieces, lift_one_tile_pieces, element, launch) in [
        (
            "Timber Tumbler standard loop",
            hybrid_model(),
            3,
            2,
            PieceId::Loop,
            false,
        ),
        (
            "Timber Tumbler large loop",
            hybrid_model(),
            4,
            3,
            PieceId::LargeLoop,
            false,
        ),
        (
            "Briar Blitz steel loop",
            wooden_twister_model(),
            3,
            2,
            PieceId::SteelLoop,
            false,
        ),
        (
            "Velocity Viper large loop",
            multi_launch_model(),
            0,
            0,
            PieceId::LargeLoop,
            true,
        ),
    ] {
        let (circuit, element_ranges) = build_composition(&Composition {
            name,
            model: model.clone(),
            lift_gentle_pieces,
            lift_one_tile_pieces,
            elements: vec![element],
            launch,
            restoration: HeightRestoration::None,
            requires_inversion: true,
            expected_inversion_passages: 1,
        });
        let element_range = &element_ranges[0].1;
        let rated = catalog()
            .iter()
            .find(|piece| piece.id == element)
            .unwrap()
            .rated_speeds
            .unwrap();
        let mut run = CoasterRun::new(&circuit, &model.train, PhysicsConfig::default());
        let outcome = (0..200_000)
            .find_map(|_| run.step(&circuit))
            .unwrap_or_else(|| panic!("{name} test run did not finish"));
        assert!(
            outcome.reports[0].completed,
            "{name} did not complete: furthest {:.1} m, notes {:?}",
            outcome.reports[0].furthest_distance, outcome.reports[0].notes
        );
        let entry_sample = outcome.reports[0]
            .samples
            .iter()
            .filter(|sample| element_range.contains(&sample.distance))
            .min_by(|first, second| first.distance.total_cmp(&second.distance))
            .unwrap_or_else(|| panic!("{name} has no loop sample"));
        assert!(
            (rated.minimum_entry..=rated.maximum_entry).contains(&entry_sample.speed),
            "{name} entered at {:.2} m/s, outside {:.2}..{:.2}",
            entry_sample.speed,
            rated.minimum_entry,
            rated.maximum_entry
        );
    }
}
