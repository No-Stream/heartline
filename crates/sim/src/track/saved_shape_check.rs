//! Checks that a ride file's saved shaped geometry is what the current solver produces for its intent.
//!
//! The builder re-solves each shaped intent when a player places it, while ride replays append the saved
//! geometry. A ride saved by an older solver therefore replays differently from the same ride built in the game.

use crate::physics::{PhysicsConfig, PreviewCache};
use crate::track::construction::Construction;
use crate::track::ride_file::{RideFile, RideHeading, RideStep};
use crate::track::shaped::{SolveContext, solve};
use crate::track::{Heading, PieceId, TrackState, catalog, coaster_models};
use crate::world::{Surface, World};

/// Replays `ride` with the builder's 5 m/s lift preview and re-solves every shaped intent at its replayed entry
/// state. Returns an error naming the first step whose saved geometry digest differs from the re-solved digest.
pub fn check_saved_shaped_geometry(ride: &RideFile) -> Result<(), String> {
    let world = World::new(
        512,
        512,
        Surface {
            corners: [8; 4],
            water: None,
        },
    );
    let physics = PhysicsConfig {
        lift_speed: 5.0,
        ..PhysicsConfig::default()
    };
    let mut model = coaster_models()
        .into_iter()
        .find(|model| model.key == ride.model)
        .ok_or_else(|| format!("unknown model {} in ride {}", ride.model, ride.name))?;
    model.train.length_metres *= f64::from(ride.cars_per_train) / f64::from(model.train.cars);
    model.train.cars = ride.cars_per_train;
    let start = TrackState {
        x: ride.station.x * 2,
        y: ride.station.y * 2,
        z: ride.station.z / 2,
        heading: match ride.station.heading {
            RideHeading::East => Heading::East,
            RideHeading::North => Heading::North,
            RideHeading::West => Heading::West,
            RideHeading::South => Heading::South,
        },
        ..TrackState::origin()
    };
    let mut construction = Construction::new(start, model.clone());
    construction
        .append(PieceId::Station, &world, &[])
        .map_err(|error| format!("{} station: {error:?}", ride.name))?;
    let mut preview_cache = PreviewCache::default();

    for (step_index, step) in ride.steps.iter().enumerate() {
        let step_number = step_index + 1;
        match step {
            RideStep::Chain(chain) => construction.set_chain(chain.chain),
            RideStep::Place(place) => {
                let piece_id = catalog()
                    .iter()
                    .find(|candidate| format!("{:?}", candidate.id) == place.place)
                    .ok_or_else(|| {
                        format!("ride step {step_number}: unknown piece {}", place.place)
                    })?
                    .id;
                construction
                    .append(piece_id, &world, &[])
                    .map_err(|error| {
                        format!("ride step {step_number} {}: {error:?}", place.place)
                    })?;
                if let Some(launch_speed) = place.launch_speed {
                    let piece_id = *construction
                        .piece_ids()
                        .last()
                        .expect("appended piece has a construction id");
                    construction
                        .set_launch_intensity(piece_id, launch_speed)
                        .map_err(|error| {
                            format!("ride step {step_number} launch speed: {error:?}")
                        })?;
                }
            }
            RideStep::Shape(shape) => {
                let entry_speed_mps = preview_cache
                    .preview(construction.pieces(), construction.model(), physics)
                    .ok_or_else(|| {
                        format!("ride step {step_number}: the replayed prefix has no station")
                    })?
                    .end_speed_mps
                    .ok_or_else(|| {
                        format!("ride step {step_number}: the replayed prefix does not reach its open end")
                    })?;
                let mut context = SolveContext {
                    entry: construction.end(),
                    entry_speed_mps,
                    model: &model,
                    train: &model.train,
                    prefix: construction.pieces(),
                    preview_cache: &mut preview_cache,
                    physics,
                };
                let solved = solve(&shape.intent, &mut context).map_err(|error| {
                    format!(
                        "ride step {step_number}: could not re-solve shaped intent {:?} at {entry_speed_mps:.6} m/s: {error:?}",
                        shape.intent
                    )
                })?;
                let saved_digest = shape.spec.resolved_geometry_digest();
                let resolved_digest = solved.spec.resolved_geometry_digest();
                if saved_digest != resolved_digest {
                    return Err(format!(
                        "ride step {step_number} at {entry_speed_mps:.6} m/s: saved shaped geometry digest {saved_digest:016x} differs from the current solver's {resolved_digest:016x}; the ride was saved by an older solver, so regenerate the step with tools/design"
                    ));
                }
                construction
                    .append_resolved(&shape.intent, &shape.spec, &world, &[])
                    .map_err(|error| format!("ride step {step_number} shaped append: {error:?}"))?;
            }
            RideStep::Close(close) => {
                if !close.close {
                    return Err(format!("ride step {step_number}: close must be true"));
                }
                construction
                    .close()
                    .map_err(|error| format!("ride step {step_number} close: {error:?}"))?;
            }
            RideStep::BlockSections(_) => {}
        }
    }
    Ok(())
}
