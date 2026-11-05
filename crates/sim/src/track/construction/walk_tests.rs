use super::{Construction, ConstructionError, PieceChoice};
use crate::demo_world::demo_world;
use crate::track::{
    CoasterModel, PieceId, TrackState, hybrid_model, multi_launch_model, wooden_twister_model,
};
use crate::world::{Surface, World};

struct Random(u64);

impl Random {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut value = self.0;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
        value ^ (value >> 31)
    }
}

fn worlds() -> Vec<(&'static str, World, i32, i32)> {
    let flat = Surface {
        corners: [0; 4],
        water: None,
    };
    let mut cramped = World::new(32, 32, flat);
    for y in 0..32 {
        cramped.set_surface(
            28,
            y,
            Surface {
                corners: [40; 4],
                water: None,
            },
        );
    }
    vec![
        ("open", World::new(128, 128, flat), 32, 128),
        ("demo", demo_world(), 100, 100),
        ("cramped", cramped, 40, 32),
    ]
}

fn rebuild(
    start: TrackState,
    model: CoasterModel,
    world: &World,
    prefix: &[(PieceId, bool)],
    chain: bool,
) -> Construction {
    let mut construction = Construction::new(start, model);
    for &(piece, piece_chain) in prefix {
        construction.set_chain(piece_chain);
        construction
            .append(piece, world, &[])
            .unwrap_or_else(|error| panic!("cannot append {piece:?}: {error:?}"));
    }
    construction.set_chain(chain);
    construction
}

fn assert_refusal_and_undo(
    construction: &mut Construction,
    world: &World,
    prefix: &[(PieceId, bool)],
    context: &str,
) {
    let visible = construction.available_choices(world, &[]);
    assert!(
        !visible.is_empty()
            && visible.iter().all(|(_, choice)| !matches!(
                choice.result,
                Err(ConstructionError::Unavailable | ConstructionError::ModelForbidden)
            )),
        "{context}: no displayable refusal: {prefix:?}"
    );
    let (id, error) = visible
        .iter()
        .find_map(|(_, choice)| choice.result.err().map(|error| (choice.id, error)))
        .unwrap();
    let before = construction.state_hash();
    assert_eq!(
        construction.append(id, world, &[]),
        Err(error),
        "{context}: {prefix:?}"
    );
    assert_eq!(
        construction.state_hash(),
        before,
        "{context}: refusal changed state: {prefix:?}"
    );
    let last_id = *construction.piece_ids().last().unwrap();
    assert_eq!(
        construction.delete_from(last_id).unwrap()[0].id,
        prefix.last().unwrap().0,
        "{context}: {prefix:?}"
    );
    let rebuilt = rebuild(
        construction.start(),
        construction.model().clone(),
        world,
        &prefix[..prefix.len() - 1],
        construction.chain(),
    );
    assert_eq!(
        construction.end(),
        rebuilt.end(),
        "{context}: undo changed endpoint: {prefix:?}"
    );
    assert_eq!(
        construction.choices(world, &[]),
        rebuilt.choices(world, &[]),
        "{context}: undo choices: {prefix:?}"
    );
}

#[test]
fn seeded_timber_tumbler_walks_have_consistent_choices() {
    run_seeded_walks(hybrid_model);
}

#[test]
fn seeded_velocity_viper_walks_have_consistent_choices() {
    run_seeded_walks(multi_launch_model);
}

#[test]
fn seeded_briar_blitz_walks_have_consistent_choices() {
    run_seeded_walks(wooden_twister_model);
}

/// FLEXIBILITY_WALKS (any positive value) switches from the fast matrix to the full one.
fn run_seeded_walks(model_fn: fn() -> CoasterModel) {
    let configured = std::env::var("FLEXIBILITY_WALKS").ok().map(|value| {
        value
            .parse::<u64>()
            .expect("FLEXIBILITY_WALKS must be an integer")
    });
    if let Some(value) = configured {
        assert!(value > 0, "FLEXIBILITY_WALKS must be positive");
    }
    let full = configured.is_some();
    let walk_count = if full { 8 } else { 1 };
    let steps = if full { 30 } else { 15 };
    for (world_name, world, x, y) in worlds()
        .into_iter()
        .filter(|(name, _, _, _)| full || *name == "open")
    {
        let ground = i32::from(world.surface((x / 2) as u16, (y / 2) as u16).corners[0]);
        {
            for height in [4, 12, 30, 100]
                .into_iter()
                .filter(|height| full || [4, 30].contains(height))
            {
                for seed in 0..walk_count {
                    let model = model_fn();
                    let start = TrackState {
                        x,
                        y,
                        z: ((ground + 1) / 2 * 2 + height) / 2,
                        ..TrackState::origin()
                    };
                    let context = format!(
                        "model={}, world={world_name}, height={height}, seed={seed}",
                        model.name
                    );
                    let mut construction = Construction::new(start, model.clone());
                    construction
                        .append(PieceId::Station, &world, &[])
                        .unwrap_or_else(|error| panic!("{context}: station: {error:?}"));
                    let mut prefix = vec![(PieceId::Station, false)];
                    let mut random = Random(seed);
                    for _ in 0..steps {
                        construction.set_chain(random.next() % 2 == 0);
                        let choices = construction.choices(&world, &[]);
                        let rebuilt =
                            rebuild(start, model.clone(), &world, &prefix, construction.chain());
                        assert_eq!(
                            choices,
                            rebuilt.choices(&world, &[]),
                            "{context}: {prefix:?}"
                        );
                        let valid: Vec<&PieceChoice> = choices
                            .iter()
                            .filter(|choice| choice.result.is_ok())
                            .collect();
                        if valid.is_empty() {
                            assert_refusal_and_undo(&mut construction, &world, &prefix, &context);
                            break;
                        }
                        let choice = valid[(random.next() as usize) % valid.len()];
                        construction
                            .append(choice.id, &world, &[])
                            .unwrap_or_else(|error| {
                                panic!(
                                    "{context}: appending {:?} after {prefix:?}: {error:?}",
                                    choice.id
                                )
                            });
                        prefix.push((choice.id, construction.chain()));
                    }
                }
            }
        }
    }
}

#[test]
fn refused_append_and_undo_restore_self_obstructed_walk() {
    let world_name = "open";
    let world = World::new(
        512,
        512,
        Surface {
            corners: [0; 4],
            water: None,
        },
    );
    let start = TrackState {
        x: 128,
        y: 256,
        z: 2,
        ..TrackState::origin()
    };
    let prefix = [
        PieceId::Station,
        PieceId::HeartlineRoll,
        PieceId::Straight,
        PieceId::TurnSmallStandalone,
        PieceId::HeartlineRoll,
        PieceId::TurnLargeRightEntry,
        PieceId::TurnLargeRight,
        PieceId::TurnLargeRightExit,
        PieceId::OffsetLeft,
        PieceId::HalfTileStraight,
        PieceId::HalfTileStraight,
    ]
    .map(|id| (id, false));
    let mut construction = rebuild(start, hybrid_model(), &world, &prefix, false);
    let choices = construction.choices(&world, &[]);
    assert!(
        choices.iter().all(|choice| choice.result.is_err()),
        "valid choices: {:?}",
        choices
            .iter()
            .filter(|choice| choice.result.is_ok())
            .map(|choice| choice.id)
            .collect::<Vec<_>>()
    );
    assert_refusal_and_undo(&mut construction, &world, &prefix, world_name);
}
