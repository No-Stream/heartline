use serde::Deserialize;
use theme_park_sim::world::{PathKind, Position, Scenery, SceneryKind, World, starter_terrain};

#[derive(Deserialize)]
struct ParkWorldDescription {
    terrain: TerrainDescription,
    paths: Vec<PathDescription>,
    scenery: Vec<SceneryDescription>,
}

#[derive(Deserialize)]
struct TerrainDescription {
    preset: String,
    edits: Vec<TerrainEditDescription>,
}

#[derive(Deserialize)]
struct TerrainEditDescription {
    command: String,
    parameters: [i32; 8],
}

#[derive(Deserialize)]
struct PathDescription {
    kind: String,
    style: u8,
    route: Vec<[i32; 3]>,
}

#[derive(Deserialize)]
struct SceneryDescription {
    kind: String,
    x: u16,
    y: u16,
    rotation: u8,
    quarter: u8,
}

pub fn starter_park_world() -> World {
    let description: ParkWorldDescription =
        serde_json::from_str(include_str!("../../../../content/parks/starter.park.json"))
            .expect("starter park description is valid JSON");
    assert_eq!(description.terrain.preset, "starter");
    let mut world = starter_terrain();

    for edit in description.terrain.edits {
        assert_eq!(edit.command, "carve", "unsupported benchmark terrain edit");
        let [x, y, size, _, _, _, bottom, top] = edit.parameters;
        for tile_y in y..y + size {
            for tile_x in x..x + size {
                let invalidated_paths = world.carve(
                    u16::try_from(tile_x).expect("carve x is nonnegative"),
                    u16::try_from(tile_y).expect("carve y is nonnegative"),
                    i16::try_from(bottom).expect("carve bottom fits i16"),
                    i16::try_from(top).expect("carve top fits i16"),
                );
                assert!(invalidated_paths.is_empty());
            }
        }
    }

    for path in description.paths {
        let kind = match path.kind.as_str() {
            "normal" => PathKind::Normal,
            "queue" => PathKind::Queue,
            other => panic!("unsupported benchmark path kind {other:?}"),
        };
        for segment in path.route.windows(2) {
            world
                .build_path_route(position(segment[0]), position(segment[1]), kind, path.style)
                .expect("starter park path segment builds");
        }
    }

    for item in description.scenery {
        let kind = match item.kind.as_str() {
            "tree_oval" => SceneryKind::TreeOval,
            "bush_flowering" => SceneryKind::BushFlowering,
            "flower_bed" => SceneryKind::FlowerBed,
            "bench" => SceneryKind::Bench,
            "lamp" => SceneryKind::Lamp,
            "planter" => SceneryKind::Planter,
            "tree_cone" => SceneryKind::TreeCone,
            "bin" => SceneryKind::Bin,
            other => panic!("unsupported benchmark scenery kind {other:?}"),
        };
        let scenery = Scenery {
            kind,
            position: Position {
                x: item.x,
                y: item.y,
                height: world.surface(item.x, item.y).corners[0],
            },
            rotation: item.rotation,
            quarter: item.quarter,
        };
        world
            .place_scenery(scenery)
            .expect("starter park scenery places");
    }

    world
}

fn position([x, y, height]: [i32; 3]) -> Position {
    Position {
        x: u16::try_from(x).expect("path x is nonnegative"),
        y: u16::try_from(y).expect("path y is nonnegative"),
        height: i16::try_from(height).expect("path height fits i16"),
    }
}
