use theme_park_sim::physics::{
    Circuit, ConstructionTestRunRecorder, PhysicsConfig, TestRun, Train, demo_circuit,
};
use theme_park_sim::track::construction::Construction;
use theme_park_sim::track::{
    CoasterModel, Heading, PieceId, TrackState, catalog, hybrid_model, multi_launch_model,
    wooden_twister_model,
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

#[test]
fn velocity_viper_completes_with_comfortable_forces() {
    let model = multi_launch_model();
    let circuit = player_circuit("Velocity Viper", model.clone());
    for (lift_speed, initial_speed) in [(PhysicsConfig::default().lift_speed, 0.0), (5.0, 2.0)] {
        let report = ride_test(&circuit, &model, lift_speed, initial_speed);
        let stats = report.statistics;
        let limits = model.comfortable_force_limits;
        assert!((6.0..=9.0).contains(&report.excitement), "{report:?}");
        assert!(
            stats.max_positive_vertical_g <= limits.maximum_vertical_g,
            "{stats:?}"
        );
        assert!(
            stats.max_negative_vertical_g >= limits.minimum_vertical_g,
            "{stats:?}"
        );
        assert!(stats.max_lateral_g <= limits.maximum_lateral_g, "{stats:?}");
        assert!(
            stats.max_forward_g <= limits.maximum_longitudinal_g,
            "{stats:?}"
        );
        assert!(
            stats.max_braking_g.abs() <= limits.maximum_longitudinal_g,
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
            // Sustained excessive forces make this a reference above the soft scale's 10-point mark.
            assert!(report.intensity > 10.0, "intensity: {}", report.intensity);
        }
    }
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
