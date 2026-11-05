use super::GuestId;
use super::behavior::{Facility, KnownPlace, PlaceKind};
use super::details::GuestMemoryDetail;
use super::park::ParkGuests;
use crate::world::{PathKind, Position, Surface, World};

fn position(x: u16, y: u16) -> Position {
    Position { x, y, height: 0 }
}

#[test]
fn removing_and_rebuilding_path_restores_food_visit() {
    let entrance = position(0, 0);
    let missing_path = position(2, 0);
    let food_queue_end = position(4, 3);
    let mut world = World::new(
        5,
        4,
        Surface {
            corners: [0; 4],
            water: None,
        },
    );
    world
        .build_path_route(entrance, position(4, 0), PathKind::Normal, 0)
        .unwrap();
    world
        .build_path_route(position(4, 0), food_queue_end, PathKind::Queue, 0)
        .unwrap();
    let food = Facility {
        place: KnownPlace {
            id: 42,
            position: food_queue_end,
            kind: PlaceKind::Food,
            minimum_height_cm: 0,
            intensity: 0,
            nausea: 0,
            ride_type: 0,
            theme: 0,
            price: 0,
        },
        capacity: 1,
        duration_ticks: 10,
        exit_position: None,
    };
    let guest = GuestId(0);
    let mut park = ParkGuests::new(17, 1, &world, entrance, 1);
    park.open_with_guests(&[food]);
    park.walking.place_on_path(guest, entrance);
    park.behaviors.guests[guest.0 as usize].needs.hunger = 100;

    assert_eq!(world.remove_path(missing_path), vec![missing_path]);
    for _ in 0..60 {
        park.tick(&world, &[food], 100, None);
    }
    assert_eq!(park.behaviors.guests[guest.0 as usize].stops_completed, 0);
    assert!(
        !park
            .guest_detail(guest)
            .unwrap()
            .memories
            .iter()
            .any(|memory| matches!(memory, GuestMemoryDetail::Food { place_id: 42, .. }))
    );

    world
        .build_path_route(missing_path, missing_path, PathKind::Normal, 0)
        .unwrap();
    let mut food_visit_completed = false;
    let mut visited_queue_paths = Vec::new();
    for _ in 0..5_000 {
        park.tick(&world, &[food], 100, None);
        if let Some(path) = park.walking.guests()[guest.0 as usize].settled_path() {
            if world
                .path_metadata(path)
                .is_some_and(|metadata| metadata.kind == PathKind::Queue)
                && visited_queue_paths.last() != Some(&path)
            {
                visited_queue_paths.push(path);
            }
        }
        if park.behaviors.guests[guest.0 as usize].stops_completed > 0 {
            food_visit_completed = true;
            break;
        }
    }

    assert!(food_visit_completed);
    assert_eq!(
        visited_queue_paths,
        vec![position(4, 1), position(4, 2), food_queue_end]
    );
    assert_eq!(park.behaviors.guests[guest.0 as usize].needs.hunger, 0);
    assert!(
        park.guest_detail(guest)
            .unwrap()
            .memories
            .iter()
            .any(|memory| matches!(memory, GuestMemoryDetail::Food { place_id: 42, .. }))
    );
}
