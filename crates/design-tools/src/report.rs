use std::collections::BTreeMap;
use std::fmt::Write as _;

use serde::{Deserialize, Serialize};
use theme_park_sim::physics::{Contribution, ForcePeakValues, TestRun};
use theme_park_sim::track::ride_file::{RideFile, RideStep};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ForceRatings {
    pub airtime: String,
    pub vertical: String,
    pub lateral: String,
    pub longitudinal: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RidePieceEstimate {
    pub piece_index: usize,
    pub piece_id: String,
    pub entry_speed: Option<f64>,
    pub rated_window: Option<(f64, f64)>,
    pub estimated: Option<ForcePeakValues>,
    pub ratings: Option<ForceRatings>,
    pub preview_status: String,
    pub notes: Vec<String>,
}

pub fn render_ride_report(
    ride: &RideFile,
    test_run: &TestRun,
    estimates: &[RidePieceEstimate],
    include_samples: bool,
) -> Result<String, String> {
    let mut estimates_by_piece = BTreeMap::new();
    for estimate in estimates {
        if estimates_by_piece
            .insert(estimate.piece_index, estimate)
            .is_some()
        {
            return Err(format!(
                "ride report has duplicate estimate for piece index {}",
                estimate.piece_index
            ));
        }
    }

    let mut peaks_by_piece = BTreeMap::new();
    for peaks in test_run.piece_force_peaks() {
        if peaks_by_piece.insert(peaks.piece_index, peaks).is_some() {
            return Err(format!(
                "ride report has duplicate measured peaks for piece index {}",
                peaks.piece_index
            ));
        }
    }

    let placed_piece_count = ride
        .steps
        .iter()
        .filter(|step| matches!(step, RideStep::Place(_)))
        .count();
    let mut piece_indices: BTreeMap<usize, ()> = (0..=placed_piece_count)
        .map(|piece_index| (piece_index, ()))
        .collect();
    piece_indices.extend(
        estimates_by_piece
            .keys()
            .map(|piece_index| (*piece_index, ())),
    );
    piece_indices.extend(peaks_by_piece.keys().map(|piece_index| (*piece_index, ())));

    let mut output = String::new();
    writeln!(output, "Ride: {} ({})", ride.name, ride.model).unwrap();
    writeln!(
        output,
        "Scores: excitement {:.2}, intensity {:.2}, nausea {:.2}",
        test_run.excitement, test_run.intensity, test_run.nausea
    )
    .unwrap();
    writeln!(
        output,
        "Test: {} after {:.1} m",
        if test_run.completed {
            "completed"
        } else {
            "stopped"
        },
        test_run.furthest_distance
    )
    .unwrap();
    writeln!(output, "Score breakdown:").unwrap();
    write_contributions(&mut output, "excitement", &test_run.breakdown.excitement);
    write_contributions(&mut output, "intensity", &test_run.breakdown.intensity);
    write_contributions(&mut output, "nausea", &test_run.breakdown.nausea);
    writeln!(output, "Statistics:").unwrap();
    writeln!(
        output,
        "  length {:.1} m; duration {:.1} s; average speed {:.1} m/s; maximum speed {:.1} m/s",
        test_run.statistics.track_length,
        test_run.statistics.duration_seconds,
        test_run.statistics.average_speed,
        test_run.statistics.max_speed
    )
    .unwrap();
    writeln!(
        output,
        "  vertical {:.2}/{:.2} g; lateral {:.2} g; forward {:.2} g; braking {:.2} g",
        test_run.statistics.max_positive_vertical_g,
        test_run.statistics.max_negative_vertical_g,
        test_run.statistics.max_lateral_g,
        test_run.statistics.max_forward_g,
        test_run.statistics.max_braking_g
    )
    .unwrap();
    writeln!(
        output,
        "  airtime {:.1} s ejector, {:.1} s floater, {} moments; strong forces {:.1} s, longest {:.1} s; lateral {:.1} s",
        test_run.statistics.ejector_airtime_seconds,
        test_run.statistics.floater_airtime_seconds,
        test_run.statistics.airtime_moments,
        test_run.statistics.strong_positive_g_seconds,
        test_run.statistics.longest_strong_positive_g_seconds,
        test_run.statistics.strong_lateral_g_seconds
    )
    .unwrap();
    writeln!(
        output,
        "  maximum height {:.1} m; biggest drop {:.1} m at {:.1} degrees; inversions {}; launches {} at {:.1} m/s; drops {}",
        test_run.statistics.max_height,
        test_run.statistics.biggest_drop,
        test_run.statistics.steepest_drop_angle_degrees,
        test_run.statistics.inversions,
        test_run.statistics.launches,
        test_run.statistics.launch_peak_speed,
        test_run.statistics.drops
    )
    .unwrap();
    let sustained = test_run.sustained_force_peaks();
    let held_g = |value: Option<f64>| value.map_or_else(|| "none".to_owned(), |g| format!("{g:.2} g"));
    writeln!(
        output,
        "  sustained by any car: highest vertical {} held 0.5 s; lowest vertical {} held 0.3 s",
        held_g(sustained.maximum_vertical_g),
        held_g(sustained.minimum_vertical_g)
    )
    .unwrap();
    writeln!(
        output,
        "# piece entry_m/s window_m/s margin_m/s est_v+/-_g front_v+/-_g worst_v+/-_g worst_lat_g worst_long_g ratings status notes"
    )
    .unwrap();

    for piece_index in piece_indices.keys().copied() {
        let estimate = estimates_by_piece.get(&piece_index).copied();
        let peaks = peaks_by_piece.get(&piece_index);
        let piece_id = estimate.map_or_else(
            || {
                if piece_index == 0 {
                    "Station".to_owned()
                } else {
                    format!("piece-{piece_index}")
                }
            },
            |estimate| estimate.piece_id.clone(),
        );
        let entry_speed = estimate.and_then(|estimate| estimate.entry_speed);
        let rated_window = estimate.and_then(|estimate| estimate.rated_window);
        let margin = match (entry_speed, rated_window) {
            (Some(speed), Some((minimum, maximum))) => Some((speed - minimum).min(maximum - speed)),
            _ => None,
        };
        let estimated = estimate.and_then(|estimate| estimate.estimated);
        let front_car = peaks.and_then(|peaks| peaks.front_car);
        let worst_car = peaks.and_then(|peaks| peaks.worst_car);
        let ratings = estimate.map_or_else(String::new, |estimate| {
            estimate
                .ratings
                .as_ref()
                .map_or_else(String::new, |ratings| {
                    format!(
                        "airtime:{} vertical:{} lateral:{} longitudinal:{}",
                        ratings.airtime, ratings.vertical, ratings.lateral, ratings.longitudinal
                    )
                })
        });
        let status = estimate.map_or("—", |estimate| estimate.preview_status.as_str());
        let notes = estimate.map_or_else(String::new, |estimate| estimate.notes.join("; "));
        writeln!(
            output,
            "{piece_index:>3} {piece_id:<24} {entry_speed:>9} {rated_window:>13} {margin:>8} {estimated:>11} {front_car:>11} {worst_car:>11} {lateral:>9} {longitudinal:>10} {ratings} {status} {notes}",
            entry_speed = format_optional(entry_speed),
            rated_window = format_window(rated_window),
            margin = format_signed(margin),
            estimated = format_vertical_peaks(estimated),
            front_car = format_vertical_peaks(front_car),
            worst_car = format_vertical_peaks(worst_car),
            lateral = format_optional(worst_car.map(|peaks| peaks.maximum_lateral_g)),
            longitudinal = format_optional(worst_car.map(|peaks| peaks.maximum_longitudinal_g)),
        )
        .unwrap();
    }

    if !test_run.notes.is_empty() {
        writeln!(output, "Notes:").unwrap();
        for note in &test_run.notes {
            writeln!(output, "- {note}").unwrap();
        }
    }

    if include_samples {
        writeln!(output, "Samples (front car, one metre spacing):").unwrap();
        writeln!(
            output,
            "distance_m speed_m/s vertical_g lateral_g longitudinal_g piece_index"
        )
        .unwrap();
        for sample in &test_run.samples {
            writeln!(
                output,
                "{:.1} {:.1} {:.2} {:.2} {:.2} {}",
                sample.distance,
                sample.speed,
                sample.vertical_g,
                sample.lateral_g,
                sample.longitudinal_g,
                sample.piece_index
            )
            .unwrap();
        }
    }

    Ok(output)
}

fn format_optional(value: Option<f64>) -> String {
    value.map_or_else(|| "—".to_owned(), |value| format!("{value:.2}"))
}

fn write_contributions(output: &mut String, category: &str, contributions: &[Contribution]) {
    writeln!(output, "  {category}:").unwrap();
    if contributions.is_empty() {
        writeln!(output, "    none").unwrap();
        return;
    }
    for contribution in contributions {
        writeln!(output, "    {}", contribution.describe()).unwrap();
    }
}

fn format_signed(value: Option<f64>) -> String {
    value.map_or_else(|| "—".to_owned(), |value| format!("{value:+.2}"))
}

fn format_window(window: Option<(f64, f64)>) -> String {
    window.map_or_else(
        || "—".to_owned(),
        |(minimum, maximum)| format!("{minimum:.2}-{maximum:.2}"),
    )
}

fn format_vertical_peaks(peaks: Option<ForcePeakValues>) -> String {
    peaks.map_or_else(
        || "—".to_owned(),
        |peaks| {
            format!(
                "{:.2}/{:.2}",
                peaks.maximum_vertical_g, peaks.minimum_vertical_g
            )
        },
    )
}

#[cfg(test)]
mod tests {
    use super::{ForceRatings, RidePieceEstimate, render_ride_report};
    use theme_park_sim::physics::ForcePeakValues;
    use theme_park_sim::physics::{CarReading, GForces, TestRunRecorder, demo_circuit};
    use theme_park_sim::track::hybrid_model;
    use theme_park_sim::track::ride_file::{
        RideFile, RideFileFormat, RideHeading, RidePlaceStep, RideStation, RideStep,
    };

    #[test]
    fn report_table_includes_estimates_front_car_and_worst_car_peaks() {
        let ride = ride_file();
        let test_run = test_run();
        let estimate = RidePieceEstimate {
            piece_index: 1,
            piece_id: "FloaterHill24x160".to_owned(),
            entry_speed: Some(33.1),
            rated_window: Some((27.0, 34.5)),
            estimated: Some(force_peaks(-0.9, 3.6, 0.1, 0.4)),
            ratings: Some(ForceRatings {
                airtime: "great".to_owned(),
                vertical: "great".to_owned(),
                lateral: "fine".to_owned(),
                longitudinal: "fine".to_owned(),
            }),
            preview_status: "reachedEnd".to_owned(),
            notes: vec!["first drop".to_owned()],
        };

        let report = render_ride_report(&ride, &test_run, &[estimate], false).unwrap();

        assert!(report.contains("Ride: Golden test (hyper)"));
        let table_row = report
            .lines()
            .find(|line| line.contains("FloaterHill24x160"))
            .unwrap();
        for expected_value in [
            "33.10",
            "27.00-34.50",
            "+1.40",
            "3.60/-0.90",
            "2.50/2.50",
            "0.20",
            "0.40",
            "airtime:great vertical:great lateral:fine longitudinal:fine",
            "reachedEnd first drop",
        ] {
            assert!(table_row.contains(expected_value), "{table_row}");
        }
    }

    #[test]
    fn report_samples_option_prints_front_car_per_metre_values() {
        let report = render_ride_report(&ride_file(), &test_run(), &[], true).unwrap();

        assert!(report.contains("Samples (front car, one metre spacing):"));
        assert!(report.contains("10.0 2.50 -0.20 0.40 1"));
    }

    fn ride_file() -> RideFile {
        RideFile {
            format: RideFileFormat::HeartlineRide,
            version: 1,
            name: "Golden test".to_owned(),
            model: "hyper".to_owned(),
            station: RideStation {
                x: 1,
                y: 2,
                z: 0,
                heading: RideHeading::North,
            },
            cars_per_train: 1,
            trains: 1,
            steps: vec![RideStep::Place(RidePlaceStep {
                place: "FloaterHill24x160".to_owned(),
                launch_speed: None,
                note: Some("first drop".to_owned()),
            })],
            expect: None,
        }
    }

    fn test_run() -> theme_park_sim::physics::TestRun {
        let circuit = demo_circuit();
        let model = hybrid_model();
        let sample_distance = circuit.pieces()[0].length() + 1.0;
        let mut recorder = TestRunRecorder::for_circuit_with_model(&circuit, &model);
        recorder.record(
            &[vec![CarReading {
                distance: sample_distance,
                speed: 10.0,
                g: GForces {
                    vertical: 2.5,
                    lateral: -0.2,
                    longitudinal: 0.4,
                },
                airtime: false,
                height_m: 0.0,
                inverted: false,
                roll: 0.0,
                piece_index: 1,
            }]],
            model.name,
        );
        recorder.finish(false, false)
    }

    fn force_peaks(
        minimum_vertical_g: f64,
        maximum_vertical_g: f64,
        maximum_lateral_g: f64,
        maximum_longitudinal_g: f64,
    ) -> ForcePeakValues {
        ForcePeakValues {
            minimum_vertical_g,
            maximum_vertical_g,
            maximum_lateral_g,
            maximum_longitudinal_g,
        }
    }
}
