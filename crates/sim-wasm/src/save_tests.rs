use crate::WasmSimulation;
use crate::builder_session::BuilderSession;
use theme_park_sim::park::CoasterId;
use theme_park_sim::park_guests::ParkGuests;
use theme_park_sim::track::ride_file::{RideFile, RideShapeStep, RideStep};
use theme_park_sim::track::shaped::{
    ElementIntent, ElementSpec, HillSizeHint, ResolvedHillProfile, ResolvedTurnGeometry,
    TurnSizeHint,
};
use theme_park_sim::track::{Hand, HillFeel, PieceFamily, PieceId};
use theme_park_sim::world::Position;

#[derive(serde::Serialize)]
enum LegacyV4ElementSpec {
    Hill {
        design_speed_mps: f64,
        cars: u8,
        target_g: f64,
        delivered_g: f64,
        height_m: f64,
        span_m: f64,
        profile: ResolvedHillProfile,
    },
    Turn {
        design_speed_mps: f64,
        cars: u8,
        hand: Hand,
        vertical_g: f64,
        lateral_g: f64,
        radius_m: f64,
        bank_degrees: f64,
        geometry: ResolvedTurnGeometry,
    },
}

impl LegacyV4ElementSpec {
    fn from_spec(spec: &ElementSpec) -> Self {
        match spec {
            ElementSpec::Hill {
                design_speed_mps,
                cars,
                target_g,
                delivered_g,
                height_m,
                span_m,
                profile,
                ..
            } => Self::Hill {
                design_speed_mps: *design_speed_mps,
                cars: *cars,
                target_g: *target_g,
                delivered_g: *delivered_g,
                height_m: *height_m,
                span_m: *span_m,
                profile: profile.clone(),
            },
            ElementSpec::Turn {
                design_speed_mps,
                cars,
                hand,
                vertical_g,
                lateral_g,
                radius_m,
                bank_degrees,
                geometry,
                ..
            } => Self::Turn {
                design_speed_mps: *design_speed_mps,
                cars: *cars,
                hand: *hand,
                vertical_g: *vertical_g,
                lateral_g: *lateral_g,
                radius_m: *radius_m,
                bank_degrees: *bank_degrees,
                geometry: geometry.clone(),
            },
            ElementSpec::Loop { .. }
            | ElementSpec::ZeroGRoll { .. }
            | ElementSpec::RavenTurn { .. } => {
                panic!("legacy v4 saves did not contain shaped inversions")
            }
        }
    }
}

fn assert_legacy_v4_element_spec_is_typed_as_unsupported(spec: &ElementSpec) {
    let legacy_spec = LegacyV4ElementSpec::from_spec(spec);
    let legacy_bytes = postcard::to_allocvec(&legacy_spec).expect("legacy shaped spec serializes");
    let (decoded, legacy_shaped_element_seen) =
        theme_park_sim::track::shaped::with_legacy_v4_save_decode_context(|| {
            postcard::take_from_bytes::<ElementSpec>(&legacy_bytes)
        });
    assert!(
        legacy_shaped_element_seen,
        "legacy shaped spec was not detected"
    );
    let error = match decoded {
        Ok((_, remaining)) => panic!(
            "legacy shaped spec decoded with {} unconsumed bytes",
            remaining.len()
        ),
        Err(error) => error,
    };
    assert!(matches!(
        super::save::decode_error(
            super::save::SaveVersion::V4,
            legacy_shaped_element_seen,
            false,
            error
        ),
        super::save::SaveLoadError::LegacyShapedV4Unsupported
    ));
}

fn shape_selection_id(intent: &ElementIntent) -> String {
    match intent {
        ElementIntent::Hill {
            feel,
            strength_g,
            size,
        } => {
            let feel = match feel {
                HillFeel::Floater => "floater",
                HillFeel::StrongFloater => "strongFloater",
                HillFeel::Ejector => "ejector",
                other => panic!("unsupported shaped hill feel {other:?}"),
            };
            let strength = strength_g.to_string();
            let size = match size {
                HillSizeHint::Auto => "auto".to_owned(),
                HillSizeHint::Height(height) => format!("fixed-{height}"),
            };
            format!("shape:hill:{feel}:{strength}:{size}")
        }
        ElementIntent::Turn { hand, size } => {
            let hand = match hand {
                Hand::Left => "left",
                Hand::Right => "right",
            };
            let size = match size {
                TurnSizeHint::Auto => "auto".to_owned(),
                TurnSizeHint::Radius(radius) => format!("fixed-{radius}"),
            };
            format!("shape:turn:{hand}:fullyBanked:{size}")
        }
        ElementIntent::Loop {
            hand,
            peak_g,
            top_feel,
        } => {
            let hand = match hand {
                Hand::Left => "left",
                Hand::Right => "right",
            };
            let top = match top_feel {
                theme_park_sim::track::LoopTopFeel::Floaty => "floaty",
                theme_park_sim::track::LoopTopFeel::Neutral => "neutral",
                theme_park_sim::track::LoopTopFeel::PressedIn => "pressedIn",
            };
            format!("shape:loop:{hand}:{peak_g}:{top}")
        }
        ElementIntent::ZeroGRoll { hand, peak_g } => {
            let hand = match hand {
                Hand::Left => "left",
                Hand::Right => "right",
            };
            format!("shape:zeroGRoll:{hand}:{peak_g}")
        }
        ElementIntent::RavenTurn {
            hand,
            peak_g,
            top_feel,
        } => {
            let hand = match hand {
                Hand::Left => "left",
                Hand::Right => "right",
            };
            let top = match top_feel {
                theme_park_sim::track::LoopTopFeel::Floaty => "floaty",
                theme_park_sim::track::LoopTopFeel::Neutral => "neutral",
                theme_park_sim::track::LoopTopFeel::PressedIn => "pressedIn",
            };
            format!("shape:ravenTurn:{hand}:{peak_g}:{top}")
        }
    }
}

fn decompress_save_fixture(compressed_bytes: &[u8]) -> Vec<u8> {
    let mut compressed = std::io::Cursor::new(compressed_bytes);
    let mut bytes = Vec::new();
    lzma_rs::xz_decompress(&mut compressed, &mut bytes).expect("decompress save fixture");
    bytes
}

fn append_shaped_intent(session: &mut BuilderSession, intent: &ElementIntent) -> ElementSpec {
    session
        .select(&shape_selection_id(intent), true)
        .expect("shaped intent is offered");
    session.append_selected().expect("shaped intent places");
    session
        .simulation()
        .editing_construction()
        .expect("shaped ride construction remains active")
        .pieces()
        .last()
        .expect("shaped intent appended a piece")
        .element_spec()
        .expect("shaped piece stores its resolved spec")
        .clone()
}

fn corrupt_saved_shaped_spec_digest(save_bytes: &[u8], serialized_spec: &[u8]) -> Vec<u8> {
    let payload_offset = 7 + 8;
    let spec_offset = save_bytes[payload_offset..]
        .windows(serialized_spec.len())
        .position(|window| window == serialized_spec)
        .expect("saved payload contains the shaped spec")
        + payload_offset;
    let mut corrupted_save = save_bytes.to_vec();
    corrupted_save[spec_offset + serialized_spec.len() - 1] ^= 1;
    let payload_checksum = super::save::checksum(&corrupted_save[payload_offset..]);
    corrupted_save[7..payload_offset].copy_from_slice(&payload_checksum.to_le_bytes());
    corrupted_save
}

fn replace_saved_spec_with_legacy_v4_geometry(
    save_bytes: &[u8],
    current_spec: &[u8],
    legacy_spec: &[u8],
) -> Vec<u8> {
    let payload_offset = 7 + 8;
    let current_spec_offset = save_bytes[payload_offset..]
        .windows(current_spec.len())
        .position(|window| window == current_spec)
        .expect("saved payload contains the shaped spec")
        + payload_offset;
    let mut legacy_save = Vec::with_capacity(save_bytes.len() + legacy_spec.len());
    legacy_save.extend_from_slice(&save_bytes[..current_spec_offset]);
    legacy_save.extend_from_slice(legacy_spec);
    legacy_save.extend_from_slice(&save_bytes[current_spec_offset + current_spec.len()..]);
    legacy_save[6] = 4;
    let payload_checksum = super::save::checksum(&legacy_save[payload_offset..]);
    legacy_save[7..payload_offset].copy_from_slice(&payload_checksum.to_le_bytes());
    legacy_save
}

fn append_timber_tumbler_fixture(simulation: &mut WasmSimulation) -> Vec<PieceId> {
    append_timber_tumbler_fixture_at(simulation, 200, 200)
}

fn append_timber_tumbler_fixture_at(
    simulation: &mut WasmSimulation,
    x: i32,
    y: i32,
) -> Vec<PieceId> {
    let ride: RideFile = serde_json::from_str(include_str!(
        "../../../content/rides/timber-tumbler.ride.json"
    ))
    .expect("Timber Tumbler ride file is valid");
    simulation.construction_select(&format!("model:{}", ride.model));
    simulation.construction_start(x, y, 40, 2);

    for step in &ride.steps {
        match step {
            RideStep::Chain(chain) => simulation.construction_set_chain(chain.chain),
            RideStep::Place(place) => {
                simulation.construction_select(&place.place);
                assert!(
                    simulation.construction_can_append(),
                    "{}: {}",
                    place.place,
                    simulation.construction_preview_reason()
                );
                simulation.construction_append();
                assert!(
                    simulation.construction_message().is_empty(),
                    "{}: {}",
                    place.place,
                    simulation.construction_message()
                );
            }
            RideStep::Shape(shape) => {
                let intent_json = serde_json::to_string(&shape.intent).unwrap();
                let spec_json = serde_json::to_string(&shape.spec).unwrap();
                let message = simulation.construction_append_resolved(&intent_json, &spec_json);
                assert!(message.is_empty(), "{}", message);
            }
            RideStep::Close(close) => assert!(close.close),
            RideStep::BlockSections(_) => {}
        }
    }

    let pieces = simulation.editing_construction().unwrap().pieces();
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
fn v5_save_round_trips_a_selected_shaped_hill() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("new"));
    simulation.construction_select("model:hyper");
    simulation.construction_start(100, 100, 20, 0);
    simulation.construction_select("shape:hill:strongFloater:-0.85:fixed-24");
    assert!(simulation.construction_message().is_empty());

    let saved = simulation.save_state_native().unwrap();
    let restored = WasmSimulation::load_state_native(&saved).unwrap();
    let selected = restored
        .construction_selected_shape_wire(true)
        .expect("selected shaped hill survives save loading");

    assert_eq!(
        selected.selection,
        "shape:hill:strongFloater:-0.85:fixed-24"
    );
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
fn generated_park_save_reinitializes_guests_for_the_requested_seed_and_count() {
    let template = WasmSimulation::new(42, 0, String::from("demo"));
    let template_bytes = template.save_state_native().unwrap();

    for (seed, guest_count) in [(17, 0), (2026, 24)] {
        let loaded = WasmSimulation::from_park_save_native(seed, guest_count, &template_bytes)
            .expect("generated park save loads");
        let entrance = loaded
            .park
            .facilities
            .entrance(&loaded.park.world)
            .expect("starter park entrance path");
        let guest_facilities = loaded.park.facilities.guest_facilities(&loaded.park.world);
        let mut expected_guests = ParkGuests::new(
            seed,
            guest_count,
            &loaded.park.world,
            entrance,
            guest_count.max(1),
        );
        expected_guests.open_with_guests(&guest_facilities);

        assert_eq!(
            loaded.park.world.state_hash(),
            template.park.world.state_hash()
        );
        assert_eq!(
            loaded.park.facilities.state_hash(),
            template.park.facilities.state_hash()
        );
        assert_eq!(
            loaded.park.simulation.state_hash(),
            expected_guests.state_hash()
        );
        assert_eq!(loaded.editing_coaster_id_value(), None);
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
        restored.editing_construction().unwrap().model().train.cars,
        4
    );
}

#[test]
fn save_round_trips_built_timber_tumbler_with_ejector_hill() {
    let mut original = WasmSimulation::new(42, 0, String::from("new"));
    let hill_piece_ids = append_timber_tumbler_fixture(&mut original);
    assert!(original.editing_construction().unwrap().is_closed());

    let bytes = original.save_state().unwrap();
    let mut restored = WasmSimulation::load_state(&bytes).unwrap();
    assert_eq!(original.state_hash(), restored.state_hash());
    let restored_hill_piece_ids: Vec<_> = restored
        .editing_construction()
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
fn save_keeps_reports_for_the_starter_and_built_coasters() {
    let mut original = WasmSimulation::new(42, 0, String::from("demo"));
    append_timber_tumbler_fixture(&mut original);
    assert_eq!(original.coaster_test(0), "");
    assert_eq!(original.coaster_test(1), "");

    let restored = WasmSimulation::load_state(&original.save_state().unwrap()).unwrap();
    assert_eq!(original.state_hash(), restored.state_hash());
    for coaster_id in [0, 1] {
        let report: serde_json::Value =
            serde_json::from_str(&restored.coaster_ride_info_json(coaster_id)).unwrap();
        assert!(
            report["report"]["rating"]["excitement"].is_number(),
            "{coaster_id}"
        );
        assert_eq!(
            restored.coaster_ride_info_json(coaster_id),
            original.coaster_ride_info_json(coaster_id)
        );
    }
}

#[test]
fn saves_use_v10_and_round_trip() {
    let compatible_v6_bytes = decompress_save_fixture(include_bytes!(
        "../tests/fixtures/coaster-grouping-v6-compatible.save.xz"
    ));
    assert!(compatible_v6_bytes.starts_with(b"TPSAVE\x06"));
    let expected = WasmSimulation::load_state_native(&compatible_v6_bytes).unwrap();
    let saved_v10 = expected.save_state().unwrap();
    assert!(saved_v10.starts_with(b"TPSAVE\x0a"));
    let v10_round_trip = WasmSimulation::load_state_native(&saved_v10).unwrap();
    assert_eq!(expected.state_hash(), v10_round_trip.state_hash());
}

#[test]
fn v8_demo_save_migrates_to_a_closed_ordinary_coaster_with_a_stationary_train() {
    let v8_bytes = decompress_save_fixture(include_bytes!(
        "../tests/fixtures/starter-park-demo-v8.save.xz"
    ));
    assert!(v8_bytes.starts_with(b"TPSAVE\x08"));

    let mut simulation = WasmSimulation::load_state_native(&v8_bytes).unwrap();
    assert!(simulation.save_state().unwrap().starts_with(b"TPSAVE\x0a"));
    assert_eq!(simulation.coasters().count(), 1);
    let coaster = simulation
        .coaster(CoasterId(0))
        .expect("migrated coaster exists");
    assert_eq!(coaster.construction.model().key, "hybrid");
    assert!(coaster.construction.is_closed());
    assert!(!coaster.coaster_open);
    assert!(coaster.coaster_operation.is_none());
    assert!(coaster.construction_train.is_none());

    let parked_train_poses = simulation.coaster_train_poses(0);
    assert!(
        !parked_train_poses.is_empty(),
        "closed coaster renders its parked train"
    );
    for _ in 0..180 {
        simulation.tick();
    }
    assert_eq!(simulation.coaster_train_poses(0), parked_train_poses);

    let saved_v10 = simulation.save_state().unwrap();
    let restored = WasmSimulation::load_state_native(&saved_v10).unwrap();
    assert_eq!(simulation.state_hash(), restored.state_hash());
    assert_eq!(restored.coaster_train_poses(0), parked_train_poses);
}

#[test]
fn v8_demo_migration_preserves_guest_state_and_assigns_the_next_coaster_id() {
    let v8_fixture = decompress_save_fixture(include_bytes!(
        "../tests/fixtures/starter-park-demo-v8.save.xz"
    ));
    let (_, payload) = super::save::checked_payload(&v8_fixture).unwrap();
    let (mut legacy, remaining) =
        postcard::take_from_bytes::<super::save_v8::WasmSimulationV8>(payload).unwrap();
    assert!(remaining.is_empty());
    let legacy_demo = legacy
        .park
        .demo_coaster
        .take()
        .expect("fixture contains the legacy demo coaster");
    let mut guests = ParkGuests::new(
        42,
        7,
        &legacy.park.world,
        Position {
            x: 218,
            y: 250,
            height: 8,
        },
        7,
    );
    guests.open_with_guests(&legacy.park.facilities.guest_facilities(&legacy.park.world));
    legacy.park.simulation = guests;
    let mut simulation = legacy.into_current().unwrap();

    append_timber_tumbler_fixture_at(&mut simulation, 200, 200);
    let existing_coaster_id = simulation.editing_coaster_id().unwrap();
    assert_eq!(existing_coaster_id, CoasterId(0));
    simulation.pending_coaster_price = 17;
    let existing_track_hash = simulation
        .coaster(existing_coaster_id)
        .unwrap()
        .construction
        .state_hash();
    let guest_state_hash = simulation.simulation.state_hash();

    let v8_payload = super::save_v8::serialize_v8_for_test(&simulation, &Some(legacy_demo), &None);
    let mut mixed_v8_bytes = b"TPSAVE\x08".to_vec();
    mixed_v8_bytes.extend(super::save::checksum(&v8_payload).to_le_bytes());
    mixed_v8_bytes.extend(v8_payload);
    let restored = WasmSimulation::load_state_native(&mixed_v8_bytes).unwrap();

    assert_eq!(restored.editing_coaster_id(), Some(existing_coaster_id));
    assert_eq!(restored.next_coaster_id, 2);
    assert_eq!(restored.coasters().count(), 2);
    assert_eq!(
        restored
            .coaster(existing_coaster_id)
            .unwrap()
            .construction
            .state_hash(),
        existing_track_hash
    );
    assert_eq!(
        restored.coaster(CoasterId(1)).unwrap().coaster_price,
        6,
        "legacy demo price matches the starter park"
    );
    assert_eq!(restored.pending_coaster_price, 17);
    assert_eq!(restored.simulation.state_hash(), guest_state_hash);
}

#[test]
fn frozen_v9_save_layout_loads_and_continues() {
    let v7_bytes = decompress_save_fixture(include_bytes!(
        "../tests/fixtures/coaster-grouping-v7.save.xz"
    ));
    let mut expected = WasmSimulation::load_state_native(&v7_bytes).unwrap();
    let mut legacy_v9 = expected.save_state().unwrap();
    assert!(legacy_v9.starts_with(b"TPSAVE\x0a"));

    // V9 and V10 have the same payload field layout. Relabel a V10 payload with the
    // old header to exercise the frozen V9 decoder and its conversion path.
    legacy_v9[6] = 9;
    let mut restored = WasmSimulation::load_state_native(&legacy_v9).unwrap();
    assert_eq!(expected.state_hash(), restored.state_hash());
    assert_eq!(
        expected.editing_construction().unwrap().pieces().len(),
        restored.editing_construction().unwrap().pieces().len()
    );

    for _ in 0..10 {
        expected.tick();
        restored.tick();
        assert_eq!(expected.state_hash(), restored.state_hash());
    }
}

#[test]
fn legacy_v4_v5_saves_load_and_keep_running() {
    let compatible_v6_bytes = decompress_save_fixture(include_bytes!(
        "../tests/fixtures/coaster-grouping-v6-compatible.save.xz"
    ));
    for old_version in [4, 5] {
        let mut legacy_bytes = compatible_v6_bytes.clone();
        legacy_bytes[6] = old_version;
        let mut expected = WasmSimulation::load_state_native(&compatible_v6_bytes).unwrap();
        let mut restored = WasmSimulation::load_state_native(&legacy_bytes).unwrap();
        assert_eq!(expected.state_hash(), restored.state_hash());

        for _ in 0..10 {
            expected.tick();
            restored.tick();
            assert_eq!(expected.state_hash(), restored.state_hash());
        }

        let saved_again = restored.save_state().unwrap();
        let reloaded = WasmSimulation::load_state_native(&saved_again).unwrap();
        assert_eq!(restored.state_hash(), reloaded.state_hash());
    }

    let mut invalid_bytes = compatible_v6_bytes;
    for unsupported_version in [1, 2, 3] {
        invalid_bytes[6] = unsupported_version;
        assert!(matches!(
            super::save::checked_payload(&invalid_bytes),
            Err(super::save::SaveLoadError::UnsupportedVersion)
        ));
    }
}

#[test]
fn tested_open_v6_fixture_keeps_its_typed_v4_v5_rejection() {
    let rich_v6_bytes = decompress_save_fixture(include_bytes!(
        "../tests/fixtures/coaster-grouping-v6.save.xz"
    ));
    let original = WasmSimulation::load_state_native(&rich_v6_bytes).unwrap();
    let coaster = original.editing_coaster().unwrap();
    assert!(coaster.construction_report.is_some());
    assert!(coaster.coaster_operation.is_some());

    for old_version in [4, 5] {
        let mut legacy_bytes = rich_v6_bytes.clone();
        legacy_bytes[6] = old_version;
        let error = match WasmSimulation::load_state_native_typed(&legacy_bytes) {
            Ok(_) => panic!("rich v{old_version} fixture unexpectedly loaded"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            super::save::SaveLoadError::LegacyForceLimitsUnsupported
        ));
    }
}

#[test]
fn tested_open_v6_fixture_saves_and_reloads_as_v10_without_changing_hash() {
    let rich_v6_bytes = decompress_save_fixture(include_bytes!(
        "../tests/fixtures/coaster-grouping-v6.save.xz"
    ));
    let mut original = WasmSimulation::load_state_native(&rich_v6_bytes).unwrap();
    let saved_v10 = original.save_state().unwrap();
    assert!(saved_v10.starts_with(b"TPSAVE\x0a"));
    let mut restored = WasmSimulation::load_state_native(&saved_v10).unwrap();
    assert_eq!(original.state_hash(), restored.state_hash());

    for _ in 0..10 {
        original.tick();
        restored.tick();
        assert_eq!(original.state_hash(), restored.state_hash());
    }
}

#[test]
fn v7_fixture_migrates_to_v10_and_continues_after_reload() {
    let v7_bytes = decompress_save_fixture(include_bytes!(
        "../tests/fixtures/coaster-grouping-v7.save.xz"
    ));
    let mut original = WasmSimulation::load_state_native(&v7_bytes).unwrap();
    let v10_bytes = original.save_state().unwrap();
    assert!(v10_bytes.starts_with(b"TPSAVE\x0a"));
    let mut restored = WasmSimulation::load_state_native(&v10_bytes).unwrap();
    assert_eq!(original.state_hash(), restored.state_hash());

    for _ in 0..30 {
        original.tick();
        restored.tick();
        assert_eq!(original.state_hash(), restored.state_hash());
    }
}

#[test]
fn unreadable_v4_save_keeps_a_typed_decode_error() {
    let payload = b"not a postcard payload";
    let mut bytes = b"TPSAVE\x04".to_vec();
    bytes.extend(super::save::checksum(payload).to_le_bytes());
    bytes.extend(payload);

    let error = match WasmSimulation::load_state_native_typed(&bytes) {
        Ok(_) => panic!("invalid v4 payload unexpectedly loaded"),
        Err(error) => error,
    };
    assert!(matches!(error, super::save::SaveLoadError::Decode(_)));
}

#[test]
fn save_round_trips_horizon_hawk_shaped_ride_under_two_mebibytes() {
    let mut ride_json: serde_json::Value = serde_json::from_str(include_str!(
        "../../../content/rides/horizon-hawk.ride.json"
    ))
    .expect("Horizon Hawk JSON is valid");
    let original_steps = ride_json["steps"]
        .as_array_mut()
        .expect("Horizon Hawk steps are an array")
        .clone();
    let shaped_step_indices: Vec<usize> = original_steps
        .iter()
        .enumerate()
        .filter_map(|(index, step)| step.get("intent").map(|_| index))
        .collect();
    let [first_shape_index, second_shape_index, ..] = shaped_step_indices.as_slice() else {
        panic!("Horizon Hawk has a shaped turn and a shaped hill");
    };
    let first_intent: ElementIntent =
        serde_json::from_value(original_steps[*first_shape_index]["intent"].clone())
            .expect("Horizon Hawk first shaped intent is valid");
    let second_intent: ElementIntent =
        serde_json::from_value(original_steps[*second_shape_index]["intent"].clone())
            .expect("Horizon Hawk second shaped intent is valid");
    ride_json["steps"] = serde_json::Value::Array(original_steps[..*first_shape_index].to_vec());
    let mut ride: RideFile = serde_json::from_value(ride_json)
        .expect("Horizon Hawk prefix before its shaped turn is valid");

    let mut prefix = BuilderSession::new(WasmSimulation::new(42, 0, String::from("new")));
    prefix
        .replay_ride(&ride)
        .expect("Horizon Hawk prefix replays before its shaped turn");
    let first_spec = append_shaped_intent(&mut prefix, &first_intent);
    assert_legacy_v4_element_spec_is_typed_as_unsupported(&first_spec);
    let first_spec_bytes = postcard::to_allocvec(&first_spec).expect("turn spec serializes");
    let legacy_first_spec_bytes =
        postcard::to_allocvec(&LegacyV4ElementSpec::from_spec(&first_spec))
            .expect("legacy v4 turn spec serializes");
    ride.steps.push(RideStep::Shape(RideShapeStep {
        intent: first_intent,
        spec: first_spec,
    }));
    ride.steps.extend(
        original_steps[*first_shape_index + 1..*second_shape_index]
            .iter()
            .map(|step| serde_json::from_value::<RideStep>(step.clone()))
            .collect::<Result<Vec<_>, _>>()
            .expect("Horizon Hawk steps between the first two shapes are classic"),
    );

    let mut before_second_shape =
        BuilderSession::new(WasmSimulation::new(42, 0, String::from("new")));
    before_second_shape
        .replay_ride(&ride)
        .expect("Horizon Hawk ride replays before its shaped hill");
    let classic_save_size = before_second_shape
        .simulation()
        .save_state_native()
        .expect("Horizon Hawk prefix saves")
        .len();
    let second_spec = append_shaped_intent(&mut before_second_shape, &second_intent);
    assert_legacy_v4_element_spec_is_typed_as_unsupported(&second_spec);
    let second_spec_bytes =
        postcard::to_allocvec(&second_spec).expect("shaped recipe serializes as postcard");
    ride.steps.push(RideStep::Shape(RideShapeStep {
        intent: second_intent,
        spec: second_spec,
    }));

    let mut original = BuilderSession::new(WasmSimulation::new(42, 0, String::from("new")));
    original
        .replay_ride(&ride)
        .expect("Horizon Hawk shaped ride replays");

    let expected_hash = original.state_hash();
    let bytes = original
        .simulation()
        .save_state_native()
        .expect("Horizon Hawk shaped ride saves");
    assert!(
        bytes.len() < 2 * 1024 * 1024,
        "save is {} bytes",
        bytes.len()
    );
    let shaped_hill_overhead = bytes.len() - classic_save_size;
    assert!(
        shaped_hill_overhead < 32 * 1024,
        "adding the shaped hill increased the save by {shaped_hill_overhead} bytes"
    );

    let corrupted_save = corrupt_saved_shaped_spec_digest(&bytes, &second_spec_bytes);
    let mismatch_error = match WasmSimulation::load_state_native_typed(&corrupted_save) {
        Ok(_) => panic!("shaped save with a changed geometry digest unexpectedly loaded"),
        Err(error) => error,
    };
    assert!(matches!(
        mismatch_error,
        super::save::SaveLoadError::ShapedGeometryDigestMismatch(mismatch)
            if mismatch.expected != mismatch.actual
    ));

    let v6_shaped_save = super::save::save_state_v6_for_test(original.simulation());
    let legacy_v4_shaped_save = replace_saved_spec_with_legacy_v4_geometry(
        &v6_shaped_save,
        &first_spec_bytes,
        &legacy_first_spec_bytes,
    );
    let v4_error = match WasmSimulation::load_state_native_typed(&legacy_v4_shaped_save) {
        Ok(_) => panic!("v4 shaped save unexpectedly loaded"),
        Err(error) => error,
    };
    assert!(matches!(
        v4_error,
        super::save::SaveLoadError::LegacyShapedV4Unsupported
    ));

    let restored = WasmSimulation::load_state_native(&bytes).expect("shaped ride save reloads");
    assert_eq!(restored.state_hash(), expected_hash);

    let mut replayed = BuilderSession::new(WasmSimulation::new(42, 0, String::from("new")));
    replayed
        .replay_ride(&ride)
        .expect("Horizon Hawk shaped ride replays after save reload");
    assert_eq!(restored.state_hash(), replayed.state_hash());
}

#[test]
fn v10_save_round_trips_shaped_inversion_intents_and_pieces() {
    for (model_key, selection, expected_piece_id) in [
        (
            "velocity_viper",
            "shape:loop:left:4:neutral",
            PieceId::ShapedLoopLeft,
        ),
        (
            "velocity_viper",
            "shape:loop:right:4:neutral",
            PieceId::ShapedLoopRight,
        ),
        (
            "velocity_viper",
            "shape:zeroGRoll:right:3.5",
            PieceId::ShapedZeroGRollRight,
        ),
        (
            "velocity_viper",
            "shape:zeroGRoll:left:3.5",
            PieceId::ShapedZeroGRollLeft,
        ),
        (
            "velocity_viper",
            "shape:ravenTurn:left:4:floaty",
            PieceId::ShapedRavenTurnLeft,
        ),
        (
            "velocity_viper",
            "shape:ravenTurn:right:4:floaty",
            PieceId::ShapedRavenTurnRight,
        ),
        (
            "wooden_twister",
            "shape:loop:left:3.5:neutral",
            PieceId::ShapedLoopLeft,
        ),
    ] {
        let mut simulation = WasmSimulation::new(42, 0, String::from("new"));
        simulation.construction_select(&format!("model:{model_key}"));
        simulation.construction_start(128, 128, 60, 0);
        let prefix_piece_ids: &[&str] = if model_key == "wooden_twister" {
            &[
                "LevelToGentleDown",
                "GentleToSteepDown",
                "SteepToGentleDown",
                "GentleDown",
                "GentleDownToLevel",
                "Straight",
            ]
        } else {
            &[
                "LevelToGentleDown",
                "GentleToSteepDown",
                "SteepDown",
                "SteepToGentleDown",
                "GentleDownToLevel",
                "Straight",
            ]
        };
        for piece_id in prefix_piece_ids {
            simulation.construction_select(piece_id);
            assert!(
                simulation.construction_can_append(),
                "{model_key} prefix piece {piece_id} cannot append: {}",
                simulation.construction_message()
            );
            simulation.construction_append();
            assert!(simulation.construction_message().is_empty());
        }
        simulation.construction_select(selection);
        assert!(
            simulation.construction_message().is_empty(),
            "{model_key} {selection}: {}",
            simulation.construction_message()
        );

        let selected_save = simulation.save_state_native().unwrap();
        assert!(selected_save.starts_with(b"TPSAVE\x0a"));
        let selected_restore = WasmSimulation::load_state_native(&selected_save).unwrap();
        let selected = selected_restore
            .construction_selected_shape_wire(true)
            .expect("selected shaped inversion survives save loading");
        assert_eq!(selected.selection, selection);

        assert!(simulation.construction_can_append());
        simulation.construction_append();
        assert!(
            simulation.construction_message().is_empty(),
            "{model_key} {selection} append: {}",
            simulation.construction_message()
        );
        assert_eq!(
            simulation
                .editing_construction()
                .unwrap()
                .pieces()
                .last()
                .unwrap()
                .id,
            expected_piece_id
        );

        let saved = simulation.save_state_native().unwrap();
        assert!(saved.starts_with(b"TPSAVE\x0a"));
        let restored = WasmSimulation::load_state_native(&saved).unwrap();
        assert_eq!(
            restored
                .editing_construction()
                .unwrap()
                .pieces()
                .last()
                .unwrap()
                .id,
            expected_piece_id
        );
    }
}

#[test]
fn invalid_load_returns_a_native_error_without_constructing_js_value() {
    let error = match WasmSimulation::load_state_native(b"invalid") {
        Ok(_) => panic!("invalid save unexpectedly loaded"),
        Err(error) => error,
    };
    assert_eq!(error, "Unsupported or invalid park save version");
}
