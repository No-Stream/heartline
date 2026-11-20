use std::collections::BTreeMap;
use std::fmt::Write as _;

use serde::{Deserialize, Serialize};
use theme_park_sim::physics::{
    Contribution, ForcePeakLocation, ForcePeakValues, PhysicsConfig, TestRun, Train,
};
use theme_park_sim::track::ride_file::{RideFile, RideStep};
use theme_park_sim::track::{CoasterModel, PieceKind, rated_speed_margin, rated_speeds};

pub(crate) type ShapedElementEntrySpeeds = BTreeMap<usize, (Option<f64>, Option<f64>)>;

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

#[derive(Clone, Debug, PartialEq)]
pub struct RatedPieceEntryMargin {
    pub piece_index: usize,
    pub piece_id: String,
    pub car_index: Option<usize>,
    pub entry_speed: Option<f64>,
    pub window: (f64, f64),
    pub rule_margin: Option<f64>,
    pub unavailable_reason: Option<String>,
    pub construction_test: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ShapedElementReportRow {
    pub piece_index: usize,
    pub element: String,
    pub design_speed_mps: f64,
    pub default_entry_speed_mps: Option<f64>,
    pub construction_entry_speed_mps: Option<f64>,
    pub intent: String,
    pub delivered: String,
    pub headroom: String,
    pub size: String,
}

pub fn render_shaped_elements_table(rows: &[ShapedElementReportRow]) -> String {
    let mut output = String::from("Elements:\n");
    writeln!(
        output,
        "# piece element design_m/s default_entry_m/s construction_entry_m/s intent delivered headroom size"
    )
    .unwrap();
    for row in rows {
        writeln!(
            output,
            "{:>3} {:<24} {:>9} {:>17} {:>23} {:<30} {:<14} {:<14} {}",
            row.piece_index,
            row.element,
            format_optional(Some(row.design_speed_mps)),
            format_optional(row.default_entry_speed_mps),
            format_optional(row.construction_entry_speed_mps),
            row.intent,
            row.delivered,
            row.headroom,
            row.size
        )
        .unwrap();
    }
    output
}

pub(crate) fn shaped_element_entry_speeds(
    circuit: &theme_park_sim::physics::Circuit,
    model: &CoasterModel,
    piece_indices: &[usize],
) -> Result<ShapedElementEntrySpeeds, String> {
    for &piece_index in piece_indices {
        if piece_index >= circuit.pieces().len() {
            return Err(format!(
                "shaped element piece index {piece_index} is outside the circuit"
            ));
        }
    }
    let default_entries = shaped_entry_speeds_for_configuration(circuit, model, false);
    let construction_entries = shaped_entry_speeds_for_configuration(circuit, model, true);
    Ok(piece_indices
        .iter()
        .map(|&piece_index| {
            (
                piece_index,
                (
                    default_entries[piece_index],
                    construction_entries[piece_index],
                ),
            )
        })
        .collect())
}

fn shaped_entry_speeds_for_configuration(
    circuit: &theme_park_sim::physics::Circuit,
    model: &CoasterModel,
    construction_test: bool,
) -> Vec<Option<f64>> {
    let (lift_speed, initial_speed) = if construction_test {
        (5.0, 2.0)
    } else {
        (PhysicsConfig::default().lift_speed, 0.0)
    };
    let start = if construction_test {
        0.0
    } else {
        circuit.pieces()[0].length() / 2.0
    };
    let mut train = Train::new(
        circuit,
        &model.train,
        PhysicsConfig {
            lift_speed,
            ..PhysicsConfig::default()
        },
        start,
        initial_speed,
    );
    let mut entries = vec![None; circuit.pieces().len()];
    let completion_distance = circuit.length() + start;
    for _ in 0..60_000 {
        train.step(circuit);
        for frame in train.take_readings() {
            if let Some(lead_car) = frame.first() {
                entries[lead_car.piece_index].get_or_insert(lead_car.speed);
            }
        }
        if !train.notes().is_empty() || train.distance() >= completion_distance {
            break;
        }
    }
    entries
}

/// Simulates both physics configurations used by the design-margin integration tests.
pub(crate) fn rated_speed_margins(
    circuit: &theme_park_sim::physics::Circuit,
    model: &CoasterModel,
) -> Result<Vec<RatedPieceEntryMargin>, String> {
    let mut margins = Vec::new();
    let rated_pieces: Vec<_> = circuit
        .pieces()
        .iter()
        .enumerate()
        .filter_map(|(piece_index, piece)| {
            rated_speeds(model, piece).map(|window| (piece_index, piece, window))
        })
        .collect();
    if rated_pieces.is_empty() {
        return Ok(margins);
    }
    for (construction_test, lift_speed, initial_speed) in [
        (false, PhysicsConfig::default().lift_speed, 0.0),
        (true, 5.0, 2.0),
    ] {
        let start = if construction_test {
            0.0
        } else {
            circuit.pieces()[0].length() / 2.0
        };
        let mut train = Train::new(
            circuit,
            &model.train,
            PhysicsConfig {
                lift_speed,
                ..PhysicsConfig::default()
            },
            start,
            initial_speed,
        );
        let mut entries = vec![vec![None; usize::from(model.train.cars)]; circuit.pieces().len()];
        let mut completion_problem = None;
        for _ in 0..60_000 {
            train.step(circuit);
            for frame in train.take_readings() {
                for (car_index, reading) in frame.iter().enumerate() {
                    let car_distance = train.distance()
                        - car_index as f64 * model.train.length_metres
                            / f64::from(model.train.cars);
                    if car_distance >= 0.0 {
                        entries[reading.piece_index][car_index].get_or_insert(reading.speed);
                    }
                }
            }
            if !train.notes().is_empty() {
                completion_problem = Some(format!("train recovered: {:?}", train.notes()));
                break;
            }
            if train.distance() >= circuit.length() + start {
                break;
            }
        }
        if completion_problem.is_none() && train.distance() < circuit.length() + start {
            completion_problem = Some("train did not complete a lap within 60,000 steps".into());
        }

        for &(piece_index, piece, window) in &rated_pieces {
            let fed_by_lift = circuit.pieces()
                [(piece_index + circuit.pieces().len() - 1) % circuit.pieces().len()]
            .kind
                == PieceKind::Lift;
            let checked_cars = if fed_by_lift {
                1
            } else {
                entries[piece_index].len()
            };
            let mut worst: Option<(usize, f64, f64)> = None;
            let mut missing_cars = Vec::new();
            for (car_index, entry_speed) in
                entries[piece_index].iter().take(checked_cars).enumerate()
            {
                let Some(entry_speed) = *entry_speed else {
                    missing_cars.push(car_index);
                    continue;
                };
                let rule_margin = rated_speed_margin(window, entry_speed);
                if worst.is_none_or(|(_, _, current_margin): (usize, f64, f64)| {
                    rule_margin < current_margin
                }) {
                    worst = Some((car_index, entry_speed, rule_margin));
                }
            }
            let unavailable_reason = if missing_cars.is_empty() {
                None
            } else {
                let missing_cars = missing_cars
                    .iter()
                    .map(usize::to_string)
                    .collect::<Vec<_>>()
                    .join(", ");
                Some(format!(
                    "{}; entry sample missing for car(s) {missing_cars}",
                    completion_problem
                        .as_deref()
                        .unwrap_or("train stopped before all cars entered this piece")
                ))
            };
            let (car_index, entry_speed, rule_margin) = worst.map_or(
                (None, None, None),
                |(car_index, entry_speed, rule_margin)| {
                    (Some(car_index), Some(entry_speed), Some(rule_margin))
                },
            );
            margins.push(RatedPieceEntryMargin {
                piece_index,
                piece_id: format!("{:?}", piece.id),
                car_index,
                entry_speed,
                window: (window.minimum_entry, window.maximum_entry),
                rule_margin,
                unavailable_reason,
                construction_test,
            });
        }
    }
    Ok(margins)
}

pub(crate) fn rated_speed_margin_report(
    circuit: &theme_park_sim::physics::Circuit,
    model: &CoasterModel,
) -> Result<String, String> {
    let margins = rated_speed_margins(circuit, model)?;
    Ok(format_rated_speed_margins(&margins))
}

pub(crate) fn format_rated_speed_margins(margins: &[RatedPieceEntryMargin]) -> String {
    let mut output = String::new();
    for construction_test in [false, true] {
        let lift_speed = if construction_test {
            5.0
        } else {
            PhysicsConfig::default().lift_speed
        };
        let initial_speed = if construction_test { 2.0 } else { 0.0 };
        writeln!(
            output,
            "Rated entry-speed margin ({}, lift {:.1} m/s, initial speed {:.1} m/s):",
            if construction_test {
                "construction-test physics"
            } else {
                "default physics"
            },
            lift_speed,
            initial_speed
        )
        .unwrap();
        writeln!(
            output,
            "# piece car entry_m/s window_m/s rule_margin_m/s status"
        )
        .unwrap();
        for margin in margins
            .iter()
            .filter(|margin| margin.construction_test == construction_test)
        {
            writeln!(
                output,
                "{:>3} {} car {} {} {:.2} to {:.2} {} {}",
                margin.piece_index,
                margin.piece_id,
                margin
                    .car_index
                    .map_or_else(|| "—".to_owned(), |car| car.to_string()),
                format_optional(margin.entry_speed),
                margin.window.0,
                margin.window.1,
                format_signed(margin.rule_margin),
                rated_margin_status(margin),
            )
            .unwrap();
        }
        writeln!(output, "Rule margin is positive when the rated-window rule passes. Windows at least 3 m/s wide require 1.50 m/s from both edges; narrower windows require entry within 0.30 m/s of the centre. Lift-fed crests check the front car only.").unwrap();
    }
    output
}

fn rated_margin_status(margin: &RatedPieceEntryMargin) -> String {
    if let Some(reason) = &margin.unavailable_reason {
        return format!("UNAVAILABLE: {reason}");
    }
    margin.rule_margin.map_or_else(
        || "UNAVAILABLE: entry samples missing".to_owned(),
        |rule_margin| {
            if rule_margin >= 0.0 {
                "PASS".to_owned()
            } else {
                "FAIL".to_owned()
            }
        },
    )
}

pub fn render_ride_report(
    ride: &RideFile,
    test_run: &TestRun,
    estimates: &[RidePieceEstimate],
    include_samples: bool,
) -> Result<String, String> {
    render_ride_report_with_margins(ride, test_run, estimates, "", include_samples)
}

pub fn render_ride_report_with_margins(
    ride: &RideFile,
    test_run: &TestRun,
    estimates: &[RidePieceEstimate],
    rated_speed_margins: &str,
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
        .filter(|step| matches!(step, RideStep::Place(_) | RideStep::Shape(_)))
        .count();
    let mut piece_indices: BTreeMap<usize, ()> = (1..=placed_piece_count)
        .map(|piece_index| (piece_index, ()))
        .collect();
    piece_indices.extend(
        estimates_by_piece
            .keys()
            .filter(|piece_index| **piece_index > 0)
            .map(|piece_index| (*piece_index, ())),
    );
    piece_indices.extend(
        peaks_by_piece
            .keys()
            .filter(|piece_index| **piece_index > 0)
            .map(|piece_index| (*piece_index, ())),
    );
    let mut piece_names: BTreeMap<usize, String> = estimates_by_piece
        .iter()
        .filter(|(piece_index, _)| **piece_index > 0)
        .map(|(piece_index, estimate)| (*piece_index, estimate.piece_id.clone()))
        .collect();
    piece_names.insert(0, "Station".to_owned());
    let mut piece_index = 0;
    for step in &ride.steps {
        match step {
            RideStep::Place(place) => {
                piece_index += 1;
                piece_names
                    .entry(piece_index)
                    .or_insert_with(|| place.place.clone());
            }
            RideStep::Shape(_) => {
                piece_index += 1;
                piece_names
                    .entry(piece_index)
                    .or_insert_with(|| "shaped element".to_owned());
            }
            _ => {}
        }
    }

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
    let peak_locations = test_run.force_peak_locations();
    writeln!(
        output,
        "  peak forces: vertical +{:.2} g at {}; vertical {:.2} g at {}; lateral {:.2} g at {}; forward {:.2} g at {}; braking {:.2} g at {}",
        test_run.statistics.max_positive_vertical_g,
        format_force_location(peak_locations.maximum_positive_vertical_g, &piece_names),
        test_run.statistics.max_negative_vertical_g,
        format_force_location(peak_locations.maximum_negative_vertical_g, &piece_names),
        test_run.statistics.max_lateral_g,
        format_force_location(peak_locations.maximum_lateral_g, &piece_names),
        test_run.statistics.max_forward_g,
        format_force_location(peak_locations.maximum_forward_g, &piece_names),
        test_run.statistics.max_braking_g,
        format_force_location(peak_locations.maximum_braking_g, &piece_names)
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
    let sustained_locations = test_run.sustained_force_peak_locations();
    writeln!(
        output,
        "  sustained by any car: highest vertical {} held 0.5 s; lowest vertical {} held 0.3 s",
        format_sustained_force(
            sustained.maximum_vertical_g,
            sustained_locations.maximum_vertical_g,
            &piece_names
        ),
        format_sustained_force(
            sustained.minimum_vertical_g,
            sustained_locations.minimum_vertical_g,
            &piece_names
        )
    )
    .unwrap();
    writeln!(
        output,
        "# piece entry_m/s window_m/s margin_m/s estimate_front_car_0_v+/-_g ratings status notes"
    )
    .unwrap();

    for piece_index in piece_indices.keys() {
        let estimate = estimates_by_piece.get(piece_index).copied();
        let piece_id = piece_names
            .get(piece_index)
            .cloned()
            .unwrap_or_else(|| format!("piece-{piece_index}"));
        let entry_speed = estimate.and_then(|estimate| estimate.entry_speed);
        let rated_window = estimate.and_then(|estimate| estimate.rated_window);
        let margin = match (entry_speed, rated_window) {
            (Some(speed), Some((minimum, maximum))) => Some((speed - minimum).min(maximum - speed)),
            _ => None,
        };
        let estimated = estimate.and_then(|estimate| estimate.estimated);
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
        let notes_suffix = if notes.is_empty() {
            String::new()
        } else {
            format!(" {notes}")
        };
        writeln!(
            output,
            "{piece_index:>3} {piece_id:<24} {entry_speed:>9} {rated_window:>13} {margin:>8} {estimated:>20} {ratings} {status}{notes_suffix}",
            entry_speed = format_optional(entry_speed),
            rated_window = format_window(rated_window),
            margin = format_signed(margin),
            estimated = format_estimated_peaks(estimated),
        )
        .unwrap();
    }

    writeln!(
        output,
        "Measured force peaks by piece and car (one-metre samples):"
    )
    .unwrap();
    writeln!(output, "# piece car vertical+/-_g lateral_g longitudinal_g").unwrap();
    for peaks in test_run.car_piece_force_peaks() {
        if peaks.piece_index == 0 {
            continue;
        }
        let piece_id = piece_names
            .get(&peaks.piece_index)
            .cloned()
            .unwrap_or_else(|| format!("piece-{}", peaks.piece_index));
        writeln!(
            output,
            "{:>3} {:<24} car {:>2} {:>12} {:>9} {:>14}",
            peaks.piece_index,
            piece_id,
            peaks.car_index,
            format_vertical_peaks(Some(peaks.peaks)),
            format_optional(Some(peaks.peaks.maximum_lateral_g)),
            format_optional(Some(peaks.peaks.maximum_longitudinal_g))
        )
        .unwrap();
    }

    if !rated_speed_margins.is_empty() {
        output.push_str(rated_speed_margins);
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

fn format_force_location(
    location: Option<ForcePeakLocation>,
    piece_names: &BTreeMap<usize, String>,
) -> String {
    location.map_or_else(
        || "location unavailable".to_owned(),
        |location| {
            let piece_id = piece_names
                .get(&location.piece_index)
                .cloned()
                .unwrap_or_else(|| format!("piece-{}", location.piece_index));
            format!(
                "{piece_id} (piece {}, car {})",
                location.piece_index, location.car_index
            )
        },
    )
}

fn format_sustained_force(
    value: Option<f64>,
    location: Option<ForcePeakLocation>,
    piece_names: &BTreeMap<usize, String>,
) -> String {
    value.map_or_else(
        || "none".to_owned(),
        |g| {
            format!(
                "{g:.2} g at {}",
                format_force_location(location, piece_names)
            )
        },
    )
}

fn write_contributions(output: &mut String, category: &str, contributions: &[Contribution]) {
    writeln!(output, "  {category}:").unwrap();
    if contributions.is_empty() {
        writeln!(output, "    none").unwrap();
        return;
    }
    for contribution in contributions {
        let mut description = contribution.describe();
        let force_details = if contribution.force_details.is_empty() {
            contribution.force_detail.iter().collect::<Vec<_>>()
        } else {
            contribution.force_details.iter().collect::<Vec<_>>()
        };
        if !force_details.is_empty() {
            description.push_str("; ");
            description.push_str(
                &force_details
                    .iter()
                    .map(|detail| {
                        format!(
                            "{} {:.2} {} at {} (piece {}, car {})",
                            detail.axis,
                            detail.measured_value,
                            detail.unit,
                            detail.location.piece_name,
                            detail.location.piece_index,
                            detail.car_index.unwrap_or(0)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", "),
            );
        }
        writeln!(output, "    {description}").unwrap();
    }
}

fn format_signed(value: Option<f64>) -> String {
    value.map_or_else(|| "—".to_owned(), |value| format!("{value:+.2}"))
}

fn format_window(window: Option<(f64, f64)>) -> String {
    window.map_or_else(
        || "—".to_owned(),
        |(minimum, maximum)| format!("{minimum:.2} to {maximum:.2}"),
    )
}

fn format_estimated_peaks(peaks: Option<ForcePeakValues>) -> String {
    peaks.map_or_else(
        || "—".to_owned(),
        |peaks| {
            format!(
                "car 0 v+/- {:.2}/{:.2} g lat {:.2} g long {:.2} g",
                peaks.maximum_vertical_g,
                peaks.minimum_vertical_g,
                peaks.maximum_lateral_g,
                peaks.maximum_longitudinal_g
            )
        },
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
    fn report_table_includes_estimates_and_measured_piece_car_peaks() {
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
            .find(|line| line.starts_with("  1 FloaterHill24x160 "))
            .unwrap();
        for expected_value in [
            "33.10",
            "27.00 to 34.50",
            "+1.40",
            "car 0 v+/- 3.60/-0.90 g lat 0.10 g long 0.40 g",
            "airtime:great vertical:great lateral:fine longitudinal:fine",
            "reachedEnd first drop",
        ] {
            assert!(table_row.contains(expected_value), "{table_row}");
        }

        let measured_peak_row = report
            .lines()
            .find(|line| line.starts_with("  1 FloaterHill24x160 ") && line.contains("car  0"))
            .unwrap();
        for expected_value in ["2.50/2.50", "0.20", "0.40"] {
            assert!(
                measured_peak_row.contains(expected_value),
                "{measured_peak_row}"
            );
        }
        for expected_value in [
            "vertical +2.50 g at FloaterHill24x160 (piece 1, car 0)",
            "vertical 0.00 g at FloaterHill24x160 (piece 1, car 0)",
            "lateral 0.20 g at FloaterHill24x160 (piece 1, car 0)",
            "forward 0.40 g at FloaterHill24x160 (piece 1, car 0)",
            "braking 0.00 g at FloaterHill24x160 (piece 1, car 0)",
        ] {
            assert!(report.contains(expected_value), "{expected_value}");
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

#[cfg(test)]
mod shaped_element_report_tests {
    use super::{
        ShapedElementReportRow, render_shaped_elements_table, shaped_element_entry_speeds,
    };
    use theme_park_sim::physics::demo_circuit;
    use theme_park_sim::track::hybrid_model;

    #[test]
    fn shaped_element_table_reports_both_entries_intent_delivery_and_headroom() {
        let table = render_shaped_elements_table(&[ShapedElementReportRow {
            piece_index: 3,
            element: "Floater hill".to_owned(),
            design_speed_mps: 34.0,
            default_entry_speed_mps: Some(33.8),
            construction_entry_speed_mps: Some(34.2),
            intent: "sustained airtime -0.5 g".to_owned(),
            delivered: "-0.48 g".to_owned(),
            headroom: "0.37 g".to_owned(),
            size: "height 36 m, span 64 m".to_owned(),
        }]);

        assert!(table.contains("default_entry_m/s construction_entry_m/s"));
        assert!(table.contains("  3 Floater hill"));
        for expected in [
            "34.00",
            "33.80",
            "34.20",
            "sustained airtime -0.5 g",
            "-0.48 g",
            "0.37 g",
            "height 36 m, span 64 m",
        ] {
            assert!(table.contains(expected), "missing {expected:?} in {table}");
        }
    }

    #[test]
    fn shaped_element_entry_speeds_cover_default_and_construction_physics() {
        let entries = shaped_element_entry_speeds(&demo_circuit(), &hybrid_model(), &[1]).unwrap();
        let (default_entry, construction_entry) = entries[&1];

        assert!(default_entry.is_some_and(f64::is_finite));
        assert!(construction_entry.is_some_and(f64::is_finite));
    }
}
