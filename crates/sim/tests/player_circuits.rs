use theme_park_sim::physics::{CoasterRun, PhysicsConfig, demo_circuit};
use theme_park_sim::track::construction::Construction;
use theme_park_sim::track::{
    PieceId, PieceKind, TrackState, catalog, hybrid_model, multi_launch_model, wooden_twister_model,
};
use theme_park_sim::world::{Surface, World};

#[test]
fn player_circuits_close() {
    for (name, model, requires_lift) in [
        ("Timber Tumbler", hybrid_model(), true),
        ("Velocity Viper", multi_launch_model(), false),
        ("Briar Blitz", wooden_twister_model(), true),
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
fn test_rides_keep_recorded_longitudinal_forces_physically_plausible() {
    let circuits = [
        ("Timber Tumbler", hybrid_model()),
        ("Velocity Viper", multi_launch_model()),
        ("Briar Blitz", wooden_twister_model()),
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
fn velocity_viper_construction_test_keeps_raw_longitudinal_force_bounded() {
    let model = multi_launch_model();
    let circuit = build_player_circuit("Velocity Viper", model.clone());
    let mut train = theme_park_sim::physics::Train::new(
        &circuit,
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
        train.step(&circuit);
        let piece_index = circuit.piece_index(train.distance());
        for frame in train.take_readings() {
            for reading in frame {
                assert!(
                    reading.g.longitudinal.abs() <= 2.5,
                    "Velocity Viper: {:.3} g on piece {piece_index} {:?}, speed {previous_speed:.6} -> {:.6} m/s",
                    reading.g.longitudinal,
                    circuit.pieces()[piece_index].kind,
                    train.speed()
                );
            }
        }
        assert!(train.notes().is_empty(), "Velocity Viper train recovered");
        if train.distance() >= circuit.length() {
            completed = true;
            break;
        }
    }
    assert!(completed, "Velocity Viper test ride did not finish");
}

fn assert_ride_force_bound(
    name: &str,
    circuit: &theme_park_sim::physics::Circuit,
    model: &theme_park_sim::track::CoasterModel,
) {
    let mut run = CoasterRun::new(circuit, &model.train, PhysicsConfig::default());
    let outcome = (0..60_000).find_map(|_| run.step(circuit));
    let outcome = outcome.unwrap_or_else(|| panic!("{name} test ride did not finish"));
    let maximum = outcome
        .reports
        .iter()
        .flat_map(|report| &report.samples)
        .map(|sample| sample.longitudinal_g.abs())
        .fold(0.0_f64, f64::max);
    assert!(
        maximum <= 2.5,
        "{name} peak longitudinal force was {maximum:.1} g"
    );
}

#[test]
fn chain_lift_brakes_a_fast_train_at_a_physical_rate() {
    let circuit = build_player_circuit("Velocity Viper", multi_launch_model());
    let model = multi_launch_model();
    let station_distance = circuit
        .pieces()
        .iter()
        .take_while(|piece| piece.kind != PieceKind::Station)
        .map(|piece| piece.length())
        .sum::<f64>()
        + circuit.pieces()[0].length() / 2.0;
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
        if train.distance() - station_distance >= circuit.length() || !train.notes().is_empty() {
            break;
        }
    }
    assert!(
        saw_fast_train_on_lift,
        "Velocity Viper did not test a chain-lift entry above lift speed"
    );
    assert!(
        maximum <= 2.5,
        "Velocity Viper peak longitudinal force was {maximum:.1} g"
    );
}
