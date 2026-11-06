//! Track geometry uses metres; grid coordinates are half tiles (two metres).
//! X and Y are horizontal, Z is up, and headings turn counterclockwise from X.
//! Positive bank rotates the up vector towards the left of travel.

pub mod catalog;
pub mod colors;
pub mod construction;
pub mod curves;
pub mod models;
pub mod supports;
#[cfg(test)]
mod vertical_top_hat;

pub use catalog::{catalog, effective_rated_speeds};
pub use colors::{CoasterColor, CoasterColors};
pub(crate) use curves::Shape;
pub use curves::{
    Bank, HEART_HEIGHT_METRES, Heading, Heartlining, HillFamily, HillFeel, HillTwist,
    HorizontalCurvature, Inversion, Piece, PieceId, PieceKind, RatedSpeeds, Slope, TrackSample,
    TrackState, Vec3, VerticalCurvature,
};
pub use models::{
    AirtimeRating, CoasterModel, ComfortableForceLimits,
    LAUNCH_MAX_ACCELERATION_METRES_PER_SECOND_SQUARED, LAUNCH_MAX_SPEED_METRES_PER_SECOND,
    LaunchParameters, LaunchSpeedBounds, LaunchType, LiftRules, Material, SupportStyle,
    TrainConfiguration, coaster_models, hybrid_model, multi_launch_model, wooden_twister_model,
};

#[cfg(test)]
mod tests;

pub mod blocks;
pub mod clearance;
pub mod proximity;

mod profile;
mod roll;

mod hill_forces;
pub use hill_forces::{PredictedHillForces, predicted_hill_forces};
