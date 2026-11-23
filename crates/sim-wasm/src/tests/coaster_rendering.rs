use super::*;
use serde_json::Value;
use theme_park_sim::park::CoasterId;
use theme_park_sim::track::construction::Construction;
use theme_park_sim::track::{Heading, PieceId, TrackState};

fn add_test_coaster(
    simulation: &mut WasmSimulation,
    model_key: &str,
    x: i32,
    y: i32,
    colors: (&str, &str, &str),
) -> CoasterId {
    let start = TrackState {
        x,
        y,
        z: 20,
        heading: Heading::East,
        ..TrackState::origin()
    };
    let mut construction = Construction::new(start, coaster_model(model_key));
    construction
        .append(PieceId::Station, &simulation.world, &[])
        .expect("station can be placed");
    construction
        .append(PieceId::Straight, &simulation.world, &[])
        .expect("straight can be placed");
    construction.set_colors(
        CoasterColors::from_names(colors.0, colors.1, colors.2)
            .expect("test colors are in the palette"),
    );
    simulation.park.add_coaster(construction)
}

fn render_list(simulation: &WasmSimulation) -> Value {
    serde_json::from_str(&simulation.coaster_render_list_json())
        .expect("coaster render list is JSON")
}

fn entry_for(entries: &Value, id: u32) -> &Value {
    entries
        .as_array()
        .expect("render list is an array")
        .iter()
        .find(|entry| entry["id"] == id)
        .expect("coaster render entry exists")
}

#[test]
fn render_list_includes_the_starter_and_each_built_coaster() {
    let demo = WasmSimulation::new(17, 0, String::from("demo"));
    let demo_entries = render_list(&demo);
    assert_eq!(demo_entries.as_array().unwrap().len(), 1);
    let demo_entry = entry_for(&demo_entries, 0);
    assert_eq!(demo_entry["colors"]["track"], "coral");
    assert!(demo_entry["trainLayout"].is_object());
    assert!(!demo.coaster_geometry(0).is_empty());
    assert!(!demo.coaster_rails(0).is_empty());
    assert!(!demo.coaster_ties(0).is_empty());
    assert!(!demo.coaster_supports(0).is_empty());
    assert_eq!(demo.editing_coaster_id_value(), None);

    let mut simulation = WasmSimulation::new(17, 0, String::from("new"));
    let first = add_test_coaster(
        &mut simulation,
        "hybrid",
        100,
        100,
        ("coral", "timber", "mustard"),
    );
    let second = add_test_coaster(
        &mut simulation,
        "wooden_twister",
        220,
        220,
        ("teal", "walnut", "plum"),
    );
    let entries = render_list(&simulation);
    assert_eq!(
        entries
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| entry["id"].as_u64().unwrap() as u32)
            .collect::<Vec<_>>(),
        [first.0, second.0]
    );
    assert_eq!(entry_for(&entries, first.0)["colors"]["track"], "coral");
    assert_eq!(
        entry_for(&entries, second.0)["trainLayout"]["model"],
        "wooden_twister"
    );
    assert!(!simulation.coaster_geometry(first.0).is_empty());
    assert!(!simulation.coaster_geometry(second.0).is_empty());
    assert!(!simulation.coaster_rails(first.0).is_empty());
    assert!(!simulation.coaster_ties(second.0).is_empty());
    assert!(!simulation.coaster_supports(first.0).is_empty());
    assert!(!simulation.coaster_supports(second.0).is_empty());
    assert_eq!(simulation.coaster_ids(), [first.0, second.0]);
    assert!(simulation.coaster_train_poses(first.0).is_empty());
    assert!(simulation.coaster_train_states(second.0).is_empty());
    assert!(simulation.coaster_riders(first.0).is_empty());
}

#[test]
fn track_revisions_change_only_for_the_edited_coaster_and_support_world_changes() {
    let mut simulation = WasmSimulation::new(17, 0, String::from("new"));
    let first = add_test_coaster(
        &mut simulation,
        "hybrid",
        100,
        100,
        ("coral", "timber", "mustard"),
    );
    let second = add_test_coaster(
        &mut simulation,
        "wooden_twister",
        220,
        220,
        ("teal", "walnut", "plum"),
    );
    let before = render_list(&simulation);
    let first_revision = entry_for(&before, first.0)["trackRevision"].clone();
    let second_revision = entry_for(&before, second.0)["trackRevision"].clone();
    let first_supports = simulation.coaster_supports(first.0);
    let second_supports = simulation.coaster_supports(second.0);

    simulation.tick();
    let after_tick = render_list(&simulation);
    assert_eq!(
        entry_for(&after_tick, first.0)["trackRevision"],
        first_revision
    );
    assert_eq!(
        entry_for(&after_tick, second.0)["trackRevision"],
        second_revision
    );

    let (world, coasters) = (&simulation.park.world, &mut simulation.park.coasters);
    coasters
        .get_mut(&second)
        .unwrap()
        .construction
        .append(PieceId::Straight, world, &[])
        .expect("second coaster can be extended");
    let after_track_edit = render_list(&simulation);
    assert_eq!(
        entry_for(&after_track_edit, first.0)["trackRevision"],
        first_revision
    );
    assert_ne!(
        entry_for(&after_track_edit, second.0)["trackRevision"],
        second_revision
    );
    assert_eq!(simulation.coaster_supports(first.0), first_supports);
    assert_ne!(simulation.coaster_supports(second.0), second_supports);

    let before_world_edit = render_list(&simulation);
    let revision_before_world_edit =
        entry_for(&before_world_edit, first.0)["trackRevision"].clone();
    let supports_before_world_edit = simulation.coaster_supports(first.0);
    for x in 49..=58 {
        for y in 49..=51 {
            simulation.set_world_surface(x, y, vec![4, 4, 4, 4], None);
        }
    }
    let after_world_edit = render_list(&simulation);
    assert_ne!(
        entry_for(&after_world_edit, first.0)["trackRevision"],
        revision_before_world_edit
    );
    assert_ne!(
        simulation.coaster_supports(first.0),
        supports_before_world_edit
    );
}
