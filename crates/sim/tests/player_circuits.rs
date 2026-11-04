use theme_park_sim::track::construction::Construction;
use theme_park_sim::track::{
    PieceId, PieceKind, TrackState, catalog, hybrid_model, multi_launch_model, wooden_twister_model,
};
use theme_park_sim::world::{Surface, World};

#[test]
fn player_circuits_close() {
    for (name, model, requires_lift) in [
        ("Timber Tumbler", hybrid_model(), true),
        ("Velocity Viper", multi_launch_model(), false),
        ("Briar Blitz", wooden_twister_model(), true),
    ] {
        let world = World::new(
            512,
            512,
            Surface {
                corners: [8; 4],
                water: None,
            },
        );
        let start = TrackState {
            x: 482,
            y: 498,
            z: 6,
            heading: theme_park_sim::track::Heading::North,
            ..TrackState::origin()
        };
        let mut construction = Construction::new(start, model);
        construction.append(PieceId::Station, &world, &[]).unwrap();
        let header = format!("[{name}]");
        let mut sections = include_str!("../../../web/e2e/player/circuits.txt").split(&header);
        sections.next().unwrap();
        let circuit = sections
            .next()
            .unwrap_or_else(|| panic!("{name} circuit missing"));
        assert!(sections.next().is_none(), "duplicate {name} circuit");
        let pieces = catalog();
        let mut count = 0;
        let mut chain_piece_count = 0;
        for name in circuit
            .lines()
            .map(str::trim)
            .take_while(|line| !line.starts_with('['))
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
        {
            match name {
                "+chain" => {
                    construction.set_chain(true);
                    continue;
                }
                "-chain" => {
                    construction.set_chain(false);
                    continue;
                }
                _ => {}
            }
            let piece = pieces
                .iter()
                .find(|piece| format!("{:?}", piece.id) == name)
                .unwrap_or_else(|| panic!("Unknown piece {name}"));
            construction
                .append(piece.id, &world, &[])
                .unwrap_or_else(|error| panic!("{name}: {error:?}"));
            if construction.pieces().last().unwrap().kind == PieceKind::Lift {
                chain_piece_count += 1;
            }
            count += 1;
        }
        assert!(count > 0);
        assert!(
            !requires_lift || chain_piece_count > 0,
            "{name} circuit has no chain-lift pieces"
        );
        let end = construction.end();
        construction.close().unwrap_or_else(|error| {
            panic!("{name} circuit does not close from {end:?}: {error:?}")
        });
    }
}
