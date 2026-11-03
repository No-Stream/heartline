//! Track geometry uses metres; grid coordinates are half tiles (two metres).
//! X and Y are horizontal, Z is up, and headings turn counterclockwise from X.
//! Positive bank rotates the up vector towards the left of travel.

pub mod catalog;
pub mod construction;
pub mod curves;
pub mod models;
pub mod supports;

pub use catalog::catalog;
pub(crate) use curves::Shape;
pub use curves::{
    Bank, Heading, HorizontalCurvature, Inversion, Piece, PieceId, PieceKind, Slope, TrackSample,
    TrackState, Vec3, VerticalCurvature,
};
pub use models::{
    CoasterModel, LaunchParameters, LaunchType, LiftType, Material, SupportStyle,
    TrainConfiguration, hybrid_model, multi_launch_model, wooden_twister_model,
};

#[cfg(test)]
mod tests;

pub mod blocks;
pub mod clearance;
pub mod proximity;
