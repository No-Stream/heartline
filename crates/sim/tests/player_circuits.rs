use theme_park_sim::physics::{
    CoasterRun, PhysicsConfig, PreviewStatus, demo_circuit, preview_open_track,
};
use theme_park_sim::track::construction::Construction;
use theme_park_sim::track::{
    PieceId, PieceKind, TrackState, catalog, family_coaster_model, hybrid_model,
    multi_launch_model, wooden_twister_model,
};
use theme_park_sim::world::{Surface, World};

#[test]
fn player_circuits_close() {
    for (name, model, requires_lift) in [
        ("Timber Tumbler", hybrid_model(), true),
        ("Velocity Viper", multi_launch_model(), false),
        ("Briar Blitz", wooden_twister_model(), true),
        ("Daisy Dash", family_coaster_model(), true),
    ] {
        let world = World::new(
            512,
            512,
            Surface {
                corners: [8; 4],
                water: None,
            },
        );
        let start = TrackState {
            x: 482,
            y: 498,
            z: 6,
            heading: theme_park_sim::track::Heading::North,
            ..TrackState::origin()
        };
        let mut construction = Construction::new(start, model);
        construction.append(PieceId::Station, &world, &[]).unwrap();
        let header = format!("[{name}]");
        let mut sections = include_str!("../../../web/e2e/player/circuits.txt").split(&header);
        sections.next().unwrap();
        let circuit = sections
            .next()
            .unwrap_or_else(|| panic!("{name} circuit missing"));
        assert!(sections.next().is_none(), "duplicate {name} circuit");
        let pieces = catalog();
        let mut count = 0;
        let mut chain_piece_count = 0;
        for name in circuit
            .lines()
            .map(str::trim)
            .take_while(|line| !line.starts_with('['))
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
        {
            match name {
                "+chain" => {
                    construction.set_chain(true);
                    continue;
                }
                "-chain" => {
                    construction.set_chain(false);
                    continue;
                }
                _ => {}
            }
            let piece = pieces
                .iter()
                .find(|piece| format!("{:?}", piece.id) == name)
                .unwrap_or_else(|| panic!("Unknown piece {name}"));
            construction
                .append(piece.id, &world, &[])
                .unwrap_or_else(|error| panic!("{name}: {error:?}"));
            if construction.pieces().last().unwrap().kind == PieceKind::Lift {
                chain_piece_count += 1;
            }
            count += 1;
        }
        assert!(count > 0);
        assert!(
            !requires_lift || chain_piece_count > 0,
            "{name} circuit has no chain-lift pieces"
        );
        let end = construction.end();
        construction.close().unwrap_or_else(|error| {
            panic!("{name} circuit does not close from {end:?}: {error:?}")
        });
    }
}

#[test]
fn daisy_dash_completes_a_test_ride_lap() {
    let model = family_coaster_model();
    let circuit = build_player_circuit("Daisy Dash", model.clone());
    let mut run = CoasterRun::new(&circuit, &model.train, PhysicsConfig::default());
    let outcome = (0..200_000)
        .find_map(|_| run.step(&circuit))
        .expect("Daisy Dash test ride did not finish");
    let report = &outcome.reports[0];

    assert!(
        report.completed && !outcome.recovered,
        "Daisy Dash did not complete a lap: {:?}",
        report.notes
    );
}

#[test]
fn timber_tumbler_completes_a_test_ride_lap() {
    let model = hybrid_model();
    let circuit = build_player_circuit("Timber Tumbler", model.clone());
    let mut run = CoasterRun::new(&circuit, &model.train, PhysicsConfig::default());
    let outcome = (0..200_000)
        .find_map(|_| run.step(&circuit))
        .expect("Timber Tumbler test ride did not finish");
    let report = &outcome.reports[0];

    assert!(
        report.completed && !outcome.recovered,
        "Timber Tumbler did not complete a lap: {:?}",
        report.notes
    );
}

fn build_player_circuit(
    name: &str,
    model: theme_park_sim::track::CoasterModel,
) -> theme_park_sim::physics::Circuit {
    let world = World::new(
        512,
        512,
        Surface {
            corners: [8; 4],
            water: None,
        },
    );
    let start = TrackState {
        x: 482,
        y: 498,
        z: 6,
        heading: theme_park_sim::track::Heading::North,
        ..TrackState::origin()
    };
    let mut construction = Construction::new(start, model.clone());
    construction.append(PieceId::Station, &world, &[]).unwrap();
    let circuit_text = include_str!("../../../web/e2e/player/circuits.txt");
    let section = circuit_text
        .split(&format!("[{name}]"))
        .nth(1)
        .unwrap_or_else(|| panic!("{name} circuit missing"));
    for name in section
        .lines()
        .map(str::trim)
        .take_while(|line| !line.starts_with('['))
    {
        if name.is_empty() || name.starts_with('#') {
            continue;
        }
        if name == "+chain" {
            construction.set_chain(true);
            continue;
        }
        if name == "-chain" {
            construction.set_chain(false);
            continue;
        }
        let id = catalog()
            .iter()
            .find(|piece| format!("{:?}", piece.id) == name)
            .unwrap()
            .id;
        construction.append(id, &world, &[]).unwrap();
    }
    construction.close().unwrap();
    construction.circuit().unwrap()
}

#[test]
fn player_journeys_build_and_ride_both_top_hats() {
    for (name, model, top_hat_id) in [
        (
            "Velocity Viper",
            multi_launch_model(),
            PieceId::LaunchedTopHat,
        ),
        ("Briar Blitz", wooden_twister_model(), PieceId::TopHat),
    ] {
        let circuit = build_player_circuit(name, model.clone());
        let top_hat_index = circuit
            .pieces()
            .iter()
            .position(|piece| piece.id == top_hat_id)
            .unwrap_or_else(|| panic!("{name} journey has no {top_hat_id:?}"));
        if name == "Velocity Viper" {
            assert_eq!(circuit.pieces()[top_hat_index - 1].kind, PieceKind::Launch);
        }
        let top_hat_start: f64 = circuit.pieces()[..top_hat_index]
            .iter()
            .map(|piece| piece.length())
            .sum();
        let top_hat = &circuit.pieces()[top_hat_index];
        let top_hat_end = top_hat_start + top_hat.length();
        let mut run = CoasterRun::new(&circuit, &model.train, PhysicsConfig::default());
        let outcome = (0..200_000)
            .find_map(|_| run.step(&circuit))
            .unwrap_or_else(|| panic!("{name} test ride did not finish"));
        let report = &outcome.reports[0];
        assert!(
            report.completed && !outcome.recovered,
            "{name}: {:?}",
            report.notes
        );
        assert!(
            report.furthest_distance >= top_hat_end,
            "{name} did not clear its top hat"
        );
        let entry_sample = report
            .samples
            .iter()
            .min_by(|left, right| {
                (left.distance - top_hat_start)
                    .abs()
                    .total_cmp(&(right.distance - top_hat_start).abs())
            })
            .unwrap();
        let rated = top_hat.rated_speeds.unwrap();
        assert!(
            (rated.minimum_entry..=rated.maximum_entry).contains(&entry_sample.speed),
            "{name} top-hat entry speed {} outside {rated:?}",
            entry_sample.speed
        );
    }
}

#[test]
fn test_rides_keep_recorded_longitudinal_forces_physically_plausible() {
    let circuits = [
        ("Timber Tumbler", hybrid_model()),
        ("Velocity Viper", multi_launch_model()),
        ("Briar Blitz", wooden_twister_model()),
        ("Daisy Dash", family_coaster_model()),
    ];
    for (name, model) in circuits {
        let circuit = build_player_circuit(name, model.clone());
        assert_ride_force_bound(name, &circuit, &model);
    }
    let model = hybrid_model();
    let circuit = demo_circuit();
    assert_ride_force_bound("demo", &circuit, &model);
}

#[test]
fn timber_preview_agrees_with_closed_test_run() {
    let model = hybrid_model();
    let circuit = build_player_circuit("Timber Tumbler", model.clone());
    let preview =
        preview_open_track(circuit.pieces(), &model.train, PhysicsConfig::default()).unwrap();
    let mut run = CoasterRun::new(&circuit, &model.train, PhysicsConfig::default());
    let outcome = (0..200_000).find_map(|_| run.step(&circuit)).unwrap();
    assert_eq!(
        preview.status,
        if outcome.recovered {
            PreviewStatus::Stalled
        } else {
            PreviewStatus::ReachedEnd
        }
    );
    for sample in &preview.report.samples {
        let recorded = outcome.reports[0]
            .samples
            .iter()
            .find(|other| other.distance == sample.distance)
            .unwrap();
        assert!((sample.speed - recorded.speed).abs() < 0.5);
        assert!((sample.vertical_g - recorded.vertical_g).abs() < 0.3);
        assert!((sample.lateral_g - recorded.lateral_g).abs() < 0.3);
        assert!(
            (sample.longitudinal_g - recorded.longitudinal_g).abs() < 0.3,
            "preview {sample:?} run {recorded:?} piece {:?}",
            circuit.pieces()[sample.piece_index].id
        );
    }
    assert!(!preview.report.samples.is_empty());
    if outcome.recovered {
        assert!(
            (preview.report.furthest_distance - outcome.reports[0].furthest_distance).abs() < 1.0
        );
    }
}

fn assert_ride_force_bound(
    name: &str,
    circuit: &theme_park_sim::physics::Circuit,
    model: &theme_park_sim::track::CoasterModel,
) {
    let mut run = CoasterRun::new(circuit, &model.train, PhysicsConfig::default());
    let outcome = (0..60_000).find_map(|_| run.step(circuit));
    let outcome = outcome.unwrap_or_else(|| panic!("{name} test ride did not finish"));
    let maximum_sample = outcome
        .reports
        .iter()
        .flat_map(|report| &report.samples)
        .max_by(|a, b| a.longitudinal_g.abs().total_cmp(&b.longitudinal_g.abs()))
        .unwrap();
    let maximum = maximum_sample.longitudinal_g.abs();
    assert!(
        maximum <= 2.5,
        "{name} peak longitudinal force was {maximum:.1} g at {maximum_sample:?}, piece {:?}",
        circuit.pieces()[maximum_sample.piece_index].id
    );
}

#[test]
fn construction_test_rides_keep_raw_longitudinal_forces_physically_plausible() {
    let circuits = [
        ("Timber Tumbler", hybrid_model()),
        ("Velocity Viper", multi_launch_model()),
        ("Briar Blitz", wooden_twister_model()),
        ("Daisy Dash", family_coaster_model()),
    ];
    for (name, model) in circuits {
        let circuit = build_player_circuit(name, model.clone());
        assert_construction_test_force_bound(name, &circuit, &model);
    }
    let model = hybrid_model();
    let circuit = demo_circuit();
    assert_construction_test_force_bound("demo", &circuit, &model);
}

fn assert_construction_test_force_bound(
    name: &str,
    circuit: &theme_park_sim::physics::Circuit,
    model: &theme_park_sim::track::CoasterModel,
) {
    let mut train = theme_park_sim::physics::Train::new(
        circuit,
        &model.train,
        PhysicsConfig {
            lift_speed: 5.0,
            ..PhysicsConfig::default()
        },
        0.0,
        2.0,
    );
    let mut completed = false;
    for _ in 0..60_000 {
        let previous_speed = train.speed();
        train.step(circuit);
        for frame in train.take_readings() {
            for reading in frame {
                let piece_index = circuit.piece_index(reading.distance);
                assert!(
                    reading.g.longitudinal.abs() <= 2.5,
                    "{name}: {:.3} g on piece {piece_index} {:?}, speed {previous_speed:.6} -> {:.6} m/s",
                    reading.g.longitudinal,
                    circuit.pieces()[piece_index].kind,
                    train.speed()
                );
            }
        }
        assert!(train.notes().is_empty(), "{name} train recovered");
        if train.distance() >= circuit.length() {
            completed = true;
            break;
        }
    }
    assert!(completed, "{name} construction test ride did not finish");
}

#[test]
fn chain_lift_brakes_a_fast_train_at_a_physical_rate() {
    let model = multi_launch_model();
    let world = World::new(
        512,
        512,
        Surface {
            corners: [8; 4],
            water: None,
        },
    );
    let start = TrackState {
        x: 482,
        y: 498,
        z: 6,
        heading: theme_park_sim::track::Heading::North,
        ..TrackState::origin()
    };
    let mut construction = Construction::new(start, model.clone());
    for piece in [PieceId::Station, PieceId::LaunchLong, PieceId::LaunchLong] {
        construction.append(piece, &world, &[]).unwrap();
    }
    construction.set_chain(true);
    for piece in [
        PieceId::LevelToGentleUp,
        PieceId::GentleUp,
        PieceId::GentleUp,
        PieceId::GentleUpToLevel,
    ] {
        construction.append(piece, &world, &[]).unwrap();
    }
    let circuit = theme_park_sim::physics::Circuit::open(construction.pieces().to_vec()).unwrap();
    let station_distance = circuit.pieces()[0].length() / 2.0;
    let mut train = theme_park_sim::physics::Train::new(
        &circuit,
        &model.train,
        PhysicsConfig::default(),
        station_distance,
        0.0,
    );
    let mut maximum = 0.0_f64;
    let mut saw_fast_train_on_lift = false;
    for _ in 0..60_000 {
        let previous_speed = train.speed();
        train.step(&circuit);
        let piece_index = circuit.piece_index(train.distance());
        if circuit.pieces()[piece_index].kind == PieceKind::Lift
            && previous_speed > PhysicsConfig::default().lift_speed
        {
            saw_fast_train_on_lift = true;
        }
        for frame in train.take_readings() {
            for reading in frame {
                maximum = maximum.max(reading.g.longitudinal.abs());
            }
        }
        if train.distance() >= circuit.length() || !train.notes().is_empty() {
            break;
        }
    }
    assert!(
        saw_fast_train_on_lift,
        "The launched train did not enter the chain lift above lift speed"
    );
    assert!(maximum <= 2.5, "Peak longitudinal force was {maximum:.1} g");
}
