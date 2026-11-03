use crate::world::{Direction, Path, Position, Surface, World};

pub fn demo_world() -> World {
    let mut world = World::new(
        512,
        512,
        Surface {
            corners: [8; 4],
            water: None,
        },
    );
    // Adjacent tiles share vertex heights, including the gentle ramp through the path.
    let height = |x: u16| 8 + i16::try_from(x.saturating_sub(245).min(15)).unwrap();
    for y in 0..512 {
        for x in 245..512 {
            let west = height(x);
            let east = height(x + 1);
            world.set_surface(
                x,
                y,
                Surface {
                    corners: [west, east, east, west],
                    water: None,
                },
            );
        }
    }
    for (center_x, center_y) in [(90_u16, 90_u16), (400, 400)] {
        let hill_height = |x: u16, y: u16| {
            let distance = x.abs_diff(center_x).max(y.abs_diff(center_y));
            i16::try_from(12_u16.saturating_sub(distance) / 2).unwrap()
        };
        for y in center_y - 12..center_y + 12 {
            for x in center_x - 12..center_x + 12 {
                world.set_surface(
                    x,
                    y,
                    Surface {
                        corners: [
                            height(x) + hill_height(x, y),
                            height(x + 1) + hill_height(x + 1, y),
                            height(x + 1) + hill_height(x + 1, y + 1),
                            height(x) + hill_height(x, y + 1),
                        ],
                        water: None,
                    },
                );
            }
        }
    }
    for y in 235..245 {
        for x in 230..240 {
            world.set_surface(
                x,
                y,
                Surface {
                    corners: [8; 4],
                    water: Some(9),
                },
            );
        }
    }
    for x in 218..280 {
        world.place_path(Path {
            position: Position {
                x,
                y: 256,
                height: height(x),
            },
            slope: if height(x + 1) > height(x) {
                Some(Direction::East)
            } else {
                None
            },
        });
    }
    for y in 250..256 {
        world.place_path(Path {
            position: Position {
                x: 218,
                y,
                height: 8,
            },
            slope: None,
        });
    }
    for y in 257..264 {
        world.place_path(Path {
            position: Position {
                x: 235,
                y,
                height: 8,
            },
            slope: None,
        });
    }
    world.set_surface(
        219,
        250,
        Surface {
            corners: [8, 7, 7, 8],
            water: None,
        },
    );
    world.place_path(Path {
        position: Position {
            x: 219,
            y: 250,
            height: 7,
        },
        slope: Some(Direction::West),
    });
    for x in 220..224 {
        let path_height = 226 - x as i16;
        world.carve(x, 250, path_height, path_height + 2);
        world.place_path(Path {
            position: Position {
                x,
                y: 250,
                height: path_height,
            },
            slope: Some(Direction::West),
        });
    }
    for y in 250..=254 {
        for x in 224..=228 {
            if x == 224 || x == 228 || y == 250 || y == 254 {
                world.carve(x, y, 3, 5);
                world.place_path(Path {
                    position: Position { x, y, height: 3 },
                    slope: None,
                });
            }
        }
    }
    world
}
