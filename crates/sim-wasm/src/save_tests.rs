use crate::WasmSimulation;
use theme_park_sim::track::PieceId;

fn append_timber_tumbler_fixture_with_ejector_hill(simulation: &mut WasmSimulation) {
    simulation.construction_select("model:hybrid");
    simulation.construction_start(200, 200, 40, 2);

    let mut sections =
        include_str!("../../../web/e2e/player/circuits.txt").split("[Timber Tumbler]");
    sections.next().expect("circuit fixture has a prefix");
    let circuit = sections
        .next()
        .expect("Timber Tumbler circuit fixture exists");
    assert!(
        !circuit.contains("[Timber Tumbler]"),
        "Timber Tumbler fixture is unique"
    );

    let mut inserted_hill = false;
    let mut straight_pieces_to_skip = 0;
    for name in circuit
        .lines()
        .map(str::trim)
        .take_while(|line| !line.starts_with('['))
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
    {
        match name {
            "+chain" => {
                simulation.construction_set_chain(true);
                continue;
            }
            "-chain" => {
                simulation.construction_set_chain(false);
                continue;
            }
            _ => {}
        }

        if straight_pieces_to_skip > 0 {
            assert_eq!(
                name, "Straight",
                "three consecutive straights form the hill span"
            );
            straight_pieces_to_skip -= 1;
            continue;
        }
        let piece_name = if name == "Straight" && !inserted_hill {
            inserted_hill = true;
            straight_pieces_to_skip = 2;
            "EjectorHill8x48"
        } else {
            name
        };
        simulation.construction_select(piece_name);
        assert!(
            simulation.construction_can_append(),
            "{piece_name}: {}",
            simulation.construction_preview_reason()
        );
        simulation.construction_append();
        assert!(
            simulation.construction_message().is_empty(),
            "{piece_name}: {}",
            simulation.construction_message()
        );
    }

    assert!(
        inserted_hill,
        "Timber Tumbler fixture has a straight run to replace"
    );
    let pieces = simulation.construction.as_ref().unwrap().pieces();
    let closure_mismatch = pieces.iter().enumerate().find_map(|(index, piece)| {
        let next = &pieces[(index + 1) % pieces.len()];
        (piece.exit != next.entry).then_some((piece.name, piece.exit, next.name, next.entry))
    });
    assert!(
        simulation.construction_can_close(),
        "circuit pieces do not join: {closure_mismatch:?}"
    );
    simulation.construction_close();
    assert!(simulation.construction_message().is_empty());
}

#[test]
fn save_restores_live_state_and_continues_deterministically() {
    let mut original = WasmSimulation::new(42, 20, String::from("demo"));
    for _ in 0..30 {
        original.tick();
    }
    let bytes = original.save_state().unwrap();
    let mut restored = WasmSimulation::load_state(&bytes).unwrap();
    assert_eq!(
        original.world.state_hash(),
        restored.world.state_hash(),
        "world"
    );
    assert_eq!(
        original.facilities.state_hash(),
        restored.facilities.state_hash(),
        "facilities"
    );
    assert_eq!(
        original.simulation.state_hash(),
        restored.simulation.state_hash(),
        "guests"
    );
    assert_eq!(original.state_hash(), restored.state_hash());
    for _ in 0..30 {
        original.tick();
        restored.tick();
        assert_eq!(original.state_hash(), restored.state_hash());
    }
}

#[test]
fn save_preserves_construction_and_a_stale_path_graph() {
    let mut original = WasmSimulation::new(42, 5, String::from("new"));
    original.construction_start(100, 100, 20, 0);
    original.construction_select("LevelToGentleUp");
    assert!(original.construction_can_append());
    original.construction_append();
    original.construction_select("GentleUp");
    assert!(original.path_build(vec![218, 251, 8, 219, 251, 8], "normal", 0));
    let bytes = original.save_state().unwrap();
    let mut restored = WasmSimulation::load_state(&bytes).unwrap();
    assert_eq!(original.state_hash(), restored.state_hash());
    assert_eq!(original.construction_track(), restored.construction_track());
    assert_eq!(
        original.construction_selected(),
        restored.construction_selected()
    );
    original.tick();
    restored.tick();
    assert_eq!(original.state_hash(), restored.state_hash());
    original.construction_append();
    restored.construction_append();
    assert_eq!(original.state_hash(), restored.state_hash());
}

#[test]
fn save_preserves_non_default_cars_per_train() {
    let mut original = WasmSimulation::new(42, 0, String::from("new"));
    original.construction_start(100, 100, 20, 0);
    assert!(original.construction_set_cars_per_train(4));
    let bytes = original.save_state().unwrap();
    let restored = WasmSimulation::load_state(&bytes).unwrap();
    assert_eq!(original.state_hash(), restored.state_hash());
    assert_eq!(
        original.construction_train_layout(),
        restored.construction_train_layout()
    );
    assert_eq!(
        restored.construction.as_ref().unwrap().model().train.cars,
        4
    );
}

#[test]
fn save_round_trips_built_timber_tumbler_with_ejector_hill() {
    let mut original = WasmSimulation::new(42, 0, String::from("new"));
    append_timber_tumbler_fixture_with_ejector_hill(&mut original);
    assert!(original.construction.as_ref().unwrap().is_closed());
    assert!(
        original
            .construction
            .as_ref()
            .unwrap()
            .pieces()
            .iter()
            .any(|piece| piece.id == PieceId::EjectorHill8x48)
    );

    let bytes = original.save_state().unwrap();
    let mut restored = WasmSimulation::load_state(&bytes).unwrap();
    assert_eq!(original.state_hash(), restored.state_hash());
    assert!(
        restored
            .construction
            .as_ref()
            .unwrap()
            .pieces()
            .iter()
            .any(|piece| piece.id == PieceId::EjectorHill8x48)
    );

    for _ in 0..30 {
        original.tick();
        restored.tick();
        assert_eq!(original.state_hash(), restored.state_hash());
    }
}
