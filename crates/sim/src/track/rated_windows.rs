//! Rated entry-speed windows measured by `examples/measure_force_windows.rs` with the filtered force evaluator.

use super::{CoasterModel, Piece, PieceId, RatedSpeeds, Slope};

/// Returns the speed headroom left after applying the design-test entry margin.
///
/// Windows at least 3 m/s wide require 1.5 m/s from either edge. Narrower
/// windows require the entry speed to stay within 0.3 m/s of the centre.
/// A nonnegative result passes either rule.
pub fn rated_speed_margin(window: RatedSpeeds, entry_speed: f64) -> f64 {
    assert!(entry_speed.is_finite());
    if window.maximum_entry - window.minimum_entry < 3.0 {
        let centre = (window.minimum_entry + window.maximum_entry) / 2.0;
        0.3 - (entry_speed - centre).abs()
    } else {
        (entry_speed - window.minimum_entry).min(window.maximum_entry - entry_speed) - 1.5
    }
}

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
    /// Lowest approach-reachable entry speed in the scan range; it does not imply a force or clearance pass.
    pub measured_from: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnavailableRatedWindow {
    pub model_key: &'static str,
    pub id: PieceId,
    pub entry_slope: Slope,
    pub exit_slope: Slope,
}

include!("rated_windows.generated.rs");

/// The measured window for a rated catalog template and coaster model.
pub fn measured_rated_window(
    model: &CoasterModel,
    piece: &Piece,
) -> Option<&'static MeasuredRatedWindow> {
    assert!(
        model.allows_piece(piece),
        "{} does not allow {:?}",
        model.key,
        piece.id
    );
    lookup_rated_window(
        model.key,
        piece.id,
        piece.entry.slope,
        piece.exit.slope,
        MEASURED_RATED_WINDOWS,
        UNAVAILABLE_RATED_WINDOWS,
    )
}

/// Returns the model-specific measured window, or `None` for an unrated piece.
pub fn rated_speeds(model: &CoasterModel, piece: &Piece) -> Option<RatedSpeeds> {
    let _design_speeds = piece.design_speeds?;
    let window = measured_rated_window(model, piece)?;
    Some(RatedSpeeds {
        minimum_entry: window.minimum_entry,
        maximum_entry: window.maximum_entry,
    })
}

fn lookup_rated_window<'a>(
    model_key: &str,
    id: PieceId,
    entry_slope: Slope,
    exit_slope: Slope,
    windows: &'a [MeasuredRatedWindow],
    unavailable_windows: &[UnavailableRatedWindow],
) -> Option<&'a MeasuredRatedWindow> {
    let mut rated_matches = windows.iter().filter(|window| {
        window.model_key == model_key
            && window.id == id
            && window.entry_slope == entry_slope
            && window.exit_slope == exit_slope
    });
    let rated = rated_matches.next();
    let mut unavailable_matches = unavailable_windows.iter().filter(|window| {
        window.model_key == model_key
            && window.id == id
            && window.entry_slope == entry_slope
            && window.exit_slope == exit_slope
    });
    let unavailable = unavailable_matches.next();

    assert!(
        rated.is_none() || rated_matches.next().is_none(),
        "{model_key} {id:?} has duplicate measured rated windows"
    );
    assert!(
        unavailable.is_none() || unavailable_matches.next().is_none(),
        "{model_key} {id:?} has duplicate unavailable rated windows"
    );
    match (rated, unavailable) {
        (Some(_), Some(_)) => panic!("{model_key} {id:?} has duplicate measured rated windows"),
        (Some(window), None) => Some(window),
        (None, Some(_)) => None,
        (None, None) => panic!(
            "{model_key} {id:?} ({entry_slope:?} to {exit_slope:?}) has no measured rated window; run make generate-rated-windows"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        MEASURED_RATED_WINDOWS, MeasuredRatedWindow, RatedWindowSource, UnavailableRatedWindow,
        lookup_rated_window,
    };
    use crate::track::{PieceId, Slope};

    const KEY: &str = "test_model";
    const ID: PieceId = PieceId::BankedStraightLeft;
    const ENTRY: Slope = Slope::UpGentle;
    const EXIT: Slope = Slope::UpGentle;

    #[test]
    fn generated_measured_from_values_do_not_exceed_their_window_maxima() {
        for window in MEASURED_RATED_WINDOWS {
            assert!(
                (0.0..=window.maximum_entry).contains(&window.measured_from),
                "{} {:?}: measured_from {} is above maximum {}",
                window.model_key,
                window.id,
                window.measured_from,
                window.maximum_entry
            );
        }
    }

    #[test]
    fn explicit_no_window_entry_returns_unavailable() {
        let unavailable = [UnavailableRatedWindow {
            model_key: KEY,
            id: ID,
            entry_slope: ENTRY,
            exit_slope: EXIT,
        }];

        assert!(lookup_rated_window(KEY, ID, ENTRY, EXIT, &[], &unavailable).is_none());
    }

    #[test]
    #[should_panic(expected = "has no measured rated window")]
    fn allowed_combination_without_a_rated_or_unavailable_entry_fails_loudly() {
        lookup_rated_window(KEY, ID, ENTRY, EXIT, &[], &[]);
    }

    #[test]
    #[should_panic(expected = "duplicate measured rated windows")]
    fn combination_cannot_have_both_rated_and_unavailable_entries() {
        let rated = [MeasuredRatedWindow {
            model_key: KEY,
            id: ID,
            entry_slope: ENTRY,
            exit_slope: EXIT,
            minimum_source: RatedWindowSource::EnergyMinimum,
            maximum_source: RatedWindowSource::ForceBound,
            minimum_entry: 20.0,
            maximum_entry: 25.0,
            measured_from: 20.0,
        }];
        let unavailable = [UnavailableRatedWindow {
            model_key: KEY,
            id: ID,
            entry_slope: ENTRY,
            exit_slope: EXIT,
        }];

        lookup_rated_window(KEY, ID, ENTRY, EXIT, &rated, &unavailable);
    }
}
