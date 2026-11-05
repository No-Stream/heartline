use crate::WasmSimulation;

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
