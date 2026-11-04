#[test]
fn diagonal_support_bounds_do_not_fill_its_bounding_rectangle() {
    let simulation = super::WasmSimulation::new(42, 0, String::from("demo"));
    let piece = simulation
        .demo_coaster()
        .expect("demo park")
        .circuit
        .pieces()[0]
        .clone();
    let point = piece.sample(piece.length() / 2.0).position;
    let segment = super::SupportSegment {
        start: point + super::Vec3::new(-40.0, 0.0, 0.0),
        end: point + super::Vec3::new(0.0, 40.0, 0.0),
    };
    let baseline = theme_park_sim::track::proximity::scan_proximity_with_supports(
        &simulation.world,
        std::slice::from_ref(&piece),
        &simulation
            .demo_coaster()
            .expect("demo park")
            .train_configuration,
        &[],
        &[],
        &[],
    );
    let diagonal = theme_park_sim::track::proximity::scan_proximity_with_supports(
        &simulation.world,
        &[piece],
        &simulation
            .demo_coaster()
            .expect("demo park")
            .train_configuration,
        &[],
        &[],
        &[theme_park_sim::track::proximity::OwnedSupportSegment {
            piece_index: None,
            segment,
        }],
    );
    assert_eq!(baseline.events_for_run(&[]), diagonal.events_for_run(&[]));
}
use super::WasmSimulation;

#[test]
fn finish_preview_does_not_change_track_and_confirmation_closes_it() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    simulation.construction_start(200, 200, 20, 0);
    for piece in [
        "TurnSmallStandalone",
        "TurnSmallStandalone",
        "Straight",
        "TurnSmallStandalone",
    ] {
        simulation.construction_select(piece);
        simulation.construction_append();
        assert!(simulation.construction_message().is_empty());
    }
    let before = simulation.construction_track();
    simulation.construction_finish_search_start();
    while !simulation.construction_finish_search_advance() {}
    simulation.construction_select("Straight");
    assert!(simulation.construction_can_finish());
    simulation.construction_finish_preview();
    assert!(
        simulation.construction_finish_preview_active(),
        "{}",
        simulation.construction_message()
    );
    assert_eq!(simulation.construction_track(), before);
    assert!(!simulation.construction_rails().is_empty());
    simulation.construction_finish_confirm();
    assert!(!simulation.construction_finish_preview_active());
    assert!(simulation.construction.as_ref().unwrap().is_closed());
    assert_ne!(simulation.construction_track(), before);
}

#[test]
fn finish_search_reports_when_the_station_is_missing() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    simulation.construction_start(200, 200, 20, 0);
    simulation.construction_delete(0);
    simulation.construction_finish_search_start();
    assert!(simulation.construction_finish_search_advance());
    assert_eq!(
        simulation.construction_finish_reason(),
        "Place a station before finishing the coaster."
    );
}

#[test]
fn finish_preview_is_cancelled_by_track_edits() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    simulation.construction_start(200, 200, 20, 0);
    for piece in [
        "TurnSmallStandalone",
        "TurnSmallStandalone",
        "Straight",
        "TurnSmallStandalone",
    ] {
        simulation.construction_select(piece);
        simulation.construction_append();
    }
    simulation.construction_finish_search_start();
    while !simulation.construction_finish_search_advance() {}
    simulation.construction_finish_preview();
    assert!(simulation.construction_finish_preview_active());
    simulation.construction_undo();
    let before = simulation.construction_track();
    simulation.construction_finish_confirm();
    assert_eq!(simulation.construction_track(), before);
    assert!(!simulation.construction.as_ref().unwrap().is_closed());
}

#[test]
fn piece_groups_follow_track_geometry_instead_of_piece_names() {
    let catalog = super::catalog();
    let expected_groups = [
        (super::PieceId::Straight, "straight"),
        (super::PieceId::TurnSmallStandalone, "turn"),
        (super::PieceId::GentleUp, "slope"),
        (super::PieceId::BankLeft, "bank"),
        (super::PieceId::Loop, "inversion"),
        (super::PieceId::Station, "special"),
        (super::PieceId::Brakes, "special"),
        (super::PieceId::Launch, "special"),
        (super::PieceId::TerrainRise, "special"),
        (super::PieceId::TerrainFall, "special"),
        (super::PieceId::ShortOffsetLeft, "straight"),
        (super::PieceId::ShortOffsetRight, "straight"),
    ];

    for (id, expected_group) in expected_groups {
        let piece = catalog.iter().find(|piece| piece.id == id).unwrap();
        assert_eq!(super::piece_group(piece), expected_group, "{id:?}");
    }

    let mut turn_named_straight = catalog
        .iter()
        .find(|piece| piece.id == super::PieceId::Straight)
        .unwrap()
        .clone();
    turn_named_straight.id = super::PieceId::TurnSmallStandalone;
    assert_eq!(super::piece_group(&turn_named_straight), "straight");

    let mut terrain_named_straight = catalog
        .iter()
        .find(|piece| piece.id == super::PieceId::Straight)
        .unwrap()
        .clone();
    terrain_named_straight.exit.z = 1;
    assert_eq!(super::piece_group(&terrain_named_straight), "special");

    let mut climbing_turn = catalog
        .iter()
        .find(|piece| piece.id == super::PieceId::TurnSmallStandalone)
        .unwrap()
        .clone();
    climbing_turn.entry.slope = theme_park_sim::track::Slope::UpGentle;
    climbing_turn.exit.slope = theme_park_sim::track::Slope::UpGentle;
    assert_eq!(super::piece_group(&climbing_turn), "turn");
}

#[test]
fn elements_preview_the_full_sequence_append_atomically_and_undo_together() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    simulation.construction_start(200, 200, 20, 0);
    let before = simulation.construction_track();

    assert!(
        serde_json::from_str::<serde_json::Value>(&simulation.construction_elements())
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .any(|element| element["id"] == "element:climb-60")
    );
    simulation.construction_select("element:climb-60");

    assert_eq!(simulation.construction_selected(), "element:climb-60");
    assert!(
        simulation.construction_can_append(),
        "{}",
        simulation.construction_message()
    );
    let preview = simulation.construction_preview();
    assert!(!preview.is_empty());
    assert!(!simulation.construction_rails().is_empty());

    simulation.construction_append();
    assert!(simulation.construction_message().is_empty());
    assert_eq!(
        track_rows(&simulation.construction_track()).len(),
        track_rows(&before).len() + preview.len()
    );

    simulation.construction_undo();
    assert_eq!(simulation.construction_track(), before);
}

#[test]
fn construction_selects_the_sixty_degree_continuation_after_steep_to_sixty_transition() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    simulation.construction_start(200, 200, 20, 0);
    simulation.construction_set_chain(true);
    simulation.construction_select("element:climb-45");
    assert!(
        simulation.construction_can_append(),
        "{}",
        simulation.construction_message()
    );
    simulation.construction_append();

    simulation.construction_select("SteepToSixtyUp");
    assert!(
        simulation.construction_can_append(),
        "{}",
        simulation.construction_message()
    );
    simulation.construction_append();

    assert_eq!(simulation.construction_selected(), "SixtyUp");
}

#[test]
fn construction_falls_back_to_the_first_valid_piece_without_a_straight_continuation() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    simulation.construction_start(200, 200, 20, 0);
    simulation.construction_select("BankLeft");
    simulation.construction_append();

    let choices: serde_json::Value =
        serde_json::from_str(&simulation.construction_choices()).unwrap();
    let choices = choices.as_array().unwrap();
    assert!(!choices.iter().any(|choice| {
        choice["heading"] == "East"
            && choice["slope"] == "Level"
            && choice["bank"] == "Left"
            && choice["refusal"].as_str().unwrap().is_empty()
    }));
    let first_valid = choices
        .iter()
        .find(|choice| choice["refusal"].as_str().unwrap().is_empty())
        .unwrap()["id"]
        .as_str()
        .unwrap();

    assert_eq!(simulation.construction_selected(), first_valid);
}

#[test]
fn chain_mode_marks_every_piece_in_an_element_as_a_lift() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    simulation.construction_start(200, 200, 20, 0);
    simulation.construction_set_chain(true);
    simulation.construction_select("element:small-left-banked-turn");

    assert!(
        simulation.construction_can_append(),
        "{}",
        simulation.construction_message()
    );
    let preview = simulation.construction_preview();
    assert_eq!(preview.len(), 3);
    assert!(
        preview
            .iter()
            .all(|piece| piece.kind == super::PieceKind::Lift)
    );

    simulation.construction_append();
    assert_eq!(
        track_rows(&simulation.construction_track())
            .iter()
            .skip(1)
            .map(|row| row["kind"].as_str().unwrap().to_owned())
            .collect::<Vec<_>>(),
        ["Lift", "Lift", "Lift"]
    );
    simulation.construction_undo();
    assert_eq!(track_rows(&simulation.construction_track()).len(), 1);
}

#[test]
fn coaster_ticket_price_defaults_to_six_and_can_be_changed_to_zero() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));

    assert_eq!(simulation.construction_coaster_price(), 6);
    simulation.construction_set_coaster_price(0);

    assert_eq!(simulation.construction_coaster_price(), 0);
    assert_eq!(simulation.construction_coaster_income(), 0);
}

#[test]
fn coaster_cannot_open_without_a_complete_circuit() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    assert!(!simulation.construction_set_open(true));
    simulation.construction_start(200, 200, 20, 0);
    assert!(!simulation.construction_set_open(true));
    assert!(!simulation.construction_is_open());
    assert!(simulation.construction_riders().is_empty());
}

#[test]
fn coaster_requires_station_paths_before_opening() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    simulation.construction_start(200, 200, 20, 0);
    for piece in [
        "TurnSmallStandalone",
        "TurnSmallStandalone",
        "Straight",
        "TurnSmallStandalone",
        "TurnSmallStandalone",
    ] {
        simulation.construction_select(piece);
        simulation.construction_append();
    }
    assert!(simulation.construction_can_close());
    simulation.construction_close();
    assert!(!simulation.construction_set_open(true));
    assert!(!simulation.construction_is_open());
}

#[test]
fn model_selection_controls_new_construction_and_launch_choices() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    simulation.construction_select("model:velocity_viper");
    simulation.construction_start(200, 200, 20, 0);
    assert_eq!(
        simulation.construction.as_ref().unwrap().model().name,
        "Velocity Viper"
    );
    assert!(
        simulation
            .construction_choices()
            .contains("\"id\":\"LaunchShort\"")
    );
    simulation.construction_select("LaunchShort");
    assert!(simulation.construction_can_append());
    let construction = simulation.construction.as_ref().unwrap();
    let obstacles =
        super::track_support_obstacles(construction.pieces(), &construction.model().train);
    let preview = simulation.construction_preview();
    let expected = super::render_preview_supports(
        &preview[0],
        &obstacles,
        &simulation.world,
        super::SupportStyle::SteelColumns,
    )
    .unwrap();
    assert_eq!(
        simulation.construction_supports(),
        simulation.encode_construction_geometry(expected, 7)
    );
    simulation.construction_append();
    assert!(simulation.construction_track().contains("Launch"));
    simulation.construction_select("model:hybrid");
    simulation.construction_start(200, 200, 20, 0);
    assert_eq!(
        simulation.construction.as_ref().unwrap().model().name,
        super::hybrid_model().name
    );
}

#[test]
fn launch_intensity_changes_hash_and_replays_to_the_same_track_state() {
    fn built_viper() -> WasmSimulation {
        let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
        simulation.construction_select("model:velocity_viper");
        simulation.construction_start(200, 200, 20, 0);
        simulation.construction_select("LaunchShort");
        simulation.construction_append();
        assert!(simulation.construction_message().is_empty());
        simulation
    }

    let mut simulation = built_viper();
    let before = simulation.state_hash();
    assert!(simulation.construction_set_launch_intensity(1, 58.1));
    assert_ne!(simulation.state_hash(), before);
    assert!(
        simulation
            .construction_track()
            .contains("\"launchTargetSpeed\":58.1")
    );
    let replay_hash = simulation.state_hash();
    assert!(!simulation.construction_set_launch_intensity(1, 58.2));

    let mut replay = built_viper();
    assert!(replay.construction_set_launch_intensity(1, 58.1));
    assert_eq!(replay.state_hash(), replay_hash);
}

#[test]
fn rail_distances_follow_placed_rail_intervals_across_pieces() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    assert!(simulation.construction_rail_distances().is_empty());
    simulation.construction_start(200, 200, 20, 0);
    simulation.construction_select("Straight");
    simulation.construction_append();
    let construction = simulation.construction.as_ref().unwrap();
    let rails = simulation.construction_placed_rails();
    let distances = simulation.construction_rail_distances();
    assert_eq!(distances.len(), rails.len() / 19);
    let mut expected = Vec::new();
    let mut start = 0.0;
    for piece in construction.pieces() {
        let intervals = (piece.length() / 2.0).ceil() as usize;
        for index in 0..intervals {
            expected.push((start + piece.length() * index as f64 / intervals as f64) as f32);
            expected.push((start + piece.length() * (index + 1) as f64 / intervals as f64) as f32);
        }
        start += piece.length();
    }
    assert_eq!(distances, expected);
}

#[test]
fn wooden_twister_geometry_distinguishes_track_and_steel_loop_supports() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    simulation.construction_select("model:wooden_twister");
    simulation.construction_start(200, 200, 20, 0);
    let rails = simulation.construction_placed_rails();
    assert!(!rails.is_empty());
    assert!(
        rails
            .chunks_exact(19)
            .all(|record| record[0] == 1_000_000.0)
    );
    let ties = simulation.construction_placed_ties();
    assert!(!ties.is_empty());
    assert!(ties.chunks_exact(13).all(|record| record[0] == 1_000_000.0));
    let supports = simulation.construction_placed_supports();
    assert!(!supports.is_empty());
    assert!(supports.chunks_exact(7).all(|record| record[0] == 0.0));

    let loop_piece = super::catalog()
        .into_iter()
        .find(|piece| piece.id == super::PieceId::SteelLoop)
        .unwrap()
        .at(super::TrackState {
            x: 200,
            y: 200,
            z: 20,
            ..super::TrackState::origin()
        })
        .unwrap();
    let supports = super::render_preview_supports(
        &loop_piece,
        &[],
        &simulation.world,
        super::SupportStyle::DenseWoodenLattice,
    )
    .unwrap();
    assert!(!supports.is_empty());
    assert!(supports.chunks_exact(7).all(|record| record[0] == -1.0));
    let id = 42;
    let model = super::wooden_twister_model();
    let supports = super::render_cached_supports(
        &[(&id, &loop_piece)],
        &simulation.world,
        &model.train,
        model.support_style,
        &[],
        &mut std::collections::HashMap::new(),
    )
    .unwrap();
    assert!(!supports.is_empty());
    assert!(supports.chunks_exact(7).all(|record| record[0] == -43.0));
}

#[test]
fn geometry_encodes_material_and_preserves_piece_identity() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    simulation.construction_start(200, 200, 20, 0);
    let wooden = simulation.construction_placed_rails();
    assert!(wooden.chunks_exact(19).all(|record| record[0] == 0.0));
    simulation.construction_select("model:velocity_viper");
    simulation.construction_start(200, 200, 20, 0);
    simulation.construction_select("LaunchShort");
    simulation.construction_append();
    for (geometry, stride) in [
        (simulation.construction_placed_rails(), 19),
        (simulation.construction_placed_ties(), 13),
        (simulation.construction_placed_supports(), 7),
        (simulation.construction_rails(), 19),
        (simulation.construction_ties(), 13),
        (simulation.construction_supports(), 7),
    ] {
        assert!(!geometry.is_empty());
        assert!(
            geometry
                .chunks_exact(stride)
                .all(|record| record[0] < 0.0 && -record[0] - 1.0 >= 0.0)
        );
    }
    let ids: std::collections::HashSet<_> = simulation
        .construction_placed_rails()
        .chunks_exact(19)
        .map(|record| (-record[0] - 1.0) as usize)
        .collect();
    assert_eq!(ids, [0, 1].into_iter().collect());
}

#[test]
fn construction_commands_validate_and_preview_without_mutating_on_failure() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    simulation.construction_start(200, 200, 20, 0);
    assert!(simulation.construction_active());
    simulation.construction_select("Straight");
    assert!(simulation.construction_can_append());
    assert!(!simulation.construction_rails().is_empty());
    let construction = simulation.construction.as_ref().unwrap();
    let obstacles =
        super::track_support_obstacles(construction.pieces(), &construction.model().train);
    let preview = simulation.construction_preview();
    let expected_supports = super::render_preview_supports(
        &preview[0],
        &obstacles,
        &simulation.world,
        super::SupportStyle::WoodenLattice,
    )
    .unwrap();
    assert_eq!(simulation.construction_supports(), expected_supports);
    simulation.construction_append();
    assert_eq!(
        simulation.construction_track(),
        "[{\"id\":0,\"label\":\"Station\",\"name\":\"Station\",\"group\":\"special\",\"slope\":\"Level\",\"kind\":\"Station\",\"launchTargetSpeed\":null,\"launchMinimumSpeed\":null,\"launchMaximumSpeed\":null},{\"id\":1,\"label\":\"Straight\",\"name\":\"Straight\",\"group\":\"straight\",\"slope\":\"Level\",\"kind\":\"Track\",\"launchTargetSpeed\":null,\"launchMinimumSpeed\":null,\"launchMaximumSpeed\":null}]"
    );
    assert!(!simulation.construction_placed_rails().is_empty());
    simulation.construction_select("GentleUp");
    simulation.construction_append();
    assert_eq!(
        simulation.construction_track(),
        "[{\"id\":0,\"label\":\"Station\",\"name\":\"Station\",\"group\":\"special\",\"slope\":\"Level\",\"kind\":\"Station\",\"launchTargetSpeed\":null,\"launchMinimumSpeed\":null,\"launchMaximumSpeed\":null},{\"id\":1,\"label\":\"Straight\",\"name\":\"Straight\",\"group\":\"straight\",\"slope\":\"Level\",\"kind\":\"Track\",\"launchTargetSpeed\":null,\"launchMinimumSpeed\":null,\"launchMaximumSpeed\":null}]"
    );
    assert!(!simulation.construction_message().is_empty());
    simulation.construction_undo();
    assert_eq!(
        simulation.construction_track(),
        "[{\"id\":0,\"label\":\"Station\",\"name\":\"Station\",\"group\":\"special\",\"slope\":\"Level\",\"kind\":\"Station\",\"launchTargetSpeed\":null,\"launchMinimumSpeed\":null,\"launchMaximumSpeed\":null}]"
    );
}

#[test]
fn deleting_middle_piece_preserves_and_identifies_retained_track() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    simulation.construction_start(200, 200, 20, 0);
    for _ in 0..2 {
        simulation.construction_select("Straight");
        simulation.construction_append();
        assert!(simulation.construction_message().is_empty());
    }
    simulation.construction_delete(1);
    assert_eq!(
        simulation.construction_track(),
        "[{\"id\":0,\"label\":\"Station\",\"name\":\"Station\",\"group\":\"special\",\"slope\":\"Level\",\"kind\":\"Station\",\"launchTargetSpeed\":null,\"launchMinimumSpeed\":null,\"launchMaximumSpeed\":null},{\"id\":2,\"label\":\"Straight\",\"name\":\"Straight\",\"group\":\"straight\",\"slope\":\"Level\",\"kind\":\"Track\",\"launchTargetSpeed\":null,\"launchMinimumSpeed\":null,\"launchMaximumSpeed\":null}]"
    );
    let rails = simulation.construction_placed_rails();
    assert!(rails.chunks_exact(19).any(|rail| rail[0] == 2.0));
    simulation.construction_delete(2);
    assert_eq!(
        simulation.construction_track(),
        "[{\"id\":0,\"label\":\"Station\",\"name\":\"Station\",\"group\":\"special\",\"slope\":\"Level\",\"kind\":\"Station\",\"launchTargetSpeed\":null,\"launchMinimumSpeed\":null,\"launchMaximumSpeed\":null}]"
    );
}

#[test]
fn terrain_change_does_not_panic_when_rendering_supports() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    simulation.construction_start(200, 200, 20, 0);
    assert!(simulation.construction_active());
    assert!(!simulation.construction_placed_supports().is_empty());
    for y in 95..105 {
        for x in 95..110 {
            simulation.set_world_surface(x, y, vec![100; 4], None);
        }
    }
    let supports = simulation.construction_placed_supports();
    assert!(supports.is_empty());
    assert_eq!(
        simulation.construction_message(),
        "Could not route supports for the placed track."
    );
}

#[test]
fn placed_support_cache_matches_full_render_after_track_changes() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    simulation.construction_start(200, 200, 20, 0);
    for _ in 0..4 {
        simulation.construction_select("Straight");
        simulation.construction_append();
        assert!(simulation.construction_message().is_empty());
    }
    for deleted in [None, Some(2), Some(4)] {
        if let Some(id) = deleted {
            simulation.construction_delete(id);
        }
        let construction = simulation.construction.as_ref().unwrap();
        let entries: Vec<_> = construction
            .piece_ids()
            .iter()
            .zip(construction.pieces())
            .chain(
                construction
                    .disconnected_segments()
                    .iter()
                    .flat_map(|segment| segment.ids.iter().zip(&segment.pieces)),
            )
            .collect();
        let pieces: Vec<_> = entries.iter().map(|(_, piece)| (*piece).clone()).collect();
        let mut expected = super::try_render_supports(&pieces, &simulation.world).unwrap();
        for record in expected.chunks_exact_mut(7) {
            record[0] = *entries[record[0] as usize].0 as f32;
        }
        let entry_count = entries.len();
        assert_eq!(simulation.park.support_render_cache.len(), entry_count);
        assert_eq!(simulation.construction_placed_supports(), expected);
        assert_eq!(simulation.park.support_render_cache.len(), entry_count);
        assert_eq!(
            simulation.park.refresh_construction_supports().unwrap(),
            expected
        );
        assert_eq!(simulation.park.support_render_cache.len(), entry_count);
        assert_eq!(simulation.construction_placed_supports(), expected);
    }
}

#[test]
fn deleting_distant_track_reuses_cached_supports() {
    let simulation = WasmSimulation::new(42, 0, String::from("demo"));
    let template = super::catalog()
        .into_iter()
        .find(|piece| piece.id == super::PieceId::Straight)
        .unwrap();
    let first = template
        .at(super::TrackState {
            x: 100,
            y: 100,
            z: 20,
            ..super::TrackState::origin()
        })
        .unwrap();
    let second = template
        .at(super::TrackState {
            x: 200,
            y: 200,
            z: 20,
            ..super::TrackState::origin()
        })
        .unwrap();
    let first_id = 1;
    let second_id = 2;
    let mut cache = std::collections::HashMap::new();
    let train = super::hybrid_model().train;
    let initial = super::render_cached_supports(
        &[(&first_id, &first), (&second_id, &second)],
        &simulation.world,
        &train,
        super::SupportStyle::WoodenLattice,
        &[],
        &mut cache,
    )
    .unwrap();
    let mut expected_initial =
        super::try_render_supports(&[first.clone(), second], &simulation.world).unwrap();
    for record in expected_initial.chunks_exact_mut(7) {
        record[0] += first_id as f32;
    }
    assert_eq!(initial, expected_initial);
    let segments = cache[&first_id].segments.as_ref().unwrap();
    assert!(!segments.is_empty());
    let original_allocation = segments.as_ptr();
    let cached = super::render_cached_supports(
        &[(&first_id, &first)],
        &simulation.world,
        &train,
        super::SupportStyle::WoodenLattice,
        &[],
        &mut cache,
    )
    .unwrap();
    assert_eq!(
        cache[&first_id].segments.as_ref().unwrap().as_ptr(),
        original_allocation
    );
    let mut expected = super::try_render_supports(&[first], &simulation.world).unwrap();
    for record in expected.chunks_exact_mut(7) {
        record[0] = first_id as f32;
    }
    assert_eq!(cached, expected);
}

#[test]
fn rendered_supports_clear_other_tracks_train_envelope() {
    let world = super::World::new(
        128,
        128,
        super::Surface {
            corners: [0; 4],
            water: None,
        },
    );
    let template = super::catalog()
        .into_iter()
        .find(|piece| piece.id == super::PieceId::Straight)
        .unwrap();
    let lower = template
        .at(super::TrackState {
            x: 50,
            y: 48,
            z: 3,
            ..super::TrackState::origin()
        })
        .unwrap();
    let upper = template
        .at(super::TrackState {
            x: 54,
            y: 46,
            z: 5,
            heading: super::Heading::North,
            ..super::TrackState::origin()
        })
        .unwrap();
    let obstacles = theme_park_sim::track::construction::track_support_obstacles(
        std::slice::from_ref(&lower),
        &super::hybrid_model().train,
    );
    let expected = super::generate_wooden_lattice(&upper, &world, &obstacles).unwrap();
    let mut expected_records = Vec::new();
    for segment in expected {
        if segment.start != segment.end {
            expected_records.push(1.0);
            super::append_vector(&mut expected_records, segment.start / 4.0);
            super::append_vector(&mut expected_records, segment.end / 4.0);
        }
    }
    let mut cache = std::collections::HashMap::new();
    let actual = super::render_cached_supports(
        &[(&0, &lower), (&1, &upper)],
        &world,
        &super::hybrid_model().train,
        super::SupportStyle::WoodenLattice,
        &[],
        &mut cache,
    )
    .unwrap();
    let upper_records: Vec<_> = actual
        .chunks_exact(7)
        .filter(|record| record[0] == 1.0)
        .flatten()
        .copied()
        .collect();
    assert_eq!(upper_records, expected_records);
    let preview = super::render_preview_supports(
        &upper,
        &obstacles,
        &world,
        super::SupportStyle::WoodenLattice,
    )
    .unwrap();
    let mut expected_preview = expected_records;
    for record in expected_preview.chunks_exact_mut(7) {
        record[0] = 0.0;
    }
    assert_eq!(preview, expected_preview);
}

#[test]
fn chain_mode_controls_placed_piece_and_refusals() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    simulation.construction_start(100, 100, 40, 0);
    assert!(!simulation.construction_chain());
    simulation.construction_set_chain(true);
    assert!(simulation.construction_chain());
    simulation.construction_select("Loop");
    assert!(!simulation.construction_can_append());
    assert_eq!(
        simulation.construction_preview_reason(),
        "No chain lift on this piece"
    );
    simulation.construction_select("LevelToGentleUp");
    simulation.construction_append();
    assert!(simulation.construction_message().is_empty());
    assert_eq!(
        simulation
            .construction
            .as_ref()
            .unwrap()
            .pieces()
            .last()
            .unwrap()
            .kind,
        super::PieceKind::Lift
    );
    simulation.construction_set_chain(false);
    assert!(!simulation.construction_chain());
}

#[test]
fn coaster_colours_start_from_the_model_and_change_the_state_hash() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    assert_eq!(simulation.construction_colors(), "null");
    assert_eq!(
        simulation.coaster_colors(),
        "{\"track\":\"coral\",\"supports\":\"timber\",\"trains\":\"sage\"}"
    );
    for (model, colors) in [
        (
            "model:hybrid",
            "{\"track\":\"coral\",\"supports\":\"timber\",\"trains\":\"sage\"}",
        ),
        (
            "model:velocity_viper",
            "{\"track\":\"teal\",\"supports\":\"cream\",\"trains\":\"coral\"}",
        ),
        (
            "model:wooden_twister",
            "{\"track\":\"timber\",\"supports\":\"timber\",\"trains\":\"slate\"}",
        ),
    ] {
        simulation.construction_select(model);
        simulation.construction_start(100, 100, 40, 0);
        assert_eq!(simulation.construction_colors(), colors, "{model}");
    }

    let before = simulation.state_hash();
    simulation.construction_set_colors("plum", "cream", "mustard");
    assert_eq!(
        simulation.construction_colors(),
        "{\"track\":\"plum\",\"supports\":\"cream\",\"trains\":\"mustard\"}"
    );
    assert!(simulation.construction_message().is_empty());
    let repainted = simulation.state_hash();
    assert_ne!(repainted, before);

    simulation.construction_set_colors("plum", "#ffffff", "mustard");
    assert_eq!(
        simulation.construction_colors(),
        "{\"track\":\"plum\",\"supports\":\"cream\",\"trains\":\"mustard\"}"
    );
    assert_eq!(simulation.construction_message(), "Unknown coaster colour");

    simulation.construction_start(120, 120, 40, 0);
    assert_eq!(
        simulation.construction_colors(),
        "{\"track\":\"timber\",\"supports\":\"timber\",\"trains\":\"slate\"}"
    );
}

#[test]
fn train_layouts_name_each_model_and_its_train_shape() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    assert_eq!(simulation.construction_train_layout(), "null");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&simulation.coaster_train_layout()).unwrap(),
        serde_json::json!({"model": "hybrid", "carsPerTrain": 6, "rowsPerCar": 2, "seatsPerRow": 2, "lengthMetres": 18.0})
    );
    for (model, key, cars, rows) in [
        ("model:hybrid", "hybrid", 6, 2),
        ("model:velocity_viper", "velocity_viper", 6, 2),
        ("model:wooden_twister", "wooden_twister", 12, 1),
    ] {
        simulation.construction_select(model);
        simulation.construction_start(100, 100, 40, 0);
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&simulation.construction_train_layout())
                .unwrap(),
            serde_json::json!({"model": key, "carsPerTrain": cars, "rowsPerCar": rows, "seatsPerRow": 2, "lengthMetres": 18.0}),
            "{model}"
        );
    }
}

#[test]
fn coaster_colours_replay_to_the_same_state() {
    let paint = |colors: [&str; 3]| {
        let mut simulation = WasmSimulation::new(7, 0, String::from("new"));
        simulation.construction_start(100, 100, 40, 0);
        simulation.construction_set_colors(colors[0], colors[1], colors[2]);
        simulation.tick();
        simulation.state_hash()
    };
    assert_eq!(
        paint(["sky", "charcoal", "rose"]),
        paint(["sky", "charcoal", "rose"])
    );
    assert_ne!(
        paint(["sky", "charcoal", "rose"]),
        paint(["sky", "charcoal", "walnut"])
    );
}

#[test]
fn lift_drop_circuit_completes_lap() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    simulation.construction_start(100, 100, 40, 0);
    for (piece, chain) in [
        ("LevelToGentleUp", true),
        ("GentleUp", true),
        ("GentleUp", true),
        ("GentleUp", true),
        ("GentleUp", true),
        ("GentleUp", true),
        ("GentleUpToLevel", true),
        ("Straight", true),
        ("OneTileStraight", false),
        ("OneTileStraight", false),
        ("OffsetLeft", false),
        ("TurnLargeEntry", false),
        ("TurnLargeExit", false),
        ("LevelToGentleDown", false),
        ("GentleDown", false),
        ("GentleDown", false),
        ("GentleDown", false),
        ("GentleDown", false),
        ("GentleDown", false),
        ("GentleDownToLevel", false),
        ("Loop", false),
        ("OffsetRight", false),
        ("TurnLargeEntry", false),
        ("TurnLargeExit", false),
        ("Straight", false),
        ("OneTileStraight", false),
        ("OneTileStraight", false),
    ] {
        simulation.construction_set_chain(chain);
        simulation.construction_select(piece);
        simulation.construction_append();
        assert!(
            simulation.construction_message().is_empty(),
            "{piece}: {}",
            simulation.construction_message()
        );
    }
    assert!(simulation.construction_can_close());
    simulation.construction_close();
    assert!(!simulation.park.support_render_cache.is_empty());
    simulation.construction_test();
    for _ in 0..9000 {
        simulation.tick();
        if simulation.construction_message() == "Train completed a lap." {
            assert!(!simulation.park.support_render_cache.is_empty());
            assert!(!simulation.park.proximity_supports().is_empty());
            return;
        }
        if simulation.construction_outcome.is_some() {
            break;
        }
    }
    let (circuit, train) = simulation.construction_train.as_ref().unwrap();
    let mut start = 0.0;
    let mut piece = "unknown";
    for candidate in circuit.pieces() {
        if train.distance() < start + candidate.length() {
            piece = candidate.name;
            break;
        }
        start += candidate.length();
    }
    panic!(
        "{}: distance={}, speed={}, length={}, piece={piece}",
        simulation.construction_message(),
        train.distance(),
        train.speed(),
        circuit.length()
    );
}

#[test]
fn opening_after_a_completed_test_clears_the_report() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    simulation.construction_start(100, 100, 40, 0);
    for (piece, chain) in [
        ("LevelToGentleUp", true),
        ("GentleUp", true),
        ("GentleUp", true),
        ("GentleUp", true),
        ("GentleUp", true),
        ("GentleUp", true),
        ("GentleUpToLevel", true),
        ("Straight", true),
        ("OneTileStraight", false),
        ("OneTileStraight", false),
        ("OffsetLeft", false),
        ("TurnLargeEntry", false),
        ("TurnLargeExit", false),
        ("LevelToGentleDown", false),
        ("GentleDown", false),
        ("GentleDown", false),
        ("GentleDown", false),
        ("GentleDown", false),
        ("GentleDown", false),
        ("GentleDownToLevel", false),
        ("Loop", false),
        ("OffsetRight", false),
        ("TurnLargeEntry", false),
        ("TurnLargeExit", false),
        ("Straight", false),
        ("OneTileStraight", false),
        ("OneTileStraight", false),
    ] {
        simulation.construction_set_chain(chain);
        simulation.construction_select(piece);
        simulation.construction_append();
        assert!(
            simulation.construction_message().is_empty(),
            "{piece}: {}",
            simulation.construction_message()
        );
    }
    assert!(simulation.construction_can_close());
    simulation.construction_close();
    simulation.construction_test();
    for _ in 0..9000 {
        simulation.tick();
        if simulation.construction_message() == "Train completed a lap." {
            let station = &simulation.construction.as_ref().unwrap().pieces()[0];
            let (queue, exit) = super::station_positions(station).unwrap();
            for (position, kind) in [
                (queue, super::PathKind::Queue),
                (exit, super::PathKind::Normal),
            ] {
                simulation.world.set_surface(
                    position.x,
                    position.y,
                    super::Surface {
                        corners: [position.height; 4],
                        water: None,
                    },
                );
                simulation
                    .world
                    .build_path_route(position, position, kind, 0)
                    .unwrap();
            }
            assert!(
                simulation
                    .construction_test_report_json()
                    .contains("\"rating\"")
            );
            assert!(simulation.construction_set_open(true));
            assert_eq!(simulation.construction_test_report_json(), "null");
            return;
        }
        if simulation.construction_outcome.is_some() {
            break;
        }
    }
    let (circuit, train) = simulation.construction_train.as_ref().unwrap();
    let mut start = 0.0;
    let mut piece = "unknown";
    for candidate in circuit.pieces() {
        if train.distance() < start + candidate.length() {
            piece = candidate.name;
            break;
        }
        start += candidate.length();
    }
    panic!(
        "{}: distance={}, speed={}, length={}, piece={piece}",
        simulation.construction_message(),
        train.distance(),
        train.speed(),
        circuit.length()
    );
}

#[test]
fn construction_closes_and_runs_physics_deterministically() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    assert_eq!(simulation.construction_test_report_json(), "null");
    simulation.construction_start(200, 200, 20, 0);
    for piece in [
        "TurnSmallStandalone",
        "TurnSmallStandalone",
        "Straight",
        "TurnSmallStandalone",
        "TurnSmallStandalone",
    ] {
        simulation.construction_select(piece);
        simulation.construction_append();
        assert!(
            simulation.construction_message().is_empty(),
            "{}",
            simulation.construction_message()
        );
    }
    assert!(simulation.construction_can_close());
    simulation.construction_close();
    assert!(simulation.construction_can_test());
    simulation.construction_test();
    assert_eq!(simulation.construction_test_report_json(), "null");
    let initial_poses = simulation.construction_train_poses();
    assert!(initial_poses.chunks_exact(11).all(|car| car[10] == 0.0));
    for _ in 0..9000 {
        simulation.tick();
        if simulation.construction_message() == "Train stopped before completing a lap." {
            break;
        }
    }
    assert_ne!(simulation.construction_train_poses(), initial_poses);
    assert_eq!(
        simulation.construction_message(),
        "Train stopped before completing a lap."
    );
    let report = simulation.construction_test_report_json();
    assert!(report.contains("\"samples\":[{"), "{report}");
    assert!(report.contains("\"longitudinalG\":"), "{report}");
    assert!(report.contains("\"rating\":{"), "{report}");
    assert!(report.contains("\"proximityEvents\":["), "{report}");
    assert!(report.contains("\"notes\":[\""), "{report}");
    let stopped_poses = simulation.construction_train_poses();
    for _ in 0..60 {
        simulation.tick();
    }
    simulation.construction_select("Straight");
    assert_eq!(simulation.construction_train_poses(), stopped_poses);
    assert_eq!(
        simulation.construction_message(),
        "Train stopped before completing a lap."
    );
    simulation.construction_delete(5);
    assert_eq!(simulation.construction_test_report_json(), "null");
    let (_, recovered_train) = simulation.construction_train.as_ref().unwrap();
    assert_eq!(recovered_train.speed(), 0.0);
    let recovered_poses = simulation.construction_train_poses();
    let station = &simulation.construction.as_ref().unwrap().pieces()[0];
    assert_eq!(recovered_train.distance(), station.length());
    let config = &simulation.construction.as_ref().unwrap().model().train;
    assert_eq!(recovered_poses.len(), usize::from(config.cars) * 11);
    for (index, car) in recovered_poses.chunks_exact(11).enumerate() {
        let distance = (station.length()
            - index as f64 * config.length_metres / f64::from(config.cars))
        .max(0.0);
        let expected = station.sample(distance).position / 4.0;
        assert_eq!(
            &car[1..4],
            &[expected.x as f32, expected.y as f32, expected.z as f32]
        );
    }
    for _ in 0..60 {
        simulation.tick();
    }
    assert_eq!(simulation.construction_train_poses(), recovered_poses);
    assert!(!simulation.construction_can_test());
    simulation.construction_delete(0);
    assert!(simulation.construction_train_poses().is_empty());
}

#[test]
fn construction_lists_are_structured_and_preserve_selection_ids() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&simulation.construction_track()).unwrap(),
        serde_json::json!([])
    );
    simulation.construction_start(200, 200, 20, 0);
    let choices: serde_json::Value =
        serde_json::from_str(&simulation.construction_choices()).unwrap();
    assert!(
        choices
            .as_array()
            .unwrap()
            .iter()
            .any(|choice| choice["id"] == "Straight")
    );
    simulation.construction_select("Straight");
    assert_eq!(simulation.construction_selected(), "Straight");
    let track: serde_json::Value = serde_json::from_str(&simulation.construction_track()).unwrap();
    assert_eq!(track[0]["id"], 0);
    assert_eq!(track[0]["label"], "Station");
    assert_eq!(track[0]["kind"], "Station");
    let elements: serde_json::Value =
        serde_json::from_str(&simulation.construction_elements()).unwrap();
    assert!(
        elements
            .as_array()
            .unwrap()
            .iter()
            .any(|element| element["id"] == "element:climb-60")
    );
}

fn track_rows(json: &str) -> Vec<serde_json::Value> {
    serde_json::from_str(json).unwrap()
}

#[test]
fn catalog_selection_ids_match_recorded_session_names() {
    for piece in super::catalog() {
        assert_eq!(
            crate::construction::wire_pieceid(piece.id),
            format!("{:?}", piece.id)
        );
    }
}
