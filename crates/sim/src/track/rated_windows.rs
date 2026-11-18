//! Rated entry-speed windows measured by `examples/measure_force_windows.rs` with the filtered force evaluator.

use super::{CoasterModel, Piece, PieceId, RatedSpeeds, Slope};

/// Why a rated window ends where it does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RatedWindowSource {
    /// The catalog's energy-derived or hand minimum passes for every allowed train, so it is kept.
    EnergyMinimum,
    /// Raised until every allowed train keeps the clearance speed and clears the piece and its tail.
    ClearanceMinimum,
    /// The edge where some allowed train first fails the duration-aware force evaluator.
    ForceBound,
    /// Forces stay within the limits past this maximum; the catalog keeps it as a design choice.
    DesignCap,
    /// Above this a descending half corkscrew clears faster than its 22 m/s clearing-speed rule.
    ClearanceMaximum,
    /// Above this a rolled hill or half corkscrew rolls faster than the 200 degrees per second design rule.
    RollRate,
    /// Above this a floater hill gives a model's default train more than Good airtime, so it would ride as an ejector.
    AirtimeFeel,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeasuredRatedWindow {
    pub model_key: &'static str,
    pub id: PieceId,
    pub entry_slope: Slope,
    pub exit_slope: Slope,
    pub minimum_source: RatedWindowSource,
    pub maximum_source: RatedWindowSource,
    pub minimum_entry: f64,
    pub maximum_entry: f64,
}

include!("rated_windows.generated.rs");

/// The measured window for a rated catalog template and coaster model.
pub fn measured_rated_window(
    model: &CoasterModel,
    piece: &Piece,
) -> &'static MeasuredRatedWindow {
    assert!(
        model.allows_piece(piece),
        "{} does not allow {:?}",
        model.key,
        piece.id
    );
    let mut matches = MEASURED_RATED_WINDOWS.iter().filter(|window| {
        window.model_key == model.key
            && window.id == piece.id
            && window.entry_slope == piece.entry.slope
            && window.exit_slope == piece.exit.slope
    });
    let window = matches.next().unwrap_or_else(|| {
        panic!(
            "{} {:?} ({:?} to {:?}) has no measured rated window; run make generate-rated-windows",
            model.key, piece.id, piece.entry.slope, piece.exit.slope
        )
    });
    assert!(
        matches.next().is_none(),
        "{} {:?} has duplicate measured rated windows",
        model.key, piece.id
    );
    window
}

/// Returns the model-specific measured window, or `None` for an unrated piece.
pub fn rated_speeds(model: &CoasterModel, piece: &Piece) -> Option<RatedSpeeds> {
    let _design_speeds = piece.design_speeds?;
    let window = measured_rated_window(model, piece);
    Some(RatedSpeeds {
        minimum_entry: window.minimum_entry,
        maximum_entry: window.maximum_entry,
    })
}
