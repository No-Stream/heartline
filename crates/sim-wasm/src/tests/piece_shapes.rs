#[test]
fn catalog_shape_variants_have_unique_piece_and_exit_slope_keys() {
    let pieces = super::catalog();
    let piece_count = pieces.len();
    let keys: std::collections::HashSet<_> = pieces
        .into_iter()
        .map(|piece| (piece.id, format!("{:?}", piece.exit.slope)))
        .collect();
    assert_eq!(keys.len(), piece_count);
}

#[test]
fn launch_shapes_expose_their_kind() {
    let json = super::piece_shapes_json();
    for id in ["LaunchShort", "Launch", "LaunchLong"] {
        let shape = json
            .split("{\"id\":")
            .find(|shape| shape.starts_with(&format!("\"{id}\"")))
            .unwrap();
        assert!(shape.contains("\"kind\":\"Launch\""));
    }
}

#[test]
fn piece_shapes_describe_every_catalog_piece_from_a_shared_entry_frame() {
    let json = super::piece_shapes_json();
    assert_eq!(json.matches("\"id\":").count(), super::catalog().len());
    let straight = json
        .split("{\"id\":")
        .find(|shape| shape.starts_with("\"Straight\""))
        .expect("straight shape");
    assert!(straight.contains("\"turn\":0"));
    assert!(straight.starts_with("\"Straight\",\"name\":\"Straight\",\"kind\":\"Track\""));
    assert!(straight.contains("[0.000,0.000,0.000,0.000,0.000,1.000]"));
    assert!(straight.contains("[16.000,0.000,0.000,0.000,0.000,1.000]]"));
    let turn = json
        .split("{\"id\":")
        .find(|shape| shape.starts_with("\"TurnSmallStandalone\""))
        .expect("small turn shape");
    assert!(turn.contains("\"turn\":2"));
}
