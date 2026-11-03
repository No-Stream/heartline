//! Guest simulation modules.
//!
//! Submodules: [`traits`], [`generation`], [`groups`], [`storage`], [`memories`],
//! [`behavior`], [`decisions`], [`needs`], [`knowledge`], [`thoughts`], [`park`],
//! [`visits`], [`walking`] and [`pathfinding`].

pub mod behavior;
pub mod decisions;
pub mod generation;
pub mod groups;
pub mod knowledge;
pub mod memories;
pub mod needs;
pub mod park;
pub mod pathfinding;
pub mod storage;
pub mod thoughts;
pub mod traits;
pub mod visits;
pub mod walking;

pub use behavior::*;
pub use storage::*;

#[cfg(test)]
mod walking_tests;
