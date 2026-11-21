//! Track geometry uses metres; grid coordinates are half tiles (two metres).
//! X and Y are horizontal, Z is up, and headings turn counterclockwise from X.
//! Positive bank rotates the up vector towards the left of travel.

pub mod catalog;
pub mod colors;
pub mod construction;
pub mod curves;
pub mod models;
pub mod ride_file;
pub mod saved_shape_check;
pub mod shaped;
pub mod supports;
#[cfg(test)]
mod vertical_top_hat;

pub use catalog::catalog;
pub use colors::{CoasterColor, CoasterColors};
pub(crate) use curves::Shape;
pub use curves::{
    Bank, HEART_HEIGHT_METRES, Hand, Heading, Heartlining, HillFamily, HillFeel, HillTwist,
    HorizontalCurvature, Inversion, Piece, PieceFamily, PieceId, PieceKind, RatedSpeeds, Slope,
    TrackSample, TrackState, Vec3, VerticalCurvature,
};
pub use models::{
    CoasterModel, DesignForceTargets, ENVELOPE_DISPLAY_LIMITS, ForceRating, ForceRatingAxis,
    ForceRatingBand, LAUNCH_MAX_ACCELERATION_METRES_PER_SECOND_SQUARED,
    LAUNCH_MAX_SPEED_METRES_PER_SECOND, LaunchParameters, LaunchSpeedBounds, LaunchType, LiftRules,
    Material, STRONG_LATERAL_G, STRONG_LONGITUDINAL_G, SupportStyle, TrainConfiguration,
    classify_envelope_force, coaster_models, envelope_rating_bands, family_coaster_model,
    great_positive_vertical_g, hybrid_model, hyper_model, multi_launch_model, wooden_twister_model,
};

#[cfg(test)]
mod tests;

pub mod blocks;
pub mod clearance;
pub mod proximity;

pub mod profile;
pub mod rated_windows;
pub use rated_windows::{rated_speed_margin, rated_speeds};
mod roll;

mod hill_forces;
pub use hill_forces::{PredictedHillForces, predicted_hill_forces};
