use theme_park_sim::track::{ComfortableForceLimits, ForceRatingAxis};
use theme_park_sim_wasm::reports::TestingSample;

const ENTRY_SPEED_TOLERANCE_METRES_PER_SECOND: f64 = 0.5;
const FINISHED_FORCE_TOLERANCE_G: f64 = 0.2;
const PLACEMENT_FORCE_TOLERANCE_G: f64 = 0.3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgreementCheck {
    FinishedEstimateVsFrontCar,
    FinishedEstimateVsWorstCar,
    PlacementEstimateVsWorstCar,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlacementFailureClassification {
    DownstreamTrack,
    EstimateError,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlacementPeakContext {
    pub peak_car_index: usize,
    pub later_car_index: Option<usize>,
    pub later_piece_index: Option<usize>,
}

#[derive(Clone, Debug, Default)]
pub struct PlacementPeakContexts {
    pub minimum_vertical_g: Option<PlacementPeakContext>,
    pub maximum_vertical_g: Option<PlacementPeakContext>,
    pub maximum_lateral_g: Option<PlacementPeakContext>,
    pub maximum_longitudinal_g: Option<PlacementPeakContext>,
}

impl PlacementPeakContexts {
    fn for_metric(&self, metric: ForcePeakMetric) -> Option<PlacementPeakContext> {
        match metric {
            ForcePeakMetric::MinimumVerticalG => self.minimum_vertical_g,
            ForcePeakMetric::MaximumVerticalG => self.maximum_vertical_g,
            ForcePeakMetric::MaximumLateralG => self.maximum_lateral_g,
            ForcePeakMetric::MaximumLongitudinalG => self.maximum_longitudinal_g,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CheckResult {
    Passed,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ForcePeaks {
    pub minimum_vertical_g: f64,
    pub maximum_vertical_g: f64,
    pub maximum_lateral_g: f64,
    pub maximum_longitudinal_g: f64,
}

impl ForcePeaks {
    pub fn from_preview_samples<'a>(
        samples: impl IntoIterator<Item = &'a TestingSample>,
    ) -> Option<Self> {
        let mut samples = samples.into_iter();
        let first_sample = samples.next()?;
        let mut peaks = Self {
            minimum_vertical_g: first_sample.vertical_g,
            maximum_vertical_g: first_sample.vertical_g,
            maximum_lateral_g: first_sample.lateral_g.abs(),
            maximum_longitudinal_g: first_sample.longitudinal_g.abs(),
        };
        for sample in samples {
            peaks.minimum_vertical_g = peaks.minimum_vertical_g.min(sample.vertical_g);
            peaks.maximum_vertical_g = peaks.maximum_vertical_g.max(sample.vertical_g);
            peaks.maximum_lateral_g = peaks.maximum_lateral_g.max(sample.lateral_g.abs());
            peaks.maximum_longitudinal_g = peaks
                .maximum_longitudinal_g
                .max(sample.longitudinal_g.abs());
        }
        Some(peaks)
    }
}

impl From<theme_park_sim::physics::ForcePeakValues> for ForcePeaks {
    fn from(peaks: theme_park_sim::physics::ForcePeakValues) -> Self {
        Self {
            minimum_vertical_g: peaks.minimum_vertical_g,
            maximum_vertical_g: peaks.maximum_vertical_g,
            maximum_lateral_g: peaks.maximum_lateral_g,
            maximum_longitudinal_g: peaks.maximum_longitudinal_g,
        }
    }
}

#[derive(Clone, Debug)]
pub struct AgreementInput {
    pub piece_index: usize,
    pub piece_name: String,
    pub model_name: String,
    pub placement_entry_speed_estimate: Option<f64>,
    pub entry_speed_estimate: Option<f64>,
    pub entry_speed_measured_front: Option<f64>,
    pub placement_estimate: Option<ForcePeaks>,
    pub finished_track_estimate: Option<ForcePeaks>,
    pub finished_track_worst_car_estimate: Option<ForcePeaks>,
    pub measured_front: Option<ForcePeaks>,
    pub measured_worst: Option<ForcePeaks>,
    pub placement_peak_contexts: PlacementPeakContexts,
    pub limits: ComfortableForceLimits,
}

#[derive(Clone, Debug)]
pub struct AgreementRow {
    pub piece_index: usize,
    pub piece_name: String,
    pub placement_entry_speed_estimate: Option<f64>,
    pub entry_speed_estimate: Option<f64>,
    pub entry_speed_measured_front: Option<f64>,
    pub placement_estimate: Option<ForcePeaks>,
    pub finished_track_estimate: Option<ForcePeaks>,
    pub finished_track_worst_car_estimate: Option<ForcePeaks>,
    pub measured_front: Option<ForcePeaks>,
    pub measured_worst: Option<ForcePeaks>,
    pub check_2: CheckResult,
    pub check_3: CheckResult,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgreementFailure {
    pub piece_index: usize,
    pub piece_name: String,
    pub check: AgreementCheck,
    pub metric: &'static str,
    pub reason: String,
    pub classification: Option<PlacementFailureClassification>,
    pub cause: Option<String>,
}

#[derive(Clone, Debug)]
pub struct AgreementReport {
    pub rows: Vec<AgreementRow>,
    pub failures: Vec<AgreementFailure>,
    pub passed: bool,
}

pub fn evaluate_agreement(inputs: &[AgreementInput]) -> AgreementReport {
    if inputs.is_empty() {
        return AgreementReport {
            rows: Vec::new(),
            failures: vec![AgreementFailure {
                piece_index: 0,
                piece_name: String::new(),
                check: AgreementCheck::FinishedEstimateVsWorstCar,
                metric: "ride",
                reason: "ride has no piece measurements".into(),
                classification: None,
                cause: None,
            }],
            passed: false,
        };
    }

    let mut rows = Vec::with_capacity(inputs.len());
    let mut failures = Vec::new();

    for input in inputs {
        let failure_start = failures.len();
        compare_finished_estimate_with_front_car(input, &mut failures);
        compare_finished_estimate_with_worst_car(input, &mut failures);
        let check_2 = result_since(&failures, failure_start);

        let failure_start = failures.len();
        compare_placement_estimate_with_worst_car(input, &mut failures);
        let check_3 = result_since(&failures, failure_start);

        rows.push(AgreementRow {
            piece_index: input.piece_index,
            piece_name: input.piece_name.clone(),
            placement_entry_speed_estimate: input.placement_entry_speed_estimate,
            entry_speed_estimate: input.entry_speed_estimate,
            entry_speed_measured_front: input.entry_speed_measured_front,
            placement_estimate: input.placement_estimate,
            finished_track_estimate: input.finished_track_estimate,
            finished_track_worst_car_estimate: input.finished_track_worst_car_estimate,
            measured_front: input.measured_front,
            measured_worst: input.measured_worst,
            check_2,
            check_3,
        });
    }

    AgreementReport {
        passed: failures
            .iter()
            .all(|failure| failure.check == AgreementCheck::PlacementEstimateVsWorstCar),
        rows,
        failures,
    }
}

fn result_since(failures: &[AgreementFailure], start: usize) -> CheckResult {
    if failures.len() == start {
        CheckResult::Passed
    } else {
        CheckResult::Failed
    }
}

fn compare_finished_estimate_with_worst_car(
    input: &AgreementInput,
    failures: &mut Vec<AgreementFailure>,
) {
    compare_force_values(
        input,
        ForceComparison {
            check: AgreementCheck::FinishedEstimateVsWorstCar,
            estimate: input.finished_track_worst_car_estimate,
            measured: input.measured_worst,
            tolerance_g: FINISHED_FORCE_TOLERANCE_G,
            compare_rating_bands: false,
            placement_contexts: None,
        },
        failures,
    );
}

fn compare_finished_estimate_with_front_car(
    input: &AgreementInput,
    failures: &mut Vec<AgreementFailure>,
) {
    match (input.entry_speed_estimate, input.entry_speed_measured_front) {
        (Some(estimated), Some(measured)) => {
            if (estimated - measured).abs() > ENTRY_SPEED_TOLERANCE_METRES_PER_SECOND {
                failures.push(failure(
                    input,
                    AgreementCheck::FinishedEstimateVsFrontCar,
                    "entry speed",
                    format!(
                        "estimate {estimated:.3} m/s differs from front car {measured:.3} m/s by more than {ENTRY_SPEED_TOLERANCE_METRES_PER_SECOND:.1} m/s"
                    ),
                ));
            }
        }
        _ => failures.push(failure(
            input,
            AgreementCheck::FinishedEstimateVsFrontCar,
            "entry speed",
            "estimate or front-car measurement is unavailable".into(),
        )),
    }

    compare_force_values(
        input,
        ForceComparison {
            check: AgreementCheck::FinishedEstimateVsFrontCar,
            estimate: input.finished_track_estimate,
            measured: input.measured_front,
            tolerance_g: FINISHED_FORCE_TOLERANCE_G,
            compare_rating_bands: false,
            placement_contexts: None,
        },
        failures,
    );
}

fn compare_placement_estimate_with_worst_car(
    input: &AgreementInput,
    failures: &mut Vec<AgreementFailure>,
) {
    compare_force_values(
        input,
        ForceComparison {
            check: AgreementCheck::PlacementEstimateVsWorstCar,
            estimate: input.placement_estimate,
            measured: input.measured_worst,
            tolerance_g: PLACEMENT_FORCE_TOLERANCE_G,
            compare_rating_bands: true,
            placement_contexts: Some(&input.placement_peak_contexts),
        },
        failures,
    );
}

#[derive(Clone, Copy)]
enum ForcePeakMetric {
    MinimumVerticalG,
    MaximumVerticalG,
    MaximumLateralG,
    MaximumLongitudinalG,
}

struct ForceComparison<'a> {
    check: AgreementCheck,
    estimate: Option<ForcePeaks>,
    measured: Option<ForcePeaks>,
    tolerance_g: f64,
    compare_rating_bands: bool,
    placement_contexts: Option<&'a PlacementPeakContexts>,
}

fn compare_force_values(
    input: &AgreementInput,
    comparison: ForceComparison<'_>,
    failures: &mut Vec<AgreementFailure>,
) {
    let ForceComparison {
        check,
        estimate,
        measured,
        tolerance_g,
        compare_rating_bands,
        placement_contexts,
    } = comparison;
    let Some((estimate, measured)) = estimate.zip(measured) else {
        failures.push(placement_failure(
            input,
            check,
            "force peaks",
            "estimate or measured samples are unavailable".into(),
            None,
        ));
        return;
    };

    for (metric, metric_name, axis, estimate_value, measured_value) in [
        (
            ForcePeakMetric::MinimumVerticalG,
            "minimum vertical g",
            ForceRatingAxis::Airtime,
            estimate.minimum_vertical_g,
            measured.minimum_vertical_g,
        ),
        (
            ForcePeakMetric::MaximumVerticalG,
            "maximum vertical g",
            ForceRatingAxis::Vertical,
            estimate.maximum_vertical_g,
            measured.maximum_vertical_g,
        ),
        (
            ForcePeakMetric::MaximumLateralG,
            "lateral g",
            ForceRatingAxis::Lateral,
            estimate.maximum_lateral_g,
            measured.maximum_lateral_g,
        ),
        (
            ForcePeakMetric::MaximumLongitudinalG,
            "longitudinal g",
            ForceRatingAxis::Longitudinal,
            estimate.maximum_longitudinal_g,
            measured.maximum_longitudinal_g,
        ),
    ] {
        let difference = (estimate_value - measured_value).abs();
        if difference > tolerance_g {
            failures.push(placement_failure(
                input,
                check,
                metric_name,
                format!(
                    "estimate {estimate_value:.3} g differs from measurement {measured_value:.3} g by {difference:.3} g, over the {tolerance_g:.1} g tolerance"
                ),
                placement_contexts.and_then(|contexts| contexts.for_metric(metric)),
            ));
        }

        if compare_rating_bands
            && rating_severity(axis, estimate_value, input)
                < rating_severity(axis, measured_value, input)
        {
            let estimated_band =
                input
                    .limits
                    .classify_force(axis, estimate_value, &input.model_name);
            let measured_band =
                input
                    .limits
                    .classify_force(axis, measured_value, &input.model_name);
            failures.push(placement_failure(
                input,
                check,
                metric_name,
                format!(
                    "estimated rating {estimated_band:?} is milder than measured rating {measured_band:?}"
                ),
                placement_contexts.and_then(|contexts| contexts.for_metric(metric)),
            ));
        }
    }
}

fn rating_severity(axis: ForceRatingAxis, value: f64, input: &AgreementInput) -> usize {
    let rating = input.limits.classify_force(axis, value, &input.model_name);
    input
        .limits
        .rating_bands(axis, &input.model_name)
        .iter()
        .position(|band| band.rating == rating)
        .expect("classified force has a rating band")
}

fn failure(
    input: &AgreementInput,
    check: AgreementCheck,
    metric: &'static str,
    reason: String,
) -> AgreementFailure {
    AgreementFailure {
        piece_index: input.piece_index,
        piece_name: input.piece_name.clone(),
        check,
        metric,
        reason,
        classification: None,
        cause: None,
    }
}

fn placement_failure(
    input: &AgreementInput,
    check: AgreementCheck,
    metric: &'static str,
    reason: String,
    peak_context: Option<PlacementPeakContext>,
) -> AgreementFailure {
    if check != AgreementCheck::PlacementEstimateVsWorstCar {
        return failure(input, check, metric, reason);
    }
    let (classification, cause) = match peak_context {
        Some(PlacementPeakContext {
            peak_car_index,
            later_car_index: Some(later_car_index),
            later_piece_index: Some(later_piece_index),
        }) => (
            PlacementFailureClassification::DownstreamTrack,
            format!(
                "car {peak_car_index} reached the measured peak while car {later_car_index} was on later piece {later_piece_index}"
            ),
        ),
        Some(PlacementPeakContext {
            peak_car_index,
            later_piece_index: None,
            ..
        }) => (
            PlacementFailureClassification::EstimateError,
            format!(
                "car {peak_car_index} reached the measured peak while no car was on a later piece"
            ),
        ),
        Some(PlacementPeakContext {
            later_car_index: None,
            later_piece_index: Some(later_piece_index),
            ..
        }) => (
            PlacementFailureClassification::EstimateError,
            format!(
                "later piece {later_piece_index} was identified without a car location"
            ),
        ),
        None => (
            PlacementFailureClassification::EstimateError,
            "the measured peak location is unavailable, so downstream track could not be established".into(),
        ),
    };
    AgreementFailure {
        piece_index: input.piece_index,
        piece_name: input.piece_name.clone(),
        check,
        metric,
        reason,
        classification: Some(classification),
        cause: Some(cause),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AgreementCheck, AgreementInput, CheckResult, ForcePeaks, PlacementFailureClassification,
        PlacementPeakContext, PlacementPeakContexts, evaluate_agreement,
    };
    use theme_park_sim::track::ComfortableForceLimits;

    fn force_peaks(minimum_vertical_g: f64, maximum_vertical_g: f64) -> ForcePeaks {
        ForcePeaks {
            minimum_vertical_g,
            maximum_vertical_g,
            maximum_lateral_g: 0.2,
            maximum_longitudinal_g: 0.3,
        }
    }

    fn agreement_input(
        placement_estimate: ForcePeaks,
        finished_track_estimate: ForcePeaks,
        measured_front: ForcePeaks,
        measured_worst: ForcePeaks,
    ) -> AgreementInput {
        AgreementInput {
            piece_index: 2,
            piece_name: "FloaterHill24x160".into(),
            model_name: "Timber Tumbler".into(),
            placement_entry_speed_estimate: Some(30.0),
            entry_speed_estimate: Some(30.0),
            entry_speed_measured_front: Some(30.0),
            placement_estimate: Some(placement_estimate),
            finished_track_estimate: Some(finished_track_estimate),
            finished_track_worst_car_estimate: Some(finished_track_estimate),
            measured_front: Some(measured_front),
            measured_worst: Some(measured_worst),
            placement_peak_contexts: PlacementPeakContexts::default(),
            limits: ComfortableForceLimits {
                minimum_vertical_g: -1.5,
                maximum_vertical_g: 5.0,
                maximum_lateral_g: 1.5,
                maximum_longitudinal_g: 2.0,
            },
        }
    }

    #[test]
    fn equal_estimates_and_measurements_pass_both_native_checks() {
        let forces = force_peaks(-0.9, 3.2);
        let report = evaluate_agreement(&[agreement_input(forces, forces, forces, forces)]);

        assert!(report.passed);
        assert!(report.failures.is_empty());
        assert_eq!(report.rows[0].check_2, CheckResult::Passed);
        assert_eq!(report.rows[0].check_3, CheckResult::Passed);
    }

    #[test]
    fn check_two_compares_finished_estimate_with_worst_car() {
        let measured_front = force_peaks(-0.9, 3.2);
        let mut measured_worst = measured_front;
        measured_worst.maximum_vertical_g += 0.3;
        let report = evaluate_agreement(&[agreement_input(
            measured_worst,
            measured_front,
            measured_front,
            measured_worst,
        )]);

        assert_eq!(report.rows[0].check_2, CheckResult::Failed);
        assert_eq!(report.rows[0].check_3, CheckResult::Passed);
        assert!(report.failures.iter().any(|failure| {
            failure.check == AgreementCheck::FinishedEstimateVsWorstCar
                && failure.metric == "maximum vertical g"
        }));
    }

    #[test]
    fn check_two_fails_when_finished_front_estimate_differs_from_front_measurement() {
        let estimated = force_peaks(-0.9, 3.2);
        let mut measured_front = estimated;
        measured_front.maximum_vertical_g += 0.3;
        let measured_worst = estimated;
        let report = evaluate_agreement(&[agreement_input(
            estimated,
            estimated,
            measured_front,
            measured_worst,
        )]);

        assert_eq!(report.rows[0].check_2, CheckResult::Failed);
    }

    #[test]
    fn check_two_compares_finished_estimate_with_front_car() {
        let measured_worst = force_peaks(-0.9, 3.2);
        let mut measured_front = measured_worst;
        measured_front.maximum_vertical_g += 0.3;
        let report = evaluate_agreement(&[agreement_input(
            measured_worst,
            measured_worst,
            measured_front,
            measured_worst,
        )]);

        assert_eq!(report.rows[0].check_2, CheckResult::Failed);
        assert_eq!(report.rows[0].check_3, CheckResult::Passed);
        assert!(report.failures.iter().any(|failure| {
            failure.check == AgreementCheck::FinishedEstimateVsFrontCar
                && failure.metric == "maximum vertical g"
        }));
    }

    #[test]
    fn placement_estimate_with_milder_band_fails_check_three() {
        let placement = force_peaks(-0.9, 2.8);
        let measured_worst = force_peaks(-0.9, 3.1);
        let finished = force_peaks(-0.9, 3.1);
        let report = evaluate_agreement(&[agreement_input(
            placement,
            finished,
            finished,
            measured_worst,
        )]);

        assert!(report.passed);
        assert!(report.failures.iter().any(|failure| {
            failure.check == AgreementCheck::PlacementEstimateVsWorstCar
                && failure.metric == "maximum vertical g"
                && failure.reason.contains("milder")
                && failure.classification == Some(PlacementFailureClassification::EstimateError)
        }));
    }

    #[test]
    fn placement_failure_is_classified_from_peak_time_context_and_is_informational() {
        let placement = force_peaks(-0.9, 2.8);
        let measured_worst = force_peaks(-0.9, 3.1);
        let mut input = agreement_input(placement, measured_worst, measured_worst, measured_worst);
        input.placement_peak_contexts.maximum_vertical_g = Some(PlacementPeakContext {
            peak_car_index: 3,
            later_car_index: Some(2),
            later_piece_index: Some(5),
        });

        let report = evaluate_agreement(&[input]);

        assert!(report.passed);
        let failure = report
            .failures
            .iter()
            .find(|failure| failure.metric == "maximum vertical g")
            .unwrap();
        assert_eq!(failure.check, AgreementCheck::PlacementEstimateVsWorstCar);
        assert_eq!(
            failure.classification,
            Some(PlacementFailureClassification::DownstreamTrack)
        );
        assert!(failure.cause.as_deref().unwrap().contains("later piece 5"));
    }

    #[test]
    fn check_three_attributes_each_force_axis_independently() {
        let estimated = ForcePeaks {
            minimum_vertical_g: -0.8,
            maximum_vertical_g: 3.0,
            maximum_lateral_g: 0.2,
            maximum_longitudinal_g: 0.2,
        };
        let measured = ForcePeaks {
            minimum_vertical_g: -1.3,
            maximum_vertical_g: 3.6,
            maximum_lateral_g: 0.9,
            maximum_longitudinal_g: 1.0,
        };
        let report =
            evaluate_agreement(&[agreement_input(estimated, estimated, estimated, measured)]);

        assert_eq!(report.rows[0].check_3, CheckResult::Failed);
        assert!(report.failures.iter().any(|failure| {
            failure.check == AgreementCheck::PlacementEstimateVsWorstCar
                && failure.metric == "minimum vertical g"
        }));
        assert!(report.failures.iter().any(|failure| {
            failure.check == AgreementCheck::PlacementEstimateVsWorstCar
                && failure.metric == "maximum vertical g"
        }));
        assert!(report.failures.iter().any(|failure| {
            failure.check == AgreementCheck::PlacementEstimateVsWorstCar
                && failure.metric == "lateral g"
        }));
        assert!(report.failures.iter().any(|failure| {
            failure.check == AgreementCheck::PlacementEstimateVsWorstCar
                && failure.metric == "longitudinal g"
        }));
    }
}
