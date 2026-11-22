use crate::WasmSimulation;
use crate::builder_session::BuilderSession;
use theme_park_sim::track::ride_file::{RideFile, RideShapeStep, RideStep};
use theme_park_sim::track::shaped::{
    ElementIntent, ElementSpec, HillSizeHint, ResolvedHillProfile, ResolvedTurnGeometry,
    TurnSizeHint,
};
use theme_park_sim::track::{Hand, HillFeel, PieceFamily, PieceId};

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
    }
}

fn append_shaped_intent(session: &mut BuilderSession, intent: &ElementIntent) -> ElementSpec {
    session
        .select(&shape_selection_id(intent), true)
        .expect("shaped intent is offered");
    session.append_selected().expect("shaped intent places");
    session
        .simulation()
        .construction
        .as_ref()
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
    let ride: RideFile = serde_json::from_str(include_str!(
        "../../../content/rides/timber-tumbler.ride.json"
    ))
    .expect("Timber Tumbler ride file is valid");
    simulation.construction_select(&format!("model:{}", ride.model));
    simulation.construction_start(200, 200, 40, 2);

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
fn save_keeps_demo_and_player_test_reports() {
    let mut original = WasmSimulation::new(42, 0, String::from("demo"));
    append_timber_tumbler_fixture(&mut original);
    assert_eq!(original.ride_test("demo"), "");
    assert_eq!(original.ride_test("player"), "");

    let restored = WasmSimulation::load_state(&original.save_state().unwrap()).unwrap();
    assert_eq!(original.state_hash(), restored.state_hash());
    for ride in ["demo", "player"] {
        let report: serde_json::Value =
            serde_json::from_str(&restored.ride_info_json(ride)).unwrap();
        assert!(
            report["report"]["rating"]["excitement"].is_number(),
            "{ride}"
        );
        assert_eq!(restored.ride_info_json(ride), original.ride_info_json(ride));
    }
}

#[test]
fn save_uses_v6_loads_classic_v4_and_v5_saves_and_rejects_legacy_v5_test_reports() {
    let simulation = WasmSimulation::new(42, 20, String::from("demo"));
    let mut bytes = simulation.save_state().unwrap();
    assert_eq!(&bytes[..7], b"TPSAVE\x06");
    for compatible_version in [4, 5] {
        bytes[6] = compatible_version;
        let restored = WasmSimulation::load_state_native(&bytes).unwrap();
        assert_eq!(simulation.state_hash(), restored.state_hash());
    }

    for old_version in [1, 2, 3] {
        bytes[6] = old_version;
        assert!(matches!(
            super::save::checked_payload(&bytes),
            Err(super::save::SaveLoadError::UnsupportedVersion)
        ));
    }

    let mut tested = WasmSimulation::new(42, 0, String::from("demo"));
    assert_eq!(tested.ride_test("demo"), "");
    let mut legacy_tested_bytes = tested.save_state().unwrap();
    legacy_tested_bytes[6] = 5;
    let error = match WasmSimulation::load_state_native_typed(&legacy_tested_bytes) {
        Ok(_) => panic!("v5 save with recorded demo test report unexpectedly loaded"),
        Err(error) => error,
    };
    assert!(matches!(
        error,
        super::save::SaveLoadError::LegacyForceLimitsUnsupported
    ));
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

    let legacy_v4_shaped_save = replace_saved_spec_with_legacy_v4_geometry(
        &bytes,
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
fn invalid_load_returns_a_native_error_without_constructing_js_value() {
    let error = match WasmSimulation::load_state_native(b"invalid") {
        Ok(_) => panic!("invalid save unexpectedly loaded"),
        Err(error) => error,
    };
    assert_eq!(error, "Unsupported or invalid park save version");
}
