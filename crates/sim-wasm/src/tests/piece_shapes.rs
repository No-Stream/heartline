#[test]
fn catalog_names_uniquely_identify_shapes_in_construction_track() {
    let names: std::collections::HashSet<_> = super::catalog()
        .into_iter()
        .map(|piece| piece.name)
        .collect();
    assert_eq!(names.len(), super::catalog().len());
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
