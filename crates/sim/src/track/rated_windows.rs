//! Rated entry-speed windows measured by `examples/measure_force_windows.rs` with the filtered force evaluator.

use super::{Piece, PieceId, RatedSpeeds, Slope};

/// Why a rated window ends where it does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RatedWindowSource {
    /// The catalog's energy-derived or hand minimum passes for every allowed train, so it is kept.
    EnergyMinimum,
    /// Raised until every allowed train keeps the clearance speed and clears the piece and its tail.
    ClearanceMinimum,
    /// The edge where some allowed train's filtered force first goes past a comfortable limit.
    ForceBound,
    /// Forces stay within the limits past this maximum; the catalog keeps it as a design choice.
    DesignCap,
    /// Above this a descending half corkscrew clears faster than its 22 m/s clearing-speed rule.
    ClearanceMaximum,
    /// Above this a rolled hill or half corkscrew rolls faster than the 200 degrees per second design rule.
    RollRate,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeasuredRatedWindow {
    pub id: PieceId,
    pub entry_slope: Slope,
    pub exit_slope: Slope,
    pub minimum_source: RatedWindowSource,
    pub maximum_source: RatedWindowSource,
    pub minimum_entry: f64,
    pub maximum_entry: f64,
}

include!("rated_windows.generated.rs");

/// The measured window for a rated catalog template, keyed by its id and connector slopes.
pub fn measured_rated_window(piece: &Piece) -> &'static MeasuredRatedWindow {
    let mut matches = MEASURED_RATED_WINDOWS.iter().filter(|window| {
        window.id == piece.id
            && window.entry_slope == piece.entry.slope
            && window.exit_slope == piece.exit.slope
    });
    let window = matches.next().unwrap_or_else(|| {
        panic!(
            "{:?} ({:?} to {:?}) has no measured rated window; run make generate-rated-windows",
            piece.id, piece.entry.slope, piece.exit.slope
        )
    });
    assert!(
        matches.next().is_none(),
        "{:?} has duplicate measured rated windows",
        piece.id
    );
    window
}

/// Replaces every template's catalog window with its measured window.
pub(super) fn apply_measured_rated_windows(pieces: &mut [Piece]) {
    let mut rated_templates = 0;
    for piece in pieces
        .iter_mut()
        .filter(|piece| piece.rated_speeds.is_some())
    {
        let window = measured_rated_window(piece);
        piece.rated_speeds = Some(RatedSpeeds {
            minimum_entry: window.minimum_entry,
            maximum_entry: window.maximum_entry,
        });
        rated_templates += 1;
    }
    assert_eq!(
        rated_templates,
        MEASURED_RATED_WINDOWS.len(),
        "the measured rated window table has rows for templates that are no longer rated; run make generate-rated-windows"
    );
}
