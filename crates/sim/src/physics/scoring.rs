use crate::track::ComfortableForceLimits;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ForceAxis {
    Airtime,
    Vertical,
    Lateral,
    Longitudinal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ForceBand {
    Weak,
    Good,
    Great,
    Fine,
    Strong,
    TooMuch,
    Dangerous,
}

pub(super) fn force_band(
    axis: ForceAxis,
    value_g: f64,
    limits: ComfortableForceLimits,
) -> ForceBand {
    assert!(
        limits.minimum_vertical_g < 0.0
            && limits.maximum_vertical_g > 0.0
            && limits.maximum_lateral_g > 0.0
            && limits.maximum_longitudinal_g > 0.0
    );
    let (magnitude, limit) = match axis {
        ForceAxis::Airtime => (-value_g, -limits.minimum_vertical_g),
        ForceAxis::Vertical => (value_g, limits.maximum_vertical_g),
        ForceAxis::Lateral => (value_g.abs(), limits.maximum_lateral_g),
        ForceAxis::Longitudinal => (value_g.abs(), limits.maximum_longitudinal_g),
    };
    match axis {
        ForceAxis::Airtime if magnitude <= SCORING.airtime_weak_limit => ForceBand::Weak,
        ForceAxis::Airtime if magnitude <= SCORING.airtime_good_limit => ForceBand::Good,
        ForceAxis::Vertical if magnitude <= SCORING.vertical_weak_limit => ForceBand::Weak,
        ForceAxis::Vertical if magnitude <= SCORING.vertical_good_limit_fraction * limit => {
            ForceBand::Good
        }
        ForceAxis::Airtime | ForceAxis::Vertical if magnitude <= limit => ForceBand::Great,
        ForceAxis::Lateral | ForceAxis::Longitudinal
            if magnitude <= SCORING.horizontal_fine_limit_fraction * limit =>
        {
            ForceBand::Fine
        }
        ForceAxis::Lateral | ForceAxis::Longitudinal if magnitude <= limit => ForceBand::Strong,
        _ if magnitude <= SCORING.excessive_limit_fraction * limit => ForceBand::TooMuch,
        _ => ForceBand::Dangerous,
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
        assert_eq!(force_band(ForceAxis::Airtime, 0.0, limits), ForceBand::Weak);
        assert_eq!(
            force_band(ForceAxis::Airtime, -0.9, limits),
            ForceBand::Good
        );
        assert_eq!(
            force_band(ForceAxis::Airtime, -1.0, limits),
            ForceBand::Great
        );
        assert_eq!(
            force_band(ForceAxis::Airtime, -1.5, limits),
            ForceBand::Great
        );
        assert_eq!(
            force_band(ForceAxis::Airtime, -1.6, limits),
            ForceBand::TooMuch
        );
        assert_eq!(
            force_band(ForceAxis::Lateral, -0.75, limits),
            ForceBand::Fine
        );
        assert_eq!(
            force_band(ForceAxis::Lateral, -1.5, limits),
            ForceBand::Strong
        );
        assert_eq!(
            force_band(ForceAxis::Lateral, -2.0, limits),
            ForceBand::Dangerous
        );
        let stricter = ComfortableForceLimits {
            minimum_vertical_g: -1.0,
            ..limits
        };
        assert_eq!(
            force_band(ForceAxis::Airtime, -1.1, stricter),
            ForceBand::TooMuch
        );
    }
}

#[cfg(test)]
mod score_tests {
    use super::*;
    use crate::physics::features::{PieceSummary, ride_features};
    use crate::physics::TestSample;
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
    fn airtime_moments_keep_paying_and_envelope_penalty_keeps_location() {
        use crate::physics::comfort::{EnvelopeAxis, EnvelopeWorst};
        let mut features = ride_features(&[], &[], &[], 0.0, false);
        features.active_airtime_moments = 10;
        let ten = score(&features, &ForceBandDurations::default());
        features.active_airtime_moments = 20;
        let twenty = score(&features, &ForceBandDurations::default());
        assert!(twenty.excitement - ten.excitement > 0.2);
        let location = Location {
            piece_index: 2,
            piece_name: "Loop",
            distance_m: 40.0,
            seconds_from_start: 5.0,
        };
        features.envelope.g_seconds = 0.45;
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
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct ForceBandDurations {
    pub(super) airtime_good: f64,
    pub(super) airtime_great: f64,
    pub(super) vertical_good: f64,
    pub(super) vertical_great: f64,
    pub(super) lateral_strong: f64,
    pub(super) longitudinal_strong: f64,
    pub(super) too_much: f64,
    pub(super) dangerous: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

#[derive(Clone, Debug, PartialEq)]
pub struct Contribution {
    pub term: Term,
    pub value: f64,
    pub measure: f64,
    pub location: Option<Location>,
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
#[derive(Clone, Debug, Default, PartialEq)]
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
    airtime_weak_limit: f64,
    airtime_good_limit: f64,
    vertical_weak_limit: f64,
    vertical_good_limit_fraction: f64,
    horizontal_fine_limit_fraction: f64,
    excessive_limit_fraction: f64,
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
    airtime_weak_limit: 0.2,
    airtime_good_limit: 0.9,
    vertical_weak_limit: 2.0,
    vertical_good_limit_fraction: 0.8,
    horizontal_fine_limit_fraction: 0.5,
    excessive_limit_fraction: 1.3,
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
    excessive: (-1.5, 2.0),
    moments: (1.2, 10.0),
    inversions: (2.0, 2.0),
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
    dead: (-1.5, 6.0),
    envelope: (-3.0, 1.0),
};
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
    let excitement = e.iter().map(|item| item.value).sum();
    Scores {
        excitement,
        intensity,
        nausea,
        breakdown,
    }
}
