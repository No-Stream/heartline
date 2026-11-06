use theme_park_sim::physics::{
    Circuit, CoasterOutcome, CoasterRun, ConstructionTestRunRecorder, PhysicsConfig, TestRun,
    Train, demo_circuit,
};
use theme_park_sim::track::construction::Construction;
use theme_park_sim::track::{
    CoasterModel, ComfortableForceLimits, Heading, Heartlining, PieceFamily, PieceId, PieceKind,
    TrackState, catalog, effective_rated_speeds, family_coaster_model, hybrid_model,
    multi_launch_model, wooden_twister_model,
};
use theme_park_sim::world::{Surface, World};

fn player_circuit(name: &str, model: CoasterModel) -> Circuit {
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
        heading: Heading::North,
        ..TrackState::origin()
    };
    let mut construction = Construction::new(start, model);
    construction.append(PieceId::Station, &world, &[]).unwrap();
    let text = include_str!("../../../web/e2e/player/circuits.txt");
    let section = text
        .split(&format!("[{name}]"))
        .nth(1)
        .unwrap_or_else(|| panic!("missing {name}"));
    for line in section
        .lines()
        .map(str::trim)
        .take_while(|line| !line.starts_with('['))
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
    {
        match line {
            "+chain" => construction.set_chain(true),
            "-chain" => construction.set_chain(false),
            piece => {
                let id = catalog()
                    .iter()
                    .find(|candidate| format!("{:?}", candidate.id) == piece)
                    .unwrap_or_else(|| panic!("unknown piece {piece}"))
                    .id;
                construction.append(id, &world, &[]).unwrap();
            }
        }
    }
    construction.close().unwrap();
    construction.circuit().unwrap()
}

fn assert_design_margins(circuit: &Circuit, model: &CoasterModel, construction_test: bool) {
    let start = if construction_test {
        0.0
    } else {
        circuit.pieces()[0].length() / 2.0
    };
    let config = if construction_test {
        PhysicsConfig {
            lift_speed: 5.0,
            ..PhysicsConfig::default()
        }
    } else {
        PhysicsConfig::default()
    };
    let mut train = Train::new(
        circuit,
        &model.train,
        config,
        start,
        if construction_test { 2.0 } else { 0.0 },
    );
    let mut entries = vec![vec![None; model.train.cars as usize]; circuit.pieces().len()];
    for _ in 0..60_000 {
        train.step(circuit);
        for frame in train.take_readings() {
            for (car, reading) in frame.iter().enumerate() {
                let car_distance = train.distance()
                    - car as f64 * model.train.length_metres / f64::from(model.train.cars);
                if car_distance < 0.0 {
                    continue;
                }
                entries[reading.piece_index][car].get_or_insert(reading.speed);
            }
        }
        assert!(
            train.notes().is_empty(),
            "train recovered: {:?}",
            train.notes()
        );
        if train.distance() >= circuit.length() + start {
            break;
        }
    }
    assert!(
        train.distance() >= circuit.length() + start,
        "train did not complete its lap"
    );
    for (piece, speeds) in circuit.pieces().iter().zip(entries) {
        if let Some(window) = effective_rated_speeds(piece, &model.train) {
            for speed in speeds {
                let speed = speed.expect("every car should enter every rated piece");
                // Windows narrower than 3 m/s cannot provide 1.5 m/s on both sides;
                // keep their entry within 0.3 m/s of the centre instead.
                if window.maximum_entry - window.minimum_entry < 3.0 {
                    let centre = (window.minimum_entry + window.maximum_entry) / 2.0;
                    assert!(
                        (speed - centre).abs() <= 0.3,
                        "{:?}: {speed:.3} m/s, centre {centre:.3}",
                        piece.id
                    );
                } else {
                    assert!(
                        speed >= window.minimum_entry + 1.5 && speed <= window.maximum_entry - 1.5,
                        "{:?}: {speed:.3} m/s, window {window:?}",
                        piece.id
                    );
                }
            }
        }
    }
}

fn construction_test(circuit: &Circuit, model: &CoasterModel) -> TestRun {
    ride_test(circuit, model, 5.0, 2.0)
}

fn ride_test(
    circuit: &Circuit,
    model: &CoasterModel,
    lift_speed: f64,
    initial_speed: f64,
) -> TestRun {
    let start_distance = if initial_speed == 0.0 {
        circuit.pieces()[0].length() / 2.0
    } else {
        0.0
    };
    let mut train = Train::new(
        circuit,
        &model.train,
        PhysicsConfig {
            lift_speed,
            ..PhysicsConfig::default()
        },
        start_distance,
        initial_speed,
    );
    let mut recorder = ConstructionTestRunRecorder::for_circuit_with_train_and_limits(
        circuit,
        &model.train,
        model.comfortable_force_limits,
    );
    let mut slow_steps = 0;
    for _ in 0..60_000 {
        train.step(circuit);
        recorder.record(&train.take_readings());
        assert!(
            train.notes().is_empty(),
            "test train failed: {:?}",
            train.notes()
        );
        if train.speed().abs() < 0.05 {
            slow_steps += 1;
            assert!(
                slow_steps < 200,
                "test train stopped before completing a lap"
            );
        } else {
            slow_steps = 0;
        }
        if train.distance() >= circuit.length() + start_distance {
            return recorder.finish(true, false);
        }
    }
    panic!("test train did not complete a lap");
}

fn default_test_ride(circuit: &Circuit, model: &CoasterModel) -> CoasterOutcome {
    let mut run = CoasterRun::new(circuit, &model.train, PhysicsConfig::default());
    (0..60_000)
        .find_map(|_| run.step(circuit))
        .expect("Timber Tumbler default-physics ride did not finish")
}

fn assert_timber_margins(circuit: &Circuit, model: &CoasterModel, construction: bool) {
    // TASK-126 will widen hill windows; TASK-128's heart-speed change is expected to
    // shift speeds by a few percent (about 0.6 m/s at 20 m/s), below this margin.
    // The force margin also leaves the brief-excursion allowance clearly unused.
    let speed_margin = 1.5;
    let force_margin = 0.25;
    let start = if construction {
        0.0
    } else {
        circuit.pieces()[0].length() / 2.0
    };
    let config = PhysicsConfig {
        lift_speed: if construction {
            5.0
        } else {
            PhysicsConfig::default().lift_speed
        },
        ..PhysicsConfig::default()
    };
    let mut train = Train::new(
        circuit,
        &model.train,
        config,
        start,
        if construction { 2.0 } else { 0.0 },
    );
    let mut entries = vec![vec![false; model.train.cars as usize]; circuit.pieces().len()];
    let limits = model.comfortable_force_limits;
    let mut minimum_speed_margin = f64::INFINITY;
    let mut minimum_force_margin = f64::INFINITY;
    for _ in 0..60_000 {
        train.step(circuit);
        for frame in train.take_readings() {
            for (car, reading) in frame.iter().enumerate() {
                let car_distance = train.distance()
                    - car as f64 * model.train.length_metres / f64::from(model.train.cars);
                if car_distance < 0.0 {
                    continue;
                }
                let piece = &circuit.pieces()[reading.piece_index];
                if !entries[reading.piece_index][car] {
                    entries[reading.piece_index][car] = true;
                    if let Some(window) = effective_rated_speeds(piece, &model.train) {
                        minimum_speed_margin = minimum_speed_margin.min(
                            (reading.speed - window.minimum_entry)
                                .min(window.maximum_entry - reading.speed),
                        );
                        assert!(
                            reading.speed >= window.minimum_entry + speed_margin
                                && reading.speed <= window.maximum_entry - speed_margin,
                            "construction={construction} car {car} {:?} entry {} window {}..{}",
                            piece.id,
                            reading.speed,
                            window.minimum_entry,
                            window.maximum_entry
                        );
                    }
                }
                minimum_force_margin = minimum_force_margin.min(
                    (limits.maximum_vertical_g - reading.g.vertical)
                        .min(reading.g.vertical - limits.minimum_vertical_g)
                        .min(limits.maximum_lateral_g - reading.g.lateral.abs())
                        .min(limits.maximum_longitudinal_g - reading.g.longitudinal.abs()),
                );
                assert!(
                    reading.g.vertical <= limits.maximum_vertical_g - force_margin
                        && reading.g.vertical >= limits.minimum_vertical_g + force_margin
                        && reading.g.lateral.abs() <= limits.maximum_lateral_g - force_margin
                        && reading.g.longitudinal.abs()
                            <= limits.maximum_longitudinal_g - force_margin,
                    "construction={construction} car {car} {:?} forces {:?}",
                    piece.id,
                    reading.g
                );
            }
        }
        assert!(
            train.notes().is_empty(),
            "train recovered: {:?}",
            train.notes()
        );
        if train.distance() >= circuit.length() + start {
            eprintln!(
                "construction={construction} minimum speed margin {minimum_speed_margin} m/s; minimum force margin {minimum_force_margin} g"
            );
            for (index, piece) in circuit.pieces().iter().enumerate() {
                if effective_rated_speeds(piece, &model.train).is_some() {
                    assert!(
                        entries[index].iter().all(|seen| *seen),
                        "missing car entry for {:?}",
                        piece.id
                    );
                }
            }
            return;
        }
    }
    panic!("train did not finish");
}

fn assert_timber_tumbler_report(run_name: &str, report: &TestRun, limits: ComfortableForceLimits) {
    assert!(report.completed, "{run_name} ride did not complete");
    let stats = report.statistics;
    assert!(
        stats.max_positive_vertical_g <= limits.maximum_vertical_g,
        "{run_name} positive vertical force: {} g",
        stats.max_positive_vertical_g
    );
    assert!(
        stats.max_negative_vertical_g >= limits.minimum_vertical_g,
        "{run_name} negative vertical force: {} g",
        stats.max_negative_vertical_g
    );
    assert!(
        stats.max_lateral_g <= limits.maximum_lateral_g,
        "{run_name} lateral force: {} g",
        stats.max_lateral_g
    );
    assert!(
        stats.max_forward_g <= limits.maximum_longitudinal_g,
        "{run_name} forward force: {} g",
        stats.max_forward_g
    );
    assert!(
        stats.max_braking_g <= limits.maximum_longitudinal_g,
        "{run_name} braking force: {} g",
        stats.max_braking_g
    );
    assert!(
        report
            .notes
            .iter()
            .all(|note| !note.contains("rated speed band")),
        "{run_name} ride has a rated speed note: {:?}",
        report.notes
    );
}

#[test]
fn velocity_viper_completes_with_comfortable_forces() {
    let model = multi_launch_model();
    let circuit = player_circuit("Velocity Viper", model.clone());
    for (lift_speed, initial_speed) in [(PhysicsConfig::default().lift_speed, 0.0), (5.0, 2.0)] {
        let report = ride_test(&circuit, &model, lift_speed, initial_speed);
        let stats = report.statistics;
        let limits = model.comfortable_force_limits;
        assert!(
            (6.0..=9.0).contains(&report.excitement),
            "excitement: {}",
            report.excitement
        );
        assert!(
            stats.max_positive_vertical_g <= limits.maximum_vertical_g - 0.25,
            "{stats:?}"
        );
        assert!(
            stats.max_negative_vertical_g >= limits.minimum_vertical_g + 0.25,
            "{stats:?}"
        );
        assert!(
            stats.max_lateral_g <= limits.maximum_lateral_g - 0.25,
            "{stats:?}"
        );
        assert!(
            stats.max_forward_g <= limits.maximum_longitudinal_g - 0.25,
            "{stats:?}"
        );
        assert!(
            stats.max_braking_g.abs() <= limits.maximum_longitudinal_g - 0.25,
            "{stats:?}"
        );
        assert!(
            !report
                .notes
                .iter()
                .any(|note| note.contains("rated speed band")),
            "{:?}",
            report.notes
        );
        assert_eq!(stats.launches, 2);
        assert_design_margins(&circuit, &model, initial_speed != 0.0);
    }
}

#[test]
fn daisy_dash_is_a_gentle_family_ride_under_both_physics_configs() {
    let model = family_coaster_model();
    let circuit = player_circuit("Daisy Dash", model.clone());
    let pieces = circuit.pieces();
    let family_hills = pieces
        .iter()
        .filter(|piece| matches!(piece.id, PieceId::FamilyHill3x32 | PieceId::FamilyHill4x40))
        .count();
    let speed_matched_turns = pieces
        .iter()
        .filter(|piece| matches!(piece.family, Some(PieceFamily::SpeedTurn { .. })))
        .count();
    assert!(family_hills >= 2, "family hills: {family_hills}");
    assert!(
        speed_matched_turns >= 3,
        "speed-matched turns: {speed_matched_turns}"
    );
    assert!(
        pieces
            .iter()
            .any(|piece| piece.kind == PieceKind::Lift && piece.entry.heading != piece.exit.heading),
        "the chain lift curves"
    );
    for (lift_speed, initial_speed) in [(PhysicsConfig::default().lift_speed, 0.0), (5.0, 2.0)] {
        let report = ride_test(&circuit, &model, lift_speed, initial_speed);
        let stats = report.statistics;
        let limits = model.comfortable_force_limits;
        eprintln!(
            "Daisy Dash at lift speed {lift_speed}: excitement {:.2}, intensity {:.2}, nausea {:.2}; stats {stats:?}",
            report.excitement, report.intensity, report.nausea
        );
        assert!(report.completed);
        assert!(
            (2.5..=4.5).contains(&report.excitement),
            "excitement: {}",
            report.excitement
        );
        assert!(
            stats.max_positive_vertical_g <= limits.maximum_vertical_g
                && stats.max_negative_vertical_g >= limits.minimum_vertical_g
                && stats.max_lateral_g <= limits.maximum_lateral_g
                && stats.max_forward_g <= limits.maximum_longitudinal_g
                && stats.max_braking_g.abs() <= limits.maximum_longitudinal_g,
            "{stats:?}"
        );
        assert_eq!(stats.inversions, 0);
        assert_eq!(stats.launches, 0);
        assert!(stats.airtime_moments >= 2, "{stats:?}");
        assert!(
            !report
                .notes
                .iter()
                .any(|note| note.contains("rated speed band")),
            "{:?}",
            report.notes
        );
        assert_design_margins(&circuit, &model, initial_speed != 0.0);
    }
}

#[test]
fn construction_reports_known_circuit_features() {
    let timber = hybrid_model();
    let velocity = multi_launch_model();
    let briar = wooden_twister_model();
    let circuits = [
        ("demo", demo_circuit(), &timber),
        (
            "Velocity Viper",
            player_circuit("Velocity Viper", velocity.clone()),
            &velocity,
        ),
        (
            "Timber Tumbler",
            player_circuit("Timber Tumbler", timber.clone()),
            &timber,
        ),
        (
            "Briar Blitz",
            player_circuit("Briar Blitz", briar.clone()),
            &briar,
        ),
    ];
    for (name, circuit, model) in circuits {
        let report = construction_test(&circuit, model);
        let stats = report.statistics;
        eprintln!(
            "{name}: excitement {:.2}, intensity {:.2}, nausea {:.2}; stats {stats:?}",
            report.excitement, report.intensity, report.nausea
        );
        assert!(report.completed, "{name}");
        assert!(
            (stats.track_length - circuit.length()).abs() < 1e-6,
            "{name}"
        );
        assert!(
            stats.duration_seconds > 0.0 && stats.max_speed > 0.0,
            "{name}"
        );
        assert!(stats.biggest_drop > 0.0 && stats.drops > 0, "{name}");
        if name == "Velocity Viper" {
            // The long launch and short launch form two separate launch sections.
            assert_eq!(stats.launches, 2);
            assert!(stats.launch_peak_speed > 0.0);
        }
        if name == "Briar Blitz" {
            // The current layout has a top hat and tight turns, but no inversion pieces.
            assert_eq!(stats.inversions, 0);
            assert_eq!(stats.inverted_seconds, 0.0);
        }
        if name == "Timber Tumbler" {
            assert!(
                stats.inversions >= 2,
                "Timber inversions: {}",
                stats.inversions
            );
            assert!(
                stats.airtime_moments >= 4,
                "Timber airtime moments: {}",
                stats.airtime_moments
            );
        }
    }
}

#[test]
fn timber_tumbler_meets_player_ride_targets_under_both_physics_configs() {
    let model = hybrid_model();
    let circuit = player_circuit("Timber Tumbler", model.clone());
    assert_timber_margins(&circuit, &model, false);
    assert_timber_margins(&circuit, &model, true);
    let heartlined_inversions = circuit
        .pieces()
        .iter()
        .filter(|piece| {
            piece.inversion.is_some() && piece.heartlining == Some(Heartlining::Heartlined)
        })
        .count();
    assert!(
        heartlined_inversions >= 2,
        "Timber Tumbler has {heartlined_inversions} heartlined inversion pieces"
    );

    let default_outcome = default_test_ride(&circuit, &model);
    assert!(!default_outcome.recovered, "default-physics ride recovered");
    assert!(!default_outcome.reports.is_empty());
    let default_excitement = default_outcome
        .reports
        .iter()
        .map(|report| report.excitement)
        .sum::<f64>()
        / default_outcome.reports.len() as f64;
    assert!(
        (8.0..=10.0).contains(&default_excitement),
        "default-physics Timber Tumbler excitement: {default_excitement}"
    );
    for (car_index, report) in default_outcome.reports.iter().enumerate() {
        assert_timber_tumbler_report(
            &format!("default physics car {car_index}"),
            report,
            model.comfortable_force_limits,
        );
    }
    assert!(
        default_outcome.reports[0].statistics.inversions >= 2,
        "default-physics inversions: {}",
        default_outcome.reports[0].statistics.inversions
    );
    assert!(
        default_outcome.reports[0].statistics.airtime_moments >= 4,
        "default-physics airtime moments: {}",
        default_outcome.reports[0].statistics.airtime_moments
    );

    let construction_report = construction_test(&circuit, &model);
    assert_timber_tumbler_report(
        "construction-test physics",
        &construction_report,
        model.comfortable_force_limits,
    );
    assert!(
        (8.0..=10.0).contains(&construction_report.excitement),
        "construction-test Timber Tumbler excitement: {}",
        construction_report.excitement
    );
    assert!(
        construction_report.statistics.inversions >= 2,
        "construction-test physics inversions: {}",
        construction_report.statistics.inversions
    );
    assert!(
        construction_report.statistics.airtime_moments >= 4,
        "construction-test physics airtime moments: {}",
        construction_report.statistics.airtime_moments
    );
}

#[test]
fn demo_and_velocity_match_provisional_excitement_references() {
    let demo_model = hybrid_model();
    let velocity_model = multi_launch_model();
    let demo = construction_test(&demo_circuit(), &demo_model);
    let velocity = construction_test(
        &player_circuit("Velocity Viper", velocity_model.clone()),
        &velocity_model,
    );
    // The current scale includes penalties for the demo's measured excessive forces.
    assert!(
        (0.4..=0.7).contains(&demo.excitement),
        "demo excitement: {}",
        demo.excitement
    );
    // The force-designed player circuit targets a positive score in its calibration band.
    assert!(
        (6.0..=9.0).contains(&velocity.excitement),
        "Velocity Viper excitement: {}",
        velocity.excitement
    );
}
