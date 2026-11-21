use std::fmt::Write as _;

use theme_park_sim::physics::{Circuit, GRAVITY, PhysicsConfig, TestRun, TestSample};
use theme_park_sim::track::PieceKind;

// Samples target each metre; tolerate one merged bin and reject larger unobserved gaps.
const MAX_SAMPLED_GAP_METRES: f64 = 2.0;

#[derive(Clone, Copy, Debug, PartialEq)]
struct BrakeEnergySummary {
    mid_course_brake_runs: usize,
    lift_energy_removed_share: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub(crate) struct BrakeRunConfigurationSpeeds {
    pub(crate) sampled_entry_speed_mps: Option<f64>,
    pub(crate) sampled_exit_speed_mps: Option<f64>,
    pub(crate) sampled_speed_removed_mps: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub(crate) struct BrakeRunSpeedRow {
    pub(crate) run_index: usize,
    pub(crate) piece_indices: Vec<usize>,
    pub(crate) mid_course: bool,
    pub(crate) default_physics: BrakeRunConfigurationSpeeds,
    pub(crate) construction_test_physics: BrakeRunConfigurationSpeeds,
}

pub(crate) fn brake_run_speed_rows(
    circuit: &Circuit,
    default_test_run: &TestRun,
    construction_test_run: &TestRun,
) -> Vec<BrakeRunSpeedRow> {
    let runs = contiguous_brake_runs(circuit);
    let pieces = circuit.pieces();
    let station_index = pieces
        .iter()
        .position(|piece| piece.kind == PieceKind::Station)
        .expect("a tested circuit contains a station");
    let station_predecessor = (station_index + pieces.len() - 1) % pieces.len();
    let piece_starts: Vec<_> = pieces
        .iter()
        .scan(0.0, |distance, piece| {
            let start = *distance;
            *distance += piece.length();
            Some(start)
        })
        .collect();

    runs.into_iter()
        .enumerate()
        .map(|(run_index, piece_indices)| {
            let first_piece = *piece_indices
                .first()
                .expect("brake runs contain at least one piece");
            let last_piece = *piece_indices
                .last()
                .expect("brake runs contain at least one piece");
            let entry_distance = piece_starts[first_piece];
            let exit_distance = piece_starts[last_piece] + pieces[last_piece].length();
            let default_physics = brake_run_configuration_speeds(
                default_test_run,
                entry_distance,
                exit_distance,
                circuit.length(),
            );
            let construction_test_physics = brake_run_configuration_speeds(
                construction_test_run,
                entry_distance,
                exit_distance,
                circuit.length(),
            );
            BrakeRunSpeedRow {
                run_index: run_index + 1,
                mid_course: last_piece != station_predecessor,
                piece_indices,
                default_physics,
                construction_test_physics,
            }
        })
        .collect()
}

fn brake_run_configuration_speeds(
    test_run: &TestRun,
    entry_distance: f64,
    exit_distance: f64,
    circuit_length: f64,
) -> BrakeRunConfigurationSpeeds {
    let sampled_entry_speed_mps =
        sampled_speed_at_distance(&test_run.samples, entry_distance, circuit_length);
    let sampled_exit_speed_mps =
        sampled_speed_at_distance(&test_run.samples, exit_distance, circuit_length);
    let sampled_speed_removed_mps = sampled_entry_speed_mps
        .zip(sampled_exit_speed_mps)
        .map(|(entry_speed, exit_speed)| entry_speed - exit_speed);
    BrakeRunConfigurationSpeeds {
        sampled_entry_speed_mps,
        sampled_exit_speed_mps,
        sampled_speed_removed_mps,
    }
}

fn sampled_speed_at_distance(
    samples: &[TestSample],
    distance: f64,
    circuit_length: f64,
) -> Option<f64> {
    let target_distance = distance.rem_euclid(circuit_length);
    samples
        .iter()
        .filter_map(|sample| {
            let direct_distance = (sample.distance - target_distance).abs();
            let distance_to_boundary = direct_distance.min(circuit_length - direct_distance);
            (distance_to_boundary <= MAX_SAMPLED_GAP_METRES / 2.0)
                .then_some((distance_to_boundary, sample.speed))
        })
        .min_by(|left, right| left.0.total_cmp(&right.0))
        .map(|(_, speed)| speed)
}

pub(crate) fn render_brake_run_speed_table(rows: &[BrakeRunSpeedRow]) -> String {
    let mut output = String::from(
        "Brake run speeds (lead-car samples nearest each brake boundary):\n# run class piece_indices default_entry_m/s default_exit_m/s default_removed_m/s construction_entry_m/s construction_exit_m/s construction_removed_m/s\n",
    );
    for row in rows {
        writeln!(
            output,
            "{:>3} {:<8} {:<16} {} {} {} {} {} {}",
            row.run_index,
            if row.mid_course {
                "mid-course"
            } else {
                "station"
            },
            format!("{:?}", row.piece_indices),
            format_speed(row.default_physics.sampled_entry_speed_mps),
            format_speed(row.default_physics.sampled_exit_speed_mps),
            format_speed(row.default_physics.sampled_speed_removed_mps),
            format_speed(row.construction_test_physics.sampled_entry_speed_mps),
            format_speed(row.construction_test_physics.sampled_exit_speed_mps),
            format_speed(row.construction_test_physics.sampled_speed_removed_mps),
        )
        .unwrap();
    }
    output
}

fn format_speed(speed: Option<f64>) -> String {
    speed.map_or_else(|| "—".to_owned(), |speed| format!("{speed:.2}"))
}

/// Summarizes non-terminal brake sections and estimates the mechanical energy they remove.
///
/// The estimate uses front-car samples, splits sampled energy changes at piece boundaries, and
/// subtracts default rolling and drag resistance. It approximates a representative car's work, not
/// synchronized work across the whole train or the exact actuator work from the physics integrator.
pub(crate) fn mid_course_brake_energy_report(
    circuit: &Circuit,
    test_run: &TestRun,
    train_cars: u8,
) -> String {
    let summary =
        summarize_brake_energy(circuit, &test_run.samples, test_run.completed, train_cars);
    let trim_energy = summary.lift_energy_removed_share.map_or_else(
        || "unavailable (insufficient sampled lift-energy coverage)".to_owned(),
        |share| format!("{share:.1}%"),
    );

    format!(
        "Mid-course brake runs: {}; estimated lift energy removed by trims from lead-car samples: {trim_energy} (mechanical energy estimate corrected for default rolling and drag resistance)\n",
        summary.mid_course_brake_runs
    )
}

fn summarize_brake_energy(
    circuit: &Circuit,
    samples: &[TestSample],
    completed: bool,
    train_cars: u8,
) -> BrakeEnergySummary {
    assert!(train_cars > 0, "a train must have at least one car");
    let pieces = circuit.pieces();
    let station_index = pieces
        .iter()
        .position(|piece| piece.kind == PieceKind::Station)
        .expect("a tested circuit contains a station");
    let station_predecessor = (station_index + pieces.len() - 1) % pieces.len();
    let brake_runs = contiguous_brake_runs(circuit);
    let mid_course_piece_indices: Vec<_> = brake_runs
        .iter()
        .filter(|run| run.last().copied() != Some(station_predecessor))
        .flatten()
        .copied()
        .collect();
    let mid_course_brake_run_count = brake_runs
        .iter()
        .filter(|run| run.last().copied() != Some(station_predecessor))
        .count();
    let trim_energy_share = lift_energy_removed_share(
        circuit,
        samples,
        completed,
        &mid_course_piece_indices,
        train_cars,
    );

    BrakeEnergySummary {
        mid_course_brake_runs: mid_course_brake_run_count,
        lift_energy_removed_share: trim_energy_share,
    }
}

fn contiguous_brake_runs(circuit: &Circuit) -> Vec<Vec<usize>> {
    let pieces = circuit.pieces();
    (0..pieces.len())
        .filter(|&index| {
            pieces[index].kind == PieceKind::Brakes
                && pieces[(index + pieces.len() - 1) % pieces.len()].kind != PieceKind::Brakes
        })
        .map(|start| {
            let mut run = Vec::new();
            let mut index = start;
            while pieces[index].kind == PieceKind::Brakes {
                run.push(index);
                index = (index + 1) % pieces.len();
            }
            run
        })
        .collect()
}

fn lift_energy_removed_share(
    circuit: &Circuit,
    samples: &[TestSample],
    completed: bool,
    mid_course_brake_piece_indices: &[usize],
    train_cars: u8,
) -> Option<f64> {
    if samples.len() < 2 {
        return None;
    }
    let mut lift_energy_gained = 0.0;
    let mut trim_energy_removed = 0.0;
    let sample_intervals = if completed {
        samples.len()
    } else {
        samples.len().saturating_sub(1)
    };
    let physics = PhysicsConfig::default();
    let total_train_mass_kg = physics.car_mass_kg * f64::from(train_cars);
    let mut ordered_samples: Vec<_> = samples.iter().collect();
    ordered_samples.sort_by(|left, right| left.distance.total_cmp(&right.distance));
    let piece_starts: Vec<_> = circuit
        .pieces()
        .iter()
        .scan(0.0, |distance, piece| {
            let start = *distance;
            *distance += piece.length();
            Some(start)
        })
        .collect();

    for sample_index in 0..sample_intervals {
        let current = ordered_samples[sample_index];
        let next = ordered_samples[(sample_index + 1) % ordered_samples.len()];
        let mut interval_distance = next.distance - current.distance;
        if interval_distance <= 0.0 {
            assert!(
                completed && sample_index + 1 == samples.len(),
                "sample distances wrap only at the end of a completed lap"
            );
            interval_distance += circuit.length();
        }
        assert!(
            interval_distance > 0.0,
            "sample intervals have positive length"
        );
        if interval_distance > MAX_SAMPLED_GAP_METRES {
            return None;
        }
        let current_energy = current.speed.powi(2) / 2.0 + GRAVITY * current.height_m;
        let next_energy = next.speed.powi(2) / 2.0 + GRAVITY * next.height_m;

        let interval_start = current.distance.rem_euclid(circuit.length());
        let mut interval_offset = 0.0;
        while interval_offset < interval_distance {
            let segment_start = (interval_start + interval_offset).rem_euclid(circuit.length());
            let piece_index = circuit.piece_index(segment_start);
            let piece_end = piece_starts[piece_index] + circuit.pieces()[piece_index].length();
            let distance_to_piece_end = if piece_end > segment_start {
                piece_end - segment_start
            } else {
                circuit.length() - segment_start + piece_end
            };
            let segment_distance = distance_to_piece_end.min(interval_distance - interval_offset);
            let start_fraction = interval_offset / interval_distance;
            let end_fraction = (interval_offset + segment_distance) / interval_distance;
            let segment_start_energy =
                current_energy + (next_energy - current_energy) * start_fraction;
            let segment_end_energy = current_energy + (next_energy - current_energy) * end_fraction;
            let start_speed = current.speed + (next.speed - current.speed) * start_fraction;
            let end_speed = current.speed + (next.speed - current.speed) * end_fraction;
            let mean_squared_speed =
                (start_speed.powi(2) + start_speed * end_speed + end_speed.powi(2)) / 3.0;
            let passive_resistance_acceleration = physics.rolling_resistance * GRAVITY
                + physics.drag_coefficient * mean_squared_speed / total_train_mass_kg;
            let passive_resistance_energy = passive_resistance_acceleration * segment_distance;

            match circuit.pieces()[piece_index].kind {
                PieceKind::Lift => {
                    lift_energy_gained += (segment_end_energy - segment_start_energy
                        + passive_resistance_energy)
                        .max(0.0);
                }
                PieceKind::Brakes if mid_course_brake_piece_indices.contains(&piece_index) => {
                    trim_energy_removed +=
                        (segment_start_energy - segment_end_energy - passive_resistance_energy)
                            .max(0.0);
                }
                PieceKind::Track | PieceKind::Launch | PieceKind::Brakes | PieceKind::Station => {}
            }
            interval_offset += segment_distance;
        }
    }

    (lift_energy_gained > 0.0).then_some(trim_energy_removed / lift_energy_gained * 100.0)
}

#[cfg(test)]
mod tests {
    use super::{BrakeEnergySummary, sampled_speed_at_distance, summarize_brake_energy};
    use theme_park_sim::physics::{Circuit, TestSample, demo_circuit};
    use theme_park_sim::track::PieceKind;

    #[test]
    fn excludes_the_terminal_brake_run_and_reports_missing_lift_energy() {
        let circuit = circuit_with_kinds(None, &[2, circuit_piece_count() - 1]);

        let summary = summarize_brake_energy(&circuit, &[], false, 6);

        assert_eq!(
            summary,
            BrakeEnergySummary {
                mid_course_brake_runs: 1,
                lift_energy_removed_share: None,
            }
        );
    }

    #[test]
    fn splits_sampled_energy_change_at_the_lift_and_brake_boundary() {
        let circuit = circuit_with_kinds(Some(1), &[2, circuit_piece_count() - 1]);
        let lift_start = circuit.pieces()[0].length();
        let brake_start = lift_start + circuit.pieces()[1].length();
        let samples = [
            sample(brake_start - 1.2, 10.0, 5.0),
            sample(brake_start - 0.2, 10.0, 15.0),
            sample(brake_start + 0.8, 10.0, 14.0),
        ];

        let summary = summarize_brake_energy(&circuit, &samples, false, 6);

        let lift_energy_removed_share = summary
            .lift_energy_removed_share
            .expect("lift energy is measured");
        assert_eq!(summary.mid_course_brake_runs, 1);
        assert!((7.0..9.0).contains(&lift_energy_removed_share));
    }

    #[test]
    fn completed_samples_wrap_across_the_terminal_brake_without_counting_it_as_a_trim() {
        let circuit = circuit_with_kinds(Some(1), &[2, circuit_piece_count() - 1]);
        let terminal_brake_start = circuit.pieces()[..circuit.pieces().len() - 1]
            .iter()
            .map(|piece| piece.length())
            .sum::<f64>();
        let mut samples = Vec::new();
        let mut distance = 1.0;
        while distance < circuit.length() - 0.5 {
            let speed = if distance >= terminal_brake_start + 0.5 {
                5.0
            } else {
                10.0
            };
            samples.push(sample(distance, speed, 0.0));
            distance += 1.0;
        }
        let near_end_distance = circuit.length() - 0.25;
        samples.insert(0, sample(near_end_distance, 5.0, 0.0));

        let summary = summarize_brake_energy(&circuit, &samples, true, 6);

        let lift_energy_removed_share = summary
            .lift_energy_removed_share
            .expect("wrapped samples still measure lift energy");
        assert_eq!(summary.mid_course_brake_runs, 1);
        assert_eq!(lift_energy_removed_share, 0.0);
    }

    #[test]
    fn incomplete_samples_with_an_unobserved_gap_do_not_report_an_energy_share() {
        let circuit = circuit_with_kinds(Some(1), &[2, circuit_piece_count() - 1]);
        let lift_start = circuit.pieces()[0].length();
        let samples = [
            sample(lift_start + 0.1, 10.0, 5.0),
            sample(lift_start + 4.1, 10.0, 15.0),
        ];

        let summary = summarize_brake_energy(&circuit, &samples, false, 6);

        assert_eq!(summary.mid_course_brake_runs, 1);
        assert_eq!(summary.lift_energy_removed_share, None);
    }

    #[test]
    fn brake_boundary_sample_lookup_wraps_at_the_station_seam() {
        let samples = [sample(0.5, 12.0, 0.0), sample(99.4, 5.0, 0.0)];

        let sampled_speed = sampled_speed_at_distance(&samples, 99.9, 100.0);

        assert_eq!(sampled_speed, Some(5.0));
    }

    fn circuit_piece_count() -> usize {
        demo_circuit().pieces().len()
    }

    fn circuit_with_kinds(lift_index: Option<usize>, brake_indices: &[usize]) -> Circuit {
        let mut pieces = demo_circuit().pieces().to_vec();
        let station_index = pieces
            .iter()
            .position(|piece| piece.kind == PieceKind::Station)
            .expect("demo circuit contains a station");
        for piece in &mut pieces {
            piece.kind = PieceKind::Track;
        }
        pieces[station_index].kind = PieceKind::Station;
        if let Some(lift_index) = lift_index {
            pieces[lift_index].kind = PieceKind::Lift;
        }
        for &brake_index in brake_indices {
            pieces[brake_index].kind = PieceKind::Brakes;
        }
        Circuit::new(pieces).expect("demo geometry still forms a circuit")
    }

    fn sample(distance: f64, speed: f64, height_m: f64) -> TestSample {
        TestSample {
            distance,
            speed,
            vertical_g: 1.0,
            lateral_g: 0.0,
            longitudinal_g: 0.0,
            airtime: false,
            force_limit_exceeded: [false; 3],
            height_m,
            inverted: false,
            roll: 0.0,
            piece_index: 0,
        }
    }
}
