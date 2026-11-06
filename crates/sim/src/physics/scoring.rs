use crate::track::ComfortableForceLimits;
pub use crate::track::{ForceRating as ForceBand, ForceRatingAxis as ForceAxis};

pub fn force_band(
    axis: ForceAxis,
    value_g: f64,
    limits: ComfortableForceLimits,
    model_key: &str,
) -> ForceBand {
    limits.classify_force(axis, value_g, model_key)
}

pub(super) fn score_band_detail(
    axis: ForceAxis,
    band: ForceBand,
    limits: ComfortableForceLimits,
    model_key: &str,
) -> Option<(usize, f64)> {
    let boundary = |rating| {
        limits
            .rating_bands(axis, model_key)
            .into_iter()
            .find(|entry| entry.rating == rating)
            .unwrap()
            .up_to
            .unwrap()
    };
    match (axis, band) {
        (ForceAxis::Airtime, ForceBand::Good) => Some((0, boundary(ForceBand::Weak))),
        (ForceAxis::Airtime, ForceBand::Great) => Some((1, boundary(ForceBand::Good))),
        (ForceAxis::Vertical, ForceBand::Good) => Some((2, boundary(ForceBand::Weak))),
        (ForceAxis::Vertical, ForceBand::Great) => Some((3, boundary(ForceBand::Good))),
        (ForceAxis::Lateral, ForceBand::Strong | ForceBand::TooMuch | ForceBand::Dangerous) => {
            Some((4, boundary(ForceBand::Fine)))
        }
        (ForceAxis::Longitudinal, ForceBand::Strong) => Some((5, boundary(ForceBand::Fine))),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{ForceAxis, ForceBand, force_band};
    use crate::track::ComfortableForceLimits;

    #[test]
    fn force_bands_match_overlay_boundaries_and_model_limits() {
        let limits = ComfortableForceLimits {
            minimum_vertical_g: -1.5,
            maximum_vertical_g: 5.0,
            maximum_lateral_g: 1.5,
            maximum_longitudinal_g: 2.0,
        };
        for (axis, value, expected) in [
            (ForceAxis::Airtime, -0.2, ForceBand::Weak),
            (ForceAxis::Airtime, -0.2001, ForceBand::Good),
            (ForceAxis::Airtime, -0.9001, ForceBand::Great),
            (ForceAxis::Airtime, -1.95, ForceBand::TooMuch),
            (ForceAxis::Airtime, -1.951, ForceBand::Dangerous),
            (ForceAxis::Vertical, 2.0, ForceBand::Weak),
            (ForceAxis::Vertical, 2.001, ForceBand::Good),
            (ForceAxis::Vertical, 2.999, ForceBand::Good),
            (ForceAxis::Vertical, 3.0, ForceBand::Great),
            (ForceAxis::Vertical, 5.0, ForceBand::Great),
            (ForceAxis::Vertical, 6.5, ForceBand::TooMuch),
            (ForceAxis::Vertical, 6.501, ForceBand::Dangerous),
            (ForceAxis::Longitudinal, -1.0, ForceBand::Fine),
            (ForceAxis::Longitudinal, -2.0, ForceBand::Strong),
            (ForceAxis::Longitudinal, -2.6, ForceBand::TooMuch),
            (ForceAxis::Longitudinal, -2.601, ForceBand::Dangerous),
        ] {
            assert_eq!(force_band(axis, value, limits, "Timber Tumbler"), expected);
        }
        assert_eq!(
            force_band(ForceAxis::Airtime, 0.0, limits, "Timber Tumbler"),
            ForceBand::Weak
        );
        assert_eq!(
            force_band(ForceAxis::Airtime, -0.9, limits, "Timber Tumbler"),
            ForceBand::Good
        );
        assert_eq!(
            force_band(ForceAxis::Airtime, -1.0, limits, "Timber Tumbler"),
            ForceBand::Great
        );
        assert_eq!(
            force_band(ForceAxis::Airtime, -1.5, limits, "Timber Tumbler"),
            ForceBand::Great
        );
        assert_eq!(
            force_band(ForceAxis::Airtime, -1.6, limits, "Timber Tumbler"),
            ForceBand::TooMuch
        );
        assert_eq!(
            force_band(ForceAxis::Lateral, -0.75, limits, "Timber Tumbler"),
            ForceBand::Fine
        );
        assert_eq!(
            force_band(ForceAxis::Lateral, -1.5, limits, "Timber Tumbler"),
            ForceBand::Strong
        );
        assert_eq!(
            force_band(ForceAxis::Lateral, -2.0, limits, "Timber Tumbler"),
            ForceBand::Dangerous
        );
        assert_eq!(
            force_band(ForceAxis::Vertical, 2.5, limits, "Briar Blitz"),
            ForceBand::Great
        );
        assert_eq!(
            force_band(ForceAxis::Vertical, 2.5, limits, "Timber Tumbler"),
            ForceBand::Good
        );
        let wooden = ComfortableForceLimits {
            maximum_vertical_g: 4.0,
            ..limits
        };
        assert_eq!(
            force_band(ForceAxis::Vertical, 2.499, wooden, "Briar Blitz"),
            ForceBand::Good
        );
        assert_eq!(
            force_band(ForceAxis::Vertical, 2.5, wooden, "Briar Blitz"),
            ForceBand::Great
        );
        let stricter = ComfortableForceLimits {
            minimum_vertical_g: -1.0,
            ..limits
        };
        assert_eq!(
            force_band(ForceAxis::Airtime, -1.1, stricter, "Timber Tumbler"),
            ForceBand::TooMuch
        );
    }
}

#[cfg(test)]
mod score_tests {
    use super::*;
    use crate::physics::TestSample;
    use crate::physics::features::{PieceSummary, ride_features};
    use crate::track::{PieceId, PieceKind, Shape};

    #[test]
    fn breakdowns_sum_and_overlay_bands_change_scores() {
        let features = ride_features(&[], &[], &[], 0.0, false);
        let baseline = score(&features, &ForceBandDurations::default());
        let bands = ForceBandDurations {
            airtime_great: 3.0,
            lateral_strong: 2.0,
            dangerous: 1.0,
            ..Default::default()
        };
        let result = score(&features, &bands);
        assert!(result.excitement > baseline.excitement);
        assert!(result.intensity > baseline.intensity);
        assert!(result.nausea > baseline.nausea);
        for (total, contributions) in [
            (result.excitement, &result.breakdown.excitement),
            (result.intensity, &result.breakdown.intensity),
            (result.nausea, &result.breakdown.nausea),
        ] {
            assert!(
                (total - contributions.iter().map(|item| item.value).sum::<f64>()).abs() < 1e-12
            );
        }
    }

    #[test]
    fn excessive_force_details_keep_axis_duration_and_peak_separate() {
        let features = ride_features(&[], &[], &[], 0.0, false);
        let location = Location {
            piece_index: 1,
            piece_name: "Turn",
            distance_m: 12.0,
            seconds_from_start: 2.0,
        };
        let mut bands = ForceBandDurations {
            too_much: 2.0,
            ..Default::default()
        };
        bands.excessive_details[2] = Some(ExcessiveForceDetail {
            value: 4.0,
            limit: 1.5,
            seconds: 2.0,
            location,
        });
        let scores = score(&features, &bands);
        let contribution = scores
            .breakdown
            .excitement
            .iter()
            .find(|item| item.term == Term::ExcessiveForces)
            .unwrap();
        assert_eq!(contribution.force_details[0].axis, "lateral");
        assert_eq!(contribution.force_details[0].measured_value, 4.0);
        assert_eq!(contribution.force_details[0].duration_seconds, Some(2.0));
    }

    #[test]
    fn force_band_details_explain_the_weighted_sustained_measure() {
        let features = ride_features(&[], &[], &[], 0.0, false);
        let location = Location {
            piece_index: 0,
            piece_name: "Track",
            distance_m: 3.0,
            seconds_from_start: 1.0,
        };
        let mut bands = ForceBandDurations {
            vertical_good: 4.0,
            lateral_strong: 2.0,
            ..Default::default()
        };
        bands.band_details[2] = Some(ExcessiveForceDetail {
            value: 3.0,
            limit: 2.0,
            seconds: 4.0,
            location,
        });
        bands.band_details[4] = Some(ExcessiveForceDetail {
            value: 1.4,
            limit: 0.75,
            seconds: 2.0,
            location,
        });
        let result = score(&features, &bands);
        let contribution = result
            .breakdown
            .intensity
            .iter()
            .find(|item| item.term == Term::SustainedForce)
            .unwrap();
        assert!((contribution.measure - 4.2).abs() < 1e-12);
        assert_eq!(contribution.force_details.len(), 2);
        assert_eq!(contribution.force_details[0].weight, Some(0.55));
        assert_eq!(contribution.force_details[1].weight, Some(1.0));
    }

    #[test]
    fn peak_and_launch_details_use_the_recorded_run_wide_peak() {
        let mut features = ride_features(&[], &[], &[], 0.0, false);
        features.statistics.max_forward_g = 3.0;
        features.statistics.launches = 1;
        let location = Location {
            piece_index: 0,
            piece_name: "Track",
            distance_m: 3.0,
            seconds_from_start: 1.0,
        };
        let mut bands = ForceBandDurations::default();
        bands.peak_details[3] = Some(ExcessiveForceDetail {
            value: 3.0,
            limit: 6.0,
            seconds: 0.0,
            location,
        });
        let result = score(&features, &bands);
        let peak = result
            .breakdown
            .intensity
            .iter()
            .find(|item| item.term == Term::PeakForce)
            .unwrap();
        assert_eq!(peak.measure, 0.5);
        assert_eq!(peak.force_details[0].axis, "peakForward");
        assert_eq!(peak.force_details[0].duration_seconds, None);
        let launch = result
            .breakdown
            .excitement
            .iter()
            .find(|item| item.term == Term::Launch)
            .unwrap();
        assert_eq!(launch.force_details[0].axis, "launchForward");
        assert_eq!(launch.force_details[0].measured_value, 3.0);
    }

    #[test]
    fn excitement_floor_is_explained_in_the_breakdown() {
        let mut features = ride_features(&[], &[], &[], 0.0, false);
        features.envelope.g_seconds = 10.0;
        let result = score(&features, &ForceBandDurations::default());
        assert_eq!(result.excitement, 0.0);
        assert!(
            result
                .breakdown
                .excitement
                .iter()
                .any(|item| item.term == Term::ExcitementFloor && item.value > 0.0)
        );
        assert!(
            result
                .breakdown
                .excitement
                .iter()
                .map(|item| item.value)
                .sum::<f64>()
                .abs()
                < 1e-12
        );
    }

    #[test]
    fn airtime_moments_keep_paying_and_envelope_penalty_keeps_location() {
        use crate::physics::comfort::{EnvelopeAxis, EnvelopeWorst};
        let mut features = ride_features(&[], &[], &[], 0.0, false);
        features.active_airtime_moments = 10;
        let ten = score(&features, &ForceBandDurations::default());
        features.active_airtime_moments = 20;
        let twenty = score(&features, &ForceBandDurations::default());
        let airtime_moments = |result: &Scores| {
            result
                .breakdown
                .excitement
                .iter()
                .find(|item| item.term == Term::AirtimeMoments)
                .unwrap()
                .value
        };
        assert!(airtime_moments(&twenty) - airtime_moments(&ten) > 0.2);
        let location = Location {
            piece_index: 2,
            piece_name: "Loop",
            distance_m: 40.0,
            seconds_from_start: 5.0,
        };
        features.envelope.g_seconds = 0.45;
        features.envelope.seconds = 1.5;
        features.envelope.worst = Some(EnvelopeWorst {
            location,
            axis: EnvelopeAxis::Vertical,
            value: 5.0,
            allowed: 4.0,
            ratio: 0.25,
        });
        let result = score(&features, &ForceBandDurations::default());
        let contribution = result
            .breakdown
            .excitement
            .iter()
            .find(|item| item.term == Term::Envelope)
            .unwrap();
        assert!((contribution.value + 1.087).abs() < 0.01);
        assert_eq!(contribution.location, Some(location));
        let detail = contribution.force_detail.as_ref().unwrap();
        assert_eq!(detail.axis, "vertical");
        assert_eq!(detail.measured_value, 5.0);
        assert_eq!(detail.limit, 4.0);
        assert_eq!(detail.unit, "g");
        assert_eq!(detail.duration_seconds, Some(1.5));
        assert_eq!(detail.location, location);
    }

    #[test]
    fn score_timing_uses_active_track_while_statistics_keep_the_full_run() {
        let pieces = [
            PieceSummary {
                id: PieceId::Station,
                name: "Station",
                kind: PieceKind::Station,
                shape: Shape::Plain,
                inversion: None,
                heartlining: None,
                length: 3.0,
                turn_angle_radians: 0.0,
            },
            PieceSummary {
                id: PieceId::GentleUp,
                name: "Lift",
                kind: PieceKind::Lift,
                shape: Shape::Plain,
                inversion: None,
                heartlining: None,
                length: 3.0,
                turn_angle_radians: 0.0,
            },
            PieceSummary {
                id: PieceId::Straight,
                name: "Track",
                kind: PieceKind::Track,
                shape: Shape::Plain,
                inversion: None,
                heartlining: None,
                length: 6.0,
                turn_angle_radians: 0.0,
            },
        ];
        let samples: Vec<_> = (0..12)
            .map(|index| {
                let piece_index = if index < 3 {
                    0
                } else if index < 6 {
                    1
                } else {
                    2
                };
                let vertical_g = if matches!(index, 1 | 4 | 7) {
                    -1.0
                } else {
                    1.0
                };
                TestSample {
                    distance: index as f64,
                    speed: [2.0, 4.0, 10.0][piece_index],
                    vertical_g,
                    lateral_g: 0.0,
                    longitudinal_g: 0.0,
                    airtime: false,
                    height_m: 0.0,
                    inverted: matches!(index, 1 | 4 | 7),
                    roll: 0.0,
                    piece_index,
                }
            })
            .collect();

        let features = ride_features(&samples, &pieces, &[], 12.0, false);
        let scores = score(&features, &ForceBandDurations::default());
        let airtime_moments = scores
            .breakdown
            .excitement
            .iter()
            .find(|item| item.term == Term::AirtimeMoments)
            .unwrap();
        let time_inverted = scores
            .breakdown
            .nausea
            .iter()
            .find(|item| item.term == Term::TimeInverted)
            .unwrap();

        assert_eq!(features.statistics.airtime_moments, 3);
        assert!((features.statistics.inverted_seconds - 0.85).abs() < 1e-12);
        assert_eq!(airtime_moments.measure, 1.0);
        assert!((time_inverted.measure - 0.1).abs() < 1e-12);
    }

    #[test]
    fn repeated_inversions_have_diminishing_returns() {
        use crate::track::{Heartlining, Inversion};
        let mut features = ride_features(&[], &[], &[], 0.0, false);
        features.inversion_passages = vec![(Inversion::VerticalLoop, Heartlining::Heartlined); 6];
        let repeated = score(&features, &ForceBandDurations::default());
        features.inversion_passages = vec![
            (Inversion::VerticalLoop, Heartlining::Heartlined),
            (Inversion::Corkscrew, Heartlining::Heartlined),
            (Inversion::HeartlineRoll, Heartlining::Heartlined),
        ];
        let varied = score(&features, &ForceBandDurations::default());
        assert!(varied.excitement > repeated.excitement);
    }
}

use super::features::{Location, RideFeatures};
use crate::track::Heartlining;

/// Seconds in the model-specific bands used by the force overlay. Excessive
/// durations are summed across axes, so simultaneous excessive axes both count.
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub(super) struct ForceBandDurations {
    pub(super) airtime_good: f64,
    pub(super) airtime_great: f64,
    pub(super) vertical_good: f64,
    pub(super) vertical_great: f64,
    pub(super) lateral_strong: f64,
    pub(super) longitudinal_strong: f64,
    pub(super) too_much: f64,
    pub(super) dangerous: f64,
    #[serde(default)]
    pub(super) excessive_details: [Option<ExcessiveForceDetail>; 4],
    #[serde(default)]
    pub(super) band_details: [Option<ExcessiveForceDetail>; 6],
    #[serde(default)]
    pub(super) peak_details: [Option<ExcessiveForceDetail>; 5],
    #[serde(default)]
    pub(super) dangerous_seconds_by_axis: [f64; 4],
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(super) struct ExcessiveForceDetail {
    pub(super) value: f64,
    pub(super) limit: f64,
    pub(super) seconds: f64,
    pub(super) location: Location,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Term {
    Airtime,
    PositiveForces,
    Laterals,
    LongitudinalForces,
    ExcessiveForces,
    AirtimeMoments,
    Inversions,
    TopSpeed,
    BiggestDrop,
    Pace,
    Launch,
    Variety,
    Whip,
    RollRate,
    FastRolls,
    RideTime,
    ShortRide,
    Proximity,
    Theming,
    DeadTime,
    Envelope,
    Sickening,
    ExcitementFloor,
    PeakForce,
    SustainedForce,
    Reversals,
    GreyOut,
    TimeInverted,
}

impl Term {
    pub fn label(self) -> &'static str {
        match self {
            Self::Airtime => "airtime",
            Self::PositiveForces => "positive forces",
            Self::Laterals => "lateral forces",
            Self::LongitudinalForces => "longitudinal forces",
            Self::ExcessiveForces => "excessive forces",
            Self::AirtimeMoments => "airtime moments",
            Self::Inversions => "inversions",
            Self::TopSpeed => "top speed",
            Self::BiggestDrop => "biggest drop",
            Self::Pace => "pace",
            Self::Launch => "launch",
            Self::Variety => "variety",
            Self::Whip => "whip",
            Self::RollRate => "roll rate",
            Self::FastRolls => "fast rolls",
            Self::RideTime => "ride time",
            Self::ShortRide => "short ride",
            Self::Proximity => "proximity",
            Self::Theming => "theming",
            Self::DeadTime => "dead time",
            Self::Envelope => "forces above the ASTM comfort envelope",
            Self::Sickening => "nausea",
            Self::ExcitementFloor => "minimum excitement",
            Self::PeakForce => "peak force",
            Self::SustainedForce => "sustained force",
            Self::Reversals => "reversals",
            Self::GreyOut => "strong positive forces",
            Self::TimeInverted => "time inverted",
        }
    }
    pub fn unit(self) -> &'static str {
        match self {
            Self::Airtime
            | Self::PositiveForces
            | Self::Laterals
            | Self::LongitudinalForces
            | Self::ExcessiveForces
            | Self::FastRolls
            | Self::RideTime
            | Self::ShortRide
            | Self::DeadTime
            | Self::SustainedForce
            | Self::GreyOut
            | Self::TimeInverted => "s",
            Self::TopSpeed | Self::Pace => "m/s",
            Self::RollRate => "degrees/s",
            Self::BiggestDrop => "m",
            Self::Launch => "g",
            Self::Envelope => "g-s",
            _ => "",
        }
    }
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Contribution {
    pub term: Term,
    pub value: f64,
    pub measure: f64,
    pub location: Option<Location>,
    #[serde(default)]
    pub force_detail: Option<ForceDetail>,
    #[serde(default)]
    pub force_details: Vec<ForceDetail>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ForceDetail {
    pub axis: String,
    pub measured_value: f64,
    pub limit: f64,
    pub unit: String,
    pub duration_seconds: Option<f64>,
    pub location: Location,
    #[serde(default)]
    pub weight: Option<f64>,
    #[serde(default)]
    pub car_index: Option<usize>,
    #[serde(default)]
    pub dangerous_seconds: Option<f64>,
}
impl Contribution {
    pub fn describe(&self) -> String {
        let location = self
            .location
            .map(|location| format!(" at {}", location.piece_name))
            .unwrap_or_default();
        let unit = self.term.unit();
        let measure = if unit.is_empty() {
            format!("{:.1}", self.measure)
        } else {
            format!("{:.1} {unit}", self.measure)
        };
        format!(
            "{} ({:+.1}, {}{})",
            self.term.label(),
            self.value,
            measure,
            location
        )
    }
}
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ScoreBreakdown {
    pub excitement: Vec<Contribution>,
    pub intensity: Vec<Contribution>,
    pub nausea: Vec<Contribution>,
}
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Scores {
    pub excitement: f64,
    pub intensity: f64,
    pub nausea: f64,
    pub(super) breakdown: ScoreBreakdown,
}

// Each pair is a contribution weight and its characteristic scale. Calibration
// anchors are maintained in calibration_tests.rs.
struct ScoringConstants {
    dangerous_duration_weight: f64,
    good_band_weight: f64,
    heartlined_nausea_weight: f64,
    non_heartlined_nausea_weight: f64,
    positive_peak_limit: f64,
    negative_peak_limit: f64,
    lateral_peak_limit: f64,
    forward_peak_limit: f64,
    braking_peak_limit: f64,
    intensity_speed_threshold: f64,
    inversion_repeat_factor: f64,
    heartlined_inversion_weight: f64,
    non_heartlined_inversion_weight: f64,
    excitement_speed_threshold: f64,
    pace_speed_threshold: f64,
    roll_rate_threshold: f64,
    short_ride_weight: f64,
    short_ride_seconds: f64,
    theming_weight: f64,
    nausea_penalty_weight: f64,
    nausea_penalty_threshold: f64,

    n_lateral: (f64, f64),
    n_inversions: (f64, f64),
    n_roll: (f64, f64),
    n_reversal: (f64, f64),
    n_positive: (f64, f64),
    n_inverted: (f64, f64),
    n_excessive: (f64, f64),
    n_envelope: (f64, f64),
    i_peak: (f64, f64),
    i_sustained: (f64, f64),
    i_excessive: (f64, f64),
    i_speed: (f64, f64),
    i_drop: (f64, f64),
    i_inversions: (f64, f64),
    i_envelope: (f64, f64),

    airtime: (f64, f64),
    positive: (f64, f64),
    lateral: (f64, f64),
    longitudinal: (f64, f64),
    excessive: (f64, f64),
    moments: (f64, f64),
    inversions: (f64, f64),
    speed: (f64, f64),
    drop: (f64, f64),
    pace: (f64, f64),
    launch: (f64, f64),
    variety: (f64, f64),
    snap: (f64, f64),
    roll: (f64, f64),
    time: (f64, f64),
    proximity: (f64, f64),
    dead: (f64, f64),
    envelope: (f64, f64),
}
const SCORING: ScoringConstants = ScoringConstants {
    dangerous_duration_weight: 2.0,
    good_band_weight: 0.55,
    heartlined_nausea_weight: 0.6,
    non_heartlined_nausea_weight: 1.0,
    positive_peak_limit: 6.0,
    negative_peak_limit: 2.0,
    lateral_peak_limit: 3.0,
    forward_peak_limit: 6.0,
    braking_peak_limit: 2.0,
    intensity_speed_threshold: 10.0,
    inversion_repeat_factor: 0.55,
    heartlined_inversion_weight: 1.0,
    non_heartlined_inversion_weight: 0.9,
    excitement_speed_threshold: 12.0,
    pace_speed_threshold: 8.0,
    roll_rate_threshold: 60.0,
    short_ride_weight: 1.0,
    short_ride_seconds: 20.0,
    theming_weight: 0.8,
    nausea_penalty_weight: 0.35,
    nausea_penalty_threshold: 5.0,

    n_lateral: (2.5, 3.0),
    n_inversions: (1.5, 3.0),
    n_roll: (1.5, 2.0),
    n_reversal: (1.5, 6.0),
    n_positive: (1.0, 4.0),
    n_inverted: (0.5, 4.0),
    n_excessive: (1.5, 2.0),
    n_envelope: (2.5, 1.0),
    i_peak: (4.0, 0.7),
    i_sustained: (3.0, 6.0),
    i_excessive: (1.0, 2.0),
    i_speed: (1.5, 25.0),
    i_drop: (1.0, 50.0),
    i_inversions: (0.6, 3.0),
    // The envelope also lowers excitement and raises nausea, so its intensity
    // contribution is smaller to avoid counting the same exposure again in full.
    i_envelope: (1.0, 1.0),

    airtime: (2.6, 2.5),
    positive: (1.0, 2.0),
    lateral: (0.7, 2.0),
    longitudinal: (0.4, 2.0),
    excessive: (-0.7, 2.0),
    moments: (1.2, 10.0),
    inversions: (2.2, 2.0),
    speed: (1.2, 20.0),
    drop: (0.8, 40.0),
    pace: (0.6, 12.0),
    // The launch contributes to excitement and intensity at the same scale.
    launch: (1.0, 1.0),
    variety: (1.0, 4.0),
    snap: (0.6, 4.0),
    roll: (0.5, 90.0),
    time: (0.8, 45.0),
    proximity: (1.0, 2.0),
    dead: (-0.5, 6.0),
    envelope: (-3.0, 1.0),
};
pub(super) fn peak_force_limits() -> [f64; 5] {
    [
        SCORING.positive_peak_limit,
        SCORING.negative_peak_limit,
        SCORING.lateral_peak_limit,
        SCORING.forward_peak_limit,
        SCORING.braking_peak_limit,
    ]
}
fn soft(measure: f64, (weight, scale): (f64, f64)) -> f64 {
    weight * (1.0 - (-measure / scale).exp())
}
fn add(
    items: &mut Vec<Contribution>,
    term: Term,
    measure: f64,
    value: f64,
    location: Option<Location>,
) {
    items.push(Contribution {
        term,
        value,
        measure,
        location,
        force_detail: None,
        force_details: Vec::new(),
    });
}
fn add_soft(
    items: &mut Vec<Contribution>,
    term: Term,
    measure: f64,
    constants: (f64, f64),
    location: Option<Location>,
) {
    add(items, term, measure, soft(measure, constants), location);
}

pub(super) fn score(features: &RideFeatures, bands: &ForceBandDurations) -> Scores {
    let stats = &features.statistics;
    let envelope = &features.envelope;
    let location = envelope.worst.as_ref().map(|worst| worst.location);
    let excessive = bands.too_much + SCORING.dangerous_duration_weight * bands.dangerous;
    let airtime = bands.airtime_good * SCORING.good_band_weight + bands.airtime_great;
    let positive = bands.vertical_good * SCORING.good_band_weight + bands.vertical_great;
    let mut breakdown = ScoreBreakdown::default();
    let n = &mut breakdown.nausea;
    add_soft(
        n,
        Term::Laterals,
        bands.lateral_strong,
        SCORING.n_lateral,
        None,
    );
    let inversion_nausea: f64 = features
        .inversion_passages
        .iter()
        .map(|(_, heartlining)| {
            if *heartlining == Heartlining::Heartlined {
                SCORING.heartlined_nausea_weight
            } else {
                SCORING.non_heartlined_nausea_weight
            }
        })
        .sum();
    add_soft(
        n,
        Term::Inversions,
        inversion_nausea,
        SCORING.n_inversions,
        None,
    );
    add_soft(
        n,
        Term::FastRolls,
        features.roll_rate_seconds,
        SCORING.n_roll,
        None,
    );
    add_soft(
        n,
        Term::Reversals,
        features.reversal_count as f64,
        SCORING.n_reversal,
        None,
    );
    add_soft(n, Term::GreyOut, positive, SCORING.n_positive, None);
    add_soft(
        n,
        Term::TimeInverted,
        features.active_inverted_seconds,
        SCORING.n_inverted,
        None,
    );
    add_soft(
        n,
        Term::ExcessiveForces,
        excessive,
        SCORING.n_excessive,
        None,
    );
    add_soft(
        n,
        Term::Envelope,
        envelope.g_seconds,
        SCORING.n_envelope,
        location,
    );
    let nausea = n.iter().map(|item| item.value).sum::<f64>();
    let i = &mut breakdown.intensity;
    let peak_ratio = (stats.max_positive_vertical_g / SCORING.positive_peak_limit)
        .max(-stats.max_negative_vertical_g / SCORING.negative_peak_limit)
        .max(stats.max_lateral_g / SCORING.lateral_peak_limit)
        .max(stats.max_forward_g / SCORING.forward_peak_limit)
        .max(stats.max_braking_g.abs() / SCORING.braking_peak_limit);
    add_soft(i, Term::PeakForce, peak_ratio, SCORING.i_peak, None);
    add_soft(
        i,
        Term::SustainedForce,
        positive + bands.lateral_strong + airtime + bands.longitudinal_strong,
        SCORING.i_sustained,
        None,
    );
    add_soft(
        i,
        Term::ExcessiveForces,
        excessive,
        SCORING.i_excessive,
        None,
    );
    add_soft(
        i,
        Term::TopSpeed,
        (stats.max_speed - SCORING.intensity_speed_threshold).max(0.0),
        SCORING.i_speed,
        None,
    );
    add_soft(
        i,
        Term::BiggestDrop,
        stats.biggest_drop,
        SCORING.i_drop,
        None,
    );
    add_soft(
        i,
        Term::Launch,
        if stats.launches > 0 {
            stats.max_forward_g
        } else {
            0.0
        },
        SCORING.launch,
        None,
    );
    add_soft(
        i,
        Term::Inversions,
        features.inversion_passages.len() as f64,
        SCORING.i_inversions,
        None,
    );
    add_soft(
        i,
        Term::Envelope,
        envelope.g_seconds,
        SCORING.i_envelope,
        location,
    );
    let intensity = i.iter().map(|item| item.value).sum::<f64>();
    let e = &mut breakdown.excitement;
    add_soft(e, Term::Airtime, airtime, SCORING.airtime, None);
    add_soft(
        e,
        Term::AirtimeMoments,
        features.active_airtime_moments as f64,
        SCORING.moments,
        None,
    );
    add_soft(e, Term::PositiveForces, positive, SCORING.positive, None);
    add_soft(
        e,
        Term::Laterals,
        bands.lateral_strong,
        SCORING.lateral,
        None,
    );
    add_soft(
        e,
        Term::LongitudinalForces,
        bands.longitudinal_strong,
        SCORING.longitudinal,
        None,
    );
    add_soft(e, Term::ExcessiveForces, excessive, SCORING.excessive, None);
    let inversion_points: f64 = features
        .inversion_passages
        .iter()
        .enumerate()
        .map(|(index, (kind, heartlining))| {
            let previous = features.inversion_passages[..index]
                .iter()
                .filter(|(previous, _)| previous == kind)
                .count();
            SCORING.inversion_repeat_factor.powi(previous as i32)
                * if *heartlining == Heartlining::Heartlined {
                    SCORING.heartlined_inversion_weight
                } else {
                    SCORING.non_heartlined_inversion_weight
                }
        })
        .sum();
    add_soft(
        e,
        Term::Inversions,
        inversion_points,
        SCORING.inversions,
        None,
    );
    add_soft(
        e,
        Term::TopSpeed,
        (stats.max_speed - SCORING.excitement_speed_threshold).max(0.0),
        SCORING.speed,
        None,
    );
    add_soft(e, Term::BiggestDrop, stats.biggest_drop, SCORING.drop, None);
    add_soft(
        e,
        Term::Pace,
        (features.active_average_speed - SCORING.pace_speed_threshold).max(0.0),
        SCORING.pace,
        None,
    );
    add_soft(
        e,
        Term::Launch,
        if stats.launches > 0 {
            stats.max_forward_g
        } else {
            0.0
        },
        SCORING.launch,
        None,
    );
    add_soft(
        e,
        Term::Variety,
        features.element_classes.len().saturating_sub(1) as f64,
        SCORING.variety,
        None,
    );
    add_soft(
        e,
        Term::Whip,
        features.snap_count as f64,
        SCORING.snap,
        None,
    );
    add_soft(
        e,
        Term::RollRate,
        (features.max_roll_rate_deg_s - SCORING.roll_rate_threshold).max(0.0),
        SCORING.roll,
        None,
    );
    add_soft(e, Term::RideTime, features.ride_seconds, SCORING.time, None);
    add(
        e,
        Term::ShortRide,
        features.ride_seconds,
        -SCORING.short_ride_weight
            * (1.0 - features.ride_seconds / SCORING.short_ride_seconds).max(0.0),
        None,
    );
    add_soft(
        e,
        Term::Proximity,
        features.proximity_strength,
        SCORING.proximity,
        None,
    );
    add(
        e,
        Term::Theming,
        features.theming,
        SCORING.theming_weight * features.theming,
        None,
    );
    add_soft(
        e,
        Term::DeadTime,
        features.dead_excess_seconds,
        SCORING.dead,
        features.longest_dead_run.map(|(location, _)| location),
    );
    add_soft(
        e,
        Term::Envelope,
        envelope.g_seconds,
        SCORING.envelope,
        location,
    );
    add(
        e,
        Term::Sickening,
        nausea,
        -SCORING.nausea_penalty_weight * (nausea - SCORING.nausea_penalty_threshold).max(0.0),
        None,
    );
    let raw_excitement = e.iter().map(|item| item.value).sum::<f64>();
    add(
        e,
        Term::ExcitementFloor,
        0.0,
        (-raw_excitement).max(0.0),
        None,
    );
    let excitement = raw_excitement.max(0.0);
    if let Some(worst) = &envelope.worst {
        use super::comfort::EnvelopeAxis;
        let (axis, unit) = match worst.axis {
            EnvelopeAxis::Vertical => ("vertical", "g"),
            EnvelopeAxis::Lateral => ("lateral", "g"),
            EnvelopeAxis::Longitudinal => ("longitudinal", "g"),
            EnvelopeAxis::VerticalLateral => ("verticalLateral", "ratio"),
            EnvelopeAxis::VerticalLongitudinal => ("verticalLongitudinal", "ratio"),
            EnvelopeAxis::LateralLongitudinal => ("lateralLongitudinal", "ratio"),
            EnvelopeAxis::Onset => ("onset", "g/s"),
        };
        let detail = ForceDetail {
            axis: axis.to_owned(),
            measured_value: worst.value,
            limit: worst.allowed,
            unit: unit.to_owned(),
            duration_seconds: Some(envelope.seconds),
            location: worst.location,
            weight: None,
            car_index: None,
            dangerous_seconds: None,
        };
        for contributions in [
            &mut breakdown.excitement,
            &mut breakdown.intensity,
            &mut breakdown.nausea,
        ] {
            contributions
                .iter_mut()
                .find(|item| item.term == Term::Envelope)
                .unwrap()
                .force_detail = Some(detail.clone());
        }
    }
    let details: Vec<_> = bands
        .excessive_details
        .iter()
        .zip(["vertical", "negativeVertical", "lateral", "longitudinal"])
        .enumerate()
        .filter_map(|(index, (detail, axis))| {
            detail.map(|detail| ForceDetail {
                axis: axis.to_owned(),
                measured_value: detail.value,
                limit: detail.limit,
                unit: "g".to_owned(),
                duration_seconds: Some(detail.seconds),
                location: detail.location,
                weight: None,
                car_index: None,
                dangerous_seconds: Some(bands.dangerous_seconds_by_axis[index]),
            })
        })
        .collect();
    for contributions in [
        &mut breakdown.excitement,
        &mut breakdown.intensity,
        &mut breakdown.nausea,
    ] {
        contributions
            .iter_mut()
            .find(|item| item.term == Term::ExcessiveForces)
            .unwrap()
            .force_details = details.clone();
    }
    let band_details: Vec<_> = bands
        .band_details
        .iter()
        .zip([
            "airtimeGood",
            "airtimeGreat",
            "verticalGood",
            "verticalGreat",
            "lateralStrong",
            "longitudinalStrong",
        ])
        .filter_map(|(detail, axis)| {
            detail.map(|detail| ForceDetail {
                axis: axis.to_owned(),
                measured_value: detail.value,
                limit: detail.limit,
                unit: "g".to_owned(),
                duration_seconds: Some(detail.seconds),
                location: detail.location,
                weight: Some(if axis == "airtimeGood" || axis == "verticalGood" {
                    SCORING.good_band_weight
                } else {
                    1.0
                }),
                car_index: None,
                dangerous_seconds: None,
            })
        })
        .collect();
    for contributions in [
        &mut breakdown.excitement,
        &mut breakdown.intensity,
        &mut breakdown.nausea,
    ] {
        for contribution in contributions {
            let axes: &[&str] = match contribution.term {
                Term::Airtime => &["airtimeGood", "airtimeGreat"],
                Term::PositiveForces | Term::GreyOut => &["verticalGood", "verticalGreat"],
                Term::Laterals => &["lateralStrong"],
                Term::LongitudinalForces => &["longitudinalStrong"],
                Term::SustainedForce => &[
                    "airtimeGood",
                    "airtimeGreat",
                    "verticalGood",
                    "verticalGreat",
                    "lateralStrong",
                    "longitudinalStrong",
                ],
                _ => &[],
            };
            if !axes.is_empty() {
                contribution.force_details = band_details
                    .iter()
                    .filter(|detail| axes.contains(&detail.axis.as_str()))
                    .cloned()
                    .collect();
            }
        }
    }
    let peak_details: Vec<_> = bands
        .peak_details
        .iter()
        .zip([
            "peakVertical",
            "peakNegativeVertical",
            "peakLateral",
            "peakForward",
            "peakBraking",
        ])
        .filter_map(|(detail, axis)| {
            detail.map(|detail| ForceDetail {
                axis: axis.to_owned(),
                measured_value: detail.value,
                limit: detail.limit,
                unit: "g".to_owned(),
                duration_seconds: None,
                location: detail.location,
                weight: None,
                car_index: None,
                dangerous_seconds: None,
            })
        })
        .collect();
    if let Some(peak) = peak_details.iter().max_by(|left, right| {
        (left.measured_value / left.limit).total_cmp(&(right.measured_value / right.limit))
    }) {
        let contribution = breakdown
            .intensity
            .iter_mut()
            .find(|item| item.term == Term::PeakForce)
            .unwrap();
        contribution.location = Some(peak.location);
        contribution.force_details = vec![peak.clone()];
    }
    if stats.launches > 0
        && let Some(forward) = peak_details
            .iter()
            .find(|detail| detail.axis == "peakForward")
    {
        for contribution in breakdown
            .excitement
            .iter_mut()
            .chain(&mut breakdown.intensity)
            .filter(|item| item.term == Term::Launch)
        {
            let mut detail = forward.clone();
            detail.axis = "launchForward".to_owned();
            detail.limit = 0.0;
            contribution.location = Some(detail.location);
            contribution.force_details = vec![detail];
        }
    }
    Scores {
        excitement,
        intensity,
        nausea,
        breakdown,
    }
}
