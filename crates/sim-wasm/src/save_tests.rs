use crate::WasmSimulation;
use theme_park_sim::track::{HillFeel, PieceFamily, PieceId};

fn append_timber_tumbler_fixture(simulation: &mut WasmSimulation) -> Vec<PieceId> {
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

        simulation.construction_select(name);
        assert!(
            simulation.construction_can_append(),
            "{name}: {}",
            simulation.construction_preview_reason()
        );
        simulation.construction_append();
        assert!(
            simulation.construction_message().is_empty(),
            "{name}: {}",
            simulation.construction_message()
        );
    }

    let pieces = simulation.construction.as_ref().unwrap().pieces();
    assert!(
        pieces.iter().any(|piece| matches!(
            piece.family,
            Some(PieceFamily::Hill(family)) if family.feel == HillFeel::Ejector
        )),
        "Timber Tumbler fixture has no ejector hill"
    );
    let hill_piece_ids: Vec<_> = pieces
        .iter()
        .filter(|piece| {
            piece.id == PieceId::AirtimeHill
                || matches!(piece.family.as_ref(), Some(PieceFamily::Hill(_)))
        })
        .map(|piece| piece.id)
        .collect();
    assert!(
        !hill_piece_ids.is_empty(),
        "Timber Tumbler fixture has no hills"
    );
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
    hill_piece_ids
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
    let hill_piece_ids = append_timber_tumbler_fixture(&mut original);
    assert!(original.construction.as_ref().unwrap().is_closed());

    let bytes = original.save_state().unwrap();
    let mut restored = WasmSimulation::load_state(&bytes).unwrap();
    assert_eq!(original.state_hash(), restored.state_hash());
    let restored_hill_piece_ids: Vec<_> = restored
        .construction
        .as_ref()
        .unwrap()
        .pieces()
        .iter()
        .filter(|piece| {
            piece.id == PieceId::AirtimeHill
                || matches!(piece.family.as_ref(), Some(PieceFamily::Hill(_)))
        })
        .map(|piece| piece.id)
        .collect();
    assert_eq!(restored_hill_piece_ids, hill_piece_ids);

    for _ in 0..30 {
        original.tick();
        restored.tick();
        assert_eq!(original.state_hash(), restored.state_hash());
    }
}

#[test]
fn save_uses_new_version_and_rejects_old_version() {
    let simulation = WasmSimulation::new(42, 20, String::from("demo"));
    let mut bytes = simulation.save_state().unwrap();
    assert_eq!(&bytes[..7], b"TPSAVE\x02");
    bytes[6] = 1;
    assert_eq!(
        super::save::checked_payload(&bytes),
        Err("Unsupported or invalid park save version")
    );
}
