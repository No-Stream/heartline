use super::tick::track_theming;
use crate::physics::{CarReading, GForces, TestRun, TestRunRecorder, demo_circuit};
use crate::track::Piece;
use crate::world::{Position, Scenery, SceneryKind, Surface, World};

fn world_with_surface(height: i16) -> World {
    World::new(
        512,
        512,
        Surface {
            corners: [height; 4],
            water: None,
        },
    )
}

fn report_for(circuit: &crate::physics::Circuit) -> TestRun {
    let mut recorder = TestRunRecorder::for_circuit(circuit);
    let mut frames = Vec::new();
    let mut distance = 0.0;
    while distance < circuit.length() {
        let sample = circuit.sample(distance);
        frames.push(vec![CarReading {
            distance,
            speed: 20.0,
            g: GForces {
                vertical: 1.0,
                lateral: 0.0,
                longitudinal: 0.0,
            },
            airtime: false,
            height_m: sample.position.z,
            inverted: sample.up.z < 0.0,
            roll: sample.roll,
            piece_index: circuit.piece_index(distance),
        }]);
        distance += 1.0;
    }
    recorder.record(&frames, "Timber Tumbler");
    recorder.finish(true, false)
}

fn place_scenery_near_track(
    world: &mut World,
    kind: SceneryKind,
    pieces: &[Piece],
    offset_x: i32,
    offset_y: i32,
) {
    let sample = pieces[0].sample(0.0);
    let tile_x = (sample.position.x / 4.0).floor() as u16;
    let tile_y = (sample.position.y / 4.0).floor() as u16;
    world
        .place_scenery(Scenery {
            kind,
            position: Position {
                x: u16::try_from(i32::from(tile_x) + offset_x).unwrap(),
                y: u16::try_from(i32::from(tile_y) + offset_y).unwrap(),
                height: sample.position.z.round() as i16,
            },
            quarter: 0,
            rotation: 0,
        })
        .expect("test scenery placement");
}

#[test]
fn nearby_scenery_adds_at_most_eight_tenths_to_a_park_ride_score() {
    let circuit = demo_circuit();
    let bare_world = world_with_surface(20);
    let mut themed_world = world_with_surface(20);
    for (kind, offset_x, offset_y) in [
        (SceneryKind::TreeCone, 3, 0),
        (SceneryKind::FlowerBed, 0, 3),
        (SceneryKind::Lamp, -3, 0),
        (SceneryKind::Bench, 0, -3),
    ] {
        place_scenery_near_track(
            &mut themed_world,
            kind,
            circuit.pieces(),
            offset_x,
            offset_y,
        );
    }

    let bare_theming = track_theming(&bare_world, circuit.pieces());
    let themed_theming = track_theming(&themed_world, circuit.pieces());
    assert_eq!(bare_theming, 0.0);
    assert!(themed_theming > bare_theming);

    let report = report_for(&circuit);
    let bare_excitement = report.excitement;
    let mut bare_report = report.clone();
    let mut themed_report = report;
    bare_report.set_theming(bare_theming);
    themed_report.set_theming(themed_theming);

    let excitement_gain = themed_report.excitement - bare_report.excitement;
    assert!(excitement_gain > 0.0);
    assert!(excitement_gain <= 0.8 + 1e-12);
    assert_eq!(bare_report.excitement, bare_excitement);
}

#[test]
fn garden_and_woodland_scenery_have_equal_track_theming_weight() {
    let circuit = demo_circuit();
    let mut garden_world = world_with_surface(20);
    let mut woodland_world = world_with_surface(20);
    place_scenery_near_track(
        &mut garden_world,
        SceneryKind::FlowerBed,
        circuit.pieces(),
        3,
        0,
    );
    place_scenery_near_track(
        &mut woodland_world,
        SceneryKind::TreeCone,
        circuit.pieces(),
        3,
        0,
    );

    assert_eq!(
        track_theming(&garden_world, circuit.pieces()),
        track_theming(&woodland_world, circuit.pieces())
    );
}
