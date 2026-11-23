use super::{Surface, World};

/// Creates the height field and water used by the starter park.
pub fn starter_terrain() -> World {
    let mut world = World::new(
        512,
        512,
        Surface {
            corners: [8; 4],
            water: None,
        },
    );
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
    world.set_surface(
        219,
        250,
        Surface {
            corners: [8, 7, 7, 8],
            water: None,
        },
    );
    world
}

#[cfg(test)]
mod tests {
    use super::starter_terrain;

    #[test]
    fn starter_terrain_preserves_the_old_demo_surface_and_water_data() {
        let terrain = starter_terrain();

        assert_eq!(terrain.dimensions(), (512, 512));
        assert_eq!(terrain.surface(244, 256).corners, [8; 4]);
        assert_eq!(terrain.surface(245, 256).corners, [8, 9, 9, 8]);
        assert_eq!(terrain.surface(260, 256).corners, [23; 4]);
        assert_eq!(terrain.surface(90, 90).corners, [14, 13, 13, 13]);
        assert_eq!(terrain.surface(387, 400).corners, [23; 4]);
        assert_eq!(terrain.surface(219, 250).corners, [8, 7, 7, 8]);
        assert_eq!(terrain.surface(230, 235).water, Some(9));
        assert_eq!(terrain.surface(229, 235).water, None);
    }
}
