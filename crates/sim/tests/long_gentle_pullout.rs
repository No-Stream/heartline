use theme_park_sim::track::construction::Construction;
use theme_park_sim::track::{Heading, PieceId, TrackState, catalog, wooden_twister_model};
use theme_park_sim::world::{Surface, World};

#[test]
fn adding_the_pullout_preserves_existing_piece_ids_in_saved_data() {
    // This discriminant was used by saved parks before the long pullout existed.
    assert_eq!(
        postcard::to_allocvec(&PieceId::GentleToSteepDown).unwrap(),
        [17]
    );
    assert_eq!(
        postcard::from_bytes::<PieceId>(&[17]).unwrap(),
        PieceId::GentleToSteepDown
    );
}

#[test]
fn briar_long_gentle_pullout_has_a_longer_and_flatter_connector() {
    let pieces = catalog();
    let standard = pieces
        .iter()
        .find(|piece| piece.id == PieceId::GentleDownToLevel)
        .unwrap();
    let long = pieces
        .iter()
        .find(|piece| piece.id == PieceId::LongGentleDownToLevel)
        .unwrap();
    let model = wooden_twister_model();

    assert!(model.allows_piece(long));
    assert_eq!(long.entry.slope, standard.entry.slope);
    assert_eq!(long.exit.slope, standard.exit.slope);
    assert_eq!(long.exit.x, 16);
    assert_eq!(long.exit.z, -4);
    assert!(long.length() > standard.length());
    assert!(long.maximum_curvature() < standard.maximum_curvature());

    let world = World::new(
        128,
        128,
        Surface {
            corners: [0; 4],
            water: None,
        },
    );
    let start = TrackState {
        x: 50,
        y: 50,
        z: 5,
        heading: Heading::North,
        ..TrackState::origin()
    };
    let mut construction = Construction::new(start, model);
    for id in [
        PieceId::Station,
        PieceId::LevelToGentleDown,
        PieceId::GentleDown,
        PieceId::GentleDown,
        PieceId::GentleDown,
    ] {
        construction.append(id, &world, &[]).unwrap();
    }
    assert!(
        construction
            .choices(&world, &[])
            .iter()
            .any(|choice| choice.id == PieceId::LongGentleDownToLevel)
    );
}
