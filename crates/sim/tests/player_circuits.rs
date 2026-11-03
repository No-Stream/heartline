use theme_park_sim::track::construction::Construction;
use theme_park_sim::track::{PieceId, TrackState, catalog, hybrid_model};
use theme_park_sim::world::{Surface, World};

#[test]
fn timber_tumbler_player_circuit_closes() {
    let world = World::new(
        128,
        128,
        Surface {
            corners: [0; 4],
            water: None,
        },
    );
    let start = TrackState {
        x: 100,
        y: 100,
        z: 40,
        ..TrackState::origin()
    };
    let mut construction = Construction::new(start, hybrid_model());
    construction.append(PieceId::Station, &world, &[]).unwrap();
    let mut sections =
        include_str!("../../../web/e2e/player/circuits.txt").split("[Timber Tumbler]");
    sections.next().unwrap();
    let circuit = sections.next().expect("Timber Tumbler circuit missing");
    assert!(
        sections.next().is_none(),
        "duplicate Timber Tumbler circuit"
    );
    let pieces = catalog();
    let mut count = 0;
    for name in circuit
        .lines()
        .map(str::trim)
        .take_while(|line| !line.starts_with('['))
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
    {
        if name == "chain on" {
            construction.set_chain(true);
            continue;
        }
        if name == "chain off" {
            construction.set_chain(false);
            continue;
        }
        let piece = pieces
            .iter()
            .find(|piece| format!("{:?}", piece.id) == name)
            .unwrap_or_else(|| panic!("Unknown piece {name}"));
        construction
            .append(piece.id, &world, &[])
            .unwrap_or_else(|error| panic!("{name}: {error:?}"));
        count += 1;
    }
    assert!(count > 0);
    construction
        .close()
        .expect("Timber Tumbler circuit does not close");
}
